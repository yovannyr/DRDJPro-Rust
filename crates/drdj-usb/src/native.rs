use crate::*;
use rusb::{Context, UsbContext};

/// libusb-backed read-only discovery. Construct this on a worker, not the UI.
pub struct NativeDiscovery(Context);
impl NativeDiscovery {
    /// Initialize libusb without opening or reconfiguring any device.
    pub fn new() -> Result<Self, DiscoveryError> {
        Context::new().map(Self).map_err(error)
    }
}
fn error(value: rusb::Error) -> DiscoveryError {
    match value {
        rusb::Error::Access => DiscoveryError::AccessDenied,
        rusb::Error::NoDevice => DiscoveryError::Disconnected,
        rusb::Error::Timeout => DiscoveryError::Timeout,
        rusb::Error::NotSupported => DiscoveryError::NotSupported,
        rusb::Error::Busy => DiscoveryError::Busy,
        other => DiscoveryError::Backend(other.to_string()),
    }
}
impl Discovery for NativeDiscovery {
    fn scan(&mut self) -> Result<Vec<DeviceInfo>, DiscoveryError> {
        let devices = self.0.devices().map_err(error)?;
        let mut result = Vec::with_capacity(devices.len());
        for device in devices.iter() {
            let descriptor = device.device_descriptor().map_err(error)?;
            let mut info = DeviceInfo {
                id: DeviceId {
                    bus: device.bus_number(),
                    address: device.address(),
                },
                ports: device.port_numbers().unwrap_or_default(),
                vendor_id: descriptor.vendor_id(),
                product_id: descriptor.product_id(),
                class: descriptor.class_code(),
                interfaces: Vec::new(),
                descriptor_error: None,
                profile: None,
            };
            match device.active_config_descriptor() {
                Ok(config) => {
                    for setting in config
                        .interfaces()
                        .flat_map(|interface| interface.descriptors())
                    {
                        info.interfaces.push(InterfaceInfo {
                            number: setting.interface_number(),
                            alternate: setting.setting_number(),
                            class: setting.class_code(),
                            subclass: setting.sub_class_code(),
                            protocol: setting.protocol_code(),
                            endpoints: setting
                                .endpoint_descriptors()
                                .map(|endpoint| EndpointInfo {
                                    address: endpoint.address(),
                                    max_packet_size: endpoint.max_packet_size(),
                                    interval: endpoint.interval(),
                                    transfer: match endpoint.transfer_type() {
                                        rusb::TransferType::Control => TransferKind::Control,
                                        rusb::TransferType::Isochronous => {
                                            TransferKind::Isochronous
                                        }
                                        rusb::TransferType::Bulk => TransferKind::Bulk,
                                        rusb::TransferType::Interrupt => TransferKind::Interrupt,
                                    },
                                })
                                .collect(),
                        });
                    }
                }
                Err(failure) => info.descriptor_error = Some(error(failure)),
            }
            result.push(info);
        }
        Ok(result)
    }
}
