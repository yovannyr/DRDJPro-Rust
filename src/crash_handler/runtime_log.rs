//! Typed crash-only diagnostics. No global `log` interception, arbitrary strings,
//! filesystem work or heap allocation in the producer. Never call native hooks.
use crossbeam::queue::ArrayQueue;
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock, Weak,
    },
    thread::{self, Thread},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod sink;
const QUEUE: usize = 256;
const BATCH: usize = 64;
const PERIOD: Duration = Duration::from_millis(100);
pub(super) const FILE_BYTES: usize = 65_536;
pub(super) const PEAK_BYTES: u64 = (FILE_BYTES * 4) as u64;
const MAX_LINE: usize = 512;
static HUB: OnceLock<Arc<Core>> = OnceLock::new();
static INSTALL: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Event {
    ScopeStarted,
    ScopeReleased,
    MonitorStarted,
    MonitorStopped,
    MonitorReady,
    MonitorExited,
    CaptureRequested,
    CaptureFailed,
    CaptureFinalized,
    CaptureUnavailable,
    StoreUnavailable,
    ReportStored,
    ReportInvalid,
    ReportDeleted,
    ReportQueued,
    SharingEnabled,
    SharingDisabled,
    SharingDeclined,
    ActionFailed,
    UploadAcknowledged,
    UploadRetry,
    UploadPassFailed,
    ParentExited,
    WatchdogTimeout,
    RecordsDropped,
    RecoveryDiscarded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Role {
    App,
    Monitor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum IoCode {
    Missing,
    Permission,
    Timeout,
    Invalid,
    NoSpace,
    Other,
}
impl From<io::ErrorKind> for IoCode {
    fn from(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::NotFound => Self::Missing,
            io::ErrorKind::PermissionDenied => Self::Permission,
            io::ErrorKind::TimedOut => Self::Timeout,
            io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => Self::Invalid,
            io::ErrorKind::StorageFull | io::ErrorKind::WriteZero => Self::NoSpace,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Id([u8; 32]);
impl Id {
    fn parse(text: &str) -> Option<Self> {
        (text.len() == 32
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| {
            let mut id = [0; 32];
            id.copy_from_slice(text.as_bytes());
            Self(id)
        })
    }
}
impl Serialize for Id {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(std::str::from_utf8(&self.0).map_err(serde::ser::Error::custom)?)
    }
}
impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| serde::de::Error::custom("invalid diagnostic ID"))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    schema: u8,
    time_ms: u64,
    pid: u32,
    role: Role,
    event: Event,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    io: Option<IoCode>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    count: Option<u64>,
}

struct Barrier {
    done: AtomicBool,
    ok: AtomicBool,
}
enum Work {
    Entry(Entry),
    Barrier(Arc<Barrier>),
}
struct Core {
    root: PathBuf,
    role: Role,
    pid: u32,
    queue: ArrayQueue<Work>,
    owners: AtomicUsize,
    dropped: AtomicU64,
    pending_loss: AtomicU64,
    write_errors: AtomicU64,
    timeouts: AtomicU64,
    recovered: AtomicU64,
    health: AtomicU8,
    thread: OnceLock<Thread>,
    file_bytes: usize,
}

impl Core {
    fn entry(&self, event: Event, id: Option<&str>, kind: Option<io::ErrorKind>) -> Entry {
        Entry {
            schema: 1,
            time_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(u64::MAX as u128) as u64,
            pid: self.pid,
            role: self.role,
            event,
            id: id.and_then(Id::parse),
            io: kind.map(IoCode::from),
            count: None,
        }
    }
    fn record(&self, event: Event, id: Option<&str>, kind: Option<io::ErrorKind>) {
        if self
            .queue
            .push(Work::Entry(self.entry(event, id, kind)))
            .is_err()
        {
            self.lose(1);
        }
    }
    fn lose(&self, count: u64) {
        self.dropped.fetch_add(count, Ordering::Relaxed);
        self.pending_loss.fetch_add(count, Ordering::Relaxed);
    }
    fn wake(&self) {
        if let Some(thread) = self.thread.get() {
            thread.unpark();
        }
    }
    fn flush(&self, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        let barrier = Arc::new(Barrier {
            done: AtomicBool::new(false),
            ok: AtomicBool::new(false),
        });
        let mut item = Work::Barrier(Arc::clone(&barrier));
        loop {
            match self.queue.push(item) {
                Ok(()) => break,
                Err(back) => item = back,
            }
            self.wake();
            if Instant::now() >= until {
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.wake();
        while !barrier.done.load(Ordering::Acquire) {
            if Instant::now() >= until {
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                return false;
            }
            thread::sleep(Duration::from_millis(1));
        }
        barrier.ok.load(Ordering::Acquire)
    }
}

/// A process-lifetime hub supports nested/repeated owners. Last-owner Drop only
/// requests a bounded drain; it never joins or permanently closes a shared hub.
pub(super) struct Guard {
    core: Arc<Core>,
}
impl Guard {
    fn start(root: &Path, role: Role, capacity: usize, file_bytes: usize) -> io::Result<Self> {
        if !root.is_absolute() || capacity == 0 || !(MAX_LINE..=FILE_BYTES).contains(&file_bytes) {
            return Err(io::Error::other("Invalid diagnostic log limits/root"));
        }
        let core = Arc::new(Core {
            root: root.into(),
            role,
            pid: std::process::id(),
            queue: ArrayQueue::new(capacity),
            owners: AtomicUsize::new(1),
            dropped: AtomicU64::new(0),
            pending_loss: AtomicU64::new(0),
            write_errors: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            recovered: AtomicU64::new(0),
            health: AtomicU8::new(0),
            thread: OnceLock::new(),
            file_bytes,
        });
        let weak = Arc::downgrade(&core);
        let handle = thread::Builder::new()
            .name("crash-log".into())
            .stack_size(256 * 1024)
            .spawn(move || writer(weak))?;
        let _ = core.thread.set(handle.thread().clone());
        core.record(Event::ScopeStarted, None, None);
        core.wake();
        Ok(Self { core })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.core.record(Event::ScopeReleased, None, None);
        if self.core.owners.fetch_sub(1, Ordering::AcqRel) == 1 {
            // A wait bound, not a durable-flush guarantee on a stalled/full disk.
            let _ = self.core.flush(Duration::from_millis(200));
        }
    }
}

pub(super) fn install(root: &Path, role: Role) -> io::Result<Guard> {
    let _install = INSTALL
        .lock()
        .map_err(|_| io::Error::other("Diagnostic initialization unavailable"))?;
    if let Some(core) = HUB.get() {
        if core.root != root || core.role != role {
            return Err(io::Error::other("Diagnostic root already configured"));
        }
        core.owners.fetch_add(1, Ordering::AcqRel);
        core.record(Event::ScopeStarted, None, None);
        core.wake();
        return Ok(Guard {
            core: Arc::clone(core),
        });
    }
    let guard = Guard::start(root, role, QUEUE, FILE_BYTES)?;
    let _ = HUB.set(Arc::clone(&guard.core));
    Ok(guard)
}

/// Control/FS/network paths only. No formatting/allocation or waiting for I/O.
/// The bounded lock-free queue is not a hard real-time/wait-free guarantee.
/// Native signal/exception callbacks deliberately never call this function.
pub(super) fn emit(event: Event, id: Option<&str>, kind: Option<io::ErrorKind>) {
    if let Some(core) = HUB.get().filter(|c| c.owners.load(Ordering::Acquire) != 0) {
        core.record(event, id, kind);
    }
}

pub(super) fn flush(timeout: Duration) -> bool {
    HUB.get().is_some_and(|c| c.flush(timeout))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Status {
    enabled: bool,
    health: u8,
    dropped: u64,
    errors: u64,
    timeouts: u64,
    recovered: u64,
}
pub(super) fn status() -> Status {
    HUB.get().map_or_else(Status::default, |c| Status {
        enabled: c.owners.load(Ordering::Acquire) != 0,
        health: c.health.load(Ordering::Acquire),
        dropped: c.dropped.load(Ordering::Relaxed),
        errors: c.write_errors.load(Ordering::Relaxed),
        timeouts: c.timeouts.load(Ordering::Relaxed),
        recovered: c.recovered.load(Ordering::Relaxed),
    })
}
impl Status {
    pub fn text(self) -> String {
        if !self.enabled {
            return "Diagnostic logging inactive".into();
        }
        let state = match self.health {
            0 => "starting",
            1 => "active",
            _ => "unavailable; capture/sharing remain independent",
        };
        if self.dropped == 0 && self.errors == 0 && self.timeouts == 0 && self.recovered == 0 {
            return format!(
                "Diagnostic logging {state} (256 KiB managed budget including staging)"
            );
        }
        format!("Diagnostic logging {state}. Unconfirmed/dropped events: {}; write failures: {}; flush timeouts: {}; recovered/discarded files: {}.", self.dropped, self.errors, self.timeouts, self.recovered)
    }
}

fn writer(weak: Weak<Core>) {
    loop {
        let Some(core) = weak.upgrade() else {
            break;
        };
        let mut retry_later = false;
        if !core.queue.is_empty() || core.health.load(Ordering::Acquire) == 0 {
            match sink::Sink::open(&core.root, core.file_bytes) {
                Ok(mut sink) => {
                    let mut entries = Vec::with_capacity(BATCH + 1);
                    if sink.recovered != 0 {
                        core.recovered.fetch_add(sink.recovered, Ordering::Relaxed);
                        let mut entry = core.entry(Event::RecoveryDiscarded, None, None);
                        entry.count = Some(sink.recovered);
                        entries.push(entry);
                    }
                    let mut barriers = Vec::new();
                    for _ in 0..BATCH {
                        match core.queue.pop() {
                            Some(Work::Entry(entry)) => entries.push(entry),
                            Some(Work::Barrier(barrier)) => barriers.push(barrier),
                            None => break,
                        }
                    }
                    let count = entries.len() as u64;
                    let lost = core.pending_loss.swap(0, Ordering::AcqRel);
                    if lost != 0 {
                        let mut entry = core.entry(Event::RecordsDropped, None, None);
                        entry.count = Some(lost);
                        entries.push(entry);
                    }
                    let ok = sink.append(&entries).is_ok();
                    if ok {
                        core.health.store(1, Ordering::Release);
                    } else {
                        core.health.store(2, Ordering::Release);
                        core.write_errors.fetch_add(1, Ordering::Relaxed);
                        core.pending_loss.fetch_add(lost, Ordering::Relaxed);
                        core.lose(count);
                    }
                    for barrier in barriers {
                        barrier.ok.store(ok, Ordering::Release);
                        barrier.done.store(true, Ordering::Release);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => retry_later = true,
                Err(_) => {
                    retry_later = true;
                    core.health.store(2, Ordering::Release);
                    core.write_errors.fetch_add(1, Ordering::Relaxed);
                    // Fail open and bound backlog even if the sink never recovers.
                    for _ in 0..BATCH {
                        match core.queue.pop() {
                            Some(Work::Entry(_)) => core.lose(1),
                            Some(Work::Barrier(b)) => {
                                b.ok.store(false, Ordering::Release);
                                b.done.store(true, Ordering::Release);
                            }
                            None => break,
                        }
                    }
                }
            }
        }
        let busy = !core.queue.is_empty();
        drop(core); // Standalone test/service cores can terminate via Weak expiry.
        if busy && !retry_later {
            thread::yield_now();
        } else {
            thread::park_timeout(PERIOD);
        }
    }
}

#[cfg(test)]
mod tests;

// Explicit isolated probe modes; no audio/UI resources and no real user reports.
pub(super) fn probe(root: &Path, mode: &str) -> io::Result<()> {
    let check = |ok| {
        if ok {
            Ok(())
        } else {
            Err(io::Error::other("Diagnostic probe failed"))
        }
    };
    if mode == "owners" {
        let first = install(root, Role::App)?;
        let second = install(root, Role::App)?;
        emit(
            Event::ReportStored,
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            None,
        );
        drop(first);
        emit(Event::ReportDeleted, None, None);
        check(flush(Duration::from_secs(3)))?;
        drop(second);
        check(!status().enabled)?;
        let resumed = install(root, Role::App)?;
        emit(Event::SharingDeclined, None, None);
        check(flush(Duration::from_secs(3)))?;
        drop(resumed);
        return check(!status().enabled);
    }
    if mode == "host" {
        struct Host;
        static HOST: Host = Host;
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        impl log::Log for Host {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool {
                true
            }
            fn log(&self, _: &log::Record<'_>) {
                CALLS.fetch_add(1, Ordering::Relaxed);
            }
            fn flush(&self) {}
        }
        log::set_logger(&HOST).map_err(|_| io::Error::other("Host logger setup failed"))?;
        log::set_max_level(log::LevelFilter::Info);
        let _owner = install(root, Role::App)?;
        log::warn!(target:"audio", "SECRET https://credentials/private");
        emit(Event::ReportStored, None, None);
        check(flush(Duration::from_secs(3)))?;
        return check(
            CALLS.load(Ordering::Relaxed) == 1 && log::max_level() == log::LevelFilter::Info,
        );
    }
    let _owner = install(root, Role::App)?;
    let batches = match mode {
        "small" => 2,
        "rotate" => 32,
        _ => return Err(io::Error::other("Unknown diagnostic probe")),
    };
    for batch in 0..batches {
        for n in 0..64 {
            emit(
                Event::ReportStored,
                Some(&format!("{:032x}", batch * 64 + n)),
                None,
            );
        }
        check(flush(Duration::from_secs(3)))?;
    }
    Ok(())
}
