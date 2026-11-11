use crate::*;
use std::{collections::BTreeMap, ffi::CString};

struct Lease {
    handle: hidapi::HidDevice,
    target: HidTarget,
    layout: ReportLayout,
    input: Vec<u8>,
}
pub(crate) struct NativeHid {
    // Handles must close before their context. All fields drop on the worker.
    sessions: BTreeMap<HidSessionId, Lease>,
    api: hidapi::HidApi,
}
fn error(value: hidapi::HidError) -> IoError {
    IoError::HidBackend(value.to_string())
}
fn target(info: &hidapi::DeviceInfo) -> HidTarget {
    HidTarget {
        path: info.path().to_bytes().to_vec(),
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        interface: info.interface_number(),
        usage_page: info.usage_page(),
        usage: info.usage(),
    }
}
fn matches(info: &hidapi::DeviceInfo, expected: &HidTarget) -> bool {
    info.path().to_bytes() == expected.path
        && info.vendor_id() == expected.vendor_id
        && info.product_id() == expected.product_id
        && info.interface_number() == expected.interface
        && info.usage_page() == expected.usage_page
        && info.usage() == expected.usage
}
fn metadata(info: &hidapi::DeviceInfo) -> HidInfo {
    HidInfo {
        target: target(info),
        bus: match info.bus_type() {
            hidapi::BusType::Usb => HidBus::Usb,
            hidapi::BusType::Bluetooth => HidBus::Bluetooth,
            hidapi::BusType::I2c => HidBus::I2c,
            hidapi::BusType::Spi => HidBus::Spi,
            _ => HidBus::Unknown,
        },
        manufacturer: info.manufacturer_string().map(str::to_owned),
        product: info.product_string().map(str::to_owned),
        serial: info.serial_number().map(str::to_owned),
        string_warning: (info.manufacturer_string().is_none()
            && info.manufacturer_string_raw().is_some())
            || (info.product_string().is_none() && info.product_string_raw().is_some())
            || (info.serial_number().is_none() && info.serial_number_raw().is_some()),
    }
}
impl NativeHid {
    pub(crate) fn new() -> Result<Self, IoError> {
        Ok(Self {
            sessions: BTreeMap::new(),
            api: hidapi::HidApi::new().map_err(error)?,
        })
    }
    fn open(&mut self, selected: HidTarget) -> Result<HidReply, IoError> {
        selected.validate()?;
        if self.sessions.len() >= 16 {
            return Err(IoError::SessionLimit);
        }
        if self.sessions.values().any(|lease| lease.target == selected) {
            return Err(IoError::Busy);
        }
        self.api.refresh_devices().map_err(error)?;
        if !self.api.device_list().any(|info| matches(info, &selected)) {
            return Err(IoError::NotFound);
        }
        let path = CString::new(selected.path.as_slice())
            .map_err(|_| IoError::InvalidParameter("HID path contains NUL"))?;
        let handle = self.api.open_path(&path).map_err(error)?;
        if !matches(&handle.get_device_info().map_err(error)?, &selected) {
            return Err(IoError::Disconnected);
        }
        // One extra byte detects a descriptor larger than the supported limit.
        let mut descriptor = vec![0; hidapi::MAX_REPORT_DESCRIPTOR_SIZE + 1];
        let length = handle
            .get_report_descriptor(&mut descriptor)
            .map_err(error)?;
        if length > hidapi::MAX_REPORT_DESCRIPTOR_SIZE {
            return Err(IoError::HidDescriptor("descriptor exceeds supported limit"));
        }
        descriptor.truncate(length);
        let layout = ReportLayout::parse(&descriptor)?;
        if layout.system_input {
            return Err(IoError::ProtectedInput);
        }
        let input = vec![0; layout.input_capacity().map_or(0, |capacity| capacity + 1)];
        let session = crate::session::next_id()
            .map(HidSessionId)
            .ok_or(IoError::SessionLimit)?;
        self.sessions.insert(
            session,
            Lease {
                handle,
                target: selected,
                layout: layout.clone(),
                input,
            },
        );
        Ok(HidReply::Opened { session, layout })
    }
    fn execute_hid(&mut self, command: HidCommand) -> Result<HidReply, IoError> {
        match command {
            HidCommand::Enumerate => {
                self.api.refresh_devices().map_err(error)?;
                Ok(HidReply::Devices(
                    self.api.device_list().map(metadata).collect(),
                ))
            }
            HidCommand::Open { target } => self.open(target),
            HidCommand::Close { session } => {
                self.sessions
                    .remove(&session)
                    .ok_or(IoError::InvalidSession)?;
                Ok(HidReply::Closed)
            }
            HidCommand::ReadInput { session } => {
                let lease = self
                    .sessions
                    .get_mut(&session)
                    .ok_or(IoError::InvalidSession)?;
                if lease.input.is_empty() {
                    return Err(IoError::NotSupported);
                }
                let received = lease
                    .handle
                    .read_timeout(&mut lease.input, 0)
                    .map_err(error)?;
                input_completion(&lease.layout, &lease.input, received)
            }
            HidCommand::WriteOutput { session, report } => {
                let lease = self.sessions.get(&session).ok_or(IoError::InvalidSession)?;
                if !lease.layout.output_valid(&report, false) {
                    return Err(IoError::InvalidParameter(
                        "HID output does not match descriptor",
                    ));
                }
                // Windows requires the maximum API report size, even for a shorter ID.
                let report = padded_report(report, lease.layout.output.values().copied().max());
                lease
                    .handle
                    .write(&report)
                    .map_err(error)
                    .map(HidReply::Written)
            }
            HidCommand::SendFeature { session, report } => {
                let lease = self.sessions.get(&session).ok_or(IoError::InvalidSession)?;
                if !lease.layout.output_valid(&report, true) {
                    return Err(IoError::InvalidParameter(
                        "HID feature does not match descriptor",
                    ));
                }
                let report = padded_report(report, lease.layout.feature.values().copied().max());
                lease.handle.send_feature_report(&report).map_err(error)?;
                Ok(HidReply::FeatureSent)
            }
            HidCommand::GetFeature { session, report_id } => {
                let lease = self.sessions.get(&session).ok_or(IoError::InvalidSession)?;
                if !lease.layout.feature.contains_key(&report_id) {
                    return Err(IoError::InvalidParameter("unknown HID feature ID"));
                }
                let length = lease
                    .layout
                    .feature
                    .values()
                    .max()
                    .map(|max| max + 1)
                    .unwrap_or_default();
                let mut data = vec![0; length + 1];
                data[0] = report_id;
                let received = lease.handle.get_feature_report(&mut data).map_err(error)?;
                feature_completion(&lease.layout, report_id, data, received).map(HidReply::Feature)
            }
        }
    }
}

