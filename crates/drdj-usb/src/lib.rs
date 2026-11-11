//! USB discovery without interface claims, transfers or audio-thread I/O.
//! Native contexts are constructed and used inside the discovery worker.
mod capture;
mod hid;
mod hid_layout;
mod io;
mod native;
mod native_hid;
pub use capture::*;
mod native_io;
mod session;
pub use hid::*;
pub use hid_layout::ReportLayout;
mod worker;
pub use io::*;
pub use native::NativeDiscovery;
use std::collections::BTreeMap;
use thiserror::Error;
pub use worker::{DiscoveryWorker, RefreshError, Snapshot};

/// Unique connection identity. Equal VID/PID does not mean the same device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceId {
    pub bus: u8,
    pub address: u8,
}

/// USB endpoint metadata, read from cached configuration descriptors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointInfo {
    pub address: u8,
    pub transfer: TransferKind,
    pub max_packet_size: u16,
    pub interval: u8,
}

/// Endpoint transfer type, independent of a native handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferKind {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

/// An interface alternate setting. Discovery never claims it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceInfo {
    pub number: u8,
    pub alternate: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub endpoints: Vec<EndpointInfo>,
}

/// Discovery metadata. Strings are deliberately unqueried in this slice:
/// obtaining string descriptors requires opening a device and control I/O.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub ports: Vec<u8>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub class: u8,
    pub interfaces: Vec<InterfaceInfo>,
    pub descriptor_error: Option<DiscoveryError>,
    /// A known profile name is detection metadata, not hardware acceptance.
    pub profile: Option<String>,
}

/// Portable discovery failures. A failed scan must not pretend devices unplugged.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DiscoveryError {
    #[error("USB access denied")]
    AccessDenied,
    #[error("USB device disappeared")]
    Disconnected,
    #[error("USB discovery timed out")]
    Timeout,
    #[error("USB backend not supported")]
    NotSupported,
    #[error("USB resource busy")]
    Busy,
    #[error("malformed USB discovery data")]
    Malformed,
    #[error("USB backend error: {0}")]
    Backend(String),
}

/// Testable discovery boundary; invoked only by its owning worker.
pub trait Discovery: Send + 'static {
    /// Inspect devices without claiming interfaces or detaching OS drivers.
    fn scan(&mut self) -> Result<Vec<DeviceInfo>, DiscoveryError>;
}

/// Explicit VID/PID profiles. No undocumented controller is marked supported.
#[derive(Clone, Debug, Default)]
pub struct DeviceRegistry(BTreeMap<(u16, u16), String>);
impl DeviceRegistry {
    /// Add a detection profile. Duplicate identity and empty names are rejected.
    pub fn register(
        &mut self,
        vendor_id: u16,
        product_id: u16,
        name: String,
    ) -> Result<(), DiscoveryError> {
        if vendor_id == 0 || name.trim().is_empty() || self.0.contains_key(&(vendor_id, product_id))
        {
            return Err(DiscoveryError::Malformed);
        }
        self.0.insert((vendor_id, product_id), name);
        Ok(())
    }
    pub(crate) fn apply(&self, devices: &mut [DeviceInfo]) {
        for device in devices {
            device.profile = self.0.get(&(device.vendor_id, device.product_id)).cloned();
        }
    }
}
