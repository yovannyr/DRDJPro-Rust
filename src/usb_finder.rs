//! USB device finder on the UI control plane; native calls stay on workers.
use crate::{usb_devices::*, AppWindow};
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

struct Finder {
    discovery: DiscoveryWorker,
    io: IoWorker,
    snapshot: Arc<Snapshot>,
    selected: Option<DeviceInfo>,
    pending: Option<(DeviceInfo, IoTicket)>,
}
impl Finder {
    fn native() -> std::io::Result<Self> {
        Ok(Self {
            discovery: DiscoveryWorker::native(DeviceRegistry::default())?,
            io: IoWorker::native()?,
            snapshot: Arc::new(Snapshot::default()),
            selected: None,
            pending: None,
        })
    }
    fn tick(&mut self, ui: &AppWindow) {
        if let Some(snapshot) = self.discovery.try_snapshot() {
            self.install(snapshot, ui);
        }
        if !self.discovery.is_running() {
            ui.set_usb_finder_status(
                "USB discovery worker stopped; reopen the finder to retry".into(),
            );
        }
        self.complete(ui);
    }
    fn complete(&mut self, ui: &AppWindow) {
        let result = self.pending.as_mut().and_then(|(_, ticket)| ticket.poll());
        if let Some(result) = result {
            let Some((device, _)) = self.pending.take() else {
                return;
            };
            ui.set_usb_inspecting(false);
            if self.selected.as_ref() == Some(&device) {
                let text = match result {
                    Ok(IoReply::Strings(strings)) => format!(
                        "{}\nManufacturer: {}\nProduct: {}\nSerial: {}{}",
                        details(&device),
                        strings.manufacturer.as_deref().unwrap_or("not available"),
                        strings.product.as_deref().unwrap_or("not available"),
                        strings.serial.as_deref().unwrap_or("not available"),
                        if strings.errors.is_empty() {
                            String::new()
                        } else {
                            format!("\nString read errors: {:?}", strings.errors)
                        },
                    ),
                    Ok(_) => "Unexpected device-string response".into(),
                    Err(error) => format!("{}\nString query failed: {error}", details(&device)),
                };
                ui.set_usb_device_details(text.into());
            }
        }
    }
    fn install(&mut self, snapshot: Arc<Snapshot>, ui: &AppWindow) {
        if snapshot.revision == self.snapshot.revision {
            return;
        }
        let changed = snapshot.devices != self.snapshot.devices;
        self.snapshot = snapshot;
        ui.set_usb_finder_status(
            match &self.snapshot.error {
                Some(error) => format!("USB scan failed: {error}; showing last known devices"),
                None => format!(
                    "{} USB devices — profiles describe detection, not hardware certification",
                    self.snapshot.devices.len()
                ),
            }
            .into(),
        );
        if changed {
            // Preserve an exact observed selection, never a row number after hotplug.
            let index = self.selected.as_ref().and_then(|selected| {
                self.snapshot
                    .devices
                    .iter()
                    .position(|device| device == selected)
            });
            ui.set_usb_device_names(ModelRc::new(VecModel::from(
                self.snapshot
                    .devices
                    .iter()
                    .map(|device| {
                        format!(
                            "{:04x}:{:04x}  bus {} address {}  — {}",
                            device.vendor_id,
                            device.product_id,
                            device.id.bus,
                            device.id.address,
                            device.profile.as_deref().unwrap_or("unmapped")
                        )
                        .into()
                    })
                    .collect::<Vec<_>>(),
            )));
            ui.set_usb_device_choice(index.map_or(-1, |index| index as i32));
            if index.is_none() {
                self.select(-1, ui);
            }
        }
    }
    fn select(&mut self, index: i32, ui: &AppWindow) {
        self.selected = usize::try_from(index)
            .ok()
            .and_then(|index| self.snapshot.devices.get(index))
            .cloned();
        self.pending = None; // late replies cannot label a new selection
        ui.set_usb_inspecting(false);
        ui.set_usb_device_details(self.selected.as_ref().map(details).unwrap_or_else(||
            "Select a device. String queries open a temporary handle without claiming an interface.".into()).into());
    }
    fn inspect(&mut self, ui: &AppWindow) {
        if self.pending.is_some() {
            return;
        }
        if self.snapshot.error.is_some() {
            ui.set_usb_device_details(
                "Refresh successfully before querying a stale device list".into(),
            );
            return;
        }
        let Some(device) = self.selected.clone() else {
            return;
        };
        match self.io.submit(IoCommand::ReadStrings {
            target: DeviceTarget::from(&device),
            timeout: Duration::from_millis(100),
        }) {
            Ok(ticket) => {
                self.pending = Some((device, ticket));
                ui.set_usb_inspecting(true);
            }
            Err(error) => {
                ui.set_usb_device_details(format!("String query not accepted: {error}").into())
            }
        }
    }
}
fn details(device: &DeviceInfo) -> String {
    format!(
        "VID {:04x} / PID {:04x} — bus {} address {}\n{} interfaces/alternates; profile: {}{}",
        device.vendor_id,
        device.product_id,
        device.id.bus,
        device.id.address,
        device.interfaces.len(),
        device
            .profile
            .as_deref()
            .unwrap_or("unmapped — no automatic initialization"),
        device
            .descriptor_error
            .as_ref()
            .map(|error| format!("\nDescriptor error: {error}"))
            .unwrap_or_default()
    )
}

