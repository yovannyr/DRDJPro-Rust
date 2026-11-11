use crate::*;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

/// Immutable UI snapshot. A backend error retains the last good device list.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub devices: Vec<DeviceInfo>,
    /// Changes since the previous scan, not a durable event queue. Slow
    /// consumers must diff their last observed `devices` against the new list.
    pub added: Vec<DeviceId>,
    /// Changes since the previous scan; see `added` for consumption semantics.
    pub removed: Vec<DeviceId>,
    pub error: Option<DiscoveryError>,
}
impl Snapshot {
    fn next(
        &self,
        result: Result<Vec<DeviceInfo>, DiscoveryError>,
        registry: &DeviceRegistry,
    ) -> Self {
        let mut next = Self {
            revision: self.revision.saturating_add(1),
            ..Self::default()
        };
        match result {
            Ok(mut devices) => {
                devices.sort_by_key(|device| device.id);
                if devices.windows(2).any(|pair| pair[0].id == pair[1].id) {
                    return self.next(Err(DiscoveryError::Malformed), registry);
                }
                registry.apply(&mut devices);
                next.added = devices
                    .iter()
                    .filter(|device| {
                        self.devices
                            .binary_search_by(|previous| previous.id.cmp(&device.id))
                            .is_err()
                    })
                    .map(|device| device.id)
                    .collect();
                next.removed = self
                    .devices
                    .iter()
                    .filter(|previous| {
                        devices
                            .binary_search_by(|device| device.id.cmp(&previous.id))
                            .is_err()
                    })
                    .map(|device| device.id)
                    .collect();
                next.devices = devices;
            }
            Err(error) => {
                next.devices = self.devices.clone();
                next.error = Some(error);
            }
        }
        next
    }
}

/// Refresh requests are coalesced, never queued without a bound.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RefreshError {
    #[error("USB discovery worker stopped")]
    Stopped,
}

/// Bounded discovery actor. UI access uses try_lock and try_send only.
/// Dropping requests stop without joining a possibly blocked native backend.
pub struct DiscoveryWorker {
    refresh: SyncSender<()>,
    snapshot: Arc<Mutex<Arc<Snapshot>>>,
    stop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}
struct Running(Arc<AtomicBool>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl DiscoveryWorker {
    /// Start libusb on a dedicated thread, polling at most every 200 ms after scans.
    pub fn native(registry: DeviceRegistry) -> std::io::Result<Self> {
        Self::spawn(NativeDiscovery::new, registry, Duration::from_millis(200))
    }
    /// Inject a backend factory. Failed initialization is retried; the factory
    /// and all scans run on the worker. Intervals must be 1 ms through 1 second.
    pub fn spawn<B: Discovery>(
        mut factory: impl FnMut() -> Result<B, DiscoveryError> + Send + 'static,
        registry: DeviceRegistry,
        interval: Duration,
    ) -> std::io::Result<Self> {
        if !(Duration::from_millis(1)..=Duration::from_secs(1)).contains(&interval) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid USB poll interval",
            ));
        }
        let (refresh, receiver) = mpsc::sync_channel(1);
        let snapshot = Arc::new(Mutex::new(Arc::new(Snapshot::default())));
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let worker_snapshot = Arc::clone(&snapshot);
        let worker_stop = Arc::clone(&stop);
        let guard = Running(Arc::clone(&running));
        thread::Builder::new()
            .name("usb-discovery".into())
            .spawn(move || {
                let _guard = guard;
                let mut backend: Option<B> = None;
                let mut current = Arc::new(Snapshot::default());
                while !worker_stop.load(Ordering::Acquire) {
                    let result = match backend.as_mut() {
                        Some(backend) => backend.scan(),
                        None => match factory() {
                            Ok(mut initialized) => {
                                let result = initialized.scan();
                                backend = Some(initialized);
                                result
                            }
                            Err(error) => Err(error),
                        },
                    };
                    if result.is_err() {
                        backend = None;
                    }
                    current = Arc::new(current.next(result, &registry));
                    if let Ok(mut slot) = worker_snapshot.lock() {
                        *slot = Arc::clone(&current);
                    }
                    match receiver.recv_timeout(interval) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            refresh,
            snapshot,
            stop,
            running,
        })
    }
    /// Borrow the latest snapshot without waiting; None means publication busy.
    pub fn try_snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snapshot.try_lock().ok().map(|slot| Arc::clone(&slot))
    }
    /// Coalesce refreshes. This never waits for native I/O or queue capacity.
    pub fn refresh(&self) -> Result<(), RefreshError> {
        if !self.is_running() {
            return Err(RefreshError::Stopped);
        }
        match self.refresh.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => Ok(()),
            Err(TrySendError::Disconnected(())) => Err(RefreshError::Stopped),
        }
    }
    /// True while the background thread is alive, including initialization.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}
