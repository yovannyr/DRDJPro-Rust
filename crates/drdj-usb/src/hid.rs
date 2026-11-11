//! Explicit HID access; enumeration never starts input capture.
use crate::*;

/// Maximum descriptor-derived API frame. Oversized layouts are unsupported.
pub const MAX_HID_REPORT_BYTES: usize = 4096;
/// Process-local HID lease identity, distinct from a USB interface lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HidSessionId(pub u64);
/// OS-native path and collection identity; this crate does not log target paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HidTarget {
    pub path: Vec<u8>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface: i32,
    pub usage_page: u16,
    pub usage: u16,
}
impl HidTarget {
    pub(crate) fn validate(&self) -> Result<(), IoError> {
        if self.path.is_empty()
            || self.path.len() > 4096
            || self.path.contains(&0)
            || self.vendor_id == 0
        {
            return Err(IoError::InvalidParameter("invalid HID target"));
        }
        if crate::hid_layout::system_usage(u32::from(self.usage_page), u32::from(self.usage)) {
            return Err(IoError::ProtectedInput);
        }
        Ok(())
    }
}
/// Physical HID bus as reported by the OS, without pretending all HID is USB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidBus {
    Usb,
    Bluetooth,
    I2c,
    Spi,
    Unknown,
}
/// Read-only metadata. Some malformed OS strings may be unavailable as Unicode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HidInfo {
    pub target: HidTarget,
    pub bus: HidBus,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub string_warning: bool,
}
/// Requests require explicit selection; no device is opened while enumerating.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HidCommand {
    Enumerate,
    Open {
        target: HidTarget,
    },
    Close {
        session: HidSessionId,
    },
    ReadInput {
        session: HidSessionId,
    },
    /// Bounded event-driven wait for a dedicated reader worker.
    ReadInputWait {
        session: HidSessionId,
        timeout: std::time::Duration,
    },
    WriteOutput {
        session: HidSessionId,
        report: Vec<u8>,
    },
    GetFeature {
        session: HidSessionId,
        report_id: u8,
    },
    SendFeature {
        session: HidSessionId,
        report: Vec<u8>,
    },
}
impl HidCommand {
    /// Check target, system-input protection and bounded frames before enqueue.
    pub fn validate(&self) -> Result<(), IoError> {
        match self {
            Self::Open { target } => target.validate(),
            Self::ReadInputWait { timeout, .. }
                if !(std::time::Duration::from_millis(1)..=std::time::Duration::from_millis(8))
                    .contains(timeout) =>
            {
                Err(IoError::InvalidParameter(
                    "HID input wait must be 1 through 8 ms",
                ))
            }
            Self::WriteOutput { report, .. } | Self::SendFeature { report, .. }
                if report.is_empty() || report.len() > MAX_HID_REPORT_BYTES =>
            {
                Err(IoError::InvalidParameter(
                    "HID report must contain an ID and fit the frame limit",
                ))
            }
            _ => Ok(()),
        }
    }
    pub(crate) fn session(&self) -> Option<HidSessionId> {
        match self {
            Self::Enumerate | Self::Open { .. } => None,
            Self::Close { session }
            | Self::ReadInput { session }
            | Self::ReadInputWait { session, .. }
            | Self::WriteOutput { session, .. }
            | Self::GetFeature { session, .. }
            | Self::SendFeature { session, .. } => Some(*session),
        }
    }
}
/// Raw API bytes plus descriptor-defined logical length; OS padding is retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HidReport {
    pub report_id: u8,
    pub data: Vec<u8>,
    pub logical_length: usize,
}
/// Typed HID completion. Native output counts may include OS padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HidReply {
    Devices(Vec<HidInfo>),
    Opened {
        session: HidSessionId,
        layout: ReportLayout,
    },
    Closed,
    Input(Option<HidReport>),
    Written(usize),
    Feature(Vec<u8>),
    FeatureSent,
}
/// One-shot nonblocking HID result, using the reviewed bounded actor core.
pub struct HidTicket(IoTicket);
impl HidTicket {
    /// Return one outcome without waiting for native I/O.
    pub fn poll(&mut self) -> Option<Result<HidReply, IoError>> {
        self.0.poll().map(|result| {
            result.and_then(|reply| match reply {
                IoReply::Hid(reply) => Ok(reply),
                _ => Err(IoError::Backend("unexpected HID completion".into())),
            })
        })
    }
}
/// HID owns a separate worker, so bulk USB transfers cannot block HID reads.
/// Submit/poll belong to the control plane and never run in an audio callback.
pub struct HidWorker(IoWorker);
impl HidWorker {
    /// Initialize HIDAPI lazily on its worker; this captures no input.
    pub fn native() -> std::io::Result<Self> {
        Self::spawn(crate::native_hid::NativeHid::new)
    }
    /// Inject a backend supporting the HID command namespace for tests.
    pub fn spawn<B: Transport>(
        factory: impl FnMut() -> Result<B, IoError> + Send + 'static,
    ) -> std::io::Result<Self> {
        IoWorker::spawn(factory).map(Self)
    }
    /// Submit explicit HID I/O without waiting for a handle or device reply.
    pub fn submit(&self, command: HidCommand) -> Result<HidTicket, IoError> {
        self.0.submit(IoCommand::Hid(command)).map(HidTicket)
    }
    /// Whether the owning native worker is alive.
    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }
}