/// Wire lazy discovery and explicit string reads. Keep the returned timer alive.
/// Creating the UI does not open USB devices; the finder starts on first refresh.
pub fn wire(ui: &AppWindow) -> Timer {
    let state = Rc::new(RefCell::new(None::<Finder>));
    let weak = ui.as_weak();
    let refresh = Rc::clone(&state);
    ui.on_usb_refresh(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut state = refresh.borrow_mut();
        if state
            .as_ref()
            .is_none_or(|finder| !finder.discovery.is_running())
        {
            match Finder::native() {
                Ok(finder) => {
                    ui.set_usb_finder_status("Scanning USB devices…".into());
                    *state = Some(finder);
                }
                Err(error) => {
                    ui.set_usb_finder_status(format!("Cannot start USB finder: {error}").into());
                    return;
                }
            }
        }
        if let Some(finder) = state.as_mut() {
            let _ = finder.discovery.refresh();
            finder.tick(&ui);
        }
    });
    let weak = ui.as_weak();
    let selection = Rc::clone(&state);
    ui.on_usb_selection_changed(move || {
        if let (Some(ui), Some(finder)) = (weak.upgrade(), selection.borrow_mut().as_mut()) {
            finder.select(ui.get_usb_device_choice(), &ui);
        }
    });
    let weak = ui.as_weak();
    let inspect = Rc::clone(&state);
    ui.on_usb_inspect(move || {
        if let (Some(ui), Some(finder)) = (weak.upgrade(), inspect.borrow_mut().as_mut()) {
            finder.inspect(&ui);
        }
    });
    let timer = Timer::default();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(33), move || {
        if let (Some(ui), Some(finder)) = (weak.upgrade(), state.borrow_mut().as_mut()) {
            finder.tick(&ui);
        }
    });
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;
    use std::{sync::mpsc, thread, time::Instant};

    struct Empty;
    impl Discovery for Empty {
        fn scan(&mut self) -> Result<Vec<DeviceInfo>, DiscoveryError> {
            Ok(Vec::new())
        }
    }
    struct Strings {
        called: mpsc::Sender<DeviceTarget>,
        gate: Option<mpsc::Receiver<()>>,
    }
    impl Transport for Strings {
        fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
            let IoCommand::ReadStrings { target, .. } = command else {
                panic!("unexpected USB write/claim");
            };
            self.called.send(target).unwrap();
            if let Some(gate) = self.gate.take() {
                gate.recv().unwrap();
            }
            Ok(IoReply::Strings(DeviceStrings {
                manufacturer: Some("Música".into()),
                product: Some("Controller".into()),
                serial: Some("fixture-serial".into()),
                errors: vec![],
            }))
        }
    }
    fn device(address: u8) -> DeviceInfo {
        DeviceInfo {
            id: DeviceId { bus: 1, address },
            ports: vec![address],
            vendor_id: 0x1234,
            product_id: 2,
            class: 0,
            interfaces: vec![],
            descriptor_error: None,
            profile: None,
        }
    }
    fn snapshot(revision: u64, devices: Vec<DeviceInfo>) -> Arc<Snapshot> {
        Arc::new(Snapshot {
            revision,
            devices,
            ..Snapshot::default()
        })
    }
    fn setup(
        gate: Option<mpsc::Receiver<()>>,
    ) -> (AppWindow, Finder, mpsc::Receiver<DeviceTarget>) {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (tx, rx) = mpsc::channel();
        let mut backend = Some(Strings { called: tx, gate });
        let finder = Finder {
            discovery: DiscoveryWorker::spawn(
                || Ok(Empty),
                DeviceRegistry::default(),
                Duration::from_secs(1),
            )
            .unwrap(),
            io: IoWorker::spawn(move || Ok(backend.take().unwrap())).unwrap(),
            snapshot: snapshot(0, vec![]),
            selected: None,
            pending: None,
        };
        (ui, finder, rx)
    }
    #[test]
    fn usb_finder_selection_survives_reordering_but_not_removal_or_identity_change() {
        let (ui, mut finder, calls) = setup(None);
        finder.install(snapshot(1, vec![device(2), device(3)]), &ui);
        ui.set_usb_device_choice(1);
        finder.select(1, &ui);
        finder.install(snapshot(2, vec![device(1), device(2), device(3)]), &ui);
        assert_eq!(ui.get_usb_device_choice(), 2);
        assert_eq!(ui.get_usb_device_names().row_count(), 3);
        assert!(ui.get_usb_device_details().contains("address 3"));
        let mut replacement = device(3);
        replacement.product_id = 99;
        finder.install(snapshot(3, vec![replacement]), &ui);
        assert_eq!(ui.get_usb_device_choice(), -1);
        assert!(finder.selected.is_none());
        assert!(
            calls.try_recv().is_err(),
            "discovery/selection must not issue reads"
        );
    }
    #[test]
    fn usb_finder_strings_are_explicit_and_include_unicode_and_serial() {
        let (ui, mut finder, calls) = setup(None);
        finder.install(snapshot(1, vec![device(2)]), &ui);
        finder.select(0, &ui);
        finder.inspect(&ui);
        assert_eq!(
            calls.recv_timeout(Duration::from_secs(2)).unwrap(),
            DeviceTarget::from(&device(2))
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while finder.pending.is_some() {
            finder.complete(&ui);
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        let text = ui.get_usb_device_details();
        assert!(text.contains("Música") && text.contains("fixture-serial"));
        assert!(!ui.get_usb_inspecting());
    }
    #[test]
    fn usb_finder_discards_late_string_reply_and_blocks_queries_of_stale_list() {
        let (release, gate) = mpsc::channel();
        let (ui, mut finder, calls) = setup(Some(gate));
        finder.install(snapshot(1, vec![device(2), device(3)]), &ui);
        finder.select(0, &ui);
        finder.inspect(&ui);
        calls.recv_timeout(Duration::from_secs(2)).unwrap();
        finder.select(1, &ui);
        release.send(()).unwrap();
        finder.complete(&ui);
        assert!(ui.get_usb_device_details().contains("address 3"));
        assert!(!ui.get_usb_device_details().contains("fixture-serial"));
        finder.install(
            Arc::new(Snapshot {
                revision: 2,
                devices: vec![device(2), device(3)],
                error: Some(DiscoveryError::AccessDenied),
                ..Snapshot::default()
            }),
            &ui,
        );
        finder.inspect(&ui);
        assert!(finder.pending.is_none());
        assert!(ui.get_usb_finder_status().contains("last known"));
        assert!(ui.get_usb_device_details().contains("stale"));
    }
    #[test]
    fn usb_finder_panel_actions_preserve_decks_without_starting_native_io() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        ui.set_left_position(42.0);
        ui.set_audio_enabled(true);
        ui.set_left_playing(true);
        ui.set_left_title("Keep this track".into());
        let count = Rc::new(std::cell::Cell::new(0));
        let observed = Rc::clone(&count);
        ui.on_usb_refresh(move || observed.set(observed.get() + 1));
        ui.show().unwrap();
        let button = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            "Show USB device finder",
        )
        .next()
        .unwrap();
        button.invoke_accessible_default_action();
        assert!(ui.get_usb_finder_open());
        assert_eq!(count.get(), 1);
        assert_eq!(ui.get_left_position(), 42.0);
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_title(), "Keep this track");
        for label in [
            "Show audio device settings",
            "Show headphone monitor",
            "Show sampler",
            "Show delay effects",
            "Show mixer EQ",
        ] {
            if !ui.get_usb_finder_open() {
                button.invoke_accessible_default_action();
            }
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, label)
                .next()
                .unwrap()
                .invoke_accessible_default_action();
            assert!(!ui.get_usb_finder_open(), "{label} must close USB finder");
        }
        button.invoke_accessible_default_action();
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            "Close USB device finder",
        )
        .next()
        .unwrap()
        .invoke_accessible_default_action();
        assert!(!ui.get_usb_finder_open());
        ui.hide().unwrap();
    }
}
