//! UI-owned bounded thumbnail cache. File I/O and decoding run on one worker,
//! independently of track loading and the realtime audio callback. Images are
//! shared Slint handles; neither the library nor decks retain full-size artwork.
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
};

// Fits the entire 200-row library page plus decks without eviction/redecode
// loops. Retained RGBA pixels are at most 16 MiB; decoder/GPU overhead is extra.
const CAPACITY: usize = 256;
const IN_FLIGHT: usize = 4;
type Pixels = (u32, u32, Vec<u8>);
type Loader = Box<dyn Fn(&Path) -> Option<Pixels> + Send>;
type Reply = (PathBuf, u64, Option<Pixels>);
type Worker = (SyncSender<(PathBuf, u64)>, Receiver<Reply>);

fn cache_key(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::{
            ffi::OsString,
            path::{Component, Prefix},
        };
        let mut components = path.components();
        let Some(Component::Prefix(prefix)) = components.next() else {
            return path.to_owned();
        };
        let mut normalized = match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => {
                PathBuf::from(format!("{}:", char::from(drive.to_ascii_uppercase())))
            }
            Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                let mut name = OsString::from(r"\\");
                name.push(server);
                name.push(r"\");
                name.push(share);
                PathBuf::from(name)
            }
            _ => return path.to_owned(),
        };
        for component in components {
            normalized.push(component.as_os_str());
        }
        normalized
    }
    #[cfg(not(windows))]
    {
        path.to_owned()
    }
}

#[derive(Clone, Default)]
pub(crate) struct Artwork {
    pub image: slint::Image,
    pub color: u32,
}

pub(crate) struct CoverCache {
    entries: VecDeque<(PathBuf, Artwork)>,
    pending: HashMap<PathBuf, u64>,
    next: u64,
    loader: Option<Loader>,
    worker: Option<Worker>,
}

impl Default for CoverCache {
    fn default() -> Self {
        Self::with_loader(Box::new(|path| {
            crate::cover_art::read_cancellable(path, &Default::default())
                .ok()
                .flatten()
        }))
    }
}

impl CoverCache {
    pub(crate) fn with_loader(loader: Loader) -> Self {
        Self {
            entries: VecDeque::new(),
            pending: HashMap::new(),
            next: 0,
            loader: Some(loader),
            worker: None,
        }
    }

    /// Nonblocking lookup/enqueue only. Missing and invalid covers are cached too.
    pub fn get(&mut self, path: &Path) -> Artwork {
        let key = cache_key(path);
        if let Some(index) = self.entries.iter().position(|(p, _)| p == &key) {
            let entry = self.entries.remove(index).expect("existing cache index");
            let result = entry.1.clone();
            self.entries.push_back(entry);
            return result;
        }
        if self.pending.contains_key(&key) || self.pending.len() >= IN_FLIGHT {
            return Artwork::default();
        }
        if let Some(loader) = self.loader.take() {
            let (tx, requests) = mpsc::sync_channel::<(PathBuf, u64)>(IN_FLIGHT);
            let (results, rx) = mpsc::sync_channel(IN_FLIGHT);
            if std::thread::Builder::new()
                .name("cover-thumbnails".into())
                .spawn(move || {
                    while let Ok((path, generation)) = requests.recv() {
                        let pixels = loader(&path);
                        if results.send((path, generation, pixels)).is_err() {
                            break;
                        }
                    }
                })
                .is_ok()
            {
                self.worker = Some((tx, rx));
            }
        }
        if let Some((tx, _)) = &self.worker {
            self.next = self.next.wrapping_add(1);
            if tx.try_send((path.into(), self.next)).is_ok() {
                self.pending.insert(key, self.next);
            }
        }
        Artwork::default()
    }

    /// Invalidate on explicit track replacement/reload, including negative entries.
    pub fn invalidate(&mut self, path: &Path) {
        let key = cache_key(path);
        self.entries.retain(|(p, _)| p != &key);
        self.pending.remove(&key);
    }

