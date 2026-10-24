//! Durable catalog I/O. A single worker owns the OS lock and all filesystem work.
use crate::library::{Catalog, Library};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
};

const MAX_BYTES: u64 = 32 * 1024 * 1024;
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
        .map(|p| p.join("DrDJPro/library.json"))
        .ok_or_else(|| "No absolute user data directory available".into())
}
fn side_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}
fn read_catalog(path: &Path) -> Result<(Catalog, Vec<u8>), String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Library file exceeds 32 MiB".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if let Some(version) = value.get("version").and_then(|v| v.as_u64()) {
        if version != 1 {
            return Err(format!("Unsupported library version {version}"));
        }
    }
    let catalog: Catalog = serde_json::from_value(value).map_err(|e| e.to_string())?;
    catalog.validate()?;
    Ok((catalog, bytes))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = side_path(path, ".tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    // Same-directory rename replaces the complete file; never truncate the live catalog.
    fs::rename(&temp, path).map_err(|e| e.to_string())
}
struct Store {
    path: PathBuf,
    _lock: File,
    known_good: Option<Vec<u8>>,
}
impl Store {
    fn open(path: PathBuf) -> Result<(Self, Library, String), String> {
        if !path.is_absolute() {
            return Err("Library path must be absolute".into());
        }
        fs::create_dir_all(path.parent().ok_or("Library path has no parent")?)
            .map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(side_path(&path, ".lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|e| format!("Library is in use or cannot be locked: {e}"))?;
        let backup = side_path(&path, ".bak");
        let primary_exists = path.try_exists().map_err(|e| e.to_string())?;
        let loaded = if primary_exists {
            Some(read_catalog(&path))
        } else {
            None
        };
        // Never downgrade an unknown schema to an older backup and overwrite newer data.
        if loaded.as_ref().is_some_and(|r| {
            r.as_ref()
                .err()
                .is_some_and(|e| e.starts_with("Unsupported library version"))
        }) {
            return Err(loaded.unwrap().err().unwrap());
        }
        let (catalog, known_good, note) = match loaded {
            Some(Ok((catalog, bytes))) => (catalog, Some(bytes), "Library restored"),
            failure => {
                if backup.try_exists().map_err(|e| e.to_string())? {
                    let (catalog, bytes) = read_catalog(&backup)
                        .map_err(|e| format!("Library and backup unavailable: {e}"))?;
                    (catalog, Some(bytes), "Library recovered from backup")
                } else if let Some(Err(error)) = failure {
                    return Err(format!("Library damaged; original preserved: {error}"));
                } else {
                    (
                        Catalog {
                            version: 1,
                            tracks: Vec::new(),
                        },
                        None,
                        "New library",
                    )
                }
            }
        };
        let library = Library::restore(catalog)?;
        Ok((
            Self {
                path,
                _lock: lock,
                known_good,
            },
            library,
            note.into(),
        ))
    }
    fn save(&mut self, catalog: &Catalog) -> Result<(), String> {
        catalog.validate()?;
        let bytes = serde_json::to_vec(catalog).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("Library file exceeds 32 MiB".into());
        }
        if let Some(previous) = &self.known_good {
            atomic_write(&side_path(&self.path, ".bak"), previous)?;
        }
        atomic_write(&self.path, &bytes)?;
        self.known_good = Some(bytes);
        Ok(())
    }
}

pub enum Event {
    Loaded(Result<(Library, String), String>),
    Saved(Result<(), String>),
}
enum Command {
    Save(Catalog),
    Close(Option<Catalog>),
}
pub struct Worker {
    tx: SyncSender<Command>,
    pub events: Receiver<Event>,
    join: JoinHandle<Result<(), String>>,
}
impl Worker {
    pub fn start(path: PathBuf) -> Result<Self, String> {
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
                if events_tx.send(Event::Loaded(Ok((library, note)))).is_err() {
                    return Ok(());
                }
                while let Ok(command) = rx.recv() {
                    match command {
                        Command::Save(catalog) => {
                            let _ = events_tx.send(Event::Saved(store.save(&catalog)));
                        }
                        Command::Close(catalog) => {
                            return catalog.map_or(Ok(()), |catalog| store.save(&catalog))
                        }
                    }
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { tx, events, join })
    }
    pub fn save(&self, catalog: Catalog) -> Result<(), String> {
        self.tx
            .try_send(Command::Save(catalog))
            .map_err(|e| e.to_string())
    }
    /// Called after the GUI event loop exits; final flush stays on the storage thread.
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
    use crate::library::SavedTrack;
    fn catalog(path: PathBuf, id: i32) -> Catalog {
        Catalog {
            version: 1,
            tracks: vec![SavedTrack { id, path }],
        }
    }
    #[test]
    fn restart_preserves_ids_unicode_and_missing_files_without_reusing_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let music = dir.path().join("Música 日本.wav");
        fs::write(&music, b"fixture").unwrap();
        let (mut store, _, _) = Store::open(path.clone()).unwrap();
        store.save(&catalog(music.clone(), 27)).unwrap();
        drop(store);
        let (_, mut library, note) = Store::open(path.clone()).unwrap();
        assert_eq!(note, "Library restored");
        assert_eq!(library.visible()[0].id, 27);
        assert_eq!(library.visible()[0].path, music);
        assert!(library.visible()[0].available);
        library.insert([dir.path().join("second.mp3")]);
        assert_eq!(library.catalog().tracks[1].id, 28);
        fs::remove_file(&music).unwrap();
        let (_, library, _) = Store::open(path).unwrap();
        assert_eq!(library.missing_count(), 1);
        assert_eq!(library.visible()[0].id, 27);
        assert_eq!(library.visible()[0].path, music);
    }
    #[test]
    fn corrupt_primary_recovers_previous_commit_and_does_not_backup_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let first = catalog(dir.path().join("first.wav"), 11);
        let second = catalog(dir.path().join("second.mp3"), 12);
        let (mut store, _, _) = Store::open(path.clone()).unwrap();
        store.save(&first).unwrap();
        store.save(&second).unwrap();
        drop(store);
        fs::write(&path, b"{partial").unwrap();
        let (mut store, library, note) = Store::open(path.clone()).unwrap();
        assert!(note.contains("recovered"));
        assert_eq!(library.catalog().tracks[0].id, 11);
        store.save(&library.catalog()).unwrap();
        assert_eq!(read_catalog(&path).unwrap().0.tracks[0].id, 11);
        assert_eq!(
            read_catalog(&side_path(&path, ".bak")).unwrap().0.tracks[0].id,
            11
        );
    }
    #[test]
    fn interrupted_temp_is_ignored_and_write_failure_preserves_valid_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let data = catalog(dir.path().join("one.wav"), 1);
        let (mut store, _, _) = Store::open(path.clone()).unwrap();
        store.save(&data).unwrap();
        drop(store);
        fs::write(side_path(&path, ".tmp"), b"unfinished").unwrap();
        let (mut store, library, _) = Store::open(path.clone()).unwrap();
        assert_eq!(library.len(), 1);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap(); // Force rename failure independent of filesystem permissions.
        assert!(store.save(&data).is_err());
        assert_eq!(
            read_catalog(&side_path(&path, ".bak")).unwrap().0.tracks[0].id,
            1
        );
        fs::remove_dir(&path).unwrap();
        drop(store);
        let (_, restored, note) = Store::open(path).unwrap();
        assert!(note.contains("recovered"));
        assert_eq!(restored.len(), 1);
    }
    #[test]
    fn unknown_schema_or_unrecoverable_damage_is_preserved_and_not_silently_reset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let (mut store, _, _) = Store::open(path.clone()).unwrap();
        let data = catalog(dir.path().join("one.wav"), 1);
        store.save(&data).unwrap();
        store.save(&data).unwrap();
        drop(store);
        let future = br#"{"version":2,"tracks":[],"newSchema":true}"#;
        fs::write(&path, future).unwrap();
        assert!(Store::open(path.clone())
            .err()
            .unwrap()
            .contains("Unsupported library version"));
        assert_eq!(fs::read(&path).unwrap(), future);
        fs::remove_file(side_path(&path, ".bak")).unwrap();
        fs::write(&path, b"broken").unwrap();
        assert!(Store::open(path.clone())
            .err()
            .unwrap()
            .contains("original preserved"));
        assert_eq!(fs::read(path).unwrap(), b"broken");
    }
    #[test]
    fn os_lock_prevents_concurrent_writers_and_is_released_on_close() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let (first, _, _) = Store::open(path.clone()).unwrap();
        assert!(Store::open(path.clone()).err().unwrap().contains("locked"));
        drop(first);
        assert!(Store::open(path).is_ok());
    }
    #[test]
    fn validates_untrusted_catalog_and_bounds_file_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let mut data = catalog(dir.path().join("one.wav"), 1);
        data.tracks.push(data.tracks[0].clone());
        assert!(data.validate().is_err());
        data.tracks.pop();
        data.tracks[0].id = -1;
        assert!(data.validate().is_err());
        data.tracks[0].id = 1;
        data.tracks[0].path = PathBuf::from("relative.wav");
        assert!(data.validate().is_err());
        let file = File::create(&path).unwrap();
        file.set_len(MAX_BYTES + 1).unwrap();
        drop(file);
        assert!(read_catalog(&path).err().unwrap().contains("32 MiB"));
    }
    #[test]
    fn worker_close_flushes_latest_snapshot_before_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        let worker = Worker::start(path.clone()).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            Event::Loaded(Ok(_))
        ));
        worker
            .save(catalog(dir.path().join("first.wav"), 1))
            .unwrap();
        // Closing during the outstanding save must persist the final, newer snapshot.
        worker
            .close(Some(catalog(dir.path().join("latest.wav"), 2)))
            .unwrap();
        let worker = Worker::start(path).unwrap();
        let Event::Loaded(Ok((library, _))) = worker.events.recv().unwrap() else {
            panic!("restore failed")
        };
        assert_eq!(library.catalog().tracks[0].id, 2);
        worker.close(None).unwrap();
    }
}
