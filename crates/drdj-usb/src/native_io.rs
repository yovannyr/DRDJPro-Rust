use crate::*;
use rusb::{Context, Device, DeviceHandle, Direction, UsbContext};
use std::{collections::BTreeMap, time::Duration};

const MAX_SESSIONS: usize = 16;
struct Lease {
    handle: DeviceHandle<Context>,
    target: DeviceTarget,
    interface: u8,
    endpoints: BTreeMap<u8, TransferKind>,
}
// rusb::DeviceHandle releases its claimed interfaces and closes on Drop,
// including early-return, disconnect, actor shutdown and panic unwinding.
pub(crate) struct NativeTransport {
    context: Context,
    sessions: BTreeMap<SessionId, Lease>,
}
impl NativeTransport {
    pub(crate) fn new() -> Result<Self, IoError> {
        Ok(Self {
            context: Context::new().map_err(error)?,
            sessions: BTreeMap::new(),
        })
    }
    fn device(&self, target: DeviceTarget) -> Result<Device<Context>, IoError> {
        let devices = self.context.devices().map_err(error)?;
        let device = devices
            .iter()
            .find(|device| {
                device.bus_number() == target.id.bus && device.address() == target.id.address
            })
            .ok_or(IoError::NotFound)?;
        let descriptor = device.device_descriptor().map_err(error)?;
        if descriptor.vendor_id() != target.vendor_id
            || descriptor.product_id() != target.product_id
        {
            return Err(IoError::Disconnected);
        }
        Ok(device)
    }
    fn open(
        &mut self,
        target: DeviceTarget,
        interface: u8,
        alternate: u8,
    ) -> Result<IoReply, IoError> {
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(IoError::SessionLimit);
        }
        if self
            .sessions
            .values()
            .any(|lease| lease.target == target && lease.interface == interface)
        {
            return Err(IoError::Busy);
        }
        let device = self.device(target)?;
        let config = device.active_config_descriptor().map_err(error)?;
        let setting = config
            .interfaces()
            .flat_map(|i| i.descriptors())
            .find(|setting| {
                setting.interface_number() == interface && setting.setting_number() == alternate
            })
            .ok_or(IoError::InvalidParameter(
                "interface/alternate not present in active configuration",
            ))?;
        let endpoints = setting
            .endpoint_descriptors()
            .map(|endpoint| {
                let kind = match endpoint.transfer_type() {
                    rusb::TransferType::Control => TransferKind::Control,
                    rusb::TransferType::Bulk => TransferKind::Bulk,
                    rusb::TransferType::Interrupt => TransferKind::Interrupt,
                    rusb::TransferType::Isochronous => TransferKind::Isochronous,
                };
                (endpoint.address(), kind)
            })
            .collect();
        let id = allocate_session_id()?;
        let handle = device.open().map_err(error)?;
        // Never detach a kernel driver, reset a device or change configuration.
        handle.claim_interface(interface).map_err(error)?;
        handle
            .set_alternate_setting(interface, alternate)
            .map_err(error)?;
        self.sessions.insert(
            id,
            Lease {
                handle,
                target,
                interface,
                endpoints,
            },
        );
        Ok(IoReply::Opened(id))
    }
    fn lease(&self, session: SessionId) -> Result<&Lease, IoError> {
        self.sessions.get(&session).ok_or(IoError::InvalidSession)
    }
    fn read(
        &self,
        session: SessionId,
        endpoint: u8,
        length: usize,
        timeout: Duration,
        kind: TransferKind,
    ) -> Result<IoReply, IoError> {
        let lease = self.lease(session)?;
        if lease.endpoints.get(&endpoint) != Some(&kind) {
            return Err(IoError::EndpointMismatch);
        }
        let mut buffer = vec![0; length];
        let received = match kind {
            TransferKind::Bulk => lease.handle.read_bulk(endpoint, &mut buffer, timeout),
            TransferKind::Interrupt => lease.handle.read_interrupt(endpoint, &mut buffer, timeout),
            _ => return Err(IoError::EndpointMismatch),
        }
        .map_err(error)?;
        buffer.truncate(received);
        Ok(IoReply::Data(buffer))
    }
    fn write(
        &self,
        session: SessionId,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
        kind: TransferKind,
    ) -> Result<IoReply, IoError> {
        let lease = self.lease(session)?;
        if lease.endpoints.get(&endpoint) != Some(&kind) {
            return Err(IoError::EndpointMismatch);
        }
        let count = match kind {
            TransferKind::Bulk => lease.handle.write_bulk(endpoint, data, timeout),
            TransferKind::Interrupt => lease.handle.write_interrupt(endpoint, data, timeout),
            _ => return Err(IoError::EndpointMismatch),
        }
        .map_err(error)?;
        Ok(IoReply::Written(count))
    }
    fn strings(&self, target: DeviceTarget, timeout: Duration) -> Result<IoReply, IoError> {
        let device = self.device(target)?;
        let descriptor = device.device_descriptor().map_err(error)?;
        if descriptor.manufacturer_string_index().is_none()
            && descriptor.product_string_index().is_none()
            && descriptor.serial_number_string_index().is_none()
        {
            return Ok(IoReply::Strings(DeviceStrings::default()));
        }
        let handle = device.open().map_err(error)?;
        let mut strings = DeviceStrings::default();
        let language = match handle.read_languages(timeout) {
            Ok(languages) => {
                let language = languages.first().copied();
                if language.is_none() {
                    strings.errors.push((
                        StringField::Languages,
                        IoError::Backend("device returned no string languages".into()),
                    ));
                }
                language
            }
            Err(failure) => {
                strings
                    .errors
                    .push((StringField::Languages, error(failure)));
                None
            }
        };
        if let Some(language) = language {
            let mut capture = |field, result: rusb::Result<String>| match result {
                Ok(value) => Some(value),
                Err(failure) => {
                    strings.errors.push((field, error(failure)));
                    None
                }
            };
            // Absence of a string index is not an I/O error. Avoid issuing reads
            // for those fields, and preserve successful strings on partial error.
            if descriptor.manufacturer_string_index().is_some() {
                strings.manufacturer = capture(
                    StringField::Manufacturer,
                    handle.read_manufacturer_string(language, &descriptor, timeout),
                );
            }
            if descriptor.product_string_index().is_some() {
                strings.product = capture(
                    StringField::Product,
                    handle.read_product_string(language, &descriptor, timeout),
                );
            }
            if descriptor.serial_number_string_index().is_some() {
                strings.serial = capture(
                    StringField::Serial,
                    handle.read_serial_number_string(language, &descriptor, timeout),
                );
            }
        }
        Ok(IoReply::Strings(strings))
    }
    fn control(
        &self,
        session: SessionId,
        setup: ControlSetup,
        input: bool,
    ) -> Result<(u8, &Lease), IoError> {
        let lease = self.lease(session)?;
        if setup.recipient == Recipient::Interface && setup.index as u8 != lease.interface {
            return Err(IoError::InvalidParameter(
                "control recipient is not the claimed interface",
            ));
        }
        if setup.recipient == Recipient::Endpoint
            && setup.index as u8 != 0
            && !lease.endpoints.contains_key(&(setup.index as u8))
        {
            return Err(IoError::EndpointMismatch);
        }
        let kind = match setup.kind {
            ControlKind::Standard => rusb::RequestType::Standard,
            ControlKind::Class => rusb::RequestType::Class,
            ControlKind::Vendor => rusb::RequestType::Vendor,
        };
        let recipient = match setup.recipient {
            Recipient::Device => rusb::Recipient::Device,
            Recipient::Interface => rusb::Recipient::Interface,
            Recipient::Endpoint => rusb::Recipient::Endpoint,
            Recipient::Other => rusb::Recipient::Other,
        };
        Ok((
            rusb::request_type(
                if input { Direction::In } else { Direction::Out },
                kind,
                recipient,
            ),
            lease,
        ))
    }
}
impl Transport for NativeTransport {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        command.validate()?;
        let session = command.session();
        let result = match command {
            IoCommand::Hid(_) => {
                return Err(IoError::InvalidParameter(
                    "HID request sent to USB transport",
                ))
            }
            IoCommand::ReadStrings { target, timeout } => self.strings(target, timeout),
            IoCommand::SetString {
                target,
                index,
                language,
                value,
                timeout,
            } => {
                let device = self.device(target)?;
                let handle = device.open().map_err(error)?;
                let descriptor = string_descriptor(&value);
                handle
                    .write_control(
                        0,
                        7,
                        (3 << 8) | u16::from(index),
                        language,
                        &descriptor,
                        timeout,
                    )
                    .map_err(error)
                    .map(IoReply::Written)
            }
            IoCommand::Open {
                target,
                interface,
                alternate,
            } => self.open(target, interface, alternate),
            IoCommand::Close { session } => {
                let lease = self
                    .sessions
                    .remove(&session)
                    .ok_or(IoError::InvalidSession)?;
                lease
                    .handle
                    .release_interface(lease.interface)
                    .map_err(error)
                    .map(|()| IoReply::Closed)
            }
            IoCommand::ClearHalt { session, endpoint } => {
                let lease = self.lease(session)?;
                if !matches!(
                    lease.endpoints.get(&endpoint),
                    Some(TransferKind::Bulk | TransferKind::Interrupt)
                ) {
                    return Err(IoError::EndpointMismatch);
                }
                lease
                    .handle
                    .clear_halt(endpoint)
                    .map_err(error)
                    .map(|()| IoReply::HaltCleared { session, endpoint })
            }
            IoCommand::ControlIn {
                session,
                setup,
                length,
                timeout,
            } => {
                let (request_type, lease) = self.control(session, setup, true)?;
                let mut data = vec![0; length];
                lease
                    .handle
                    .read_control(
                        request_type,
                        setup.request,
                        setup.value,
                        setup.index,
                        &mut data,
                        timeout,
                    )
                    .map_err(error)
                    .map(|size| {
                        data.truncate(size);
                        IoReply::Data(data)
                    })
            }
            IoCommand::ControlOut {
                session,
                setup,
                data,
                timeout,
            } => {
                let (request_type, lease) = self.control(session, setup, false)?;
                lease
                    .handle
                    .write_control(
                        request_type,
                        setup.request,
                        setup.value,
                        setup.index,
                        &data,
                        timeout,
                    )
                    .map_err(error)
                    .map(IoReply::Written)
            }
            IoCommand::BulkIn {
                session,
                endpoint,
                length,
                timeout,
            } => self.read(session, endpoint, length, timeout, TransferKind::Bulk),
            IoCommand::BulkOut {
                session,
                endpoint,
                data,
                timeout,
            } => self.write(session, endpoint, &data, timeout, TransferKind::Bulk),
            IoCommand::InterruptIn {
                session,
                endpoint,
                length,
                timeout,
            } => self.read(session, endpoint, length, timeout, TransferKind::Interrupt),
            IoCommand::InterruptOut {
                session,
                endpoint,
                data,
                timeout,
            } => self.write(session, endpoint, &data, timeout, TransferKind::Interrupt),
        };
        finish(&mut self.sessions, session, result)
    }
}