    /// Convert only small thumbnails on the UI thread; never wait for the worker.
    pub fn poll(&mut self) -> bool {
        let replies: Vec<_> = self
            .worker
            .as_ref()
            .map_or_else(Vec::new, |(_, rx)| rx.try_iter().take(IN_FLIGHT).collect());
        let mut changed = false;
        for (path, generation, pixels) in replies {
            let path = cache_key(&path);
            if self.pending.get(&path) != Some(&generation) {
                continue;
            }
            self.pending.remove(&path);
            let artwork = pixels.map_or_else(Artwork::default, |(w, h, pixels)| Artwork {
                color: crate::cover_art::dominant_color(&pixels),
                image: slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
                    &pixels, w, h,
                )),
            });
            if self.entries.len() == CAPACITY {
                self.entries.pop_front();
            }
            self.entries.push_back((path, artwork));
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn slow_cover_never_blocks_lookup_queue_is_bounded_and_missing_is_cached() {
        i_slint_backend_testing::init_no_event_loop();
        let (started, observed) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let mut cache = CoverCache::with_loader(Box::new(move |path| {
            started.send(path.to_owned()).ok()?;
            wait.recv().ok()?;
            None
        }));
        for i in 0..100 {
            assert_eq!(cache.get(Path::new(&i.to_string())).image.size().width, 0);
        }
        assert_eq!(cache.pending.len(), IN_FLIGHT);
        assert_eq!(
            observed.recv_timeout(Duration::from_secs(5)).unwrap(),
            Path::new("0")
        );
        release.send(()).unwrap();
        // The next request starts only after the first result has been delivered.
        observed.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(cache.poll());
        assert!(!cache.pending.contains_key(Path::new("0")));
        cache.get(Path::new("0"));
        assert!(!cache.pending.contains_key(Path::new("0")));
        // Release the remaining requests so the test owns no blocked worker.
        for _ in 1..IN_FLIGHT {
            release.send(()).unwrap();
        }
    }

    #[test]
    fn lru_is_bounded_and_invalidation_removes_cached_failure() {
        let mut cache = CoverCache::default();
        for i in 0..CAPACITY {
            cache
                .entries
                .push_back((PathBuf::from(i.to_string()), Artwork::default()));
        }
        cache.get(Path::new("0"));
        assert_eq!(cache.entries.back().unwrap().0, Path::new("0"));
        cache.invalidate(Path::new("0"));
        assert_eq!(cache.entries.len(), CAPACITY - 1);
    }

    #[test]
    fn completion_evicts_lru_and_stale_results_cannot_restore_invalidated_cover() {
        i_slint_backend_testing::init_no_event_loop();
        let mut cache = CoverCache::default();
        let (results, rx) = mpsc::sync_channel(IN_FLIGHT);
        let (tx, _requests) = mpsc::sync_channel(IN_FLIGHT);
        cache.worker = Some((tx, rx));
        cache.loader = None;
        for i in 0..CAPACITY {
            let path = PathBuf::from(i.to_string());
            cache.pending.insert(path.clone(), i as u64);
            results.send((path, i as u64, None)).unwrap();
            assert!(cache.poll());
        }
        cache.get(Path::new("0"));
        let path = PathBuf::from("new");
        cache.pending.insert(path.clone(), 100);
        cache.invalidate(&path);
        cache.pending.insert(path.clone(), 101);
        results
            .send((path.clone(), 100, Some((1, 1, vec![255; 4]))))
            .unwrap();
        assert!(!cache.poll());
        assert_eq!(cache.pending.get(&path), Some(&101));
        results
            .send((path.clone(), 101, Some((1, 1, vec![255; 4]))))
            .unwrap();
        assert!(cache.poll());
        assert_eq!(cache.entries.len(), CAPACITY);
        assert!(cache.entries.iter().any(|(p, _)| p == Path::new("0")));
        assert!(!cache.entries.iter().any(|(p, _)| p == Path::new("1")));
        assert_eq!(cache.get(&path).image.size().width, 1);
    }

    #[test]
    fn complete_library_page_and_decks_do_not_thrash_or_enqueue_again() {
        let mut cache = CoverCache::default();
        let (results, rx) = mpsc::sync_channel(IN_FLIGHT);
        let (tx, requests) = mpsc::sync_channel(IN_FLIGHT);
        cache.worker = Some((tx, rx));
        cache.loader = None;
        for i in 0..202 {
            let path = PathBuf::from(i.to_string());
            cache.pending.insert(path.clone(), i);
            results.send((path, i, None)).unwrap();
            assert!(cache.poll());
        }
        for i in 0..202 {
            cache.get(Path::new(&i.to_string()));
        }
        assert!(requests.try_recv().is_err());
        assert!(cache.pending.is_empty());
        assert_eq!(cache.entries.len(), 202);
    }

    #[cfg(windows)]
    #[test]
    fn windows_canonical_and_library_paths_share_identity() {
        assert_eq!(
            cache_key(Path::new(r"\\?\C:\Music\Track.mp3")),
            cache_key(Path::new(r"C:\Music\Track.mp3"))
        );
        assert_eq!(
            cache_key(Path::new(r"\\?\UNC\server\share\Track.mp3")),
            cache_key(Path::new(r"\\server\share\Track.mp3"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn native_windows_names_and_case_sensitive_files_never_collide() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let a = PathBuf::from(OsString::from_wide(&[0x43, 0x3a, 0x5c, 0xd800]));
        let b = PathBuf::from(OsString::from_wide(&[0x43, 0x3a, 0x5c, 0xd801]));
        assert_ne!(cache_key(&a), cache_key(&b));
        assert_ne!(
            cache_key(Path::new(r"C:\Music\A.mp3")),
            cache_key(Path::new(r"C:\Music\a.mp3"))
        );
    }
}