impl Drop for DiscoveryWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.refresh.try_send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::mpsc, time::Instant};

    fn device(address: u8) -> DeviceInfo {
        DeviceInfo {
            id: DeviceId { bus: 1, address },
            ports: vec![address],
            vendor_id: 0x1234,
            product_id: 1,
            class: 0,
            interfaces: Vec::new(),
            descriptor_error: None,
            profile: None,
        }
    }
    fn wait(worker: &DiscoveryWorker, revision: u64) -> Arc<Snapshot> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Some(snapshot) = worker.try_snapshot().filter(|s| s.revision >= revision) {
                return snapshot;
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("discovery worker did not publish revision {revision}");
    }
    #[test]
    fn equal_vid_pid_devices_are_distinct_and_removal_is_explicit() {
        let registry = DeviceRegistry::default();
        let first = Snapshot::default().next(Ok(vec![device(2), device(1)]), &registry);
        assert_eq!(first.devices.len(), 2);
        assert_eq!(first.added, vec![device(1).id, device(2).id]);
        let second = first.next(Ok(vec![device(2)]), &registry);
        assert_eq!(second.removed, vec![device(1).id]);
        assert!(second.added.is_empty());
        assert_eq!(second.devices[0].id, device(2).id);
    }
    #[test]
    fn unavailable_optional_port_metadata_does_not_look_like_a_disconnect() {
        let registry = DeviceRegistry::default();
        let first = Snapshot::default().next(Ok(vec![device(1)]), &registry);
        let mut changed = device(1);
        changed.ports.clear();
        let second = first.next(Ok(vec![changed]), &registry);
        assert!(second.added.is_empty());
        assert!(second.removed.is_empty());
        assert!(second.devices[0].ports.is_empty());
    }
    #[test]
    fn failed_and_malformed_scans_preserve_last_good_devices_without_false_unplugs() {
        let registry = DeviceRegistry::default();
        let first = Snapshot::default().next(Ok(vec![device(1)]), &registry);
        for result in [
            Err(DiscoveryError::AccessDenied),
            Ok(vec![device(2), device(2)]),
        ] {
            let failed = first.next(result, &registry);
            assert!(failed.error.is_some());
            assert_eq!(failed.devices, first.devices);
            assert!(failed.added.is_empty());
            assert!(failed.removed.is_empty());
            assert_eq!(failed.revision, first.revision + 1);
            let recovered = failed.next(Ok(Vec::new()), &registry);
            assert!(recovered.error.is_none());
            assert_eq!(recovered.removed, vec![device(1).id]);
        }
    }
    #[test]
    fn registry_rejects_duplicates_and_only_marks_explicit_profiles() {
        let mut registry = DeviceRegistry::default();
        registry.register(0x1234, 1, "Test fixture".into()).unwrap();
        assert!(registry.register(0x1234, 1, "Replacement".into()).is_err());
        assert!(registry.register(0x1234, 2, "  ".into()).is_err());
        let snapshot = Snapshot::default().next(Ok(vec![device(1)]), &registry);
        assert_eq!(snapshot.devices[0].profile.as_deref(), Some("Test fixture"));
        let unknown = Snapshot::default().next(Ok(vec![device(1)]), &DeviceRegistry::default());
        assert!(unknown.devices[0].profile.is_none());
    }
    type ScanQueue = VecDeque<Result<Vec<DeviceInfo>, DiscoveryError>>;
    struct Mock(Arc<Mutex<ScanQueue>>);
    impl Discovery for Mock {
        fn scan(&mut self) -> Result<Vec<DeviceInfo>, DiscoveryError> {
            self.0
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(vec![device(2)]))
        }
    }
    #[test]
    fn initialization_and_scans_run_off_caller_thread_and_recover_after_failure() {
        let caller = thread::current().id();
        let (where_tx, where_rx) = mpsc::channel();
        let results = Arc::new(Mutex::new(VecDeque::from([
            Err(DiscoveryError::Timeout),
            Ok(vec![device(2)]),
        ])));
        let mut attempts = 0;
        let worker = DiscoveryWorker::spawn(
            move || {
                where_tx.send(thread::current().id()).unwrap();
                attempts += 1;
                if attempts == 1 {
                    Err(DiscoveryError::AccessDenied)
                } else {
                    Ok(Mock(Arc::clone(&results)))
                }
            },
            DeviceRegistry::default(),
            Duration::from_secs(1),
        )
        .unwrap();
        assert_ne!(
            where_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            caller
        );
        assert_eq!(wait(&worker, 1).error, Some(DiscoveryError::AccessDenied));
        worker.refresh().unwrap();
        assert_eq!(wait(&worker, 2).error, Some(DiscoveryError::Timeout));
        worker.refresh().unwrap();
        let recovered = wait(&worker, 3);
        assert!(recovered.error.is_none());
        assert_eq!(recovered.devices, vec![device(2)]);
    }
    struct Blocked {
        entered: Option<mpsc::Sender<()>>,
        release: mpsc::Receiver<()>,
        finished: mpsc::Sender<()>,
    }
    impl Discovery for Blocked {
        fn scan(&mut self) -> Result<Vec<DeviceInfo>, DiscoveryError> {
            if let Some(entered) = self.entered.take() {
                entered.send(()).unwrap();
            }
            self.release.recv().unwrap();
            self.finished.send(()).unwrap();
            Ok(Vec::new())
        }
    }
    #[test]
    fn refresh_and_drop_do_not_wait_for_blocked_backend_and_thread_exits_after_release() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let mut backend = Some(Blocked {
            entered: Some(entered_tx),
            release: release_rx,
            finished: finished_tx,
        });
        let worker = DiscoveryWorker::spawn(
            move || Ok(backend.take().unwrap()),
            DeviceRegistry::default(),
            Duration::from_secs(1),
        )
        .unwrap();
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..1000 {
            worker.refresh().unwrap();
        }
        let running = Arc::clone(&worker.running);
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let dropper = thread::spawn(move || {
            drop(worker);
            dropped_tx.send(()).unwrap();
        });
        let dropped = dropped_rx.recv_timeout(Duration::from_millis(200));
        release_tx.send(()).unwrap();
        dropper.join().unwrap();
        assert!(dropped.is_ok(), "Drop waited for backend I/O");
        finished_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while running.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(!running.load(Ordering::Acquire));
    }
    #[test]
    fn invalid_poll_intervals_are_rejected_before_starting_backend() {
        let factory = || -> Result<Mock, DiscoveryError> { panic!("must not initialize") };
        assert!(
            DiscoveryWorker::spawn(factory, DeviceRegistry::default(), Duration::ZERO).is_err()
        );
        assert!(
            DiscoveryWorker::spawn(factory, DeviceRegistry::default(), Duration::from_secs(2))
                .is_err()
        );
    }
}