// IDs are process-local and never persisted. Recreating a native worker cannot
// make an old or another worker's ID address a different active lease.
fn allocate_session_id() -> Result<SessionId, IoError> {
    crate::session::next_id()
        .map(SessionId)
        .ok_or(IoError::SessionLimit)
}

fn string_descriptor(value: &str) -> Vec<u8> {
    let mut descriptor = Vec::with_capacity(2 + value.len() * 2);
    descriptor.extend_from_slice(&[0, 3]);
    for unit in value.encode_utf16() {
        descriptor.extend_from_slice(&unit.to_le_bytes());
    }
    descriptor[0] = descriptor.len() as u8; // validated maximum is 254 bytes
    descriptor
}
fn error(value: rusb::Error) -> IoError {
    match value {
        rusb::Error::Access => IoError::AccessDenied,
        rusb::Error::NoDevice => IoError::Disconnected,
        rusb::Error::NotFound => IoError::NotFound,
        rusb::Error::Busy => IoError::Busy,
        rusb::Error::Timeout => IoError::Timeout,
        rusb::Error::Pipe => IoError::Stall,
        rusb::Error::NotSupported => IoError::NotSupported,
        rusb::Error::InvalidParam => {
            IoError::InvalidParameter("native USB backend rejected parameters")
        }
        other => IoError::Backend(other.to_string()),
    }
}

