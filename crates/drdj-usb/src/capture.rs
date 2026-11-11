//! Continuous HID acquisition on a dedicated reader; no output work on this lane.
use crate::*;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

/// Fixed report queue. Overflow is visible and the oldest report is discarded.
pub const HID_CAPTURE_CAPACITY: usize = 256;
/// Connection epochs change on each successful open, even if a custom backend
/// reuses its session IDs. Consumers must release held controls on epoch/loss.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureStatus {
    Starting,
    Connected {
        epoch: u64,
        session: HidSessionId,
    },
    Reconnecting {
        epoch: u64,
        previous_session: Option<HidSessionId>,
        error: IoError,
    },
    /// Permanent policy/descriptor failures require explicit reconfiguration.
    Failed {
        epoch: u64,
        previous_session: Option<HidSessionId>,
        error: IoError,
    },
    Stopped,
}
/// Observed on the reader, not a physical USB arrival timestamp.
#[derive(Clone, Debug)]
pub struct CapturedReport {
    pub capture_id: u64,
    pub epoch: u64,
    pub session: HidSessionId,
    pub sequence: u64,
    pub observed_at: Instant,
    pub report: HidReport,
}
/// A bounded drain plus loss/connection state; no transient status-only queue.
#[derive(Clone, Debug)]
pub struct CaptureBatch {
    pub status: CaptureStatus,
    pub reports: Vec<CapturedReport>,
    pub overflowed_total: u64,
    pub discarded_total: u64,
}
struct Queue {
    status: CaptureStatus,
    reports: VecDeque<CapturedReport>,
    overflowed: u64,
    discarded: u64,
}
impl Queue {
    fn clear(&mut self) {
        self.discarded = self.discarded.saturating_add(self.reports.len() as u64);
        self.reports.clear();
    }
    fn push(&mut self, report: CapturedReport) {
        if self.reports.len() == HID_CAPTURE_CAPACITY {
            self.reports.pop_front();
            self.overflowed = self.overflowed.saturating_add(1);
        }
        self.reports.push_back(report);
    }
}
struct Stopped(Arc<Mutex<Queue>>);
impl Drop for Stopped {
    fn drop(&mut self) {
        let mut queue = self.0.lock().unwrap_or_else(|e| e.into_inner());
        queue.clear();
        if !matches!(queue.status, CaptureStatus::Failed { .. }) {
            queue.status = CaptureStatus::Stopped;
        }
    }
}