#[cfg(windows)]
fn padded_report(mut report: Vec<u8>, maximum: Option<usize>) -> Vec<u8> {
    if let Some(maximum) = maximum {
        report.resize(report.len().max(maximum + 1), 0);
    }
    report
}
#[cfg(not(windows))]
fn padded_report(report: Vec<u8>, _maximum: Option<usize>) -> Vec<u8> {
    report
}
impl Transport for NativeHid {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        let IoCommand::Hid(command) = command else {
            return Err(IoError::InvalidParameter(
                "USB request sent to HID transport",
            ));
        };
        command.validate()?;
        let session = command.session();
        let result = self.execute_hid(command);
        finish_session(&mut self.sessions, session, result).map(IoReply::Hid)
    }
}

fn input_completion(
    layout: &ReportLayout,
    buffer: &[u8],
    received: usize,
) -> Result<HidReply, IoError> {
    if received == 0 {
        return Ok(HidReply::Input(None));
    }
    if received > buffer.len() || received > layout.input_capacity().unwrap_or_default() {
        return Err(IoError::InvalidHidReport);
    }
    let (report_id, logical_length) = layout.input_identity(&buffer[..received])?;
    Ok(HidReply::Input(Some(HidReport {
        report_id,
        data: buffer[..received].to_vec(),
        logical_length,
    })))
}
fn feature_completion(
    layout: &ReportLayout,
    report_id: u8,
    mut data: Vec<u8>,
    received: usize,
) -> Result<Vec<u8>, IoError> {
    let logical = layout
        .feature
        .get(&report_id)
        .ok_or(IoError::InvalidParameter("unknown HID feature ID"))?
        + 1;
    let capacity = layout
        .feature
        .values()
        .max()
        .map(|max| max + 1)
        .unwrap_or_default();
    if received < logical
        || received > capacity
        || received > data.len()
        || data.first() != Some(&report_id)
    {
        return Err(IoError::InvalidHidReport);
    }
    data.truncate(received);
    Ok(data)
}
fn finish_session<T>(
    sessions: &mut BTreeMap<HidSessionId, T>,
    session: Option<HidSessionId>,
    result: Result<HidReply, IoError>,
) -> Result<HidReply, IoError> {
    if matches!(
        &result,
        Err(IoError::HidBackend(_) | IoError::InvalidHidReport | IoError::Disconnected)
    ) {
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
    fn layout() -> ReportLayout {
        ReportLayout::parse(&[
            0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 3, 0x81, 2, 0x95, 2, 0xb1, 2, 0xc0,
        ])
        .unwrap()
    }
    #[test]
    fn input_sentinel_detects_api_truncation_instead_of_accepting_a_valid_prefix() {
        let layout = layout();
        assert_eq!(
            input_completion(&layout, &[1, 2, 3, 99], 0).unwrap(),
            HidReply::Input(None)
        );
        assert!(
            matches!(input_completion(&layout, &[1,2,3,99], 3).unwrap(), HidReply::Input(Some(HidReport { data, .. })) if data == vec![1,2,3])
        );
        assert_eq!(
            input_completion(&layout, &[1, 2, 3, 99], 4),
            Err(IoError::InvalidHidReport)
        );
        assert_eq!(
            input_completion(&layout, &[1, 2, 3, 99], 5),
            Err(IoError::InvalidHidReport)
        );
        assert_eq!(
            input_completion(&layout, &[1, 2, 3, 99], 2),
            Err(IoError::InvalidHidReport)
        );
    }
    #[test]
    fn feature_sentinel_rejects_oversized_short_and_wrong_id_completions() {
        let layout = layout();
        assert_eq!(
            feature_completion(&layout, 0, vec![0, 1, 2, 99], 3).unwrap(),
            vec![0, 1, 2]
        );
        assert_eq!(
            feature_completion(&layout, 0, vec![0, 1, 2, 99], 4),
            Err(IoError::InvalidHidReport)
        );
        assert_eq!(
            feature_completion(&layout, 0, vec![0, 1, 2, 99], 2),
            Err(IoError::InvalidHidReport)
        );
        assert_eq!(
            feature_completion(&layout, 0, vec![1, 1, 2, 99], 3),
            Err(IoError::InvalidHidReport)
        );
    }
    struct Dropped(Arc<AtomicUsize>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn oversized_completions_invalidate_only_the_affected_hid_lease() {
        for result in [
            input_completion(&layout(), &[1, 2, 3, 99], 4),
            feature_completion(&layout(), 0, vec![0, 1, 2, 99], 4).map(HidReply::Feature),
        ] {
            let dropped = Arc::new(AtomicUsize::new(0));
            let mut sessions = BTreeMap::from([
                (HidSessionId(1), Dropped(Arc::clone(&dropped))),
                (HidSessionId(2), Dropped(Arc::clone(&dropped))),
            ]);
            assert_eq!(
                finish_session(&mut sessions, Some(HidSessionId(1)), result),
                Err(IoError::InvalidHidReport)
            );
            assert!(!sessions.contains_key(&HidSessionId(1)));
            assert!(sessions.contains_key(&HidSessionId(2)));
            assert_eq!(dropped.load(Ordering::SeqCst), 1);
        }
    }
}
