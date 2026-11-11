//! Bounded transfer actor. Handles and native calls stay on its worker.
use crate::{DeviceId, DeviceInfo};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
        Arc,
    },
    thread,
    time::Duration,
};
use thiserror::Error;

/// Spec transfer limit, enforced before enqueue and again by the native backend.
pub const MAX_TRANSFER_BYTES: usize = 16 * 1024;
/// Accepted requests awaiting the native worker; no unbounded queue.
pub const IO_QUEUE_CAPACITY: usize = 16;
/// Recheck expected VID/PID when resolving a bus/address selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceTarget {
    pub id: DeviceId,
    pub vendor_id: u16,
    pub product_id: u16,
}
impl From<&DeviceInfo> for DeviceTarget {
    fn from(value: &DeviceInfo) -> Self {
        Self {
            id: value.id,
            vendor_id: value.vendor_id,
            product_id: value.product_id,
        }
    }
}
/// Worker-assigned lease identity; forged/expired identities are rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionId(pub u64);
/// USB request type; direction is determined by the command variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlKind {
    Standard,
    Class,
    Vendor,
}
/// USB recipient, kept explicit rather than accepting reserved bmRequestType bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipient {
    Device,
    Interface,
    Endpoint,
    Other,
}
/// Setup packet fields for a control transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlSetup {
    pub kind: ControlKind,
    pub recipient: Recipient,
    pub request: u8,
    pub value: u16,
    pub index: u16,
}