/// Explicitly selected reader. Construction/enumeration never selects a target
/// automatically. Control/UI consumers drain packets; audio callbacks do not.
pub struct HidCapture {
    id: u64,
    queue: Arc<Mutex<Queue>>,
    stop: Arc<AtomicBool>,
    wake: SyncSender<()>,
}
impl HidCapture {
    /// Start capture only for an explicitly selected non-system HID target.
    pub fn native(target: HidTarget) -> std::io::Result<Self> {
        Self::spawn(crate::native_hid::CaptureHid::new, target)
    }
    /// Inject worker-local native transport for deterministic packet tests.
    /// On input failure the backend must return its error before potentially
    /// blocking resource destruction; capture publishes loss before dropping it.
    pub fn spawn<B: Transport>(
        mut factory: impl FnMut() -> Result<B, IoError> + Send + 'static,
        target: HidTarget,
    ) -> std::io::Result<Self> {
        HidCommand::Open {
            target: target.clone(),
        }
        .validate()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        let id = crate::session::next_id()
            .ok_or_else(|| std::io::Error::other("capture identity exhausted"))?;
        let queue = Arc::new(Mutex::new(Queue {
            status: CaptureStatus::Starting,
            reports: VecDeque::with_capacity(HID_CAPTURE_CAPACITY),
            overflowed: 0,
            discarded: 0,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (wake, receiver) = mpsc::sync_channel(1);
        let worker_queue = Arc::clone(&queue);
        let worker_stop = Arc::clone(&stop);
        let guard = Stopped(Arc::clone(&queue));
        thread::Builder::new()
            .name("hid-capture".into())
            .spawn(move || {
                let _guard = guard;
                let mut epoch = 0u64;
                let mut sequence = 0u64;
                while !worker_stop.load(Ordering::Acquire) {
                    let mut previous_session = None;
                    // Retain ownership outside the operation so loss is visible
                    // even while the native backend's destructor is blocked.
                    let mut backend = None;
                    let outcome = (|| -> Result<(), IoError> {
                        backend = Some(factory()?);
                        let backend = backend.as_mut().ok_or(IoError::Stopped)?;
                        if worker_stop.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        let reply = backend.execute(IoCommand::Hid(HidCommand::Open {
                            target: target.clone(),
                        }))?;
                        let IoReply::Hid(HidReply::Opened { session, layout }) = reply else {
                            return Err(IoError::Backend("unexpected capture open reply".into()));
                        };
                        previous_session = Some(session);
                        if layout.contains_system_input() {
                            return Err(IoError::ProtectedInput);
                        }
                        if layout.input_capacity().is_none() {
                            return Err(IoError::NotSupported);
                        }
                        epoch = epoch.checked_add(1).ok_or(IoError::SessionLimit)?;
                        {
                            let mut queue = worker_queue.lock().unwrap_or_else(|e| e.into_inner());
                            queue.clear();
                            queue.status = CaptureStatus::Connected { epoch, session };
                        }
                        while !worker_stop.load(Ordering::Acquire) {
                            let reply =
                                backend.execute(IoCommand::Hid(HidCommand::ReadInputWait {
                                    session,
                                    timeout: Duration::from_millis(5),
                                }))?;
                            let IoReply::Hid(HidReply::Input(report)) = reply else {
                                return Err(IoError::Backend(
                                    "unexpected capture input reply".into(),
                                ));
                            };
                            if worker_stop.load(Ordering::Acquire) {
                                break;
                            }
                            if let Some(report) = report {
                                let (report_id, logical_length) =
                                    layout.input_identity(&report.data)?;
                                if report.report_id != report_id
                                    || report.logical_length != logical_length
                                {
                                    return Err(IoError::InvalidHidReport);
                                }
                                sequence = sequence.checked_add(1).ok_or(IoError::SessionLimit)?;
                                let report = CapturedReport {
                                    capture_id: id,
                                    epoch,
                                    session,
                                    sequence,
                                    observed_at: Instant::now(),
                                    report,
                                };
                                worker_queue
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .push(report);
                            }
                        }
                        Ok(())
                    })();
                    if worker_stop.load(Ordering::Acquire) {
                        let mut queue = worker_queue.lock().unwrap_or_else(|e| e.into_inner());
                        queue.clear();
                        queue.status = CaptureStatus::Stopped;
                        drop(queue);
                        drop(backend);
                        break;
                    }
                    let mut retry = true;
                    if let Err(error) = outcome {
                        retry = matches!(
                            &error,
                            IoError::NotFound
                                | IoError::Disconnected
                                | IoError::AccessDenied
                                | IoError::Busy
                                | IoError::Timeout
                                | IoError::HidBackend(_)
                        );
                        let mut queue = worker_queue.lock().unwrap_or_else(|e| e.into_inner());
                        queue.clear();
                        queue.status = if retry {
                            CaptureStatus::Reconnecting {
                                epoch,
                                previous_session,
                                error,
                            }
                        } else {
                            CaptureStatus::Failed {
                                epoch,
                                previous_session,
                                error,
                            }
                        };
                    }
                    drop(backend); // loss/queue invalidation was already published
                    if !retry {
                        break;
                    }
                    match receiver.recv_timeout(Duration::from_millis(250)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            id,
            queue,
            stop,
            wake,
        })
    }
    /// Stable identity of this explicit capture subscription.
    pub fn id(&self) -> u64 {
        self.id
    }
    /// Nonblocking bounded drain. On epoch change, loss or non-connected status,
    /// consumers must reset gesture state/release controls before applying data.
    pub fn try_drain(&self, maximum: usize) -> Option<CaptureBatch> {
        let mut queue = self.queue.try_lock().ok()?;
        let take = maximum.min(queue.reports.len()).min(HID_CAPTURE_CAPACITY);
        Some(CaptureBatch {
            status: queue.status.clone(),
            reports: queue.reports.drain(..take).collect(),
            overflowed_total: queue.overflowed,
            discarded_total: queue.discarded,
        })
    }
}
impl Drop for HidCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
    }
}
