//! Durable overlays and thumbnail blobs share the bounded source catalog.
use crate::{
    apple::{thumbnail, Basis, Lookup, Metadata, Thumbnail},
    PreparedTrack, Store, MAX_TRACKS,
};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
const CACHE_BYTES: i64 = 128 * 1024 * 1024;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
impl Store {
    /// Original XML records are never overwritten by provider metadata.
    pub fn apple_metadata(
        &self,
        source: i64,
        tracks: &[PreparedTrack],
    ) -> Result<HashMap<u64, Metadata>, String> {
        let ids: HashMap<_, _> = tracks.iter().map(|t| (t.track.id, t)).collect();
        let mut q=self.connection.prepare("SELECT a.track_id,a.basis,a.metadata FROM apple_metadata a JOIN tracks t ON a.source_id=t.source_id AND a.track_id=t.track_id WHERE a.source_id=?1 AND t.active=1 LIMIT 250001").map_err(err)?;
        let mut out = HashMap::new();
        for (count, row) in q
            .query_map([source], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(err)?
            .enumerate()
        {
            if count >= MAX_TRACKS {
                return Err("Apple cache record limit exceeded".into());
            }
            let (id, basis, metadata) = row.map_err(err)?;
            if basis.len() > 262144 || metadata.len() > 8192 {
                return Err("Invalid Apple cache record".into());
            }
            let id = id.parse::<u64>().map_err(err)?;
            if let Some(t) = ids.get(&id) {
                let basis: Basis = serde_json::from_str(&basis).map_err(err)?;
                if t.file.available && basis == Basis::of(t) {
                    let metadata: Metadata = serde_json::from_str(&metadata).map_err(err)?;
                    metadata.validate()?;
                    out.insert(id, metadata);
                }
            }
        }
        Ok(out)
    }
    /// Re-checks source identity inside the write transaction. A stale lookup
    /// cannot overwrite a different active XML record with the same track ID.
    pub fn save_apple(
        &mut self,
        source: i64,
        row: &PreparedTrack,
        value: &Lookup,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        value.metadata.validate()?;
        if cancel() {
            return Err("Apple lookup cancelled".into());
        }
        if !row.file.available || value.artwork.as_ref().is_some_and(|a| a.len() > 524288) {
            return Err("Invalid Apple cache data".into());
        }
        if let Some(art) = &value.artwork {
            thumbnail(art)?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let json: Option<String> = tx
            .query_row(
                "SELECT data FROM tracks WHERE source_id=?1 AND track_id=?2 AND active=1",
                params![source, row.track.id.to_string()],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let current: PreparedTrack =
            serde_json::from_str(json.as_deref().ok_or("Source track is no longer active")?)
                .map_err(err)?;
        let basis = Basis::of(row);
        if !basis.same_source(&current) {
            return Err(
                "Source or file stamp changed; IMPORT / SYNC the XML before retrying Apple lookup"
                    .into(),
            );
        }
        let basis = serde_json::to_string(&basis).map_err(err)?;
        let metadata = serde_json::to_string(&value.metadata).map_err(err)?;
        let used:i64=tx.query_row("SELECT coalesce(sum(length(CAST(basis AS BLOB))+length(CAST(metadata AS BLOB))+coalesce(length(art),0)),0) FROM apple_metadata WHERE NOT(source_id=?1 AND track_id=?2)",params![source,row.track.id.to_string()],|r|r.get(0)).map_err(err)?;
        if used
            + basis.len() as i64
            + metadata.len() as i64
            + value.artwork.as_ref().map_or(0, |a| a.len() as i64)
            > CACHE_BYTES
        {
            return Err("Apple cache reached128MiB; existing cached data retained".into());
        }
        tx.execute("INSERT INTO apple_metadata(source_id,track_id,basis,metadata,art) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(source_id,track_id) DO UPDATE SET basis=excluded.basis,metadata=excluded.metadata,art=excluded.art",params![source,row.track.id.to_string(),basis,metadata,value.artwork]).map_err(err)?;
        if cancel() {
            return Err("Apple lookup cancelled".into());
        }
        tx.commit().map_err(err)
    }
    /// Decodes only the selected thumbnail on a worker, with image limits.
    pub fn apple_artwork(&self, source: i64, basis: &Basis) -> Result<Option<Thumbnail>, String> {
        let value:Option<(String,Option<Vec<u8>>)>=self.connection.query_row("SELECT basis,art FROM apple_metadata WHERE source_id=?1 AND track_id=?2 AND coalesce(length(art),0)<=524288",params![source,basis.track_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
        let Some((stored, art)) = value else {
            return Ok(None);
        };
        if stored.len() > 262144 || serde_json::from_str::<Basis>(&stored).map_err(err)? != *basis {
            return Ok(None);
        };
        art.map(|a| thumbnail(&a)).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_invalidation_stale_write_and_cancel_preserve_original() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("source.db");
        let row:PreparedTrack=serde_json::from_value(serde_json::json!({"track":{"id":1,"persistent_id":"P1","location":"file:///C:/song.wav","title":"Song","artist":"Artist","album":"Local","album_artist":"","genre":"","bpm":null,"rating":80,"play_count":10,"duration_ms":120000,"date_added":null},"file":{"path_url":"file:///C:/song.wav","available":true,"size":100,"modified_ms":1}})).unwrap();
        let mut store = Store::open(&db).unwrap();
        let source = store
            .sync(
                "file:///C:/Library.xml",
                None,
                &crate::Prepared {
                    tracks: vec![row.clone()],
                    ..Default::default()
                },
                &|| false,
            )
            .unwrap()
            .source_id;
        let lookup = Lookup {
            metadata: Metadata {
                song_id: "123".into(),
                title: "Song".into(),
                artist: "Artist".into(),
                album: "Apple".into(),
            },
            artwork: None,
        };
        store.save_apple(source, &row, &lookup, &|| false).unwrap();
        store
            .sync(
                "file:///C:/Library.xml",
                None,
                &crate::Prepared::default(),
                &|| false,
            )
            .unwrap();
        assert!(
            store
                .apple_metadata(source, std::slice::from_ref(&row))
                .unwrap()
                .is_empty(),
            "archived cache rows must not count as active records"
        );
        store
            .sync(
                "file:///C:/Library.xml",
                None,
                &crate::Prepared {
                    tracks: vec![row.clone()],
                    ..Default::default()
                },
                &|| false,
            )
            .unwrap();
        drop(store);
        let mut store = Store::open(&db).unwrap();
        assert_eq!(
            store
                .apple_metadata(source, std::slice::from_ref(&row))
                .unwrap()[&1]
                .album,
            "Apple"
        );
        assert!(store
            .apple_artwork(source, &Basis::of(&row))
            .unwrap()
            .is_none());
        assert_eq!(
            store.snapshot(source).unwrap().tracks[0].track.album,
            "Local"
        );
        let mut changed = row.clone();
        changed.file.modified_ms = Some(2);
        assert!(store
            .apple_metadata(source, std::slice::from_ref(&changed))
            .unwrap()
            .is_empty());
        store
            .sync(
                "file:///C:/Library.xml",
                None,
                &crate::Prepared {
                    tracks: vec![changed.clone()],
                    ..Default::default()
                },
                &|| false,
            )
            .unwrap();
        let newer = Lookup {
            metadata: Metadata {
                album: "Newer Apple".into(),
                ..lookup.metadata.clone()
            },
            artwork: None,
        };
        store
            .save_apple(source, &changed, &newer, &|| false)
            .unwrap();
        assert!(
            store.save_apple(source, &row, &lookup, &|| false).is_err(),
            "old lookup must not overwrite a newer file generation"
        );
        assert_eq!(
            store
                .apple_metadata(source, std::slice::from_ref(&changed))
                .unwrap()[&1]
                .album,
            "Newer Apple"
        );
        assert!(store.save_apple(source, &row, &lookup, &|| true).is_err());
        let mut replaced = row.clone();
        replaced.track.title = "Other".into();
        store
            .sync(
                "file:///C:/Library.xml",
                None,
                &crate::Prepared {
                    tracks: vec![replaced],
                    ..Default::default()
                },
                &|| false,
            )
            .unwrap();
        assert!(store.save_apple(source, &row, &lookup, &|| false).is_err());
        assert!(store
            .apple_metadata(source, &store.snapshot(source).unwrap().tracks)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn migrates_owned_v1_and_rejects_foreign_database() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v1.db");
        let s = Store::open(&p).unwrap();
        s.connection
            .execute_batch("DROP TABLE apple_metadata;PRAGMA user_version=1;")
            .unwrap();
        drop(s);
        let s = Store::open(&p).unwrap();
        assert_eq!(
            s.connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                .unwrap(),
            2
        );
    }
}
