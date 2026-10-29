//! SQLite catalog storage. The worker owns every database/file operation.
use crate::library::{self, Catalog, Library, SavedTrack};
use crate::preferences::QuantizePreferences;
const QUANTIZE_PREFS_QUERY: &str = "SELECT CASE WHEN length(CAST(value AS BLOB))<=1024 THEN value ELSE '' END FROM library_meta WHERE key='quantize_preferences_v1'";
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const APPLICATION_ID: i32 = 0x4452444a;
const SCHEMA_VERSION: i32 = 8;
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(60);
static STAGE_ID: AtomicU64 = AtomicU64::new(0);

pub fn default_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DRDJ_LIBRARY_PATH") {
        let path = PathBuf::from(path);
        return if path.is_absolute() {
            Ok(path)
        } else {
            Err("DRDJ_LIBRARY_PATH must be absolute".into())
        };
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")));
    base.filter(|p| p.is_absolute())
        .map(|p| p.join("DrDJPro/library.sqlite3"))
        .ok_or_else(|| "No absolute user data directory available".into())
}
pub(crate) fn side_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}
fn paths(requested: &Path) -> Result<(PathBuf, PathBuf), String> {
    if !requested.is_absolute() {
        return Err("Library path must be absolute".into());
    }
    if requested
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        Ok((requested.with_extension("sqlite3"), requested.to_owned()))
    } else {
        Ok((requested.to_owned(), requested.with_extension("json")))
    }
}
fn lock(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(side_path(path, ".lock"))
        .map_err(|e| e.to_string())?;
    file.try_lock()
        .map_err(|e| format!("Library is in use or cannot be locked: {e}"))?;
    Ok(file)
}
fn connect(path: &Path, create: bool) -> Result<Connection, String> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let connection = Connection::open_with_flags(path, flags).map_err(|e| e.to_string())?;
    connection
        .busy_timeout(Duration::from_millis(200))
        .map_err(|e| e.to_string())?;
    connection
        .pragma_update(None, "trusted_schema", false)
        .map_err(|e| e.to_string())?;
    Ok(connection)
}
#[derive(Debug)]
enum InspectError {
    Preserve(String),
    Damaged(String),
}
impl From<rusqlite::Error> for InspectError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Damaged(error.to_string())
    }
}
fn inspect(connection: &Connection) -> Result<Option<Catalog>, InspectError> {
    let app: i32 = connection.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let version: i32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if app == 0 && version == 0 {
        let count: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )?;
        if count == 0 {
            return Ok(None);
        }
    }
    if app != APPLICATION_ID {
        return Err(InspectError::Preserve(
            "Not a DrDJ library database; original preserved".into(),
        ));
    }
    if !(1..=SCHEMA_VERSION).contains(&version) {
        return Err(InspectError::Preserve(format!(
            "Unsupported SQLite library version {version}; original preserved"
        )));
    }
    let check: String = connection.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != "ok" {
        return Err(InspectError::Damaged(check));
    }
    let sql = if version == 1 {
        "SELECT id,path,path_key,'','','',NULL,0,NULL,0 FROM tracks ORDER BY id LIMIT ?1"
    } else {
        "SELECT t.id,t.path,t.path_key,COALESCE(m.title,''),COALESCE(m.artist,''),COALESCE(m.album,''),m.duration_ms,COALESCE(m.scanned,0),m.error,COALESCE(m.duration_exact,0) FROM tracks t LEFT JOIN track_metadata m ON m.track_id=t.id ORDER BY t.id LIMIT ?1"
    };
    let mut stmt = connection.prepare(sql)?;
    let mut rows = stmt.query([library::MAX_TRACKS as i64 + 1])?;
    let mut tracks = Vec::new();
    while let Some(row) = rows.next()? {
        let path = PathBuf::from(row.get::<_, String>(1)?);
        if row.get::<_, String>(2)? != library::identity(&path).to_string_lossy() {
            return Err(InspectError::Damaged(
                "Invalid library path identity".into(),
            ));
        }
        tracks.push(SavedTrack {
            hot_cues: Default::default(),
            saved_loops: Default::default(),
            grid: None,
            id: row.get(0)?,
            path,
            metadata: crate::track_metadata::TrackMetadata {
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                duration_ms: row
                    .get::<_, Option<i64>>(6)?
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| InspectError::Damaged("Negative metadata duration".into()))?,
                scanned: row.get(7)?,
                error: row.get(8)?,
                duration_exact: row.get(9)?,
            },
        });
    }
    if version >= 4 {
        let indices: HashMap<_, _> = tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
        let mut statement = connection.prepare(if version >= 8 {
            "SELECT track_id,bpm,anchor_ns,beats_per_bar,meter_changes FROM track_grids LIMIT ?1"
        } else if version >= 7 {
            "SELECT track_id,bpm,anchor_ns,beats_per_bar,'[]' FROM track_grids LIMIT ?1"
        } else {
            "SELECT track_id,bpm,anchor_ns,4,'[]' FROM track_grids LIMIT ?1"
        })?;
        let mut rows = statement.query([library::MAX_TRACKS as i64 + 1])?;
        let mut count = 0;
        while let Some(row) = rows.next()? {
            count += 1;
            if count > library::MAX_TRACKS {
                return Err(InspectError::Damaged("Too many saved grids".into()));
            }
            let index = indices
                .get(&row.get::<_, i32>(0)?)
                .ok_or_else(|| InspectError::Damaged("Orphaned track grid".into()))?;
            let anchor = u64::try_from(row.get::<_, i64>(2)?)
                .map_err(|_| InspectError::Damaged("Negative grid anchor".into()))?;
            tracks[*index].grid = Some(
                crate::beatgrid::BeatGrid::new(row.get(1)?, Duration::from_nanos(anchor))
                    .and_then(|grid| {
                        grid.with_beats_per_bar(row.get::<_, u8>(3).map_err(|e| e.to_string())?)
                    })
                    .map_err(InspectError::Damaged)?,
            );
            let json: String = row.get(4)?;
            if json.len() > 2048 {
                return Err(InspectError::Damaged("Oversized meter map".into()));
            }
            let changes: Vec<crate::beatgrid::MeterChange> =
                serde_json::from_str(&json).map_err(|e| InspectError::Damaged(e.to_string()))?;
            if changes.len() > crate::beatgrid::MAX_METER_CHANGES {
                return Err(InspectError::Damaged("Too many meter changes".into()));
            }
            let mut last = 1;
            let mut grid = tracks[*index].grid.unwrap();
            for change in changes {
                if change.bar <= last {
                    return Err(InspectError::Damaged(
                        "Unsorted or duplicate meter changes".into(),
                    ));
                }
                last = change.bar;
                grid = grid
                    .with_meter_change(change.bar, change.beats)
                    .map_err(InspectError::Damaged)?;
            }
            tracks[*index].grid = Some(grid);
        }
    }
    if version >= 5 {
        read_hot_cues(connection, &mut tracks)?;
    }
    if version >= 6 {
        read_saved_loops(connection, &mut tracks)?;
    }
    let crates = if version >= 3 {
        read_crates(connection)?
    } else {
        Default::default()
    };
    let catalog = Catalog {
        version: 1,
        tracks,
        crates,
    };
    catalog.validate().map_err(InspectError::Damaged)?;
    Ok(Some(catalog))
}
fn read_crates(connection: &Connection) -> Result<crate::crates::CrateCatalog, InspectError> {
    let value: String = connection.query_row(
        "SELECT value FROM library_meta WHERE key='last_crate_id'",
        [],
        |r| r.get(0),
    )?;
    let mut catalog = crate::crates::CrateCatalog {
        last_id: value
            .parse()
            .map_err(|_| InspectError::Damaged("Invalid crate ID sequence".into()))?,
        entries: Vec::new(),
    };
    let mut stmt =
        connection.prepare("SELECT id,name,name_key FROM crates ORDER BY id LIMIT ?1")?;
    let mut rows = stmt.query([crate::crates::MAX_CRATES as i64 + 1])?;
    let mut indices = HashMap::new();
    while let Some(row) = rows.next()? {
        let id: i32 = row.get(0)?;
        let name: String = row.get(1)?;
        if row.get::<_, String>(2)? != crate::crates::name_key(&name) {
            return Err(InspectError::Damaged("Invalid crate name identity".into()));
        }
        indices.insert(id, catalog.entries.len());
        catalog.entries.push(crate::crates::Crate {
            id,
            name,
            tracks: HashSet::new(),
        });
    }
    let mut stmt = connection.prepare("SELECT crate_id,track_id FROM crate_tracks LIMIT ?1")?;
    let mut rows = stmt.query([crate::crates::MAX_MEMBERSHIPS as i64 + 1])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        count += 1;
        if count > crate::crates::MAX_MEMBERSHIPS {
            return Err(InspectError::Damaged(
                "Crate membership limit exceeded".into(),
            ));
        }
        let id: i32 = row.get(0)?;
        let index = indices
            .get(&id)
            .ok_or_else(|| InspectError::Damaged("Orphaned crate membership".into()))?;
        catalog.entries[*index].tracks.insert(row.get(1)?);
    }
    Ok(catalog)
}
fn create_saved_loop_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE track_saved_loops(
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        slot INTEGER NOT NULL CHECK(slot>=0 AND slot<8),
        start_ns INTEGER NOT NULL CHECK(start_ns>=0),
        end_ns INTEGER NOT NULL CHECK(end_ns>start_ns AND end_ns<=604800000000000),
        name TEXT NOT NULL CHECK(length(name)<=128), PRIMARY KEY(track_id,slot)) STRICT;",
        )
        .map_err(|e| e.to_string())
}
fn read_saved_loops(
    connection: &Connection,
    tracks: &mut [SavedTrack],
) -> Result<(), InspectError> {
    let indices: HashMap<_, _> = tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
    let limit = library::MAX_TRACKS * crate::saved_loops::SLOT_COUNT;
    let mut statement = connection
        .prepare("SELECT track_id,slot,start_ns,end_ns,name FROM track_saved_loops LIMIT ?1")?;
    let mut rows = statement.query([limit as i64 + 1])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        count += 1;
        if count > limit {
            return Err(InspectError::Damaged("Too many saved loops".into()));
        }
        let index = indices
            .get(&row.get::<_, i32>(0)?)
            .ok_or_else(|| InspectError::Damaged("Orphaned saved loop".into()))?;
        let slot = usize::try_from(row.get::<_, i64>(1)?)
            .map_err(|_| InspectError::Damaged("Negative loop slot".into()))?;
        if tracks[*index]
            .saved_loops
            .slots()
            .get(slot)
            .is_some_and(Option::is_some)
        {
            return Err(InspectError::Damaged("Duplicate saved loop slot".into()));
        }
        let start = u64::try_from(row.get::<_, i64>(2)?)
            .map_err(|_| InspectError::Damaged("Negative loop start".into()))?;
        let end = u64::try_from(row.get::<_, i64>(3)?)
            .map_err(|_| InspectError::Damaged("Negative loop end".into()))?;
        let value = crate::saved_loops::SavedLoop::new(
            Duration::from_nanos(start),
            Duration::from_nanos(end),
            row.get(4)?,
        )
        .map_err(InspectError::Damaged)?;
        tracks[*index]
            .saved_loops
            .set(slot, Some(value))
            .map_err(InspectError::Damaged)?;
    }
    Ok(())
}
fn write_saved_loops(connection: &Connection, catalog: &Catalog) -> Result<(), String> {
    connection
        .execute("DELETE FROM track_saved_loops", [])
        .map_err(|e| e.to_string())?;
    write_saved_loops_rows(connection, &catalog.tracks)
}
fn write_saved_loops_rows(connection: &Connection, tracks: &[SavedTrack]) -> Result<(), String> {
    let mut statement=connection.prepare("INSERT INTO track_saved_loops(track_id,slot,start_ns,end_ns,name) VALUES(?1,?2,?3,?4,?5)").map_err(|e|e.to_string())?;
    for track in tracks {
        for (slot, value) in track.saved_loops.slots().iter().enumerate() {
            if let Some(value) = value {
                statement
                    .execute(rusqlite::params![
                        track.id,
                        slot as i64,
                        value.start().as_nanos() as i64,
                        value.end().as_nanos() as i64,
                        value.name()
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
fn create_hot_cue_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE track_hot_cues(
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        slot INTEGER NOT NULL CHECK(slot>=0 AND slot<8),
        position_ns INTEGER NOT NULL CHECK(position_ns>=0 AND position_ns<=604800000000000),
        name TEXT NOT NULL CHECK(length(name)<=128),
        rgb INTEGER NOT NULL CHECK(rgb>=0 AND rgb<=16777215),
        PRIMARY KEY(track_id,slot)) STRICT;",
        )
        .map_err(|e| e.to_string())
}
fn read_hot_cues(connection: &Connection, tracks: &mut [SavedTrack]) -> Result<(), InspectError> {
    let indices: HashMap<_, _> = tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
    let limit = library::MAX_TRACKS * crate::hot_cues::SLOT_COUNT;
    let mut statement = connection
        .prepare("SELECT track_id,slot,position_ns,name,rgb FROM track_hot_cues LIMIT ?1")?;
    let mut rows = statement.query([limit as i64 + 1])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        count += 1;
        if count > limit {
            return Err(InspectError::Damaged("Too many saved hot cues".into()));
        }
        let index = indices
            .get(&row.get::<_, i32>(0)?)
            .ok_or_else(|| InspectError::Damaged("Orphaned hot cue".into()))?;
        let slot = usize::try_from(row.get::<_, i64>(1)?)
            .map_err(|_| InspectError::Damaged("Negative cue slot".into()))?;
        if tracks[*index]
            .hot_cues
            .slots()
            .get(slot)
            .is_some_and(Option::is_some)
        {
            return Err(InspectError::Damaged("Duplicate hot cue slot".into()));
        }
        let ns = u64::try_from(row.get::<_, i64>(2)?)
            .map_err(|_| InspectError::Damaged("Negative cue position".into()))?;
        let rgb = u32::try_from(row.get::<_, i64>(4)?)
            .map_err(|_| InspectError::Damaged("Invalid cue color".into()))?;
        let cue = crate::hot_cues::HotCue::new(Duration::from_nanos(ns), row.get(3)?, rgb)
            .map_err(InspectError::Damaged)?;
        tracks[*index]
            .hot_cues
            .set(slot, Some(cue))
            .map_err(InspectError::Damaged)?;
    }
    Ok(())
}
fn write_hot_cues(connection: &Connection, catalog: &Catalog) -> Result<(), String> {
    connection
        .execute("DELETE FROM track_hot_cues", [])
        .map_err(|e| e.to_string())?;
    write_hot_cues_rows(connection, &catalog.tracks)
}
fn write_hot_cues_rows(connection: &Connection, tracks: &[SavedTrack]) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "INSERT INTO track_hot_cues(track_id,slot,position_ns,name,rgb) VALUES(?1,?2,?3,?4,?5)",
        )
        .map_err(|e| e.to_string())?;
    for track in tracks {
        for (slot, cue) in track.hot_cues.slots().iter().enumerate() {
            if let Some(cue) = cue {
                statement
                    .execute(rusqlite::params![
                        track.id,
                        slot as i64,
                        cue.position().as_nanos() as i64,
                        cue.name(),
                        cue.rgb()
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
fn create_grid_schema(connection: &Connection) -> Result<(), String> {
    connection.execute_batch("CREATE TABLE track_grids(track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,bpm REAL NOT NULL CHECK(bpm>=40 AND bpm<=240),anchor_ns INTEGER NOT NULL CHECK(anchor_ns>=0 AND anchor_ns<=604800000000000)) STRICT;").map_err(|e|e.to_string())
}
fn add_grid_meter(connection: &Connection) -> Result<(), String> {
    connection.execute_batch("ALTER TABLE track_grids ADD COLUMN beats_per_bar INTEGER NOT NULL DEFAULT 4 CHECK(beats_per_bar BETWEEN 1 AND 16);").map_err(|e| e.to_string())
}
fn add_grid_meter_changes(connection: &Connection) -> Result<(), String> {
    connection.execute_batch("ALTER TABLE track_grids ADD COLUMN meter_changes TEXT NOT NULL DEFAULT '[]' CHECK(length(meter_changes)<=2048);").map_err(|e| e.to_string())
}
fn write_grids(connection: &Connection, catalog: &Catalog) -> Result<(), String> {
    connection
        .execute("DELETE FROM track_grids", [])
        .map_err(|e| e.to_string())?;
    write_grids_rows(connection, &catalog.tracks)
}
fn write_grids_rows(connection: &Connection, tracks: &[SavedTrack]) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "INSERT INTO track_grids(track_id,bpm,anchor_ns,beats_per_bar,meter_changes) VALUES(?1,?2,?3,?4,?5)",
        )
        .map_err(|e| e.to_string())?;
    for track in tracks {
        if let Some(grid) = track.grid {
            statement
                .execute(rusqlite::params![
                    track.id,
                    grid.bpm(),
                    grid.anchor().as_nanos() as i64,
                    grid.beats_per_bar(),
                    serde_json::to_string(grid.meter_changes()).map_err(|e| e.to_string())?
                ])
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn create_crate_schema(connection: &Connection) -> Result<(), String> {
    connection.execute_batch("CREATE TABLE crates(id INTEGER PRIMARY KEY CHECK(id>0 AND id<2147483647),name TEXT NOT NULL CHECK(length(name)>0 AND length(name)<=128),name_key TEXT NOT NULL UNIQUE) STRICT;
        CREATE TABLE crate_tracks(crate_id INTEGER NOT NULL REFERENCES crates(id) ON DELETE CASCADE,track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,PRIMARY KEY(crate_id,track_id)) STRICT;
        CREATE INDEX crate_tracks_track ON crate_tracks(track_id);
        INSERT INTO library_meta(key,value) VALUES('last_crate_id','0');").map_err(|e|e.to_string())
}
fn write_crates(
    connection: &Connection,
    catalog: &crate::crates::CrateCatalog,
) -> Result<(), String> {
    // Replace this bounded relationship snapshot inside the same transaction as track updates.
    connection
        .execute_batch("DELETE FROM crate_tracks; DELETE FROM crates;")
        .map_err(|e| e.to_string())?;
    let mut crates = connection
        .prepare("INSERT INTO crates(id,name,name_key) VALUES(?1,?2,?3)")
        .map_err(|e| e.to_string())?;
    let mut members = connection
        .prepare("INSERT INTO crate_tracks(crate_id,track_id) VALUES(?1,?2)")
        .map_err(|e| e.to_string())?;
    for entry in &catalog.entries {
        crates
            .execute(rusqlite::params![
                entry.id,
                entry.name,
                crate::crates::name_key(&entry.name)
            ])
            .map_err(|e| e.to_string())?;
        for track in &entry.tracks {
            members
                .execute(rusqlite::params![entry.id, track])
                .map_err(|e| e.to_string())?;
        }
    }
    connection.execute("INSERT INTO library_meta(key,value) VALUES('last_crate_id',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[catalog.last_id.to_string()]).map_err(|e|e.to_string())?;
    Ok(())
}
fn configure(connection: &Connection) -> Result<(), String> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|e| e.to_string())?;
    // One storage connection: rollback journaling avoids unnecessary WAL sidecars.
    let mode: String = connection
        .query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err("Cannot enable SQLite rollback journaling".into());
    }
    connection
        .pragma_update(None, "synchronous", "EXTRA")
        .map_err(|e| e.to_string())
}
fn write_tracks(connection: &Connection, catalog: &Catalog) -> Result<(), String> {
    let old = inspect(connection)
        .map_err(|e| format!("Cannot read catalog before save: {e:?}"))?
        .map_or_else(Vec::new, |c| c.tracks);
    let old: HashMap<_, _> = old.into_iter().map(|t| (t.id, t.path)).collect();
    let new: HashMap<_, _> = catalog.tracks.iter().map(|t| (t.id, &t.path)).collect();
    // Delete changed/removed identities first so path swaps cannot violate uniqueness midway.
    let mut remove = connection
        .prepare("DELETE FROM tracks WHERE id=?1")
        .map_err(|e| e.to_string())?;
    for (id, path) in &old {
        if new.get(id).is_none_or(|new_path| **new_path != *path) {
            remove.execute([id]).map_err(|e| e.to_string())?;
        }
    }
    let mut insert = connection
        .prepare("INSERT INTO tracks(id,path,path_key) VALUES (?1,?2,?3)")
        .map_err(|e| e.to_string())?;
    for track in &catalog.tracks {
        if old.get(&track.id) == Some(&track.path) {
            continue;
        }
        let path = track
            .path
            .to_str()
            .ok_or("Library path is not valid UTF-8")?;
        let key = library::identity(&track.path);
        insert
            .execute(rusqlite::params![
                track.id,
                path,
                key.to_str().ok_or("Invalid path identity")?
            ])
            .map_err(|e| e.to_string())?;
    }
    let mut meta = connection.prepare("INSERT INTO track_metadata(track_id,title,artist,album,duration_ms,scanned,error,duration_exact) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
        ON CONFLICT(track_id) DO UPDATE SET title=excluded.title,artist=excluded.artist,album=excluded.album,duration_ms=excluded.duration_ms,scanned=excluded.scanned,error=excluded.error,duration_exact=excluded.duration_exact").map_err(|e| e.to_string())?;
    for track in &catalog.tracks {
        let m = &track.metadata;
        meta.execute(rusqlite::params![
            track.id,
            m.title,
            m.artist,
            m.album,
            m.duration_ms.map(|ms| ms as i64),
            m.scanned,
            m.error,
            m.duration_exact
        ])
        .map_err(|e| e.to_string())?;
    }
    write_crates(connection, &catalog.crates)?;
    write_grids(connection, catalog)?;
    write_hot_cues(connection, catalog)?;
    write_saved_loops(connection, catalog)
}
fn create_metadata_schema(connection: &Connection) -> Result<(), String> {
    connection.execute_batch("CREATE TABLE track_metadata(track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
        title TEXT NOT NULL CHECK(length(title)<=512), artist TEXT NOT NULL CHECK(length(artist)<=512), album TEXT NOT NULL CHECK(length(album)<=512),
        duration_ms INTEGER CHECK(duration_ms>=0 AND duration_ms<=604800000), scanned INTEGER NOT NULL CHECK(scanned IN (0,1)), error TEXT CHECK(length(error)<=512), duration_exact INTEGER NOT NULL CHECK(duration_exact IN (0,1)) CHECK(duration_exact=0 OR duration_ms IS NOT NULL)) STRICT;")
        .map_err(|e| e.to_string())
}
fn initialize(connection: &mut Connection, catalog: &Catalog, source: &str) -> Result<(), String> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    transaction.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY CHECK(id>0 AND id<2147483647), path TEXT NOT NULL, path_key TEXT NOT NULL UNIQUE) STRICT;
        CREATE TABLE library_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;").map_err(|e| e.to_string())?;
    create_metadata_schema(&transaction)?;
    create_crate_schema(&transaction)?;
    create_grid_schema(&transaction)?;
    add_grid_meter(&transaction)?;
    add_grid_meter_changes(&transaction)?;
    create_hot_cue_schema(&transaction)?;
    create_saved_loop_schema(&transaction)?;
    transaction
        .pragma_update(None, "application_id", APPLICATION_ID)
        .map_err(|e| e.to_string())?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(|e| e.to_string())?;
    transaction
        .execute(
            "INSERT INTO library_meta(key,value) VALUES ('initial_source',?1)",
            [source],
        )
        .map_err(|e| e.to_string())?;
    write_tracks(&transaction, catalog)?;
    transaction.commit().map_err(|e| e.to_string())
}
fn stage(path: &Path) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = side_path(
        path,
        &format!(
            ".stage-{}-{stamp}-{}",
            std::process::id(),
            STAGE_ID.fetch_add(1, Ordering::Relaxed)
        ),
    );
    File::options()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    Ok(path)
}
fn backup_to(connection: &Connection, path: &Path) -> Result<(), String> {
    let temp = stage(path)?;
    let result = (|| {
        connection
            .backup(rusqlite::MAIN_DB, &temp, rusqlite::backup::NO_PROGRESS)
            .map_err(|e| e.to_string())?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temp)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn recover(path: &Path) -> Result<(), String> {
    for suffix in ["-journal", "-wal", "-shm"] {
        if side_path(path, suffix)
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            return Err("Damaged database has recovery sidecars; original preserved".into());
        }
    }
    let backup = side_path(path, ".bak");
    let source = Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    source
        .pragma_update(None, "trusted_schema", false)
        .map_err(|e| e.to_string())?;
    inspect(&source)
        .map_err(|e| format!("SQLite backup unavailable: {e:?}"))?
        .ok_or("SQLite backup is uninitialized")?;
    if path.try_exists().map_err(|e| e.to_string())? {
        let preserved = stage(&side_path(path, ".damaged"))?;
        let mut output = OpenOptions::new()
            .write(true)
            .open(&preserved)
            .map_err(|e| e.to_string())?;
        io::copy(
            &mut File::open(path).map_err(|e| e.to_string())?,
            &mut output,
        )
        .map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
    }
    backup_to(&source, path)
}
struct Store {
    last_checkpoint: Option<Instant>,
    checkpoint_empty: bool,
    backup_pending: bool,
    has_tracks: bool,
    backfill_error: Option<String>,
    path: PathBuf,
    connection: Connection,
    _locks: [File; 2],
}
impl Store {
    fn open(requested: PathBuf) -> Result<(Self, Library, String), String> {
        let (path, legacy) = paths(&requested)?;
        fs::create_dir_all(path.parent().ok_or("Library path has no parent")?)
            .map_err(|e| e.to_string())?;
        // The second lock also excludes a still-running JSON-era application.
        let locks = [lock(&path)?, lock(&legacy)?];
        let exists = path.try_exists().map_err(|e| e.to_string())?;
        let opened = if exists {
            connect(&path, false)
                .map_err(InspectError::Damaged)
                .and_then(|connection| {
                    let catalog = inspect(&connection)?;
                    Ok((connection, catalog))
                })
                .map(Some)
        } else {
            Ok(None)
        };
        let has_backup = side_path(&path, ".bak")
            .try_exists()
            .map_err(|e| e.to_string())?;
        let (mut connection, catalog, note) = match opened {
            Ok(Some((connection, Some(catalog)))) => {
                (connection, catalog, "SQLite library restored".to_string())
            }
            Err(InspectError::Preserve(error)) => return Err(error),
            Err(InspectError::Damaged(error)) if !has_backup => {
                return Err(format!(
                    "SQLite library damaged; original preserved: {error}"
                ))
            }
            other => {
                // Drop any empty/uninitialized connection before Windows file replacement.
                drop(other);
                if has_backup {
                    recover(&path)?;
                    let connection = connect(&path, false)?;
                    let catalog = inspect(&connection)
                        .map_err(|e| format!("Recovered database invalid: {e:?}"))?
                        .ok_or("Recovered database empty")?;
                    (
                        connection,
                        catalog,
                        "SQLite library recovered from backup".to_string(),
                    )
                } else {
                    // Validate legacy input BEFORE creating or modifying the database.
                    let (catalog, source) = crate::legacy_catalog::load(&legacy)?;
                    let mut connection = connect(&path, true)?;
                    configure(&connection)?;
                    initialize(&mut connection, &catalog, source)?;
                    let note = if source == "New SQLite library" {
                        source.into()
                    } else {
                        format!("Migrated {source} to SQLite")
                    };
                    (connection, catalog, note)
                }
            }
        };
        configure(&connection)?;
        let version: i32 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if version < SCHEMA_VERSION {
            backup_to(&connection, &side_path(&path, ".bak"))?;
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            if version == 1 {
                create_metadata_schema(&tx)?;
            }
            if version < 3 {
                create_crate_schema(&tx)?;
            }
            if version < 4 {
                create_grid_schema(&tx)?;
            }
            if version < 5 {
                create_hot_cue_schema(&tx)?;
            }
            if version < 6 {
                create_saved_loop_schema(&tx)?;
            }
            if version < 7 {
                add_grid_meter(&tx)?;
            }
            if version < 8 {
                add_grid_meter_changes(&tx)?;
            }
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
        let pending: std::collections::HashMap<_, _> = catalog
            .tracks
            .iter()
            .filter(|t| t.metadata.needs_refresh(&t.path))
            .map(|t| (t.id, t.metadata.clone()))
            .collect();
        let mut library = Library::restore(catalog)?;
        let mut store = Self {
            last_checkpoint: None,
            checkpoint_empty: true,
            backup_pending: false,
            has_tracks: library.len() > 0,
            backfill_error: None,
            path,
            connection,
            _locks: locks,
        };
        let enriched = library.catalog();
        if enriched
            .tracks
            .iter()
            .any(|t| pending.get(&t.id).is_some_and(|old| old != &t.metadata) && t.metadata.scanned)
        {
            // Reading succeeded. Optional enrichment persistence must not hide that catalog.
            store.backfill_error = store.save(&enriched).err();
            if store.backfill_error.is_none() {
                library.acknowledge(library.revision());
            }
        }
        Ok((store, library, note))
    }

    fn checkpoint(&mut self, force: bool) -> Result<(), String> {
        let due = force
            || self.last_checkpoint.is_none()
            || (self.backup_pending
                && (self
                    .last_checkpoint
                    .is_some_and(|t| t.elapsed() >= CHECKPOINT_INTERVAL)
                    || (self.checkpoint_empty && self.has_tracks)));
        if !due {
            return Ok(());
        }
        let catalog = inspect(&self.connection)
            .map_err(|e| format!("Cannot checkpoint damaged catalog: {e:?}"))?
            .ok_or("Catalog not initialized")?;
        backup_to(&self.connection, &side_path(&self.path, ".bak"))?;
        self.checkpoint_empty = catalog.tracks.is_empty();
        self.last_checkpoint = Some(Instant::now());
        self.backup_pending = false;
        Ok(())
    }
    fn read_quantize_preferences(&self) -> Result<QuantizePreferences, String> {
        let value: Option<String> = self
            .connection
            .query_row(QUANTIZE_PREFS_QUERY, [], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(value) = value else {
            return Ok(QuantizePreferences::default());
        };
        let preferences: QuantizePreferences = serde_json::from_str(&value)
            .map_err(|e| format!("Invalid Quantize preferences: {e}"))?;
        preferences.validate()?;
        Ok(preferences)
    }
    #[cfg(test)]
    fn save_delta(&mut self, delta: &crate::library::CatalogDelta) -> Result<(), String> {
        self.save_state(delta, None)
    }
    fn save_state(
        &mut self,
        delta: &crate::library::CatalogDelta,
        preferences: Option<QuantizePreferences>,
    ) -> Result<(), String> {
        let preferences_json = preferences
            .map(|p| {
                p.validate()?;
                serde_json::to_string(&p).map_err(|e| e.to_string())
            })
            .transpose()?;
        if delta.tracks.is_empty()
            && delta.crates.is_none()
            && delta.memberships.is_empty()
            && preferences.is_none()
        {
            return self.checkpoint(true);
        }
        Catalog {
            version: 1,
            tracks: delta.tracks.clone(),
            crates: Default::default(),
        }
        .validate()?;
        self.checkpoint(false)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let app: i32 = tx
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let version: i32 = tx
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if app != APPLICATION_ID || version != SCHEMA_VERSION {
            return Err("Library schema changed; original preserved".into());
        }
        let mut added = 0;
        for track in &delta.tracks {
            let path = track
                .path
                .to_str()
                .ok_or("Library path is not valid UTF-8")?;
            let existing: Option<String> = tx
                .query_row("SELECT path FROM tracks WHERE id=?1", [track.id], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(|e| e.to_string())?;
            if existing.as_ref().is_some_and(|old| old != path) {
                return Err("Delta cannot change track identity/path".into());
            }
            if existing.is_none() {
                added += 1;
            }
            let key = library::identity(&track.path);
            tx.execute(
                "INSERT INTO tracks(id,path,path_key) VALUES(?1,?2,?3) ON CONFLICT(id) DO NOTHING",
                rusqlite::params![track.id, path, key.to_str().ok_or("Invalid path identity")?],
            )
            .map_err(|e| e.to_string())?;
            let m = &track.metadata;
            tx.execute("INSERT INTO track_metadata(track_id,title,artist,album,duration_ms,scanned,error,duration_exact) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
                ON CONFLICT(track_id) DO UPDATE SET title=excluded.title,artist=excluded.artist,album=excluded.album,duration_ms=excluded.duration_ms,scanned=excluded.scanned,error=excluded.error,duration_exact=excluded.duration_exact
                WHERE track_metadata.title IS NOT excluded.title OR track_metadata.artist IS NOT excluded.artist OR track_metadata.album IS NOT excluded.album OR track_metadata.duration_ms IS NOT excluded.duration_ms OR track_metadata.scanned IS NOT excluded.scanned OR track_metadata.error IS NOT excluded.error OR track_metadata.duration_exact IS NOT excluded.duration_exact",
                rusqlite::params![track.id,m.title,m.artist,m.album,m.duration_ms.map(|v|v as i64),m.scanned,m.error,m.duration_exact]).map_err(|e|e.to_string())?;
            // Fixed-size cue/loop sets keep per-track writes bounded, independent of catalog size.
            for sql in [
                "DELETE FROM track_grids WHERE track_id=?1",
                "DELETE FROM track_hot_cues WHERE track_id=?1",
                "DELETE FROM track_saved_loops WHERE track_id=?1",
            ] {
                tx.execute(sql, [track.id]).map_err(|e| e.to_string())?;
            }
        }
        if added > 0 {
            let count: i64 = tx
                .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            if count > library::MAX_TRACKS as i64 {
                return Err("Library track limit exceeded".into());
            }
        }
        write_grids_rows(&tx, &delta.tracks)?;
        write_hot_cues_rows(&tx, &delta.tracks)?;
        write_saved_loops_rows(&tx, &delta.tracks)?;
        if let Some(crates) = &delta.crates {
            let previous: String = tx
                .query_row(
                    "SELECT value FROM library_meta WHERE key='last_crate_id'",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            let previous: i32 = previous.parse().map_err(|_| "Invalid crate ID sequence")?;
            if crates.last_id < previous || crates.last_id == i32::MAX {
                return Err("Invalid crate ID sequence".into());
            }
            let mut ids = HashSet::new();
            for (id, name) in &crates.entries {
                if *id <= 0 || *id > crates.last_id || !ids.insert(*id) {
                    return Err("Invalid crate identity".into());
                }
                if let Some(name) = name {
                    crate::crates::CrateCatalog {
                        last_id: crates.last_id,
                        entries: vec![crate::crates::Crate {
                            id: *id,
                            name: name.clone(),
                            tracks: HashSet::new(),
                        }],
                    }
                    .validate(&HashSet::new())?;
                }
            }
            // Delete first, then reserve temporary keys for all renamed rows. A NUL-prefixed
            // key cannot be produced by a valid crate name and exists only inside this transaction.
            for (id, name) in &crates.entries {
                if name.is_none() {
                    tx.execute("DELETE FROM crates WHERE id=?1", [id])
                        .map_err(|e| e.to_string())?;
                }
            }
            for (id, name) in &crates.entries {
                if name.is_some() {
                    tx.execute(
                        "UPDATE crates SET name_key=char(0)||CAST(id AS TEXT) WHERE id=?1",
                        [id],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
            for (id, name) in &crates.entries {
                if let Some(name) = name {
                    tx.execute("INSERT INTO crates(id,name,name_key) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name,name_key=excluded.name_key", rusqlite::params![id, name, crate::crates::name_key(name)]).map_err(|e| e.to_string())?;
                }
            }
            let count: i64 = tx
                .query_row("SELECT COUNT(*) FROM crates", [], |row| row.get(0))
                .map_err(|e| e.to_string())?;
            if count > crate::crates::MAX_CRATES as i64 {
                return Err("Crate limit exceeded".into());
            }
            tx.execute(
                "UPDATE library_meta SET value=?1 WHERE key='last_crate_id'",
                [crates.last_id.to_string()],
            )
            .map_err(|e| e.to_string())?;
        }
        for &(crate_id, track_id, present) in &delta.memberships {
            if crate_id <= 0 || crate_id == i32::MAX || track_id <= 0 || track_id == i32::MAX {
                return Err("Invalid crate membership identity".into());
            }
            let valid: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM crates WHERE id=?1) AND EXISTS(SELECT 1 FROM tracks WHERE id=?2)",
                    rusqlite::params![crate_id, track_id],
                    |row| row.get(0),
                ).map_err(|e| e.to_string())?;
            if !valid {
                return Err("Invalid crate membership identity".into());
            }
            let sql = if present {
                "INSERT INTO crate_tracks(crate_id,track_id) VALUES(?1,?2) ON CONFLICT(crate_id,track_id) DO NOTHING"
            } else {
                "DELETE FROM crate_tracks WHERE crate_id=?1 AND track_id=?2"
            };
            tx.execute(sql, rusqlite::params![crate_id, track_id])
                .map_err(|e| e.to_string())?;
        }
        if let Some(json) = preferences_json {
            tx.execute("INSERT INTO library_meta(key,value) VALUES('quantize_preferences_v1',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        self.has_tracks = self.has_tracks || added > 0;
        self.backup_pending = true;
        Ok(())
    }
    fn save(&mut self, catalog: &Catalog) -> Result<(), String> {
        catalog.validate()?;
        if catalog.tracks.iter().any(|t| t.path.to_str().is_none()) {
            return Err("Library path is not valid UTF-8".into());
        }
        // A complete SQLite backup of the previous committed state, never a raw live-file copy.
        inspect(&self.connection)
            .map_err(|e| format!("Cannot save damaged catalog: {e:?}"))?
            .ok_or("Catalog not initialized")?;
        backup_to(&self.connection, &side_path(&self.path, ".bak"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        write_tracks(&transaction, catalog)?;
        transaction.commit().map_err(|e| e.to_string())?;
        self.has_tracks = !catalog.tracks.is_empty();
        self.last_checkpoint = None;
        self.backup_pending = true;
        Ok(())
    }
}

pub struct LoadedLibrary {
    pub library: Library,
    pub note: String,
    pub backfill_error: Option<String>,
    pub quantize_preferences: QuantizePreferences,
    pub preference_warning: Option<String>,
}
pub enum Event {
    Loaded(Result<Box<LoadedLibrary>, String>),
    Saved(Result<(), String>),
    CheckpointFailed(String),
}
enum Command {
    #[cfg(test)]
    Save(Catalog),
    #[cfg(test)]
    Close(Option<Catalog>),
    SaveDelta(crate::library::CatalogDelta, Option<QuantizePreferences>),
    CloseDelta(
        Option<crate::library::CatalogDelta>,
        Option<QuantizePreferences>,
    ),
}
pub struct Worker {
    tx: SyncSender<Command>,
    pub events: Receiver<Event>,
    join: JoinHandle<Result<(), String>>,
}
impl Worker {
    pub fn start(path: PathBuf) -> Result<Self, String> {
        Self::start_with_checkpoint_interval(path, CHECKPOINT_INTERVAL)
    }
    fn start_with_checkpoint_interval(path: PathBuf, interval: Duration) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        let (events_tx, events) = mpsc::channel();
        let join = thread::Builder::new()
            .name("library-storage".into())
            .spawn(move || {
                let (mut store, library, note) = match Store::open(path) {
                    Ok(loaded) => loaded,
                    Err(error) => {
                        let _ = events_tx.send(Event::Loaded(Err(error.clone())));
                        return Err(error);
                    }
                };
                let (quantize_preferences, preference_warning) =
                    match store.read_quantize_preferences() {
                        Ok(prefs) => (prefs, None),
                        Err(error) => (QuantizePreferences::default(), Some(error)),
                    };
                let loaded = LoadedLibrary {
                    library,
                    note,
                    backfill_error: store.backfill_error.take(),
                    quantize_preferences,
                    preference_warning,
                };
                if events_tx.send(Event::Loaded(Ok(Box::new(loaded)))).is_err() {
                    return Ok(());
                }
                let mut retry_wait = false;
                loop {
                    let timeout = if !retry_wait && store.backup_pending {
                        store
                            .last_checkpoint
                            .map_or(interval, |time| interval.saturating_sub(time.elapsed()))
                    } else {
                        interval
                    };
                    let command = match rx.recv_timeout(timeout) {
                        Ok(command) => command,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if store.backup_pending {
                                let result = store.checkpoint(true);
                                retry_wait = result.is_err();
                                if let Err(error) = result {
                                    let _ = events_tx.send(Event::CheckpointFailed(error));
                                }
                            }
                            continue;
                        }
                    };
                    match command {
                        Command::SaveDelta(delta, preferences) => {
                            let result = store.save_state(&delta, preferences);
                            retry_wait = result.is_err();
                            let _ = events_tx.send(Event::Saved(result));
                        }
                        Command::CloseDelta(delta, preferences) => {
                            if delta.is_some() || preferences.is_some() {
                                let delta = delta.unwrap_or(crate::library::CatalogDelta {
                                    revision: 0,
                                    tracks: vec![],
                                    crates: None,
                                    memberships: vec![],
                                });
                                store.save_state(&delta, preferences)?;
                            }
                            return store.checkpoint(true);
                        }
                        #[cfg(test)]
                        Command::Save(catalog) => {
                            let result = store.save(&catalog);
                            retry_wait = result.is_err();
                            let _ = events_tx.send(Event::Saved(result));
                        }
                        #[cfg(test)]
                        Command::Close(catalog) => {
                            return catalog.map_or(Ok(()), |catalog| store.save(&catalog))
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx, events, join })
    }
    #[cfg(test)]
    pub fn save_delta(&self, delta: crate::library::CatalogDelta) -> Result<(), String> {
        self.save_state(delta, None)
    }
    pub fn save_state(
        &self,
        delta: crate::library::CatalogDelta,
        preferences: Option<QuantizePreferences>,
    ) -> Result<(), String> {
        self.tx
            .try_send(Command::SaveDelta(delta, preferences))
            .map_err(|e| e.to_string())
    }
    #[cfg(test)]
    pub fn close_delta(self, delta: Option<crate::library::CatalogDelta>) -> Result<(), String> {
        self.close_state(delta, None)
    }
    pub fn close_state(
        self,
        delta: Option<crate::library::CatalogDelta>,
        preferences: Option<QuantizePreferences>,
    ) -> Result<(), String> {
        let _ = self.tx.send(Command::CloseDelta(delta, preferences));
        self.join
            .join()
            .map_err(|_| "Library worker panicked".to_string())?
    }
    #[cfg(test)]
    pub fn save(&self, catalog: Catalog) -> Result<(), String> {
        self.tx
            .try_send(Command::Save(catalog))
            .map_err(|e| e.to_string())
    }
    /// Called after the GUI event loop exits; final flush stays on the storage thread.
    #[cfg(test)]
    pub fn close(self, catalog: Option<Catalog>) -> Result<(), String> {
        let _ = self.tx.send(Command::Close(catalog));
        self.join
            .join()
            .map_err(|_| "Library worker panicked".to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantize_preferences_roundtrip_and_atomic_delta_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        assert_eq!(
            store.read_quantize_preferences().unwrap(),
            QuantizePreferences::default()
        );
        let mut prefs = QuantizePreferences {
            enabled: false,
            ..Default::default()
        };
        prefs.decks[0].cues = true;
        prefs.decks[0].step = 0;
        prefs.decks[1].triggers = true;
        prefs.decks[1].auto_loop_snap = true;
        prefs.decks[1].step = 5;
        let data = catalog(dir.path().join("missing.wav"), 27);
        let delta = library::CatalogDelta {
            revision: 1,
            tracks: data.tracks,
            crates: None,
            memberships: vec![],
        };
        store.connection.execute_batch("CREATE TRIGGER reject_prefs BEFORE INSERT ON library_meta WHEN NEW.key='quantize_preferences_v1' BEGIN SELECT RAISE(ABORT,'injected preference failure'); END;").unwrap();
        assert!(store.save_state(&delta, Some(prefs)).is_err());
        assert!(rows(&store).is_empty());
        assert_eq!(
            store.read_quantize_preferences().unwrap(),
            QuantizePreferences::default()
        );
        store
            .connection
            .execute_batch("DROP TRIGGER reject_prefs")
            .unwrap();
        store.save_state(&delta, Some(prefs)).unwrap();
        assert_eq!(rows(&store), vec![27]);
        drop(store);
        let (store, _, _) = Store::open(db.clone()).unwrap();
        assert_eq!(store.read_quantize_preferences().unwrap(), prefs);
        drop(store);
        let worker = Worker::start(db.clone()).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            Event::Loaded(Ok(_))
        ));
        prefs.enabled = true;
        worker.close_state(None, Some(prefs)).unwrap();
        let (store, _, _) = Store::open(db).unwrap();
        assert_eq!(store.read_quantize_preferences().unwrap(), prefs);
    }
    #[test]
    fn malformed_quantize_preferences_warn_without_catalog_loss_or_rewrite() {
        let mut invalid_step = serde_json::to_value(QuantizePreferences::default()).unwrap();
        invalid_step["decks"][0]["step"] = 6.into();
        let mut future = serde_json::to_value(QuantizePreferences::default()).unwrap();
        future["version"] = 2.into();
        let mut unknown = serde_json::to_value(QuantizePreferences::default()).unwrap();
        unknown["extra"] = true.into();
        for raw in [
            "[".to_string(),
            "{}".into(),
            "x".repeat(1025),
            format!("x\0{}", "x".repeat(1_000_000)),
            invalid_step.to_string(),
            future.to_string(),
            unknown.to_string(),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("library.sqlite3");
            let (mut store, _, _) = Store::open(db.clone()).unwrap();
            store
                .save(&catalog(dir.path().join("missing.wav"), 27))
                .unwrap();
            store
                .connection
                .execute(
                    "INSERT INTO library_meta(key,value) VALUES('quantize_preferences_v1',?1)",
                    [&raw],
                )
                .unwrap();
            let bounded: String = store
                .connection
                .query_row(QUANTIZE_PREFS_QUERY, [], |row| row.get(0))
                .unwrap();
            assert!(
                bounded.len() <= 1024,
                "untrusted preference payload must be bounded before reading into Rust"
            );
            drop(store);
            let worker = Worker::start(db.clone()).unwrap();
            let Event::Loaded(Ok(loaded)) = worker.events.recv().unwrap() else {
                panic!("catalog must remain readable")
            };
            assert_eq!(loaded.library.len(), 1);
            assert!(loaded.preference_warning.is_some());
            assert_eq!(loaded.quantize_preferences, QuantizePreferences::default());
            worker.close_delta(None).unwrap();
            let connection = connect(&db, false).unwrap();
            let preserved: String = connection
                .query_row(
                    "SELECT value FROM library_meta WHERE key='quantize_preferences_v1'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(preserved, raw);
        }
    }
    fn catalog(path: PathBuf, id: i32) -> Catalog {
        Catalog {
            version: 1,
            crates: Default::default(),
            tracks: vec![SavedTrack {
                hot_cues: Default::default(),
                saved_loops: Default::default(),
                id,
                path,
                metadata: Default::default(),
                grid: None,
            }],
        }
    }
    fn rows(store: &Store) -> Vec<i32> {
        inspect(&store.connection)
            .unwrap()
            .unwrap()
            .tracks
            .iter()
            .map(|t| t.id)
            .collect()
    }
    fn json(path: &Path, catalog: &Catalog) -> Vec<u8> {
        let bytes = serde_json::to_vec(catalog).unwrap();
        fs::write(path, &bytes).unwrap();
        bytes
    }
    #[test]
    fn malformed_meter_maps_are_rejected_instead_of_merged_or_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _, _) = Store::open(dir.path().join("library.sqlite3")).unwrap();
        let mut data = catalog(dir.path().join("missing.wav"), 27);
        data.tracks[0].grid = Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap());
        store.save(&data).unwrap();
        for json in [
            "[",
            r#"[{"bar":2,"beats":0}]"#,
            r#"[{"bar":1,"beats":3}]"#,
            r#"[{"bar":2,"beats":3},{"bar":2,"beats":5}]"#,
            r#"[{"bar":3,"beats":3},{"bar":2,"beats":5}]"#,
            r#"[{"bar":2,"beats":3,"unknown":true}]"#,
        ] {
            store
                .connection
                .execute("UPDATE track_grids SET meter_changes=?1", [json])
                .unwrap();
            assert!(
                matches!(inspect(&store.connection), Err(InspectError::Damaged(_))),
                "{json}"
            );
        }
        let excessive: Vec<_> = (2..=18)
            .map(|bar| crate::beatgrid::MeterChange { bar, beats: 3 })
            .collect();
        store
            .connection
            .execute(
                "UPDATE track_grids SET meter_changes=?1",
                [serde_json::to_string(&excessive).unwrap()],
            )
            .unwrap();
        assert!(matches!(
            inspect(&store.connection),
            Err(InspectError::Damaged(_))
        ));
    }
    #[test]
    fn schema_seven_upgrade_preserves_grid_and_persists_mixed_meter_deltas() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("missing.wav"), 27);
        let base = crate::beatgrid::BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .with_beats_per_bar(3)
            .unwrap();
        data.tracks[0].grid = Some(base);
        store.save(&data).unwrap();
        store
            .connection
            .execute_batch(
                "ALTER TABLE track_grids DROP COLUMN meter_changes; PRAGMA user_version=7;",
            )
            .unwrap();
        drop(store);
        let (mut store, mut library, _) = Store::open(db.clone()).unwrap();
        let path = library.catalog().tracks[0].path.clone();
        assert_eq!(library.grid(&path), Some(base));
        let mixed = base
            .with_meter_change(2, 5)
            .unwrap()
            .with_meter_change(7, 4)
            .unwrap();
        library.set_grid(&path, Some(mixed)).unwrap();
        store.save_delta(&library.changes()).unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db.clone()).unwrap();
        assert_eq!(library.grid(&path), Some(mixed));
        let mut data = library.catalog();
        data.tracks[0].grid = Some(mixed.without_meter_change(2).unwrap());
        store.save(&data).unwrap();
        drop(store);
        let (_, library, _) = Store::open(db).unwrap();
        assert_eq!(
            library.grid(&path).unwrap().meter_changes(),
            &[crate::beatgrid::MeterChange { bar: 7, beats: 4 }]
        );
    }
    #[test]
    fn due_checkpoint_failure_retains_pending_backup_and_empty_delta_retries_it() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let data = catalog(dir.path().join("checkpoint-marker.wav"), 1);
        store
            .save_delta(&library::CatalogDelta {
                revision: 1,
                tracks: data.tracks,
                crates: None,
                memberships: Vec::new(),
            })
            .unwrap();
        let backup = side_path(&db, ".bak");
        fs::remove_file(&backup).unwrap();
        fs::create_dir(&backup).unwrap();
        store.last_checkpoint = Some(Instant::now() - Duration::from_secs(61));
        assert!(store.checkpoint(false).is_err());
        assert!(store.backup_pending);
        assert_eq!(rows(&store), vec![1]);
        fs::remove_dir(&backup).unwrap();
        store
            .save_delta(&library::CatalogDelta {
                revision: 1,
                tracks: Vec::new(),
                crates: None,
                memberships: Vec::new(),
            })
            .unwrap();
        assert!(backup.is_file());
        assert!(!store.backup_pending);
    }
    #[test]
    fn timed_checkpoint_updates_backup_while_worker_is_idle() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let worker =
            Worker::start_with_checkpoint_interval(db.clone(), Duration::from_millis(20)).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            Event::Loaded(Ok(_))
        ));
        let data = catalog(dir.path().join("checkpoint-marker.wav"), 1);
        worker
            .save_delta(library::CatalogDelta {
                revision: 1,
                tracks: data.tracks,
                crates: None,
                memberships: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            Event::Saved(Ok(()))
        ));
        let needle = b"checkpoint-marker.wav";
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            let bytes = fs::read(side_path(&db, ".bak")).unwrap();
            if bytes.windows(needle.len()).any(|s| s == needle) {
                break;
            }
            assert!(
                Instant::now() < until,
                "idle checkpoint did not refresh backup"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        worker.close_delta(None).unwrap();
        let (_store, restored, _) = Store::open(db).unwrap();
        assert_eq!(restored.len(), 1);
    }
    #[test]
    fn delta_saves_bound_row_writes_preserve_other_tracks_and_roll_back_failed_cues() {
        for size in [1000, 10000] {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("library.sqlite3");
            let (mut store, _, _) = Store::open(db.clone()).unwrap();
            let mut data = catalog(dir.path().join("1.wav"), 1);
            let template = data.tracks[0].clone();
            for id in 2..=size {
                let mut record = template.clone();
                record.id = id;
                record.path = dir.path().join(format!("{id}.wav"));
                data.tracks.push(record);
            }
            let cue = |position, name| {
                crate::hot_cues::HotCue::new(Duration::from_secs(position), name, 0x112233).unwrap()
            };
            data.tracks[42]
                .hot_cues
                .set(0, Some(cue(1, "Unrelated".into())))
                .unwrap();
            store.save(&data).unwrap();
            let mut target = data.tracks[41].clone();
            target
                .hot_cues
                .set(0, Some(cue(1, "First".into())))
                .unwrap();
            let mut delta = library::CatalogDelta {
                revision: 1,
                tracks: vec![target],
                crates: None,
                memberships: Vec::new(),
            };
            store.save_delta(&delta).unwrap();
            let checkpoint = store.last_checkpoint;
            let backup = fs::read(side_path(&db, ".bak")).unwrap();
            let before = store.connection.total_changes();
            delta.tracks[0]
                .hot_cues
                .set(0, Some(cue(2, "Second".into())))
                .unwrap();
            let start = Instant::now();
            store.save_delta(&delta).unwrap();
            let elapsed = start.elapsed();
            let writes = store.connection.total_changes() - before;
            assert!(
                (1..=20).contains(&writes),
                "{size} tracks changed {writes} rows"
            );
            assert_eq!(checkpoint, store.last_checkpoint);
            assert_eq!(fs::read(side_path(&db, ".bak")).unwrap(), backup);
            store.connection.execute_batch("CREATE TRIGGER reject_delta BEFORE INSERT ON track_hot_cues WHEN NEW.track_id=42 BEGIN SELECT RAISE(ABORT,'injected delta failure'); END;").unwrap();
            delta.tracks[0]
                .hot_cues
                .set(0, Some(cue(3, "Latest".into())))
                .unwrap();
            assert!(store.save_delta(&delta).is_err());
            let restored = inspect(&store.connection).unwrap().unwrap();
            assert_eq!(
                restored
                    .tracks
                    .iter()
                    .find(|t| t.id == 42)
                    .unwrap()
                    .hot_cues
                    .slots()[0]
                    .as_ref()
                    .unwrap()
                    .name(),
                "Second"
            );
            assert_eq!(
                restored
                    .tracks
                    .iter()
                    .find(|t| t.id == 43)
                    .unwrap()
                    .hot_cues
                    .slots()[0]
                    .as_ref()
                    .unwrap()
                    .name(),
                "Unrelated"
            );
            store
                .connection
                .execute_batch("DROP TRIGGER reject_delta")
                .unwrap();
            store.save_delta(&delta).unwrap();
            store.checkpoint(true).unwrap();
            drop(store);
            let (_store, restored, _) = Store::open(db).unwrap();
            assert_eq!(restored.len(), size as usize);
            assert_eq!(
                restored.hot_cues(&delta.tracks[0].path).slots()[0]
                    .as_ref()
                    .unwrap()
                    .name(),
                "Latest"
            );
            println!("delta SQL: {size} tracks, {writes} changed rows, {elapsed:?}");
        }
    }
    #[test]
    fn migrates_unicode_and_missing_paths_with_ids_and_preserves_both_json_files() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("library.json");
        let music = dir.path().join("Música 日本.wav");
        fs::write(&music, b"fixture").unwrap();
        let mut data = catalog(music.clone(), 27);
        data.tracks.push(SavedTrack {
            hot_cues: Default::default(),
            saved_loops: Default::default(),
            id: 91,
            path: dir.path().join("missing.mp3"),
            metadata: Default::default(),
            grid: None,
        });
        let original = json(&legacy, &data);
        let backup = json(
            &side_path(&legacy, ".bak"),
            &catalog(dir.path().join("old.wav"), 4),
        );
        let (store, mut library, note) = Store::open(legacy.clone()).unwrap();
        assert!(note.contains("Migrated JSON library"));
        assert_eq!(store.path, legacy.with_extension("sqlite3"));
        assert_eq!(
            library.visible().iter().find(|t| t.id == 27).unwrap().path,
            music
        );
        assert_eq!(library.missing_count(), 1);
        library.insert([dir.path().join("next.wav")]);
        assert_eq!(library.catalog().tracks.last().unwrap().id, 92);
        assert_eq!(rows(&store), vec![27, 91]);
        let source: String = store
            .connection
            .query_row(
                "SELECT value FROM library_meta WHERE key='initial_source'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(source, "JSON library");
        assert_eq!(fs::read(&legacy).unwrap(), original);
        assert_eq!(fs::read(side_path(&legacy, ".bak")).unwrap(), backup);
        drop(store);
        // An old file changed by another program must NEVER be imported over initialized SQLite.
        fs::write(&legacy, b"now invalid").unwrap();
        let (store, library, note) = Store::open(legacy).unwrap();
        assert_eq!(note, "SQLite library restored");
        assert_eq!(rows(&store), vec![27, 91]);
        assert_eq!(library.len(), 2);
    }
    #[test]
    fn json_backup_migrates_when_primary_is_damaged_or_missing() {
        for absent in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let legacy = dir.path().join("library.json");
            if !absent {
                fs::write(&legacy, b"partial").unwrap();
            }
            let bytes = json(
                &side_path(&legacy, ".bak"),
                &catalog(dir.path().join("one.wav"), 9),
            );
            let (store, _, note) = Store::open(legacy.clone()).unwrap();
            assert!(note.contains("JSON backup"));
            assert_eq!(rows(&store), vec![9]);
            assert_eq!(fs::read(side_path(&legacy, ".bak")).unwrap(), bytes);
            if !absent {
                assert_eq!(fs::read(legacy).unwrap(), b"partial");
            }
        }
    }
    #[test]
    fn damaged_unknown_or_invalid_json_never_creates_database_or_changes_sources() {
        for bytes in [
            b"broken".as_slice(),
            br#"{"version":2,"tracks":[],"future":true}"#,
            br#"{"version":1,"tracks":[{"id":0,"path":"relative.wav"}]}"#,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let legacy = dir.path().join("library.json");
            fs::write(&legacy, bytes).unwrap();
            if bytes.starts_with(b"{\"version\":2") {
                json(
                    &side_path(&legacy, ".bak"),
                    &catalog(dir.path().join("old.wav"), 1),
                );
            }
            assert!(Store::open(legacy.clone()).is_err());
            assert_eq!(fs::read(&legacy).unwrap(), bytes);
            assert!(!legacy.with_extension("sqlite3").exists());
        }
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("large.json");
        File::create(&legacy)
            .unwrap()
            .set_len(32 * 1024 * 1024 + 1)
            .unwrap();
        assert!(crate::legacy_catalog::load(&legacy)
            .err()
            .unwrap()
            .contains("32 MiB"));
    }
    #[test]
    fn fresh_database_and_legacy_override_resolve_to_same_profile_without_losing_ids() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, note) = Store::open(db.clone()).unwrap();
        assert_eq!(note, "New SQLite library");
        store
            .save(&catalog(dir.path().join("one.wav"), 27))
            .unwrap();
        drop(store);
        let (store, mut library, _) = Store::open(db.with_extension("json")).unwrap();
        assert_eq!(rows(&store), vec![27]);
        library.insert([dir.path().join("new.mp3")]);
        assert_eq!(library.catalog().tracks[1].id, 28);
        assert!(paths(Path::new("relative.sqlite3")).is_err());
    }
    #[test]
    fn schema_creation_and_import_are_one_transaction_and_can_be_retried() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let mut connection = connect(&db, true).unwrap();
        configure(&connection).unwrap();
        let mut data = catalog(dir.path().join("first.wav"), 1);
        data.tracks.push(SavedTrack {
            hot_cues: Default::default(),
            saved_loops: Default::default(),
            id: 0,
            path: dir.path().join("invalid.wav"),
            metadata: Default::default(),
            grid: None,
        });
        assert!(initialize(&mut connection, &data, "test fault").is_err());
        assert!(inspect(&connection).unwrap().is_none());
        let count: i32 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        data.tracks.pop();
        initialize(&mut connection, &data, "retry").unwrap();
        assert_eq!(inspect(&connection).unwrap().unwrap().tracks[0].id, 1);
    }
    #[test]
    fn transaction_failure_rolls_back_prior_deletions_and_partial_inserts_then_retry_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _, _) = Store::open(dir.path().join("library.sqlite3")).unwrap();
        store
            .save(&catalog(dir.path().join("first.wav"), 1))
            .unwrap();
        store.connection.execute_batch("CREATE TRIGGER fail_insert BEFORE INSERT ON tracks WHEN NEW.id=3 BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
        let mut data = catalog(dir.path().join("second.wav"), 2);
        data.tracks.push(SavedTrack {
            hot_cues: Default::default(),
            saved_loops: Default::default(),
            id: 3,
            path: dir.path().join("third.mp3"),
            metadata: Default::default(),
            grid: None,
        });
        assert!(store
            .save(&data)
            .err()
            .unwrap()
            .contains("injected failure"));
        assert_eq!(rows(&store), vec![1]);
        store
            .connection
            .execute_batch("DROP TRIGGER fail_insert")
            .unwrap();
        store.save(&data).unwrap();
        assert_eq!(rows(&store), vec![2, 3]);
    }
    #[test]
    fn future_schema_and_foreign_sqlite_database_are_never_overwritten_from_json_or_backup() {
        for foreign in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("library.sqlite3");
            let (mut store, _, _) = Store::open(db.clone()).unwrap();
            store.save(&catalog(dir.path().join("one.wav"), 1)).unwrap();
            if foreign {
                store
                    .connection
                    .pragma_update(None, "application_id", 42)
                    .unwrap();
            } else {
                store
                    .connection
                    .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                    .unwrap();
            }
            drop(store);
            json(
                &db.with_extension("json"),
                &catalog(dir.path().join("old.wav"), 2),
            );
            let original = fs::read(&db).unwrap();
            assert!(Store::open(db.clone())
                .err()
                .unwrap()
                .contains("original preserved"));
            assert_eq!(fs::read(db).unwrap(), original);
        }
    }
    #[test]
    fn damaged_sqlite_recovers_previous_good_backup_and_preserves_damaged_file() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        store
            .save(&catalog(dir.path().join("first.wav"), 11))
            .unwrap();
        store
            .save(&catalog(dir.path().join("second.mp3"), 12))
            .unwrap();
        drop(store);
        fs::write(&db, b"damaged SQLite database").unwrap();
        let (store, library, note) = Store::open(db).unwrap();
        assert!(note.contains("recovered from backup"));
        assert_eq!(library.catalog().tracks[0].id, 11);
        assert_eq!(rows(&store), vec![11]);
        let preserved = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .find(|f| f.file_name().to_string_lossy().contains(".damaged.stage-"))
            .unwrap();
        assert_eq!(
            fs::read(preserved.path()).unwrap(),
            b"damaged SQLite database"
        );
    }
    #[test]
    fn unrecoverable_database_is_not_recreated_from_stale_json() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        fs::write(&db, b"broken SQLite").unwrap();
        json(
            &db.with_extension("json"),
            &catalog(dir.path().join("stale.wav"), 1),
        );
        assert!(Store::open(db.clone()).is_err());
        assert_eq!(fs::read(&db).unwrap(), b"broken SQLite");
        fs::write(side_path(&db, ".bak"), b"broken backup").unwrap();
        assert!(Store::open(db.clone()).is_err());
        assert_eq!(fs::read(db).unwrap(), b"broken SQLite");
    }
    #[test]
    fn locks_exclude_current_and_json_era_writers_and_release_on_close() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let legacy = db.with_extension("json");
        let old = lock(&legacy).unwrap();
        assert!(Store::open(db.clone()).err().unwrap().contains("locked"));
        assert!(!db.exists());
        drop(old);
        let (store, _, _) = Store::open(db.clone()).unwrap();
        assert!(Store::open(db.clone()).is_err());
        assert!(lock(&legacy).is_err());
        drop(store);
        assert!(Store::open(db).is_ok());
    }
    #[test]
    fn worker_close_flushes_latest_snapshot_before_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let worker = Worker::start(db.clone()).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            Event::Loaded(Ok(_))
        ));
        worker
            .save(catalog(dir.path().join("first.wav"), 1))
            .unwrap();
        worker
            .close(Some(catalog(dir.path().join("latest.wav"), 2)))
            .unwrap();
        let worker = Worker::start(db).unwrap();
        let Event::Loaded(Ok(loaded)) = worker.events.recv().unwrap() else {
            panic!("restore failed")
        };
        assert_eq!(loaded.library.catalog().tracks[0].id, 2);
        worker.close(None).unwrap();
    }
    // Invoked only by the isolated child-process test below; normal suite execution is a no-op.
    #[test]
    fn crash_fixture() {
        let Some(path) = std::env::var_os("DRDJ_SQLITE_CRASH_TEST_PATH") else {
            return;
        };
        let db = PathBuf::from(path);
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        store
            .save(&catalog(db.with_file_name("committed.wav"), 1))
            .unwrap();
        store
            .connection
            .execute_batch("PRAGMA cache_size=1; BEGIN IMMEDIATE; DELETE FROM tracks;")
            .unwrap();
        for id in 2..2002 {
            let path = db.with_file_name(format!("pending-{id}.wav"));
            store
                .connection
                .execute(
                    "INSERT INTO tracks(id,path,path_key) VALUES (?1,?2,?2)",
                    rusqlite::params![id, path.to_str().unwrap()],
                )
                .unwrap();
        }
        fs::write(
            side_path(&db, ".ready"),
            b"uncommitted transaction with cache spills",
        )
        .unwrap();
        loop {
            thread::park();
        }
    }
    #[test]
    fn process_termination_recovers_hot_journal_and_releases_os_locks() {
        use std::process::{Child, Command, Stdio};
        struct TestChild(Child);
        impl Drop for TestChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let mut child = TestChild(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "library_store::tests::crash_fixture",
                    "--nocapture",
                ])
                .env("DRDJ_SQLITE_CRASH_TEST_PATH", &db)
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !side_path(&db, ".ready").exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child failed before ready"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "child did not reach uncommitted transaction"
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert!(side_path(&db, "-journal").exists());
        assert!(Store::open(db.clone()).err().unwrap().contains("locked"));
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let (store, _, note) = Store::open(db).unwrap();
        assert_eq!(note, "SQLite library restored");
        assert_eq!(rows(&store), vec![1]);
        let check: String = store
            .connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(check, "ok");
    }
    #[test]
    fn upgrades_version_one_transactionally_and_caches_tags_for_missing_media() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let file = crate::test_support::tagged_wav(
            "Old catalog title",
            "Original artist",
            "Original album",
        );
        let connection = connect(&db, true).unwrap();
        connection.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,path TEXT NOT NULL,path_key TEXT NOT NULL UNIQUE) STRICT; CREATE TABLE library_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL) STRICT;").unwrap();
        connection
            .pragma_update(None, "application_id", APPLICATION_ID)
            .unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        connection
            .execute(
                "INSERT INTO tracks VALUES(73,?1,?2)",
                rusqlite::params![
                    file.path().to_str().unwrap(),
                    library::identity(file.path()).to_str().unwrap()
                ],
            )
            .unwrap();
        drop(connection);
        let (store, library, _) = Store::open(db.clone()).unwrap();
        let version: i32 = store
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let meta = &library.visible()[0].metadata;
        assert_eq!(meta.title, "Old catalog title");
        assert_eq!(meta.artist, "Original artist");
        assert_eq!(meta.album, "Original album");
        assert_eq!(meta.duration_ms, Some(1000));
        assert!(!meta.duration_exact);
        assert_eq!(rows(&store), vec![73]);
        assert_eq!(
            inspect(&store.connection).unwrap().unwrap().tracks[0].metadata,
            *meta
        );
        drop(store);
        drop(file);
        let (store, library, _) = Store::open(db).unwrap();
        assert_eq!(rows(&store), vec![73]);
        assert_eq!(library.missing_count(), 1);
        assert_eq!(library.visible()[0].metadata.title, "Old catalog title");
    }
    #[test]
    fn failed_metadata_schema_upgrade_preserves_version_one_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let connection = connect(&db, true).unwrap();
        connection.execute_batch("CREATE TABLE tracks(id INTEGER PRIMARY KEY,path TEXT NOT NULL,path_key TEXT NOT NULL UNIQUE) STRICT; CREATE TABLE library_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL) STRICT; CREATE TABLE track_metadata(block_upgrade INTEGER);").unwrap();
        connection
            .pragma_update(None, "application_id", APPLICATION_ID)
            .unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        let path = dir.path().join("missing.wav");
        connection
            .execute(
                "INSERT INTO tracks VALUES(9,?1,?2)",
                rusqlite::params![
                    path.to_str().unwrap(),
                    library::identity(&path).to_str().unwrap()
                ],
            )
            .unwrap();
        drop(connection);
        assert!(Store::open(db.clone()).is_err());
        let connection = connect(&db, false).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT id FROM tracks", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            9
        );
    }
    #[test]
    fn crate_memberships_and_deleted_id_high_water_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("one.wav"), 1);
        let first = data.crates.create("Música 日本").unwrap();
        let last = data.crates.create("Warmup").unwrap();
        data.crates.add(first, 1).unwrap();
        data.crates.add(last, 1).unwrap();
        store.save(&data).unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db.clone()).unwrap();
        let mut data = library.catalog();
        assert_eq!(data.crates.entries.len(), 2);
        assert_eq!(data.crates.entries[0].tracks, HashSet::from([1]));
        data.crates.delete(last).unwrap();
        data.crates.delete(first).unwrap();
        store.save(&data).unwrap();
        drop(store);
        let (_, library, _) = Store::open(db).unwrap();
        let mut data = library.catalog();
        assert!(data.crates.entries.is_empty());
        assert!(data.crates.create("Next").unwrap() > last);
        assert_eq!(data.tracks[0].id, 1);
    }
    #[test]
    fn membership_deltas_are_bounded_atomic_and_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("1.wav"), 1);
        let template = data.tracks[0].clone();
        for id in 2..=2000 {
            let mut track = template.clone();
            track.id = id;
            track.path = dir.path().join(format!("{id}.wav"));
            data.tracks.push(track);
        }
        let first = data.crates.create("First").unwrap();
        let other = data.crates.create("Other").unwrap();
        data.crates.add_many(first, &(1..=1000).collect()).unwrap();
        data.crates
            .add_many(other, &(500..=1500).collect())
            .unwrap();
        store.save(&data).unwrap();
        let mut library = Library::restore(data).unwrap();
        store.connection.execute_batch("CREATE TRIGGER no_snapshot BEFORE DELETE ON crates BEGIN SELECT RAISE(ABORT,'unexpected snapshot'); END;").unwrap();
        assert_eq!(
            library
                .add_paths_to_crate(
                    first,
                    &[dir.path().join("1001.wav"), dir.path().join("2000.wav")]
                )
                .unwrap(),
            2
        );
        let delta = library.changes();
        assert!(delta.crates.is_none());
        assert_eq!(delta.memberships.len(), 2);
        let before = store.connection.total_changes();
        store.save_delta(&delta).unwrap();
        assert_eq!(store.connection.total_changes() - before, 2);
        library.acknowledge(delta.revision);
        library.select_crate(first).unwrap();
        library.select(1);
        assert!(library.remove_selected_from_crate().unwrap());
        assert_eq!(
            library
                .add_paths_to_crate(first, &[dir.path().join("1501.wav")])
                .unwrap(),
            1
        );
        let delta = library.changes();
        store.connection.execute_batch("CREATE TRIGGER reject_membership BEFORE INSERT ON crate_tracks WHEN NEW.track_id=1501 BEGIN SELECT RAISE(ABORT,'membership failure'); END;").unwrap();
        assert!(store
            .save_delta(&delta)
            .unwrap_err()
            .contains("membership failure"));
        let old = inspect(&store.connection).unwrap().unwrap();
        assert!(old.crates.entries[0].tracks.contains(&1));
        assert!(!old.crates.entries[0].tracks.contains(&1501));
        assert_eq!(old.crates.entries[1].tracks.len(), 1001);
        store
            .connection
            .execute_batch("DROP TRIGGER reject_membership")
            .unwrap();
        let before = store.connection.total_changes();
        store.save_delta(&delta).unwrap();
        assert_eq!(store.connection.total_changes() - before, 2);
        library.acknowledge(delta.revision);
        assert!(!library.has_changes());
        store
            .connection
            .execute_batch("DROP TRIGGER no_snapshot")
            .unwrap();
        drop(store);
        let (_, restored, _) = Store::open(db).unwrap();
        let catalog = restored.catalog();
        let first_entry = catalog
            .crates
            .entries
            .iter()
            .find(|c| c.id == first)
            .unwrap();
        let other_entry = catalog
            .crates
            .entries
            .iter()
            .find(|c| c.id == other)
            .unwrap();
        assert!(!first_entry.tracks.contains(&1));
        assert!(first_entry.tracks.contains(&1501));
        assert!(first_entry.tracks.contains(&1001));
        assert!(first_entry.tracks.contains(&2000));
        assert_eq!(other_entry.tracks.len(), 1001);
    }

    #[test]
    fn crate_structure_deltas_swap_names_without_rewriting_memberships_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("1.wav"), 1);
        let template = data.tracks[0].clone();
        for id in 2..=1000 {
            let mut track = template.clone();
            track.id = id;
            track.path = dir.path().join(format!("{id}.wav"));
            data.tracks.push(track);
        }
        let a = data.crates.create("Alpha").unwrap();
        let b = data.crates.create("Beta").unwrap();
        data.crates.add_many(a, &(1..=1000).collect()).unwrap();
        data.crates.add_many(b, &(1..=500).collect()).unwrap();
        store.save(&data).unwrap();
        let mut library = Library::restore(data).unwrap();
        library.rename_crate(a, "Temporary").unwrap();
        library.rename_crate(b, "Alpha").unwrap();
        library.rename_crate(a, "Beta").unwrap();
        let delta = library.changes();
        assert_eq!(delta.crates.as_ref().unwrap().entries.len(), 2);
        assert!(delta.memberships.is_empty());
        store.connection.execute_batch("CREATE TRIGGER no_membership_delete BEFORE DELETE ON crate_tracks BEGIN SELECT RAISE(ABORT,'unexpected membership deletion'); END; CREATE TRIGGER fail_rename BEFORE UPDATE ON crates WHEN NEW.name='Beta' AND OLD.name='Alpha' BEGIN SELECT RAISE(ABORT,'rename failure'); END;").unwrap();
        assert!(store
            .save_delta(&delta)
            .unwrap_err()
            .contains("rename failure"));
        let before = inspect(&store.connection).unwrap().unwrap();
        assert_eq!(before.crates.entries[0].name, "Alpha");
        assert_eq!(before.crates.entries[1].name, "Beta");
        assert_eq!(before.crates.entries[0].tracks.len(), 1000);
        store
            .connection
            .execute_batch("DROP TRIGGER fail_rename")
            .unwrap();
        let changes = store.connection.total_changes();
        store.save_delta(&delta).unwrap();
        assert!(store.connection.total_changes() - changes <= 5);
        store
            .connection
            .execute_batch("DROP TRIGGER no_membership_delete")
            .unwrap();
        drop(store);
        let (_, restored, _) = Store::open(db).unwrap();
        assert_eq!(restored.crates().entries[0].name, "Beta");
        assert_eq!(restored.crates().entries[1].name, "Alpha");
        assert_eq!(restored.crates().entries[0].tracks.len(), 1000);
        assert_eq!(restored.crates().entries[1].tracks.len(), 500);
    }

    #[test]
    fn crate_structure_create_then_delete_before_save_keeps_high_water() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, mut library, _) = Store::open(db.clone()).unwrap();
        let id = library.create_crate("Transient").unwrap();
        library.delete_crate(id).unwrap();
        let delta = library.changes();
        assert_eq!(delta.crates.as_ref().unwrap().entries, vec![(id, None)]);
        store.save_delta(&delta).unwrap();
        library.acknowledge(delta.revision);
        assert!(!library.has_changes());
        drop(store);
        let (_, mut restored, _) = Store::open(db).unwrap();
        assert!(restored.crates().entries.is_empty());
        assert_eq!(restored.create_crate("Next").unwrap(), id + 1);
    }

    #[test]
    fn crate_structure_inflight_create_delete_and_high_water_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, mut library, _) = Store::open(db.clone()).unwrap();
        let path = dir.path().join("new.wav");
        library.insert([path.clone()]);
        let a = library.create_crate("First").unwrap();
        library
            .add_paths_to_crate(a, std::slice::from_ref(&path))
            .unwrap();
        let inflight = library.changes();
        library.delete_crate(a).unwrap();
        let b = library.create_crate("Second").unwrap();
        library.add_paths_to_crate(b, &[path]).unwrap();
        store.save_delta(&inflight).unwrap();
        library.acknowledge(inflight.revision);
        let latest = library.changes();
        assert_eq!(
            latest.crates.as_ref().unwrap().entries,
            vec![(a, None), (b, Some("Second".into()))]
        );
        assert_eq!(latest.memberships, vec![(b, 1, true)]);
        store.save_delta(&latest).unwrap();
        library.acknowledge(latest.revision);
        assert!(!library.has_changes());
        drop(store);
        let (_, mut restored, _) = Store::open(db).unwrap();
        assert_eq!(restored.crates().entries.len(), 1);
        assert_eq!(restored.crates().entries[0].tracks, HashSet::from([1]));
        assert_eq!(restored.create_crate("Next").unwrap(), b + 1);
    }

    #[test]
    fn membership_inflight_and_mixed_snapshot_saves_preserve_latest_revision() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, mut library, _) = Store::open(db.clone()).unwrap();
        let paths = [dir.path().join("a.wav"), dir.path().join("b.wav")];
        library.insert(paths.clone());
        let id = library.create_crate("Set").unwrap();
        library.add_paths_to_crate(id, &paths[..1]).unwrap();
        let mixed = library.changes();
        assert!(mixed.crates.is_some());
        assert_eq!(mixed.memberships, vec![(id, 1, true)]);
        store.save_delta(&mixed).unwrap();
        library.acknowledge(mixed.revision);
        library.select_crate(id).unwrap();
        library.select(1);
        library.remove_selected_from_crate().unwrap();
        let inflight = library.changes();
        library.add_paths_to_crate(id, &paths[..1]).unwrap();
        library.add_paths_to_crate(id, &paths[1..]).unwrap();
        store.save_delta(&inflight).unwrap();
        library.acknowledge(inflight.revision);
        let latest = library.changes();
        assert_eq!(latest.memberships, vec![(id, 1, true), (id, 2, true)]);
        store.save_delta(&latest).unwrap();
        library.acknowledge(latest.revision);
        assert!(!library.has_changes());
        drop(store);
        let (_, restored, _) = Store::open(db).unwrap();
        assert_eq!(restored.crates().entries[0].tracks, HashSet::from([1, 2]));
    }

    #[test]
    fn membership_and_track_changes_commit_together_and_reject_bad_identity() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, mut library, _) = Store::open(dir.path().join("library.sqlite3")).unwrap();
        let crate_id = library.create_crate("Set").unwrap();
        let delta = library.changes();
        store.save_delta(&delta).unwrap();
        library.acknowledge(delta.revision);
        let path = dir.path().join("new.wav");
        library.insert([path.clone()]);
        assert_eq!(library.add_paths_to_crate(crate_id, &[path]).unwrap(), 1);
        let delta = library.changes();
        assert!(delta.crates.is_none());
        assert_eq!(delta.tracks.len(), 1);
        assert_eq!(delta.memberships, vec![(crate_id, 1, true)]);
        store.save_delta(&delta).unwrap();
        let restored = inspect(&store.connection).unwrap().unwrap();
        assert_eq!(restored.crates.entries[0].tracks, HashSet::from([1]));
        for (id, track, present) in [(crate_id, 999, true), (999, 1, false), (0, 1, true)] {
            let bad = library::CatalogDelta {
                revision: 0,
                tracks: Vec::new(),
                crates: None,
                memberships: vec![(id, track, present)],
            };
            assert!(store.save_delta(&bad).is_err());
            let restored = inspect(&store.connection).unwrap().unwrap();
            assert_eq!(restored.crates.entries[0].tracks, HashSet::from([1]));
        }
    }

    #[test]
    fn crate_snapshot_failure_rolls_back_names_memberships_and_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _, _) = Store::open(dir.path().join("library.sqlite3")).unwrap();
        let mut data = catalog(dir.path().join("one.wav"), 1);
        data.tracks.push(SavedTrack {
            hot_cues: Default::default(),
            saved_loops: Default::default(),
            id: 2,
            path: dir.path().join("two.wav"),
            metadata: Default::default(),
            grid: None,
        });
        let id = data.crates.create("Original").unwrap();
        data.crates.add(id, 1).unwrap();
        store.save(&data).unwrap();
        store.connection.execute_batch("CREATE TRIGGER fail_member BEFORE INSERT ON crate_tracks WHEN NEW.track_id=2 BEGIN SELECT RAISE(ABORT,'membership failure'); END;").unwrap();
        data.crates.rename(id, "Changed").unwrap();
        data.crates.remove(id, 1).unwrap();
        data.crates.add(id, 2).unwrap();
        data.crates.create("New").unwrap();
        assert!(store
            .save(&data)
            .err()
            .unwrap()
            .contains("membership failure"));
        let old = inspect(&store.connection).unwrap().unwrap();
        assert_eq!(old.crates.last_id, id);
        assert_eq!(old.crates.entries[0].name, "Original");
        assert_eq!(old.crates.entries[0].tracks, HashSet::from([1]));
        store
            .connection
            .execute_batch("DROP TRIGGER fail_member")
            .unwrap();
        store.save(&data).unwrap();
        assert_eq!(
            inspect(&store.connection).unwrap().unwrap().crates.entries[0].tracks,
            HashSet::from([2])
        );
    }
    #[test]
    fn cached_id3_failure_is_retried_and_recovery_warning_persists() {
        let tagged = crate::test_support::tagged_mp3("bad", "artist", "album");
        let mut bytes = std::fs::read(tagged.path()).unwrap();
        bytes[20] = 255;
        let file = crate::test_support::TempFile::new("mp3", &bytes);
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(file.path().to_path_buf(), 27);
        data.tracks[0].metadata.scanned = true;
        data.tracks[0].metadata.error =
            Some("malformed stream: id3v2: invalid text encoding".into());
        store.save(&data).unwrap();
        drop(store);
        let (store, library, _) = Store::open(db.clone()).unwrap();
        let cached = library.catalog().tracks[0].metadata.clone();
        assert!(cached
            .error
            .as_deref()
            .unwrap()
            .starts_with(crate::media_input::TAG_WARNING));
        assert!(cached.duration_ms.is_some());
        assert!(!cached.needs_refresh(file.path()));
        assert_eq!(
            inspect(&store.connection).unwrap().unwrap().tracks[0].metadata,
            cached
        );
        drop(store);
        drop(file);
        let (_, library, _) = Store::open(db).unwrap();
        assert_eq!(library.catalog().tracks[0].metadata, cached);
    }
    #[test]
    fn unavailable_cached_id3_failure_does_not_trigger_backfill_write() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("missing.mp3"), 27);
        data.tracks[0].metadata.scanned = true;
        data.tracks[0].metadata.error =
            Some("malformed stream: id3v2: invalid text encoding".into());
        store.save(&data).unwrap();
        store.connection.execute_batch("CREATE TRIGGER forbid_backfill BEFORE INSERT ON track_metadata BEGIN SELECT RAISE(ABORT,'unexpected write'); END;").unwrap();
        drop(store);
        let (store, library, _) = Store::open(db).unwrap();
        assert!(store.backfill_error.is_none());
        assert_eq!(
            library.catalog().tracks[0].metadata,
            data.tracks[0].metadata
        );
        assert!(!library.has_changes());
    }
    #[test]
    fn schema_two_upgrade_and_failed_upgrade_preserve_cached_metadata() {
        for fail in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("library.sqlite3");
            let (mut store, _, _) = Store::open(db.clone()).unwrap();
            let mut data = catalog(dir.path().join("missing.wav"), 27);
            data.tracks[0].metadata.title = "Cached title".into();
            data.tracks[0].metadata.scanned = true;
            store.save(&data).unwrap();
            drop(store);
            let connection = connect(&db, false).unwrap();
            connection.execute_batch("DROP TABLE track_saved_loops; DROP TABLE track_hot_cues; DROP TABLE track_grids; DROP TABLE crate_tracks; DROP TABLE crates; DELETE FROM library_meta WHERE key='last_crate_id'; PRAGMA user_version=2;").unwrap();
            if fail {
                connection
                    .execute_batch(
                        "CREATE TABLE crates(preserve INTEGER); INSERT INTO crates VALUES(99);",
                    )
                    .unwrap();
            }
            drop(connection);
            if fail {
                assert!(Store::open(db.clone()).is_err());
                let connection = connect(&db, false).unwrap();
                assert_eq!(
                    connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    2
                );
                assert_eq!(
                    connection
                        .query_row("SELECT title FROM track_metadata", [], |r| r
                            .get::<_, String>(0))
                        .unwrap(),
                    "Cached title"
                );
                assert_eq!(
                    connection
                        .query_row("SELECT preserve FROM crates", [], |r| r.get::<_, i32>(0))
                        .unwrap(),
                    99
                );
            } else {
                let (store, library, _) = Store::open(db).unwrap();
                assert_eq!(library.visible()[0].id, 27);
                assert_eq!(library.visible()[0].metadata.title, "Cached title");
                assert!(library.crates().entries.is_empty());
                assert_eq!(
                    store
                        .connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    SCHEMA_VERSION
                );
            }
        }
    }
    #[test]
    fn schema_six_grids_upgrade_to_four_beats_and_save_new_meter() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("missing.wav"), 27);
        data.tracks[0].grid = Some(crate::beatgrid::BeatGrid::new(120.0, Duration::ZERO).unwrap());
        store.save(&data).unwrap();
        store
            .connection
            .execute_batch(
                "ALTER TABLE track_grids DROP COLUMN meter_changes; ALTER TABLE track_grids DROP COLUMN beats_per_bar; PRAGMA user_version=6;",
            )
            .unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db.clone()).unwrap();
        let mut data = library.catalog();
        assert_eq!(data.tracks[0].grid.unwrap().beats_per_bar(), 4);
        data.tracks[0].grid = Some(data.tracks[0].grid.unwrap().with_beats_per_bar(7).unwrap());
        store.save(&data).unwrap();
        drop(store);
        let (_, restored, _) = Store::open(db).unwrap();
        assert_eq!(
            restored.catalog().tracks[0].grid.unwrap().beats_per_bar(),
            7
        );
    }

    #[test]
    fn fractional_grid_and_nanosecond_anchor_survive_restart_and_explicit_clear() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("missing.wav"), 27);
        let grid = crate::beatgrid::BeatGrid::new(128.5, Duration::new(1, 123456789))
            .unwrap()
            .with_beats_per_bar(3)
            .unwrap()
            .with_meter_change(2, 5)
            .unwrap()
            .with_meter_change(9, 7)
            .unwrap();
        data.tracks[0].grid = Some(grid);
        let id = data.crates.create("Set").unwrap();
        data.crates.add(id, 27).unwrap();
        store.save(&data).unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db.clone()).unwrap();
        let mut data = library.catalog();
        assert_eq!(data.tracks[0].grid, Some(grid));
        assert_eq!(data.crates.entries[0].tracks, HashSet::from([27]));
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("grid"));
        data.tracks[0].grid = None;
        store.save(&data).unwrap();
        drop(store);
        let (_, library, _) = Store::open(db).unwrap();
        assert!(library.catalog().tracks[0].grid.is_none());
    }
    #[test]
    fn failed_grid_save_rolls_back_grid_metadata_and_crates_together() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _, _) = Store::open(dir.path().join("library.sqlite3")).unwrap();
        let mut data = catalog(dir.path().join("missing.wav"), 1);
        let old = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
        data.tracks[0].grid = Some(old);
        let id = data.crates.create("Original").unwrap();
        data.crates.add(id, 1).unwrap();
        store.save(&data).unwrap();
        store.connection.execute_batch("CREATE TRIGGER fail_grid BEFORE INSERT ON track_grids BEGIN SELECT RAISE(ABORT,'grid write failure'); END;").unwrap();
        data.tracks[0].grid =
            Some(crate::beatgrid::BeatGrid::new(130., Duration::from_secs(1)).unwrap());
        data.tracks[0].metadata.title = "New title".into();
        data.crates.rename(id, "Changed").unwrap();
        assert!(store
            .save(&data)
            .err()
            .unwrap()
            .contains("grid write failure"));
        let restored = inspect(&store.connection).unwrap().unwrap();
        assert_eq!(restored.tracks[0].grid, Some(old));
        assert_eq!(restored.tracks[0].metadata.title, "");
        assert_eq!(restored.crates.entries[0].name, "Original");
        store
            .connection
            .execute_batch("DROP TRIGGER fail_grid")
            .unwrap();
        store.save(&data).unwrap();
        assert_eq!(
            inspect(&store.connection).unwrap().unwrap().tracks[0].grid,
            data.tracks[0].grid
        );
    }
    #[test]
    fn schema_three_grid_upgrade_or_failure_preserves_metadata_and_memberships() {
        for fail in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("library.sqlite3");
            let (mut store, _, _) = Store::open(db.clone()).unwrap();
            let mut data = catalog(dir.path().join("missing.wav"), 9);
            data.tracks[0].metadata.title = "Keep".into();
            data.tracks[0].metadata.scanned = true;
            let id = data.crates.create("Keep crate").unwrap();
            data.crates.add(id, 9).unwrap();
            store.save(&data).unwrap();
            drop(store);
            let connection = connect(&db, false).unwrap();
            connection
                .execute_batch(
                    "DROP TABLE track_saved_loops; DROP TABLE track_hot_cues; DROP TABLE track_grids; PRAGMA user_version=3;",
                )
                .unwrap();
            if fail {
                connection.execute_batch("CREATE TABLE track_grids(preserve INTEGER); INSERT INTO track_grids VALUES(99);").unwrap();
            }
            drop(connection);
            if fail {
                assert!(Store::open(db.clone()).is_err());
                let connection = connect(&db, false).unwrap();
                assert_eq!(
                    connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    3
                );
                assert_eq!(
                    connection
                        .query_row("SELECT preserve FROM track_grids", [], |r| r
                            .get::<_, i32>(0))
                        .unwrap(),
                    99
                );
                assert_eq!(
                    connection
                        .query_row("SELECT name FROM crates", [], |r| r.get::<_, String>(0))
                        .unwrap(),
                    "Keep crate"
                );
            } else {
                let (store, library, _) = Store::open(db).unwrap();
                assert_eq!(library.visible()[0].metadata.title, "Keep");
                assert_eq!(library.crates().entries[0].tracks, HashSet::from([9]));
                assert!(library.visible()[0].grid.is_none());
                assert_eq!(
                    store
                        .connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    SCHEMA_VERSION
                );
            }
        }
    }
    #[test]
    fn hot_cues_restart_sparse_slots_clear_and_atomic_retry() {
        use crate::hot_cues::HotCue;
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("missing.wav"), 27);
        let cue = HotCue::new(Duration::new(2, 123456789), "入り口 🎵".into(), 0x12abef).unwrap();
        data.tracks[0].hot_cues.set(0, Some(cue.clone())).unwrap();
        data.tracks[0].hot_cues.set(7, Some(cue.clone())).unwrap();
        let grid = crate::beatgrid::BeatGrid::new(128.5, Duration::from_millis(12)).unwrap();
        data.tracks[0].grid = Some(grid);
        let id = data.crates.create("Keep").unwrap();
        data.crates.add(id, 27).unwrap();
        store.save(&data).unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db.clone()).unwrap();
        let mut data = library.catalog();
        assert_eq!(data.tracks[0].id, 27);
        assert_eq!(data.tracks[0].hot_cues.slots()[0], Some(cue.clone()));
        assert_eq!(data.tracks[0].hot_cues.slots()[7], Some(cue.clone()));
        assert!(data.tracks[0].hot_cues.slots()[1..7]
            .iter()
            .all(Option::is_none));
        assert!(!serde_json::to_string(&data).unwrap().contains("hot_cues"));
        store.connection.execute_batch("CREATE TRIGGER fail_cue BEFORE INSERT ON track_hot_cues BEGIN SELECT RAISE(ABORT,'cue write failure'); END;").unwrap();
        data.tracks[0].hot_cues.set(0, None).unwrap();
        data.tracks[0].metadata.title = "Changed".into();
        data.tracks[0].grid = None;
        data.crates.rename(id, "Changed").unwrap();
        assert!(store.save(&data).unwrap_err().contains("cue write failure"));
        let saved = inspect(&store.connection).unwrap().unwrap();
        assert_eq!(saved.tracks[0].hot_cues.slots()[0], Some(cue.clone()));
        assert_eq!(saved.tracks[0].metadata.title, "");
        assert_eq!(saved.tracks[0].grid, Some(grid));
        assert_eq!(saved.crates.entries[0].name, "Keep");
        store
            .connection
            .execute_batch("DROP TRIGGER fail_cue")
            .unwrap();
        store.save(&data).unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db).unwrap();
        let mut data = library.catalog();
        assert!(data.tracks[0].hot_cues.slots()[0].is_none());
        assert_eq!(data.tracks[0].hot_cues.slots()[7], Some(cue));
        assert_eq!(data.tracks[0].metadata.title, "Changed");
        assert!(data.tracks[0].grid.is_none());
        assert_eq!(data.crates.entries[0].name, "Changed");
        data.tracks.clear();
        data.crates.entries[0].tracks.clear();
        store.save(&data).unwrap();
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM track_hot_cues", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn schema_four_upgrade_and_failure_preserve_grid_metadata_and_crates() {
        for fail in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("library.sqlite3");
            let (mut store, _, _) = Store::open(db.clone()).unwrap();
            let mut data = catalog(dir.path().join("missing.wav"), 9);
            data.tracks[0].metadata.title = "Keep".into();
            data.tracks[0].metadata.scanned = true;
            let grid = crate::beatgrid::BeatGrid::new(123.45, Duration::from_nanos(123)).unwrap();
            data.tracks[0].grid = Some(grid);
            let id = data.crates.create("Keep crate").unwrap();
            data.crates.add(id, 9).unwrap();
            store.save(&data).unwrap();
            drop(store);
            let connection = connect(&db, false).unwrap();
            connection
                .execute_batch("ALTER TABLE track_grids DROP COLUMN meter_changes; ALTER TABLE track_grids DROP COLUMN beats_per_bar; DROP TABLE track_saved_loops; DROP TABLE track_hot_cues; PRAGMA user_version=4;")
                .unwrap();
            if fail {
                connection.execute_batch("CREATE TABLE track_hot_cues(preserve INTEGER); INSERT INTO track_hot_cues VALUES(99);").unwrap();
            }
            drop(connection);
            if fail {
                assert!(Store::open(db.clone()).is_err());
                let connection = connect(&db, false).unwrap();
                assert_eq!(
                    connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    4
                );
                assert_eq!(
                    connection
                        .query_row("SELECT preserve FROM track_hot_cues", [], |r| r
                            .get::<_, i32>(0))
                        .unwrap(),
                    99
                );
                let saved = inspect(&connection).unwrap().unwrap();
                assert_eq!(saved.tracks[0].grid, Some(grid));
                assert_eq!(saved.tracks[0].metadata.title, "Keep");
                assert_eq!(saved.crates.entries[0].tracks, HashSet::from([9]));
            } else {
                let (store, library, _) = Store::open(db).unwrap();
                assert_eq!(
                    store
                        .connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    SCHEMA_VERSION
                );
                let saved = library.catalog();
                assert_eq!(saved.tracks[0].grid, Some(grid));
                assert_eq!(saved.tracks[0].metadata.title, "Keep");
                assert_eq!(saved.crates.entries[0].tracks, HashSet::from([9]));
                assert!(saved.tracks[0].hot_cues.slots().iter().all(Option::is_none));
            }
        }
    }
    #[test]
    fn inspect_rejects_orphaned_and_invalid_cue_rows() {
        for values in [
            "99,0,0,'',0",
            "1,8,0,'',0",
            "1,0,-1,'',0",
            "1,0,0,'bad'||char(10),0",
            "1,0,0,'',16777216",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let (mut store, _, _) = Store::open(dir.path().join("library.sqlite3")).unwrap();
            store
                .save(&catalog(dir.path().join("missing.wav"), 1))
                .unwrap();
            // A malformed table can evade SQL constraints; read validation must still reject it.
            store.connection.execute_batch("DROP TABLE track_hot_cues; CREATE TABLE track_hot_cues(track_id INTEGER,slot INTEGER,position_ns INTEGER,name TEXT,rgb INTEGER) STRICT;").unwrap();
            store
                .connection
                .execute_batch(&format!("INSERT INTO track_hot_cues VALUES({values})"))
                .unwrap();
            assert!(matches!(
                inspect(&store.connection),
                Err(InspectError::Damaged(_))
            ));
        }
    }
    #[test]
    fn saved_loop_restart_clear_and_failed_save_roll_back_all_track_data() {
        use crate::saved_loops::SavedLoop;
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let (mut store, _, _) = Store::open(db.clone()).unwrap();
        let mut data = catalog(dir.path().join("missing.wav"), 27);
        let value = SavedLoop::new(
            Duration::new(1, 123456789),
            Duration::new(4, 987654321),
            "繰り返し 🎵".into(),
        )
        .unwrap();
        data.tracks[0]
            .saved_loops
            .set(0, Some(value.clone()))
            .unwrap();
        data.tracks[0]
            .saved_loops
            .set(7, Some(value.clone()))
            .unwrap();
        let cue =
            crate::hot_cues::HotCue::new(Duration::ZERO, "Keep cue".into(), 0x123456).unwrap();
        data.tracks[0].hot_cues.set(1, Some(cue.clone())).unwrap();
        let grid = crate::beatgrid::BeatGrid::new(123.45, Duration::from_nanos(123)).unwrap();
        data.tracks[0].grid = Some(grid);
        let id = data.crates.create("Keep crate").unwrap();
        data.crates.add(id, 27).unwrap();
        store.save(&data).unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db.clone()).unwrap();
        let mut data = library.catalog();
        assert_eq!(data.tracks[0].id, 27);
        assert_eq!(data.tracks[0].saved_loops.slots()[0], Some(value.clone()));
        assert_eq!(data.tracks[0].saved_loops.slots()[7], Some(value.clone()));
        assert!(data.tracks[0].saved_loops.slots()[1..7]
            .iter()
            .all(Option::is_none));
        assert!(!serde_json::to_string(&data)
            .unwrap()
            .contains("saved_loops"));
        store.connection.execute_batch("CREATE TRIGGER fail_loop BEFORE INSERT ON track_saved_loops BEGIN SELECT RAISE(ABORT,'loop write failure'); END;").unwrap();
        data.tracks[0].saved_loops.set(0, None).unwrap();
        data.tracks[0].metadata.title = "Changed".into();
        data.tracks[0].grid = None;
        data.tracks[0].hot_cues.set(1, None).unwrap();
        data.crates.rename(id, "Changed").unwrap();
        assert!(store
            .save(&data)
            .unwrap_err()
            .contains("loop write failure"));
        let saved = inspect(&store.connection).unwrap().unwrap();
        assert_eq!(saved.tracks[0].saved_loops.slots()[0], Some(value.clone()));
        assert_eq!(saved.tracks[0].metadata.title, "");
        assert_eq!(saved.tracks[0].grid, Some(grid));
        assert_eq!(saved.tracks[0].hot_cues.slots()[1], Some(cue));
        assert_eq!(saved.crates.entries[0].name, "Keep crate");
        store
            .connection
            .execute_batch("DROP TRIGGER fail_loop")
            .unwrap();
        store.save(&data).unwrap();
        drop(store);
        let (mut store, library, _) = Store::open(db).unwrap();
        let mut data = library.catalog();
        assert!(data.tracks[0].saved_loops.slots()[0].is_none());
        assert_eq!(data.tracks[0].saved_loops.slots()[7], Some(value));
        assert_eq!(data.tracks[0].metadata.title, "Changed");
        assert!(data.tracks[0].grid.is_none());
        assert!(data.tracks[0].hot_cues.slots()[1].is_none());
        assert_eq!(data.crates.entries[0].name, "Changed");
        data.tracks.clear();
        data.crates.entries[0].tracks.clear();
        store.save(&data).unwrap();
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM track_saved_loops", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn schema_five_saved_loop_upgrade_and_failure_preserve_cues_grid_and_crates() {
        for fail in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let db = dir.path().join("library.sqlite3");
            let (mut store, _, _) = Store::open(db.clone()).unwrap();
            let mut data = catalog(dir.path().join("missing.wav"), 9);
            data.tracks[0].metadata.title = "Keep".into();
            data.tracks[0].metadata.scanned = true;
            let cue = crate::hot_cues::HotCue::new(
                Duration::from_nanos(123),
                "Keep cue".into(),
                0x123456,
            )
            .unwrap();
            data.tracks[0].hot_cues.set(7, Some(cue.clone())).unwrap();
            let grid = crate::beatgrid::BeatGrid::new(123.45, Duration::from_nanos(123)).unwrap();
            data.tracks[0].grid = Some(grid);
            let id = data.crates.create("Keep crate").unwrap();
            data.crates.add(id, 9).unwrap();
            store.save(&data).unwrap();
            drop(store);
            let connection = connect(&db, false).unwrap();
            connection
                .execute_batch("ALTER TABLE track_grids DROP COLUMN meter_changes; ALTER TABLE track_grids DROP COLUMN beats_per_bar; DROP TABLE track_saved_loops; PRAGMA user_version=5;")
                .unwrap();
            if fail {
                connection.execute_batch("CREATE TABLE track_saved_loops(preserve INTEGER); INSERT INTO track_saved_loops VALUES(99);").unwrap();
            }
            drop(connection);
            let saved = if fail {
                assert!(Store::open(db.clone()).is_err());
                let connection = connect(&db, false).unwrap();
                assert_eq!(
                    connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    5
                );
                assert_eq!(
                    connection
                        .query_row("SELECT preserve FROM track_saved_loops", [], |r| r
                            .get::<_, i32>(0))
                        .unwrap(),
                    99
                );
                inspect(&connection).unwrap().unwrap()
            } else {
                let (store, library, _) = Store::open(db).unwrap();
                assert_eq!(
                    store
                        .connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i32>(0))
                        .unwrap(),
                    SCHEMA_VERSION
                );
                library.catalog()
            };
            assert_eq!(saved.tracks[0].id, 9);
            assert_eq!(saved.tracks[0].metadata.title, "Keep");
            assert_eq!(saved.tracks[0].grid, Some(grid));
            assert_eq!(saved.tracks[0].hot_cues.slots()[7], Some(cue));
            assert_eq!(saved.crates.entries[0].tracks, HashSet::from([9]));
            assert!(saved.tracks[0]
                .saved_loops
                .slots()
                .iter()
                .all(Option::is_none));
        }
    }
    #[test]
    fn invalid_orphaned_and_duplicate_saved_loop_rows_are_rejected() {
        for values in [
            "(99,0,0,1,'')",
            "(1,8,0,1,'')",
            "(1,-1,0,1,'')",
            "(1,0,-1,1,'')",
            "(1,0,1,1,'')",
            "(1,0,2,1,'')",
            "(1,0,0,604800000000001,'')",
            "(1,0,0,1,'bad'||char(10))",
            "(1,0,0,1,''),(1,0,0,2,'duplicate')",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let (mut store, _, _) = Store::open(dir.path().join("library.sqlite3")).unwrap();
            store
                .save(&catalog(dir.path().join("missing.wav"), 1))
                .unwrap();
            store.connection.execute_batch("DROP TABLE track_saved_loops; CREATE TABLE track_saved_loops(track_id INTEGER,slot INTEGER,start_ns INTEGER,end_ns INTEGER,name TEXT) STRICT;").unwrap();
            store
                .connection
                .execute_batch(&format!("INSERT INTO track_saved_loops VALUES{values}"))
                .unwrap();
            assert!(matches!(
                inspect(&store.connection),
                Err(InspectError::Damaged(_))
            ));
        }
    }
}
