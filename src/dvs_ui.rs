//! Explicit DVS arming and input ownership; no native input calls on UI/audio.
use crate::{
    controller::{AudioController, DvsLease},
    playback::DeckId,
    AppWindow, DvsLicenseWindow,
};
use drdj_dvs::{InputCapture, InputDevice, InputInventory, InputSnapshot, InputState, Record};
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Gate {
    armed: Instant,
    epoch: Option<u64>,
    live: bool,
    last_seen: Instant,
}
enum Decision {
    Wait,
    Apply(f64, f64),
    Disarm(&'static str),
}
impl Gate {
    fn new() -> Self {
        Self {
            armed: Instant::now(),
            epoch: None,
            live: true,
            last_seen: Instant::now(),
        }
    }
    fn decide(&mut self, state: &InputSnapshot, now: Instant, offset: f64) -> Decision {
        if !self.live {
            return Decision::Wait;
        }
        self.last_seen = now;
        let reason = if (self.epoch.is_some() || matches!(state.state, InputState::Tracking(_)))
            && now.saturating_duration_since(state.observed_at) > Duration::from_millis(50)
        {
            Some("input observation expired")
        } else if self.epoch.is_none()
            && now.saturating_duration_since(self.armed) > Duration::from_secs(3)
        {
            Some("no valid timecode within 3 seconds")
        } else if self.epoch.is_some_and(|epoch| epoch != state.epoch) {
            Some("input discontinuity")
        } else {
            match state.state {
                InputState::Failed(_) | InputState::Stopped => Some("input stopped or failed"),
                InputState::SignalLost if self.epoch.is_some() => Some("timecode signal lost"),
                InputState::Starting | InputState::SignalLost
                    if now.saturating_duration_since(self.armed) > Duration::from_secs(3) =>
                {
                    Some("no valid timecode within 3 seconds")
                }
                _ => None,
            }
        };
        if let Some(reason) = reason {
            self.live = false;
            return Decision::Disarm(reason);
        }
        if let InputState::Tracking(observation) = state.state {
            let seconds = observation.position_seconds
                + offset
                + now
                    .saturating_duration_since(state.observed_at)
                    .as_secs_f64()
                    * observation.speed;
            if !seconds.is_finite() || !observation.speed.is_finite() {
                self.live = false;
                return Decision::Disarm("invalid timecode");
            }
            self.epoch = Some(state.epoch);
            return Decision::Apply(seconds, observation.speed);
        }
        Decision::Wait
    }
}
struct Session {
    capture: InputCapture,
    lease: Option<DvsLease>,
    gate: Gate,
    offset: f64,
    device: InputDevice,
    tracking_status: bool,
}
struct State {
    controller: Rc<AudioController>,
    devices: Vec<InputDevice>,
    scan: Option<mpsc::Receiver<Result<InputInventory, String>>>,
    sessions: [Option<Session>; 2],
}
fn deck(index: usize) -> DeckId {
    if index == 0 {
        DeckId::A
    } else {
        DeckId::B
    }
}
fn status(ui: &AppWindow, index: usize, text: String) {
    if index == 0 {
        ui.set_dvs_a_status(text.into());
    } else {
        ui.set_dvs_b_status(text.into());
    }
}
impl State {
    fn refresh(&mut self, ui: &AppWindow) {
        if self.scan.is_some() {
            return;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("dvs-input-list".into())
            .spawn(move || {
                let _ = tx.send(drdj_dvs::enumerate_inputs());
            }) {
            Ok(_) => {
                self.scan = Some(rx);
                ui.set_dvs_status("Reading input metadata…".into());
            }
            Err(error) => ui.set_dvs_status(format!("Input scan could not start: {error}").into()),
        }
    }
    fn stop(&mut self, index: usize, ui: &AppWindow) {
        if let Some(mut session) = self.sessions[index].take() {
            if let Some(lease) = session.lease.take() {
                self.controller.dvs_release(lease);
            }
            drop(session);
        }
        status(
            ui,
            index,
            format!("{}: stopped", if index == 0 { "A" } else { "B" }),
        );
    }
    fn arm(&mut self, index: usize, ui: &AppWindow) {
        let selected = usize::try_from(ui.get_dvs_input_choice())
            .ok()
            .and_then(|index| self.devices.get(index))
            .cloned();
        let Some(device) = selected else {
            ui.set_dvs_status("Refresh and select an input first".into());
            return;
        };
        if self.sessions[1 - index]
            .as_ref()
            .is_some_and(|session| session.device.name == device.name)
        {
            ui.set_dvs_status("This input is already assigned to the other deck".into());
            return;
        }
        let record = match ui.get_dvs_record_choice() {
            0 => Record::SeratoSecondEditionA,
            1 => Record::SeratoSecondEditionB,
            2 => Record::SeratoCd,
            _ => return,
        };
        self.stop(index, ui);
        let result = (|| -> Result<Session, String> {
            let lease = self.controller.dvs_arm(deck(index))?;
            match InputCapture::native(device.clone(), record, ui.get_dvs_phono()) {
                Ok(capture) => Ok(Session {
                    capture,
                    lease: Some(lease),
                    gate: Gate::new(),
                    offset: f64::from(ui.get_dvs_offset_ms()) / 1000.0,
                    device,
                    tracking_status: false,
                }),
                Err(error) => {
                    self.controller.dvs_release(lease);
                    Err(error.to_string())
                }
            }
        })();
        match result {
            Ok(session) => {
                self.sessions[index] = Some(session);
                status(
                    ui,
                    index,
                    format!(
                        "{}: armed — waiting for timecode",
                        if index == 0 { "A" } else { "B" }
                    ),
                );
            }
            Err(error) => status(ui, index, format!("DVS arm failed: {error}")),
        }
    }
    fn tick(&mut self, ui: &AppWindow) {
        if let Some(scan) = self.scan.as_ref() {
            let result = match scan.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Input enumeration worker stopped".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.scan = None;
                match result {
                    Ok(inventory) => {
                        let selected = usize::try_from(ui.get_dvs_input_choice())
                            .ok()
                            .and_then(|index| self.devices.get(index))
                            .cloned();
                        self.devices = inventory.devices;
                        let index = selected
                            .as_ref()
                            .and_then(|old| self.devices.iter().position(|device| device == old));
                        ui.set_dvs_input_names(ModelRc::new(VecModel::from(
                            self.devices
                                .iter()
                                .map(|device| {
                                    format!("{} — {} Hz, channels 1/2", device.name, device.rate)
                                        .into()
                                })
                                .collect::<Vec<_>>(),
                        )));
                        ui.set_dvs_input_choice(index.map_or(-1, |index| index as i32));
                        ui.set_dvs_status(
                            format!(
                                "{} inputs. {}",
                                self.devices.len(),
                                inventory.errors.join("; ")
                            )
                            .into(),
                        );
                    }
                    Err(error) => {
                        self.devices.clear();
                        ui.set_dvs_input_choice(-1);
                        ui.set_dvs_input_names(ModelRc::default());
                        ui.set_dvs_status(format!("Input scan failed: {error}").into());
                    }
                }
            }
        }
        for (index, slot) in self.sessions.iter_mut().enumerate() {
            let Some(session) = slot else {
                continue;
            };
            let Some(lease) = session.lease.as_mut() else {
                continue;
            };
            let decision = match session.capture.try_snapshot() {
                Some(state) => {
                    if let InputState::Failed(error) = &state.state {
                        ui.set_dvs_status(error.into());
                    }
                    session.gate.decide(&state, Instant::now(), session.offset)
                }
                None if (session.gate.epoch.is_some()
                    && session.gate.last_seen.elapsed() > Duration::from_millis(50))
                    || (session.gate.epoch.is_none()
                        && session.gate.armed.elapsed() > Duration::from_secs(3)) =>
                {
                    Decision::Disarm("input snapshot unavailable")
                }
                None => Decision::Wait,
            };
            let error = match decision {
                Decision::Wait => None,
                Decision::Disarm(reason) => Some(reason.to_string()),
                Decision::Apply(seconds, speed) => {
                    self.controller.dvs_apply(lease, seconds, speed).err()
                }
            };
            if let Some(error) = error {
                session.gate.live = false;
                if let Some(lease) = session.lease.take() {
                    self.controller.dvs_release(lease);
                }
                status(
                    ui,
                    index,
                    format!(
                        "{}: {error}; ARM required",
                        if index == 0 { "A" } else { "B" }
                    ),
                );
                *slot = None;
            } else if session.gate.epoch.is_some() && !session.tracking_status {
                session.tracking_status = true;
                // A static status avoids formatting at the 5ms control cadence.
                status(
                    ui,
                    index,
                    format!("{}: DVS tracking", if index == 0 { "A" } else { "B" }),
                );
            }
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for session in self.sessions.iter_mut().flatten() {
            if let Some(lease) = session.lease.take() {
                self.controller.dvs_release(lease);
            }
        }
    }
}
pub fn wire(ui: &AppWindow, controller: Rc<AudioController>) -> Timer {
    ui.set_dvs_available(true);
    let license = Rc::new(RefCell::new(None::<DvsLicenseWindow>));
    let weak = ui.as_weak();
    ui.on_dvs_license(move || {
        let mut license = license.borrow_mut();
        if license.is_none() {
            match DvsLicenseWindow::new() {
                Ok(dialog) => {
                    dialog.set_license_text(format!("DVS decoder copyright (C) 2026 Mark Hills. GPL-3.0-only; no warranty.\nCombined DVS builds are distributed on GPLv3-compatible terms.\nSource and build instructions: https://github.com/yovannyr/DRDJPro-Rust\nPinned xwax source: be863572137929f70aa3ff0cef3b6b76f4973005\n\n{}\n\n{}",include_str!("../THIRD_PARTY_NOTICES.md"),include_str!("../crates/drdj-dvs/vendor/xwax/COPYING")).into());
                    let weak = dialog.as_weak();
                    dialog.on_dismiss(move || { if let Some(dialog)=weak.upgrade() {let _=dialog.hide();} });
                    *license=Some(dialog);
                }
                Err(error) => {if let Some(ui)=weak.upgrade(){ui.set_dvs_status(format!("Cannot open license window: {error}").into());}return;}
            }
        }
        if let Some(dialog)=license.as_ref() {let _=dialog.show();}
    });
    let state = Rc::new(RefCell::new(State {
        controller,
        devices: vec![],
        scan: None,
        sessions: [None, None],
    }));
    let weak = ui.as_weak();
    let refresh = Rc::clone(&state);
    ui.on_dvs_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            refresh.borrow_mut().refresh(&ui);
        }
    });
    let weak = ui.as_weak();
    let arm = Rc::clone(&state);
    ui.on_dvs_arm(move |index| {
        if let (Ok(index), Some(ui)) = (usize::try_from(index), weak.upgrade()) {
            if index < 2 {
                arm.borrow_mut().arm(index, &ui);
            }
        }
    });
    let weak = ui.as_weak();
    let stop = Rc::clone(&state);
    ui.on_dvs_stop(move |index| {
        if let (Ok(index), Some(ui)) = (usize::try_from(index), weak.upgrade()) {
            if index < 2 {
                stop.borrow_mut().stop(index, &ui);
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
    fn tracking(now: Instant, epoch: u64) -> InputSnapshot {
        InputSnapshot {
            revision: 1,
            epoch,
            state: InputState::Tracking(drdj_dvs::Observation {
                position_seconds: 2.0,
                speed: 0.95,
                stamp_age_seconds: 0.0,
            }),
            lost_frames: 0,
            observed_at: now,
        }
    }
    #[test]
    fn dvs_gate_extrapolates_fresh_stamps_and_never_rearms_after_loss() {
        let mut gate = Gate::new();
        let now = gate.armed;
        assert!(
            matches!(gate.decide(&tracking(now,1),now+Duration::from_millis(10),0.1),Decision::Apply(seconds,_) if (seconds-2.1095).abs()<1e-8)
        );
        let loss = tracking(now + Duration::from_millis(10), 2);
        assert!(matches!(
            gate.decide(&loss, loss.observed_at, 0.0),
            Decision::Disarm(_)
        ));
        assert!(matches!(
            gate.decide(&tracking(now, 2), now, 0.0),
            Decision::Wait
        ));
    }
    #[test]
    fn dvs_gate_rejects_stale_invalid_and_silent_signals_but_allows_bounded_initialization() {
        let mut gate = Gate::new();
        let now = gate.armed;
        let mut initial = tracking(now, 1);
        initial.state = InputState::Starting;
        assert!(matches!(
            gate.decide(&initial, now + Duration::from_millis(200), 0.0),
            Decision::Wait
        ));
        assert!(matches!(
            gate.decide(&tracking(now, 1), now + Duration::from_millis(60), 0.0),
            Decision::Disarm(_)
        ));
        let mut gate = Gate::new();
        let now = gate.armed;
        assert!(matches!(
            gate.decide(&initial, now + Duration::from_secs(4), 0.0),
            Decision::Disarm(_)
        ));
        let mut gate = Gate::new();
        let now = gate.armed;
        let mut bad = tracking(now, 1);
        if let InputState::Tracking(ref mut observation) = bad.state {
            observation.speed = f64::NAN;
        }
        assert!(matches!(gate.decide(&bad, now, 0.0), Decision::Disarm(_)));
        let mut gate = Gate::new();
        let now = gate.armed;
        gate.decide(&tracking(now, 1), now, 0.0);
        let mut loss = tracking(now, 1);
        loss.state = InputState::SignalLost;
        assert!(matches!(gate.decide(&loss, now, 0.0), Decision::Disarm(_)));
    }
}
