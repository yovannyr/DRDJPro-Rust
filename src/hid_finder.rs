//! Explicit HID collection/profile selection and control-plane dispatch.
use crate::{usb_devices::*, AppWindow};
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::RefCell,
    io::Read,
    path::PathBuf,
    rc::Rc,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

fn controls_available(ui: &AppWindow, deck: usize) -> bool {
    ui.get_audio_enabled()
        && !ui.get_loading()
        && !ui.get_audio_device_switching()
        && match deck {
            0 => ui.get_left_loaded() && !ui.get_left_busy(),
            1 => ui.get_right_loaded() && !ui.get_right_busy(),
            _ => false,
        }
}
fn pitch_bounds(ui: &AppWindow, deck: usize) -> Option<(f64, f64)> {
    let (low, high) = if deck == 0 {
        (ui.get_left_pitch_min(), ui.get_left_pitch_max())
    } else {
        (ui.get_right_pitch_min(), ui.get_right_pitch_max())
    };
    (low.is_finite() && high.is_finite() && low <= high)
        .then_some((f64::from(low), f64::from(high)))
}
#[derive(Default)]
struct Pending {
    seek: [Option<f64>; 2],
    pitch: [Option<f64>; 2],
}
impl Pending {
    fn flush(&mut self, ui: &AppWindow) {
        for deck in 0..2 {
            if !controls_available(ui, deck) {
                self.seek[deck] = None;
                self.pitch[deck] = None;
                continue;
            }
            if let Some(unit) = self.seek[deck].take() {
                if deck == 0 {
                    ui.invoke_seek_left((unit * 100.0) as f32);
                } else {
                    ui.invoke_seek_right((unit * 100.0) as f32);
                }
            }
            if let Some(percent) = self.pitch[deck].take() {
                if deck == 0 {
                    ui.invoke_pitch_left(percent as f32);
                } else {
                    ui.invoke_pitch_right(percent as f32);
                }
            }
        }
    }
    fn accept(&mut self, ui: &AppWindow, action: HidMappedAction) {
        match action {
            HidMappedAction::Toggle { deck } => {
                self.flush(ui);
                if controls_available(ui, usize::from(deck)) {
                    if deck == 0 {
                        ui.invoke_toggle_left();
                    } else {
                        ui.invoke_toggle_right();
                    }
                }
            }
            HidMappedAction::Seek { deck, unit } if deck < 2 => {
                self.seek[usize::from(deck)] = Some(unit)
            }
            HidMappedAction::Pitch { deck, unit } if deck < 2 => {
                if let Some((low, high)) = pitch_bounds(ui, usize::from(deck)) {
                    self.pitch[usize::from(deck)] = Some(low + (high - low) * unit);
                }
            }
            HidMappedAction::PitchDelta { deck, percent } if deck < 2 => {
                self.flush(ui);
                if controls_available(ui, usize::from(deck)) {
                    ui.invoke_hid_pitch_delta(i32::from(deck), percent as f32);
                }
            }
            _ => {}
        }
    }
}
struct StreamMapping {
    capture_id: u64,
    mapper: HidMapper,
    epoch: Option<u64>,
    last_sequence: Option<u64>,
    overflow: u64,
    discarded: u64,
}
impl StreamMapping {
    fn new(capture_id: u64, profile: Arc<HidProfile>) -> Result<Self, IoError> {
        Ok(Self {
            capture_id,
            mapper: HidMapper::with_profile(profile)?,
            epoch: None,
            last_sequence: None,
            overflow: 0,
            discarded: 0,
        })
    }
    fn reset(&mut self) {
        self.mapper.reset();
        self.last_sequence = None;
    }
    fn consume(&mut self, batch: CaptureBatch, ui: &AppWindow) -> Result<(), String> {
        match batch.status {
            CaptureStatus::Connected {
                epoch,
                session,
                layout,
            } => {
                if self.epoch != Some(epoch) {
                    self.mapper
                        .profile()
                        .validate_layout(&layout)
                        .map_err(|error| error.to_string())?;
                    self.reset();
                    self.epoch = Some(epoch);
                    ui.set_hid_status(format!("Connected: {}", self.mapper.profile().name).into());
                }
                if self.overflow != batch.overflowed_total
                    || self.discarded != batch.discarded_total
                {
                    self.reset();
                    ui.set_hid_status(
                        "HID report loss: controls reset; buttons await release".into(),
                    );
                }
                self.overflow = batch.overflowed_total;
                self.discarded = batch.discarded_total;
                if batch.reports.iter().any(|frame| {
                    frame.capture_id != self.capture_id
                        || frame.epoch != epoch
                        || frame.session != session
                }) {
                    return Err("Stale HID report identity; select controller again".into());
                }
                let mut pending = Pending::default();
                for frame in batch.reports {
                    if frame.observed_at.elapsed() > Duration::from_millis(100) {
                        self.reset();
                        pending = Pending::default();
                        self.last_sequence = Some(frame.sequence);
                        continue;
                    }
                    if self
                        .last_sequence
                        .is_some_and(|previous| previous.checked_add(1) != Some(frame.sequence))
                    {
                        self.reset();
                        pending = Pending::default();
                    }
                    self.last_sequence = Some(frame.sequence);
                    for action in self
                        .mapper
                        .decode(&frame.report)
                        .map_err(|error| error.to_string())?
                        .iter()
                    {
                        pending.accept(ui, action);
                    }
                }
                pending.flush(ui);
                Ok(())
            }
            CaptureStatus::Starting => Ok(()),
            CaptureStatus::Reconnecting { error, .. } => {
                self.reset();
                self.epoch = None;
                ui.set_hid_status(
                    format!("HID reconnecting: {error}; buttons await release").into(),
                );
                Ok(())
            }
            CaptureStatus::Failed { error, .. } => Err(error.to_string()),
            CaptureStatus::Stopped => Err("HID reader stopped".into()),
        }
    }
}
struct Active {
    capture: HidCapture,
    mapping: StreamMapping,
}
struct State {
    worker: Option<HidWorker>,
    enumeration: Option<HidTicket>,
    devices: Vec<HidInfo>,
    selected: Option<HidTarget>,
    profile: Option<Arc<HidProfile>>,
    profile_job: Option<mpsc::Receiver<Result<HidProfile, String>>>,
    active: Option<Active>,
    last_scan: Instant,
}
impl State {
    fn refresh(&mut self, ui: &AppWindow) {
        if self.enumeration.is_some() {
            return;
        }
        self.last_scan = Instant::now();
        if self
            .worker
            .as_ref()
            .is_none_or(|worker| !worker.is_running())
        {
            match HidWorker::native() {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    ui.set_hid_status(format!("Cannot start HID enumeration: {error}").into());
                    return;
                }
            }
        }
        if let Some(worker) = &self.worker {
            match worker.submit(HidCommand::Enumerate) {
                Ok(ticket) => {
                    self.enumeration = Some(ticket);
                    self.last_scan = Instant::now();
                }
                Err(error) => {
                    ui.set_hid_status(format!("HID enumeration not accepted: {error}").into())
                }
            }
        }
    }
    fn install(&mut self, devices: Vec<HidInfo>, ui: &AppWindow) {
        let index = self
            .selected
            .as_ref()
            .and_then(|selected| devices.iter().position(|device| &device.target == selected));
        ui.set_hid_device_names(ModelRc::new(VecModel::from(
            devices
                .iter()
                .map(|device| {
                    format!(
                        "{:04x}:{:04x} — {} (usage {:04x}:{:04x}, interface {}) — {}",
                        device.target.vendor_id,
                        device.target.product_id,
                        device.product.as_deref().unwrap_or("unnamed HID"),
                        device.target.usage_page,
                        device.target.usage,
                        device.target.interface,
                        if self
                            .profile
                            .as_ref()
                            .is_some_and(|profile| profile.matches(&device.target))
                        {
                            "profile match"
                        } else {
                            "unmapped"
                        }
                    )
                    .into()
                })
                .collect::<Vec<_>>(),
        )));
        self.devices = devices;
        ui.set_hid_device_choice(index.map_or(-1, |index| index as i32));
        if index.is_none() {
            self.selected = None;
            ui.set_hid_device_details(
                "Select a collection. Enumeration never starts input capture.".into(),
            );
        } else {
            self.details(ui);
        }
    }
    fn details(&self, ui: &AppWindow) {
        if let Some(device) = self
            .devices
            .iter()
            .find(|device| Some(&device.target) == self.selected.as_ref())
        {
            ui.set_hid_device_details(format!("Manufacturer: {}\nProduct: {}\nSerial: {}\nBus {:?}, interface {}, usage {:04x}:{:04x}{}",
                device.manufacturer.as_deref().unwrap_or("unavailable"),device.product.as_deref().unwrap_or("unavailable"),device.serial.as_deref().unwrap_or("unavailable"),device.bus,device.target.interface,device.target.usage_page,device.target.usage,
                if device.string_warning {"\nSome native strings could not be decoded"}else{""}).into());
        }
    }
    fn stop(&mut self, ui: &AppWindow) {
        self.active = None;
        ui.set_hid_active(false);
    }
    fn select(&mut self, ui: &AppWindow) {
        self.stop(ui);
        self.selected = usize::try_from(ui.get_hid_device_choice())
            .ok()
            .and_then(|index| self.devices.get(index))
            .map(|device| device.target.clone());
        self.details(ui);
        ui.set_hid_status("Selection changed; explicitly enable a matching profile".into());
    }
    fn start(&mut self, ui: &AppWindow) {
        if !ui.get_audio_enabled() || self.active.is_some() || self.profile_job.is_some() {
            return;
        }
        let result = (|| -> Result<Active, String> {
            let target = self.selected.clone().ok_or("Select a HID collection")?;
            let profile = self
                .profile
                .as_ref()
                .ok_or("Load a documented HID mapping")?;
            if !profile.matches(&target) {
                return Err("Profile does not match selected VID/PID/interface/usage".into());
            }
            let capture = HidCapture::native(target).map_err(|error| error.to_string())?;
            let mapping = StreamMapping::new(capture.id(), Arc::clone(profile))
                .map_err(|error| error.to_string())?;
            Ok(Active { capture, mapping })
        })();
        match result {
            Ok(active) => {
                self.active = Some(active);
                ui.set_hid_active(true);
                ui.set_hid_status("Opening explicitly selected controller…".into());
            }
            Err(error) => ui.set_hid_status(format!("HID enable failed: {error}").into()),
        }
    }
    fn load(&mut self, path: PathBuf, ui: &AppWindow) {
        if self.profile_job.is_some() {
            return;
        }
        self.stop(ui);
        let (sender, receiver) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("hid-profile".into())
            .spawn(move || {
                let result = (|| -> Result<HidProfile, String> {
                    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
                    let mut bytes = Vec::with_capacity(MAX_HID_PROFILE_BYTES + 1);
                    file.take((MAX_HID_PROFILE_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)
                        .map_err(|error| error.to_string())?;
                    HidProfile::parse(&bytes).map_err(|error| error.to_string())
                })();
                let _ = sender.send(result);
            }) {
            Ok(_) => {
                self.profile_job = Some(receiver);
                ui.set_hid_profile_loading(true);
                ui.set_hid_status("Loading HID mapping…".into());
            }
            Err(error) => ui.set_hid_status(format!("Cannot load HID mapping: {error}").into()),
        }
    }
    fn tick(&mut self, ui: &AppWindow) {
        if (ui.get_usb_finder_open() && ui.get_hid_page() || self.active.is_some())
            && self.last_scan.elapsed() >= Duration::from_secs(1)
        {
            self.refresh(ui);
        }
        if let Some(result) = self.enumeration.as_mut().and_then(HidTicket::poll) {
            self.enumeration = None;
            match result {
                Ok(HidReply::Devices(devices)) => self.install(devices, ui),
                Ok(_) => ui.set_hid_status("Unexpected HID enumeration response".into()),
                Err(error) => ui.set_hid_status(
                    format!("HID enumeration failed: {error}; showing last known collections")
                        .into(),
                ),
            }
        }
        let result = self
            .profile_job
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("Profile loader stopped".into())),
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            self.profile_job = None;
            ui.set_hid_profile_loading(false);
            match result {
                Ok(profile) => {
                    ui.set_hid_profile_name(profile.name.as_str().into());
                    self.profile = Some(Arc::new(profile));
                    ui.set_hid_status(
                        "Profile loaded; select a matching collection and enable it".into(),
                    );
                    self.refresh(ui);
                }
                Err(error) => ui.set_hid_status(format!("HID profile rejected: {error}").into()),
            }
        }
        if !ui.get_audio_enabled() {
            self.stop(ui);
        }
        let outcome = self.active.as_mut().and_then(|active| {
            active
                .capture
                .try_drain(32)
                .map(|batch| active.mapping.consume(batch, ui))
        });
        if let Some(Err(error)) = outcome {
            self.stop(ui);
            ui.set_hid_status(format!("HID stopped: {error}").into());
        }
    }
}
/// Wire metadata-only enumeration, bounded file parsing and explicit capture.
/// Keep the timer alive. No context or device is opened until a user action.
pub fn wire(ui: &AppWindow) -> Timer {
    let state = Rc::new(RefCell::new(State {
        worker: None,
        enumeration: None,
        devices: vec![],
        selected: None,
        profile: None,
        profile_job: None,
        active: None,
        last_scan: Instant::now(),
    }));
    let weak = ui.as_weak();
    let refresh = Rc::clone(&state);
    ui.on_hid_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            refresh.borrow_mut().refresh(&ui);
        }
    });
    let weak = ui.as_weak();
    let select = Rc::clone(&state);
    ui.on_hid_selected(move || {
        if let Some(ui) = weak.upgrade() {
            select.borrow_mut().select(&ui);
        }
    });
    let weak = ui.as_weak();
    let start = Rc::clone(&state);
    ui.on_hid_start(move || {
        if let Some(ui) = weak.upgrade() {
            start.borrow_mut().start(&ui);
        }
    });
    let weak = ui.as_weak();
    let stop = Rc::clone(&state);
    ui.on_hid_stop(move || {
        if let Some(ui) = weak.upgrade() {
            let mut state = stop.borrow_mut();
            state.stop(&ui);
            ui.set_hid_status("HID controller stopped".into());
        }
    });
    let weak = ui.as_weak();
    let load = Rc::clone(&state);
    ui.on_hid_load_profile(move || {
        // Do not hold RefCell across a modal native picker.
        if let Some(ui) = weak.upgrade() {
            load.borrow_mut().stop(&ui);
            ui.set_hid_status("HID input paused while choosing a mapping".into());
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("HID mapping", &["json"])
            .pick_file()
        {
            if let Some(ui) = weak.upgrade() {
                load.borrow_mut().load(path, &ui);
            }
        }
    });
    let timer = Timer::default();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(5), move || {
        if let Some(ui) = weak.upgrade() {
            state.borrow_mut().tick(&ui);
        }
    });
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn hid_encoder_batches_deliver_deltas_without_reseeding_from_stale_display() {
        let ui = ui();
        let deltas = Rc::new(RefCell::new(Vec::new()));
        let received = Rc::clone(&deltas);
        ui.on_hid_pitch_delta(move |deck, delta| received.borrow_mut().push((deck, delta)));
        for _ in 0..3 {
            let mut pending = Pending::default();
            pending.accept(
                &ui,
                HidMappedAction::PitchDelta {
                    deck: 0,
                    percent: 0.25,
                },
            );
            pending.flush(&ui);
        }
        assert_eq!(*deltas.borrow(), vec![(0, 0.25), (0, 0.25), (0, 0.25)]);
        assert_eq!(ui.get_left_pitch_percent(), 0.0);
    }
    fn profile() -> Arc<HidProfile> {
        Arc::new(HidProfile {
            name: "synthetic".into(),
            vendor_id: 0x1234,
            product_id: 1,
            interface: 1,
            usage_page: 0xff00,
            usage: 1,
            bindings: vec![HidBinding {
                when: None,
                field: HidField {
                    report_id: 0,
                    bit_offset: 0,
                    bit_width: 8,
                    signed: false,
                },
                action: HidBindingAction::Toggle {
                    deck: 0,
                    zero_is_pressed: false,
                },
            }],
        })
    }
    fn batch(epoch: u64, packets: &[(u64, u8)], overflow: u64) -> CaptureBatch {
        CaptureBatch {
            status: CaptureStatus::Connected {
                epoch,
                session: HidSessionId(1),
                layout: Arc::new(
                    ReportLayout::parse(&[
                        0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 1, 0x81, 2, 0xc0,
                    ])
                    .unwrap(),
                ),
            },
            reports: packets
                .iter()
                .map(|(sequence, value)| CapturedReport {
                    capture_id: 42,
                    epoch,
                    session: HidSessionId(1),
                    sequence: *sequence,
                    observed_at: Instant::now(),
                    report: HidReport {
                        report_id: 0,
                        data: vec![*value],
                        logical_length: 1,
                    },
                })
                .collect(),
            overflowed_total: overflow,
            discarded_total: 0,
        }
    }
    fn ui() -> AppWindow {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        ui.set_audio_enabled(true);
        ui.set_left_loaded(true);
        ui.set_right_loaded(true);
        ui
    }
    #[test]
    fn hid_mapping_dispatches_to_real_ui_callbacks_and_resets_after_loss_or_epoch() {
        let ui = ui();
        let calls = Rc::new(Cell::new(0));
        let count = Rc::clone(&calls);
        ui.on_toggle_left(move || count.set(count.get() + 1));
        let mut mapping = StreamMapping::new(42, profile()).unwrap();
        mapping
            .consume(batch(1, &[(1, 1), (2, 0), (3, 1)], 0), &ui)
            .unwrap();
        assert_eq!(calls.get(), 1);
        mapping.consume(batch(1, &[(5, 1)], 1), &ui).unwrap();
        assert_eq!(calls.get(), 1, "held state after overflow cannot autoplay");
        mapping.consume(batch(2, &[(6, 1)], 1), &ui).unwrap();
        assert_eq!(calls.get(), 1, "reconnect requires release");
        mapping
            .consume(batch(2, &[(7, 0), (8, 1)], 1), &ui)
            .unwrap();
        assert_eq!(calls.get(), 2);
    }
    #[test]
    fn hid_mapping_refuses_foreign_and_expired_reports_and_busy_transport() {
        let ui = ui();
        let calls = Rc::new(Cell::new(0));
        let count = Rc::clone(&calls);
        ui.on_toggle_left(move || count.set(count.get() + 1));
        let mut mapping = StreamMapping::new(42, profile()).unwrap();
        let mut foreign = batch(1, &[(1, 0), (2, 1)], 0);
        foreign.reports[1].capture_id = 99;
        assert!(mapping.consume(foreign, &ui).is_err());
        assert_eq!(calls.get(), 0);
        let mut expired = batch(1, &[(1, 0), (2, 1)], 0);
        for frame in &mut expired.reports {
            frame.observed_at = Instant::now() - Duration::from_secs(1);
        }
        mapping.consume(expired, &ui).unwrap();
        assert_eq!(calls.get(), 0);
        ui.set_loading(true);
        mapping
            .consume(batch(1, &[(3, 0), (4, 1)], 0), &ui)
            .unwrap();
        assert_eq!(calls.get(), 0);
        ui.set_loading(false);
        mapping.consume(batch(1, &[(5, 1)], 0), &ui).unwrap();
        assert_eq!(calls.get(), 0, "busy press must not replay on availability");
        mapping
            .consume(batch(1, &[(6, 0), (7, 1)], 0), &ui)
            .unwrap();
        assert_eq!(calls.get(), 1);
    }
    #[test]
    fn hid_axes_are_coalesced_clamped_and_apply_before_transport() {
        let ui = ui();
        ui.set_left_pitch_min(-8.0);
        ui.set_left_pitch_max(8.0);
        ui.set_left_pitch_percent(0.0);
        let seen = Rc::new(RefCell::new(Vec::<String>::new()));
        let events = Rc::clone(&seen);
        ui.on_pitch_left(move |value| events.borrow_mut().push(format!("pitch {value}")));
        let events = Rc::clone(&seen);
        ui.on_toggle_left(move || events.borrow_mut().push("play".into()));
        let mut pending = Pending::default();
        for _ in 0..100 {
            pending.accept(&ui, HidMappedAction::Pitch { deck: 0, unit: 1.0 });
        }
        pending.accept(&ui, HidMappedAction::Toggle { deck: 0 });
        assert_eq!(*seen.borrow(), vec!["pitch 8", "play"]);
        ui.set_right_busy(true);
        pending.accept(&ui, HidMappedAction::Seek { deck: 1, unit: 0.5 });
        pending.flush(&ui);
        assert_eq!(seen.borrow().len(), 2);
    }
    #[test]
    fn hid_setup_switches_pages_and_preserves_audio_state_without_native_enumeration() {
        let ui = ui();
        ui.set_usb_finder_open(true);
        ui.set_dvs_available(true);
        ui.set_dvs_page(true);
        ui.set_left_position(42.0);
        ui.set_left_playing(true);
        let scans = Rc::new(Cell::new(0));
        let count = Rc::clone(&scans);
        ui.on_hid_refresh(move || count.set(count.get() + 1));
        ui.show().unwrap();
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            "Switch USB HID setup page",
        )
        .next()
        .unwrap()
        .invoke_accessible_default_action();
        assert!(ui.get_hid_page());
        assert!(!ui.get_dvs_page());
        assert_eq!(scans.get(), 1);
        assert_eq!(ui.get_left_position(), 42.0);
        assert!(ui.get_left_playing());
        ui.hide().unwrap();
    }
}
