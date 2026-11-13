//! Separate transactional source catalog. Native application track IDs stay in
//! the existing library database; this store retains imported source semantics.
use crate::{Playlist, Prepared, PreparedTrack, MAX_LINKS, MAX_TRACKS};
use rusqlite::{params, Connection, OptionalExtension};
use std::{collections::HashSet, path::Path, time::Duration};
const APP_ID: i32 = 0x44524954;
const MAX_RECORD: usize = 256 * 1024;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: i64,
    pub xml_url: String,
    pub media_root_url: Option<String>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FileInfo, Track};
    fn track(id: u64) -> PreparedTrack {
        PreparedTrack {
            track: Track {
                id,
                persistent_id: format!("P{id}"),
                location: format!("file:///C:/Music/{id}.mp3"),
                title: format!("Track{id}"),
                artist: "Artist".into(),
                album: "Album".into(),
                album_artist: "Group".into(),
                genre: "House".into(),
                bpm: Some(128.),
                rating: Some(80),
                play_count: Some(12),
                duration_ms: Some(120000),
                date_added: Some("2020-01-02T03:04:05Z".into()),
            },
            file: FileInfo {
                path_url: format!("file:///C:/Music/{id}.mp3"),
                available: false,
                size: None,
                modified_ms: None,
            },
        }
    }
    fn source(dir: &Path) -> String {
        url::Url::from_file_path(dir.join("Library.xml"))
            .unwrap()
            .to_string()
    }
    fn prepared() -> Prepared {
        Prepared {
            tracks: vec![track(1), track(2)],
            playlists: vec![Playlist {
                id: "P".into(),
                name: "Ordered".into(),
                tracks: vec![2, 1, 2],
            }],
            ..Default::default()
        }
    }
    #[test]
    fn reopen_preserves_all_metadata_order_and_incremental_noop_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.sqlite");
        let xml = source(dir.path());
        let p = prepared();
        let mut store = Store::open(&path).unwrap();
        let report = store.sync(&xml, None, &p, &|| false).unwrap();
        assert_eq!(report.added, 2);
        let before = store.connection.total_changes();
        let report = store.sync(&xml, None, &p, &|| false).unwrap();
        assert_eq!(report.unchanged, 2);
        assert!(report.changed_ids.is_empty());
        assert_eq!(report.playlists_changed, 0);
        assert_eq!(store.connection.total_changes(), before);
        drop(store);
        let mut store = Store::open(&path).unwrap();
        let snap = store.snapshot(report.source_id).unwrap();
        assert_eq!(snap.tracks, p.tracks);
        assert_eq!(snap.playlists, p.playlists);
        let mut changed = p.clone();
        changed.tracks[1].track.rating = Some(100);
        let report = store.sync(&xml, None, &changed, &|| false).unwrap();
        assert_eq!((report.updated, report.unchanged), (1, 1));
        assert_eq!(report.changed_ids, HashSet::from([2]));
    }
    #[test]
    fn cancelled_transaction_rolls_back_partial_source_and_track_writes() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("source.sqlite")).unwrap();
        let xml = source(dir.path());
        let mut p = prepared();
        p.playlists.clear();
        let calls = std::cell::Cell::new(0);
        let result = store.sync(&xml, None, &p, &|| {
            calls.set(calls.get() + 1);
            calls.get() >= 5
        });
        assert!(result.is_err());
        assert!(store.sources().unwrap().is_empty());
        let report = store.sync(&xml, None, &p, &|| false).unwrap();
        p.tracks[0].track.title = "changed".into();
        calls.set(0);
        assert!(store
            .sync(&xml, None, &p, &|| {
                calls.set(calls.get() + 1);
                calls.get() >= 5
            })
            .is_err());
        assert_eq!(
            store.snapshot(report.source_id).unwrap().tracks[0]
                .track
                .title,
            "Track1"
        );
    }
    #[test]
    fn removed_source_records_are_archived_and_playlist_changes_are_ordered() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("source.sqlite")).unwrap();
        let xml = source(dir.path());
        let mut p = prepared();
        store.sync(&xml, None, &p, &|| false).unwrap();
        p.tracks.pop();
        p.playlists[0].tracks = vec![1];
        let r = store.sync(&xml, None, &p, &|| false).unwrap();
        assert_eq!((r.deactivated, r.playlists_changed), (1, 1));
        assert_eq!(store.snapshot(r.source_id).unwrap().tracks.len(), 1);
        let archived: i64 = store
            .connection
            .query_row("SELECT count(*) FROM tracks WHERE active=0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(archived, 1);
    }
    #[test]
    fn invalid_inputs_and_corrupt_stored_identity_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("source.sqlite")).unwrap();
        let xml = source(dir.path());
        let mut p = prepared();
        p.tracks[0].track.bpm = Some(f64::NAN);
        assert!(store.sync(&xml, None, &p, &|| false).is_err());
        assert!(store.sources().unwrap().is_empty());
        let mut p = prepared();
        p.playlists[0].tracks.push(99);
        assert!(store.sync(&xml, None, &p, &|| false).is_err());
        let r = store.sync(&xml, None, &prepared(), &|| false).unwrap();
        store
            .connection
            .execute("UPDATE tracks SET track_id='999' WHERE track_id='1'", [])
            .unwrap();
        assert!(store.snapshot(r.source_id).is_err());
    }
    #[test]
    fn unrelated_database_is_not_modified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("other.sqlite");
        {
            let c = Connection::open(&path).unwrap();
            c.execute("CREATE TABLE original(value TEXT)", []).unwrap();
        }
        assert!(Store::open(&path).is_err());
        let c = Connection::open(path).unwrap();
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='sources'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }
    #[test]
    fn incremental_store_budget_for_fifty_thousand_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("large.sqlite")).unwrap();
        let xml = source(dir.path());
        let mut p = Prepared {
            tracks: (1..=50_000).map(track).collect(),
            ..Default::default()
        };
        let r = store.sync(&xml, None, &p, &|| false).unwrap();
        assert_eq!(r.added, 50_000);
        for track in p.tracks.iter_mut().take(100) {
            track.track.rating = Some(100);
        }
        let now = std::time::Instant::now();
        let r = store.sync(&xml, None, &p, &|| false).unwrap();
        let elapsed = now.elapsed();
        eprintln!("iTunesSQLite50k/100changed sync{elapsed:?}");
        assert_eq!((r.updated, r.unchanged), (100, 49_900));
        assert!(
            elapsed < Duration::from_secs(5),
            "isolated SQLite sync budget; XML/filesystem/UI excluded"
        );
    }
    #[test]
    fn concurrent_writer_cannot_mix_snapshot_generations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.sqlite");
        let xml = source(dir.path());
        let mut reader = Store::open(&path).unwrap();
        let p = prepared();
        let r = reader.sync(&xml, None, &p, &|| false).unwrap();
        let mut writer = Store::open(&path).unwrap();
        let mut next = p.clone();
        next.tracks.pop();
        next.playlists[0].tracks = vec![1];
        next.tracks[0].track.title = "new generation".into();
        let old = reader
            .snapshot_with_checkpoint(r.source_id, || {
                writer.sync(&xml, None, &next, &|| false).unwrap();
            })
            .unwrap();
        assert_eq!(old.tracks, p.tracks);
        assert_eq!(old.playlists, p.playlists);
        let new = reader.snapshot(r.source_id).unwrap();
        assert_eq!(new.tracks, next.tracks);
        assert_eq!(new.playlists, next.playlists);
    }
}
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub source: Source,
    pub tracks: Vec<PreparedTrack>,
    pub playlists: Vec<Playlist>,
}
#[derive(Clone, Debug, Default)]
pub struct SyncReport {
    pub source_id: i64,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub deactivated: usize,
    pub playlists_changed: usize,
    pub changed_ids: HashSet<u64>,
}
/// Worker-owned SQLite connection; never pass to UI or audio callbacks.
pub struct Store {
    connection: Connection,
}
fn cancelled(cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("iTunes sync cancelled".into())
    } else {
        Ok(())
    }
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn file_url(value: &str) -> Result<(), String> {
    if value.len() > 131072 || value.chars().any(char::is_control) {
        return Err("Invalid file URI length/characters".into());
    }
    let u = url::Url::parse(value).map_err(err)?;
    if u.scheme() != "file"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.to_file_path().is_err()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        Err("Source must be a platform-local file URI".into())
    } else {
        Ok(())
    }
}
fn validate_track(t: &PreparedTrack) -> Result<std::path::PathBuf, String> {
    file_url(&t.file.path_url)?;
    let path = t.file.path()?;
    if t.track.id == 0
        || [
            &t.track.title,
            &t.track.artist,
            &t.track.album,
            &t.track.album_artist,
            &t.track.genre,
            &t.track.persistent_id,
        ]
        .iter()
        .any(|s| s.chars().count() > 512 || s.chars().any(char::is_control))
        || t.track.location.len() > 32768
        || t.track.date_added.as_ref().is_some_and(|s| s.len() > 32768)
        || t.track
            .bpm
            .is_some_and(|b| !b.is_finite() || !(0.0..=1000.).contains(&b))
        || t.track.rating.is_some_and(|r| r > 100)
        || t.track.duration_ms.is_some_and(|d| d > 604800000)
    {
        return Err("Invalid source track metadata".into());
    }
    let original = url::Url::parse(&t.track.location).map_err(err)?;
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if original.scheme() != "file"
        || ![
            "mp3", "wav", "flac", "aif", "aiff", "ogg", "opus", "aac", "m4a", "mp4", "alac",
        ]
        .contains(&ext.as_str())
    {
        return Err("Source track is not a supported local audio file".into());
    }
    Ok(if cfg!(windows) {
        std::path::PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path
    })
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err("Source catalog path must be absolute".into());
        }
        let c = Connection::open(path).map_err(err)?;
        c.busy_timeout(Duration::from_secs(1)).map_err(err)?;
        c.pragma_update(None, "trusted_schema", false)
            .map_err(err)?;
        let app: i32 = c
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(err)?;
        let version: i32 = c
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(err)?;
        if app == 0 && version == 0 {
            let count: i64 = c
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                    [],
                    |r| r.get(0),
                )
                .map_err(err)?;
            if count != 0 {
                return Err("Refusing to initialize an unrelated source database".into());
            }
            c.execute_batch("BEGIN IMMEDIATE;
                CREATE TABLE sources(id INTEGER PRIMARY KEY,xml_url TEXT NOT NULL UNIQUE CHECK(length(xml_url)<=131072),media_root_url TEXT CHECK(length(media_root_url)<=131072)) STRICT;
                CREATE TABLE tracks(source_id INTEGER NOT NULL REFERENCES sources(id),track_id TEXT NOT NULL CHECK(length(track_id)<=20),data TEXT NOT NULL CHECK(length(CAST(data AS BLOB))<=262144),active INTEGER NOT NULL CHECK(active IN(0,1)),position INTEGER NOT NULL CHECK(position>=0 AND position<250000),PRIMARY KEY(source_id,track_id)) STRICT;
                CREATE INDEX active_tracks ON tracks(source_id,active,position);
                CREATE TABLE playlists(source_id INTEGER NOT NULL REFERENCES sources(id),playlist_id TEXT NOT NULL CHECK(length(playlist_id)<=512),name TEXT NOT NULL CHECK(length(name)<=512),items TEXT NOT NULL CHECK(length(CAST(items AS BLOB))<=24000000),position INTEGER NOT NULL CHECK(position>=0 AND position<4096),PRIMARY KEY(source_id,playlist_id)) STRICT;
                PRAGMA application_id=1146243412;PRAGMA user_version=1;COMMIT;").map_err(err)?;
        } else if app != APP_ID || version != 1 {
            return Err("Unsupported or unrelated iTunes source catalog".into());
        }
        c.pragma_update(None, "foreign_keys", true).map_err(err)?;
        c.pragma_update(None, "journal_mode", "WAL").map_err(err)?;
        c.pragma_update(None, "synchronous", "FULL").map_err(err)?;
        let page_size: i64 = c
            .pragma_query_value(None, "page_size", |r| r.get(0))
            .map_err(err)?;
        if !(512..=65536).contains(&page_size) {
            return Err("Invalid SQLite page size".into());
        }
        c.pragma_update(None, "max_page_count", (1024 * 1024 * 1024i64) / page_size)
            .map_err(err)?;
        Ok(Self { connection: c })
    }
    pub fn sources(&self) -> Result<Vec<Source>, String> {
        let mut q = self
            .connection
            .prepare("SELECT id,xml_url,media_root_url FROM sources ORDER BY id LIMIT 33")
            .map_err(err)?;
        let rows = q
            .query_map([], |r| {
                Ok(Source {
                    id: r.get(0)?,
                    xml_url: r.get(1)?,
                    media_root_url: r.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        if rows.len() > 32 {
            return Err("Source count exceeds32".into());
        }
        for source in &rows {
            if source.id <= 0 {
                return Err("Invalid source ID".into());
            }
            file_url(&source.xml_url)?;
            if let Some(url) = &source.media_root_url {
                file_url(url)?;
            }
        }
        Ok(rows)
    }
    /// Only changed/new source records are written. Removed source tracks become
    /// inactive archive records; no local application track is deleted.
    pub fn sync(
        &mut self,
        xml_url: &str,
        media_root_url: Option<&str>,
        p: &Prepared,
        cancel: &dyn Fn() -> bool,
    ) -> Result<SyncReport, String> {
        file_url(xml_url)?;
        if let Some(url) = media_root_url {
            file_url(url)?;
        }
        if p.tracks.len() > MAX_TRACKS || p.playlists.len() > 4096 {
            return Err("Source record limit exceeded".into());
        }
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        let mut playlist_ids = HashSet::new();
        let mut links = 0usize;
        for t in &p.tracks {
            cancelled(cancel)?;
            let key = validate_track(t)?;
            if t.track.id == 0
                || !ids.insert(t.track.id)
                || !paths.insert(key)
                || serde_json::to_vec(t).map_err(err)?.len() > MAX_RECORD
            {
                return Err("Invalid/duplicate source track".into());
            }
        }
        for playlist in &p.playlists {
            cancelled(cancel)?;
            links = links.saturating_add(playlist.tracks.len());
            if playlist.id.is_empty()
                || playlist.id.len() > 512
                || playlist.name.chars().count() > 512
                || !playlist_ids.insert(&playlist.id)
                || links > MAX_LINKS
                || playlist.tracks.iter().any(|id| !ids.contains(id))
            {
                return Err("Invalid/duplicate source playlist".into());
            }
        }
        cancelled(cancel)?;
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM sources WHERE xml_url=?1", [xml_url], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        let source_id = if let Some(id) = existing {
            id
        } else {
            let count: i64 = tx
                .query_row("SELECT count(*) FROM sources", [], |r| r.get(0))
                .map_err(err)?;
            if count >= 32 {
                return Err("Source limit32 reached".into());
            }
            tx.execute(
                "INSERT INTO sources(xml_url,media_root_url) VALUES(?1,?2)",
                params![xml_url, media_root_url],
            )
            .map_err(err)?;
            tx.last_insert_rowid()
        };
        tx.execute(
            "UPDATE sources SET media_root_url=?2 WHERE id=?1 AND media_root_url IS NOT ?2",
            params![source_id, media_root_url],
        )
        .map_err(err)?;
        let mut report = SyncReport {
            source_id,
            ..Default::default()
        };
        {
            let mut q = tx
                .prepare("SELECT track_id FROM tracks WHERE source_id=?1 AND active=1")
                .map_err(err)?;
            let mut removed = q
                .query_map([source_id], |r| r.get::<_, String>(0))
                .map_err(err)?
                .collect::<Result<HashSet<_>, _>>()
                .map_err(err)?;
            let mut prior = tx
                .prepare(
                    "SELECT data,active,position FROM tracks WHERE source_id=?1 AND track_id=?2",
                )
                .map_err(err)?;
            let mut upsert=tx.prepare("INSERT INTO tracks(source_id,track_id,data,active,position) VALUES(?1,?2,?3,1,?4) ON CONFLICT(source_id,track_id) DO UPDATE SET data=excluded.data,active=1,position=excluded.position").map_err(err)?;
            for (position, t) in p.tracks.iter().enumerate() {
                cancelled(cancel)?;
                let id = t.track.id.to_string();
                removed.remove(&id);
                let json = serde_json::to_string(t).map_err(err)?;
                let previous: Option<(String, i32, i64)> = prior
                    .query_row(params![source_id, id], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })
                    .optional()
                    .map_err(err)?;
                if previous.as_ref().is_some_and(|(old, active, pos)| {
                    old == &json && *active == 1 && *pos == position as i64
                }) {
                    report.unchanged += 1;
                    continue;
                }
                if previous.is_some() {
                    report.updated += 1;
                } else {
                    report.added += 1;
                }
                report.changed_ids.insert(t.track.id);
                upsert
                    .execute(params![source_id, id, json, position as i64])
                    .map_err(err)?;
            }
            for id in removed {
                cancelled(cancel)?;
                tx.execute(
                    "UPDATE tracks SET active=0 WHERE source_id=?1 AND track_id=?2",
                    params![source_id, id],
                )
                .map_err(err)?;
                report.deactivated += 1;
            }
        }
        {
            let mut q = tx
                .prepare("SELECT playlist_id FROM playlists WHERE source_id=?1")
                .map_err(err)?;
            let mut removed = q
                .query_map([source_id], |r| r.get::<_, String>(0))
                .map_err(err)?
                .collect::<Result<HashSet<_>, _>>()
                .map_err(err)?;
            let mut prior=tx.prepare("SELECT name,items,position FROM playlists WHERE source_id=?1 AND playlist_id=?2").map_err(err)?;
            let mut upsert=tx.prepare("INSERT INTO playlists(source_id,playlist_id,name,items,position) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(source_id,playlist_id) DO UPDATE SET name=excluded.name,items=excluded.items,position=excluded.position").map_err(err)?;
            for (position, playlist) in p.playlists.iter().enumerate() {
                cancelled(cancel)?;
                removed.remove(&playlist.id);
                let json = serde_json::to_string(&playlist.tracks).map_err(err)?;
                let old: Option<(String, String, i64)> = prior
                    .query_row(params![source_id, playlist.id], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })
                    .optional()
                    .map_err(err)?;
                if old.as_ref().is_some_and(|(name, items, pos)| {
                    name == &playlist.name && items == &json && *pos == position as i64
                }) {
                    continue;
                }
                upsert
                    .execute(params![
                        source_id,
                        playlist.id,
                        playlist.name,
                        json,
                        position as i64
                    ])
                    .map_err(err)?;
                report.playlists_changed += 1;
            }
            for id in removed {
                cancelled(cancel)?;
                tx.execute(
                    "DELETE FROM playlists WHERE source_id=?1 AND playlist_id=?2",
                    params![source_id, id],
                )
                .map_err(err)?;
                report.playlists_changed += 1;
            }
        }
        cancelled(cancel)?;
        tx.commit().map_err(err)?;
        Ok(report)
    }
    pub fn snapshot(&self, source_id: i64) -> Result<Snapshot, String> {
        self.snapshot_with_checkpoint(source_id, || {})
    }
    fn snapshot_with_checkpoint(
        &self,
        source_id: i64,
        checkpoint: impl FnOnce(),
    ) -> Result<Snapshot, String> {
        let tx = self.connection.unchecked_transaction().map_err(err)?;
        let snapshot = (|| -> Result<Snapshot, String> {
            let source = tx
                .query_row(
                    "SELECT id,xml_url,media_root_url FROM sources WHERE id=?1",
                    [source_id],
                    |r| {
                        Ok(Source {
                            id: r.get(0)?,
                            xml_url: r.get(1)?,
                            media_root_url: r.get(2)?,
                        })
                    },
                )
                .map_err(err)?;
            checkpoint();
            file_url(&source.xml_url)?;
            if let Some(url) = &source.media_root_url {
                file_url(url)?;
            }
            let mut q=tx.prepare("SELECT track_id,CASE WHEN length(CAST(data AS BLOB))<=262144 THEN data ELSE '' END FROM tracks WHERE source_id=?1 AND active=1 ORDER BY position LIMIT 250001").map_err(err)?;
            let mut tracks = Vec::new();
            let mut paths = HashSet::new();
            for row in q
                .query_map([source_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(err)?
            {
                if tracks.len() >= MAX_TRACKS {
                    return Err("Source track limit exceeded".into());
                }
                let (id, json) = row.map_err(err)?;
                let track = serde_json::from_str::<PreparedTrack>(&json).map_err(err)?;
                if id != track.track.id.to_string() || !paths.insert(validate_track(&track)?) {
                    return Err("Invalid stored track identity".into());
                }
                tracks.push(track);
            }
            let ids: HashSet<_> = tracks.iter().map(|t| t.track.id).collect();
            let mut q=tx.prepare("SELECT playlist_id,name,CASE WHEN length(CAST(items AS BLOB))<=24000000 THEN items ELSE '' END FROM playlists WHERE source_id=?1 ORDER BY position LIMIT 4097").map_err(err)?;
            let mut playlists = Vec::new();
            let mut links = 0;
            for row in q
                .query_map([source_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(err)?
            {
                let (id, name, json) = row.map_err(err)?;
                let values: Vec<u64> = serde_json::from_str(&json).map_err(err)?;
                links += values.len();
                if playlists.len() >= 4096
                    || links > MAX_LINKS
                    || values.iter().any(|id| !ids.contains(id))
                {
                    return Err("Invalid stored playlist references".into());
                }
                playlists.push(Playlist {
                    id,
                    name,
                    tracks: values,
                });
            }
            Ok(Snapshot {
                source,
                tracks,
                playlists,
            })
        })()?;
        tx.commit().map_err(err)?;
        Ok(snapshot)
    }
}