fn finish<T>(
    sessions: &mut BTreeMap<SessionId, T>,
    session: Option<SessionId>,
    result: Result<IoReply, IoError>,
) -> Result<IoReply, IoError> {
    if matches!(&result, Err(IoError::Disconnected)) {
        if let Some(session) = session {
            sessions.remove(&session);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct LeaseDrop(Arc<AtomicUsize>);
    impl Drop for LeaseDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn disconnection_releases_only_the_affected_session_and_transient_errors_keep_it() {
        let closed = Arc::new(AtomicUsize::new(0));
        let mut sessions = BTreeMap::from([
            (SessionId(1), LeaseDrop(Arc::clone(&closed))),
            (SessionId(2), LeaseDrop(Arc::clone(&closed))),
        ]);
        assert_eq!(
            finish(&mut sessions, Some(SessionId(1)), Err(IoError::Timeout)),
            Err(IoError::Timeout)
        );
        assert_eq!(sessions.len(), 2);
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        assert_eq!(
            finish(
                &mut sessions,
                Some(SessionId(1)),
                Err(IoError::Disconnected)
            ),
            Err(IoError::Disconnected)
        );
        assert!(!sessions.contains_key(&SessionId(1)));
        assert!(sessions.contains_key(&SessionId(2)));
        assert_eq!(closed.load(Ordering::SeqCst), 1);
        drop(sessions);
        assert_eq!(closed.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn native_errors_remain_typed_and_actionable() {
        for (native, expected) in [
            (rusb::Error::NoDevice, IoError::Disconnected),
            (rusb::Error::Access, IoError::AccessDenied),
            (rusb::Error::Busy, IoError::Busy),
            (rusb::Error::Timeout, IoError::Timeout),
            (rusb::Error::Pipe, IoError::Stall),
            (rusb::Error::NotSupported, IoError::NotSupported),
        ] {
            assert_eq!(error(native), expected);
        }
    }
    #[test]
    fn session_allocators_on_different_workers_never_alias() {
        let first = std::thread::spawn(|| {
            (0..64)
                .map(|_| allocate_session_id().unwrap())
                .collect::<Vec<_>>()
        });
        let second = std::thread::spawn(|| {
            (0..64)
                .map(|_| allocate_session_id().unwrap())
                .collect::<Vec<_>>()
        });
        let mut ids = first.join().unwrap();
        ids.extend(second.join().unwrap());
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert!(ids.iter().all(|id| id.0 != 0));
    }
    #[test]
    fn string_descriptor_uses_usb_utf16_length_and_type_with_unicode() {
        assert_eq!(string_descriptor("A"), vec![4, 3, 65, 0]);
        assert_eq!(
            string_descriptor("DJ🎵"),
            vec![10, 3, 68, 0, 74, 0, 0x3c, 0xd8, 0xb5, 0xdf]
        );
        let largest = string_descriptor(&"a".repeat(126));
        assert_eq!(largest.len(), 254);
        assert_eq!(largest[0], 254);
    }
}