/// Explicit operations. Nothing opens, claims, detaches or resets automatically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IoCommand {
    /// Separate command namespace; handled only by a HID transport worker.
    Hid(crate::HidCommand),
    ReadStrings {
        target: DeviceTarget,
        timeout: Duration,
    },
    /// Only string descriptors may be written; identity/configuration
    /// descriptors would invalidate managed interface state.
    SetString {
        target: DeviceTarget,
        index: u8,
        language: u16,
        value: String,
        timeout: Duration,
    },
    Open {
        target: DeviceTarget,
        interface: u8,
        alternate: u8,
    },
    Close {
        session: SessionId,
    },
    /// Use libusb's managed clear-halt operation to synchronize data toggles.
    ClearHalt {
        session: SessionId,
        endpoint: u8,
    },
    ControlIn {
        session: SessionId,
        setup: ControlSetup,
        length: usize,
        timeout: Duration,
    },
    ControlOut {
        session: SessionId,
        setup: ControlSetup,
        data: Vec<u8>,
        timeout: Duration,
    },
    BulkIn {
        session: SessionId,
        endpoint: u8,
        length: usize,
        timeout: Duration,
    },
    BulkOut {
        session: SessionId,
        endpoint: u8,
        data: Vec<u8>,
        timeout: Duration,
    },
    InterruptIn {
        session: SessionId,
        endpoint: u8,
        length: usize,
        timeout: Duration,
    },
    InterruptOut {
        session: SessionId,
        endpoint: u8,
        data: Vec<u8>,
        timeout: Duration,
    },
}
impl IoCommand {
    /// Validate queue payload and finite, nonzero native transfer deadlines.
    pub fn validate(&self) -> Result<(), IoError> {
        match self {
            Self::ControlOut { setup, .. } if setup.kind == ControlKind::Standard => {
                return Err(IoError::InvalidParameter(
                    "raw standard writes require a managed command",
                ));
            }
            Self::ControlIn { setup, .. }
                if setup.kind == ControlKind::Standard
                    && !matches!(setup.request, 0 | 6 | 8 | 10 | 12) =>
            {
                return Err(IoError::InvalidParameter(
                    "standard control input must be a query",
                ));
            }
            Self::SetString {
                target,
                index,
                language,
                value,
                ..
            } => {
                if target.vendor_id == 0
                    || *index == 0
                    || *language == 0
                    || value.len() > 504
                    || value.encode_utf16().count() > 126
                {
                    return Err(IoError::InvalidParameter(
                        "invalid USB string descriptor selection or length",
                    ));
                }
            }
            Self::ClearHalt { endpoint, .. } if endpoint & 0x70 != 0 || endpoint & 0x0f == 0 => {
                return Err(IoError::InvalidParameter("invalid halt endpoint"));
            }
            _ => {}
        }
        let (length, timeout) = match self {
            Self::Open { target, .. } => {
                return if target.vendor_id == 0 {
                    Err(IoError::InvalidParameter("invalid vendor identity"))
                } else {
                    Ok(())
                }
            }
            Self::Close { .. } | Self::ClearHalt { .. } => return Ok(()),
            Self::Hid(command) => return command.validate(),
            Self::ReadStrings { timeout, .. } | Self::SetString { timeout, .. } => (0, *timeout),
            Self::ControlIn {
                length, timeout, ..
            }
            | Self::BulkIn {
                length, timeout, ..
            }
            | Self::InterruptIn {
                length, timeout, ..
            } => (*length, *timeout),
            Self::ControlOut { data, timeout, .. }
            | Self::BulkOut { data, timeout, .. }
            | Self::InterruptOut { data, timeout, .. } => (data.len(), *timeout),
        };
        if length > MAX_TRANSFER_BYTES {
            return Err(IoError::InvalidParameter("transfer exceeds 16 KiB"));
        }
        if !(Duration::from_millis(1)..=Duration::from_millis(250)).contains(&timeout) {
            return Err(IoError::InvalidParameter(
                "timeout must be 1 through 250 ms",
            ));
        }
        let endpoint = match self {
            Self::BulkIn { endpoint, .. } | Self::InterruptIn { endpoint, .. } => {
                Some((*endpoint, true))
            }
            Self::BulkOut { endpoint, .. } | Self::InterruptOut { endpoint, .. } => {
                Some((*endpoint, false))
            }
            _ => None,
        };
        if let Some((endpoint, input)) = endpoint {
            if endpoint & 0x70 != 0
                || endpoint & 0x0f == 0
                || (endpoint & 0x80 != 0) != input
                || length == 0
            {
                return Err(IoError::InvalidParameter(
                    "invalid endpoint direction, number or length",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn session(&self) -> Option<SessionId> {
        match self {
            Self::Open { .. }
            | Self::ReadStrings { .. }
            | Self::SetString { .. }
            | Self::Hid(_) => None,
            Self::Close { session }
            | Self::ClearHalt { session, .. }
            | Self::ControlIn { session, .. }
            | Self::ControlOut { session, .. }
            | Self::BulkIn { session, .. }
            | Self::BulkOut { session, .. }
            | Self::InterruptIn { session, .. }
            | Self::InterruptOut { session, .. } => Some(*session),
        }
    }
}

/// Expected I/O failures; no caller receives a native pointer or stale handle.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum IoError {
    #[error("invalid USB parameter: {0}")]
    InvalidParameter(&'static str),
    #[error("USB device not found")]
    NotFound,
    #[error("USB device disconnected")]
    Disconnected,
    #[error("USB access denied")]
    AccessDenied,
    #[error("USB resource busy")]
    Busy,
    #[error("USB transfer timed out")]
    Timeout,
    #[error("USB endpoint stalled")]
    Stall,
    #[error("USB backend not supported")]
    NotSupported,
    #[error("invalid or expired USB session")]
    InvalidSession,
    #[error("endpoint is not part of the claimed interface/alternate or transfer type")]
    EndpointMismatch,
    #[error("USB I/O queue full; request not accepted")]
    QueueFull,
    #[error("USB I/O worker stopped; request not completed")]
    Stopped,
    #[error("USB session capacity reached")]
    SessionLimit,
    #[error("USB backend: {0}")]
    Backend(String),
    #[error("HID backend: {0}")]
    HidBackend(String),
    #[error("HID descriptor: {0}")]
    HidDescriptor(&'static str),
    #[error("system keyboard/mouse/control HID collections cannot be opened as DJ input")]
    ProtectedInput,
    #[error("HID report does not match the descriptor")]
    InvalidHidReport,
}
/// Which optional string descriptor failed; the rest can still be displayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringField {
    Languages,
    Manufacturer,
    Product,
    Serial,
}
/// Device strings returned without claiming an interface.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceStrings {
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub errors: Vec<(StringField, IoError)>,
}
/// Exactly one outcome for each accepted request. Short reads/writes are explicit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IoReply {
    Hid(crate::HidReply),
    Strings(DeviceStrings),
    Opened(SessionId),
    Closed,
    HaltCleared { session: SessionId, endpoint: u8 },
    Data(Vec<u8>),
    Written(usize),
}
/// Worker-owned transport seam for deterministic hardware-free tests.
pub trait Transport: Send + 'static {
    /// Execute one validated request on the owning worker. Never replay writes
    /// implicitly; callers need to decide whether a retry is safe.
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError>;
}

struct Pending {
    command: IoCommand,
    reply: SyncSender<Result<IoReply, IoError>>,
}
/// One-shot, nonblocking completion. Dropped tickets never block the worker.
pub struct IoTicket {
    receiver: Receiver<Result<IoReply, IoError>>,
    finished: bool,
}
impl IoTicket {
    /// Return an outcome once; None means pending or already consumed.
    pub fn poll(&mut self) -> Option<Result<IoReply, IoError>> {
        if self.finished {
            return None;
        }
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(IoError::Stopped),
        };
        self.finished = true;
        Some(result)
    }
}
/// Native transport actor; no blocking call on submit, poll or drop.
/// This is a control/UI API using channels and ticket allocations, not an
/// audio-callback API or a claim of lock-free execution.
pub struct IoWorker {
    sender: SyncSender<Pending>,
    stop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}
struct Running(Arc<AtomicBool>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl IoWorker {
    /// Construct native libusb lazily on the I/O worker, never the caller thread.
    pub fn native() -> std::io::Result<Self> {
        Self::spawn(crate::native_io::NativeTransport::new)
    }
    /// Inject a worker-local factory. Failed initialization can be retried by
    /// submitting another request; commands are never automatically replayed.
    pub fn spawn<B: Transport>(
        mut factory: impl FnMut() -> Result<B, IoError> + Send + 'static,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Pending>(IO_QUEUE_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let worker_stop = Arc::clone(&stop);
        let guard = Running(Arc::clone(&running));
        thread::Builder::new()
            .name("usb-io".into())
            .spawn(move || {
                let _running = guard;
                let mut backend: Option<B> = None;
                while !worker_stop.load(Ordering::Acquire) {
                    let Ok(pending) = receiver.recv() else {
                        break;
                    };
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = match backend.as_mut() {
                        Some(backend) => backend.execute(pending.command),
                        None => match factory() {
                            Ok(mut initialized) => {
                                let result = initialized.execute(pending.command);
                                backend = Some(initialized);
                                result
                            }
                            Err(error) => Err(error),
                        },
                    };
                    let _ = pending.reply.try_send(result);
                }
            })?;
        Ok(Self {
            sender,
            stop,
            running,
        })
    }
    /// Validate and enqueue once. QueueFull means no native action took place.
    /// No automatic retry: a timed-out write may already have reached hardware.
    pub fn submit(&self, command: IoCommand) -> Result<IoTicket, IoError> {
        command.validate()?;
        if !self.is_running() {
            return Err(IoError::Stopped);
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(Pending { command, reply })
            .map_err(|error| match error {
                TrySendError::Full(_) => IoError::QueueFull,
                TrySendError::Disconnected(_) => IoError::Stopped,
            })?;
        Ok(IoTicket {
            receiver,
            finished: false,
        })
    }
    /// Whether the owning worker is alive, including an in-flight native call.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}
impl Drop for IoWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
