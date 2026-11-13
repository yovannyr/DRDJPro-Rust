//! Generic remote-window composition; all engine actions stay on the UI thread.
use crate::{remote_input::MidiPads, AppWindow, RemoteDisplayWindow};
use drdj_remote::{
    Config, Cue, DisplayWorker, PadEvent, PadProfile, PadState, Resolution, Snapshot,
    VelocityCurve, Widgets,
};
use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

fn snapshot(ui: &AppWindow, view: &RemoteDisplayWindow) -> Snapshot {
    let a = view.get_deck() != 1;
    let (title, artist, bpm, key, loaded, playing, position, duration, range, active, cues, eq) =
        if a {
            (
                ui.get_left_title(),
                ui.get_left_artist(),
                ui.get_left_platter_bpm(),
                ui.get_left_musical_key(),
                ui.get_left_loaded(),
                ui.get_left_playing(),
                ui.get_left_position(),
                ui.get_left_duration_seconds(),
                ui.get_left_loop_range(),
                ui.get_left_loop_active(),
                ui.get_left_hot_cues(),
                [ui.get_eq_low_a(), ui.get_eq_mid_a(), ui.get_eq_high_a()],
            )
        } else {
            (
                ui.get_right_title(),
                ui.get_right_artist(),
                ui.get_right_platter_bpm(),
                ui.get_right_musical_key(),
                ui.get_right_loaded(),
                ui.get_right_playing(),
                ui.get_right_position(),
                ui.get_right_duration_seconds(),
                ui.get_right_loop_range(),
                ui.get_right_loop_active(),
                ui.get_right_hot_cues(),
                [ui.get_eq_low_b(), ui.get_eq_mid_b(), ui.get_eq_high_b()],
            )
        };
    let duration = if duration.is_finite() {
        duration.clamp(0., 86400.)
    } else {
        0.
    };
    let position = if position.is_finite() {
        position.clamp(0., 100.) / 100.
    } else {
        0.
    };
    let time = |seconds: f32| {
        let t = seconds as u32;
        format!("{:02}:{:02}", t / 60, t % 60)
    };
    Snapshot {
        config: Config {
            resolution: if view.get_resolution() == 1 {
                Resolution::Half
            } else {
                Resolution::Full
            },
            transport_page: view.get_transport_page(),
            widgets: Widgets {
                track: view.get_track(),
                loops: view.get_loops(),
                cues: view.get_cues(),
                levels: view.get_levels(),
                eq: view.get_eq(),
                crossfader: view.get_crossfader(),
            },
        },
        title: title.to_string(),
        artist: artist.to_string(),
        bpm: bpm.to_string(),
        key: key.to_string(),
        time: format!("{}/{}", time(position * duration), time(duration)),
        duration_seconds: duration,
        loaded,
        playing,
        position,
        loop_range: (active && range.valid).then_some((range.start / 100., range.end / 100.)),
        cues: std::array::from_fn(|i| {
            cues.row_data(i)
                .filter(|c| c.valid && c.occupied)
                .map_or(Cue::default(), |c| Cue {
                    position: c.position / 100.,
                    rgb: [c.tint.red(), c.tint.green(), c.tint.blue()],
                    occupied: true,
                    active: (c.position / 100. - position).abs() * duration < 0.075,
                })
        }),
        levels: [ui.get_channel_a_peak_level(), ui.get_channel_b_peak_level()],
        eq: eq.map(|v| (v + 24.) / 36.),
        crossfader: ui.get_crossfader(),
        ..Default::default()
    }
}
fn route(ui: &AppWindow, event: PadEvent) {
    let (pad, held) = match event {
        PadEvent::Down { pad, .. } => (pad, true),
        PadEvent::Up { pad } => (pad, false),
        PadEvent::Pressure { .. } => return,
    };
    if pad >= 16 {
        return;
    }
    let a = pad < 8;
    if held
        && (!ui.get_audio_enabled()
            || ui.get_loading()
            || ui.get_audio_device_switching()
            || if a {
                !ui.get_left_loaded() || ui.get_left_busy()
            } else {
                !ui.get_right_loaded() || ui.get_right_busy()
            })
    {
        return;
    }
    // Input 15 is reserved for the independent remote pad session; MC4000 uses16.
    if a {
        ui.invoke_hold_hot_cue_left(i32::from(pad % 8), held, 15);
    } else {
        ui.invoke_hold_hot_cue_right(i32::from(pad % 8), held, 15);
    }
}
struct Pads {
    worker: Option<MidiPads>,
    mapper: Option<PadState>,
    epoch: u64,
    ports: Vec<String>,
    held_since: [Option<Instant>; 16],
    notice: Option<String>,
    last_status: Option<(u64, bool, String)>,
}
impl Pads {
    fn new() -> Self {
        Self {
            worker: None,
            mapper: None,
            epoch: 0,
            ports: Vec::new(),
            held_since: [None; 16],
            notice: None,
            last_status: None,
        }
    }
    fn release(&mut self, ui: &AppWindow) {
        if let Some(mapper) = &mut self.mapper {
            for event in mapper.release_all().into_iter().flatten() {
                route(ui, event);
            }
        }
        self.held_since.fill(None);
    }
    fn stop(&mut self, ui: &AppWindow) {
        self.release(ui);
        self.mapper = None;
        if let Some(worker) = &mut self.worker {
            if let Ok(epoch) = worker.stop() {
                self.epoch = epoch;
            }
        }
    }
    fn ensure_worker(&mut self) -> Result<&mut MidiPads, String> {
        if self.worker.is_none() {
            self.worker = Some(MidiPads::start().map_err(|e| e.to_string())?);
        }
        Ok(self.worker.as_mut().expect("worker initialized"))
    }
    fn open(&mut self, ui: &AppWindow, view: &RemoteDisplayWindow) -> Result<(), String> {
        self.notice = None;
        self.last_status = None;
        self.stop(ui);
        if let Some(worker) = &self.worker {
            for _ in 0..256 {
                if worker.events.pop().is_none() {
                    break;
                }
            }
        }
        let index =
            usize::try_from(view.get_pad_port()).map_err(|_| "Select a MIDI input first")?;
        let name = self
            .ports
            .get(index)
            .cloned()
            .ok_or("Selected MIDI input is stale")?;
        let profile = PadProfile {
            channel: u8::try_from(view.get_channel() - 1).map_err(|_| "Invalid channel")?,
            first_note: u8::try_from(view.get_first_note()).map_err(|_| "Invalid first note")?,
            curve: match view.get_curve() {
                1 => VelocityCurve::Exponential,
                2 => VelocityCurve::Logarithmic,
                _ => VelocityCurve::Linear,
            },
        };
        let mapper = PadState::new(profile).map_err(str::to_owned)?;
        self.epoch = self.ensure_worker()?.open(name)?;
        self.mapper = Some(mapper);
        Ok(())
    }
    fn poll(&mut self, ui: &AppWindow, view: &RemoteDisplayWindow) {
        let Some(worker) = &self.worker else {
            return;
        };
        let Some(status) = worker.snapshot() else {
            return;
        };
        if status.ports != self.ports {
            let selected = usize::try_from(view.get_pad_port())
                .ok()
                .and_then(|i| self.ports.get(i))
                .cloned();
            self.ports = status.ports;
            view.set_pad_port(
                selected
                    .and_then(|name| self.ports.iter().position(|n| n == &name))
                    .map_or(-1, |i| i as i32),
            );
            view.set_pad_ports(ModelRc::new(VecModel::from(
                self.ports
                    .iter()
                    .map(|n| n.as_str().into())
                    .collect::<Vec<_>>(),
            )));
        }
        if status.epoch != self.epoch {
            return;
        }
        let current_status = (status.epoch, status.connected, status.message.clone());
        if self.last_status.as_ref() != Some(&current_status) {
            if self.notice.is_none() {
                view.set_pad_status(status.message.clone().into());
            }
            self.last_status = Some(current_status);
        }
        if !status.connected || worker.overflow.load(Ordering::Acquire) {
            self.release(ui);
            return;
        }
        let now = Instant::now();
        if self
            .held_since
            .iter()
            .flatten()
            .any(|since| now.saturating_duration_since(*since) > Duration::from_secs(5))
        {
            self.stop(ui);
            self.notice = Some("Pad hold timeout; reopen explicitly".into());
            view.set_pad_status(self.notice.as_deref().unwrap_or_default().into());
            return;
        }
        let mut packets = Vec::with_capacity(64);
        for _ in 0..64 {
            match worker.events.pop() {
                Some(p) => packets.push(p),
                None => break,
            }
        }
        if packets.iter().any(|p| {
            p.epoch == self.epoch
                && now.saturating_duration_since(p.received) > Duration::from_millis(100)
        }) {
            self.stop(ui);
            self.notice = Some("Stale MIDI input; stopped, reopen explicitly".into());
            view.set_pad_status(self.notice.as_deref().unwrap_or_default().into());
            return;
        }
        for packet in packets {
            if packet.epoch != self.epoch {
                continue;
            }
            let Some(mapper) = &mut self.mapper else {
                continue;
            };
            for event in mapper
                .decode_message(&packet.bytes[..packet.len])
                .into_iter()
                .flatten()
            {
                match event {
                    PadEvent::Down { pad, velocity } => {
                        self.held_since[usize::from(pad)] = Some(now);
                        view.set_pad_status(
                            format!("Pad {} velocity {}", pad + 1, velocity).into(),
                        );
                    }
                    PadEvent::Pressure { pad, value } => {
                        view.set_pad_status(format!("Pad {} pressure {}", pad + 1, value).into());
                    }
                    PadEvent::Up { pad } => {
                        self.held_since[usize::from(pad)] = None;
                        view.set_pad_status(format!("Pad {} released", pad + 1).into());
                    }
                }
                route(ui, event);
            }
        }
    }
}
struct Session {
    view: RemoteDisplayWindow,
    display: DisplayWorker,
    pads: Rc<RefCell<Pads>>,
    closed: Rc<Cell<bool>>,
    last_frame: Option<Arc<drdj_remote::Frame>>,
    next_update: Instant,
}
/// Install the REMOTE action. No renderer, MIDI discovery or input starts until requested.
pub fn wire(ui: &AppWindow) -> Timer {
    wire_state(ui).0
}
type SessionState = Rc<RefCell<Option<Session>>>;
fn wire_state(ui: &AppWindow) -> (Timer, SessionState) {
    let state = Rc::new(RefCell::new(None::<Session>));
    let closing = Rc::clone(&state);
    let weak_close = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak_close.upgrade() {
            ui.invoke_close_web_browsers();
        }
        if let Some(session) = closing.borrow_mut().take() {
            if let Some(ui) = weak_close.upgrade() {
                session.pads.borrow_mut().stop(&ui);
            }
            let _ = session.view.hide();
        }
        // Independent auxiliary windows must not keep the app/audio alive.
        let _ = slint::quit_event_loop();
        slint::CloseRequestResponse::HideWindow
    });
    let current = Rc::clone(&state);
    let weak = ui.as_weak();
    ui.on_show_remote_display(move || {
        if let Some(session) = current.borrow().as_ref() {
            let _ = session.view.show();
            return;
        }
        let result = (|| -> Result<Session, String> {
            let view = RemoteDisplayWindow::new().map_err(|e| e.to_string())?;
            let display = DisplayWorker::start().map_err(|e| e.to_string())?;
            let pads = Rc::new(RefCell::new(Pads::new()));
            let closed = Rc::new(Cell::new(false));
            let flag = Rc::clone(&closed);
            view.on_dismiss(move || flag.set(true));
            let flag = Rc::clone(&closed);
            view.window().on_close_requested(move || {
                flag.set(true);
                slint::CloseRequestResponse::HideWindow
            });
            let p = Rc::clone(&pads);
            let v = view.as_weak();
            view.on_refresh_pads(move || {
                if let Some(v) = v.upgrade() {
                    let result = p.borrow_mut().ensure_worker().and_then(|w| w.refresh());
                    if let Err(e) = result {
                        p.borrow_mut().notice = Some(e.clone());
                        v.set_pad_status(e.into());
                    }
                }
            });
            let p = Rc::clone(&pads);
            let u = weak.clone();
            let v = view.as_weak();
            view.on_open_pads(move || {
                if let (Some(u), Some(v)) = (u.upgrade(), v.upgrade()) {
                    let result = p.borrow_mut().open(&u, &v);
                    if let Err(e) = result {
                        p.borrow_mut().notice = Some(e.clone());
                        v.set_pad_status(e.into());
                    }
                }
            });
            let p = Rc::clone(&pads);
            let u = weak.clone();
            let v = view.as_weak();
            view.on_stop_pads(move || {
                if let Some(u) = u.upgrade() {
                    let mut pads = p.borrow_mut();
                    pads.stop(&u);
                    pads.notice =
                        Some("MIDI pads stopped; reopen after configuration changes".into());
                    if let Some(v) = v.upgrade() {
                        v.set_pad_status(pads.notice.as_deref().unwrap_or_default().into());
                    }
                }
            });
            view.show().map_err(|e| e.to_string())?;
            Ok(Session {
                view,
                display,
                pads,
                closed,
                last_frame: None,
                next_update: Instant::now(),
            })
        })();
        match result {
            Ok(session) => *current.borrow_mut() = Some(session),
            Err(e) => {
                if let Some(u) = weak.upgrade() {
                    u.set_status(format!("Remote display: {e}").into());
                }
            }
        }
    });
    let timer = Timer::default();
    let weak = ui.as_weak();
    let timer_state = Rc::clone(&state);
    timer.start(TimerMode::Repeated, Duration::from_millis(2), move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut current = timer_state.borrow_mut();
        let Some(session) = current.as_mut() else {
            return;
        };
        if session.closed.get() {
            session.pads.borrow_mut().stop(&ui);
            let _ = session.view.hide();
            *current = None;
            return;
        }
        session.pads.borrow_mut().poll(&ui, &session.view);
        let now = Instant::now();
        if now < session.next_update {
            return;
        }
        let period = Duration::from_nanos(33_333_333);
        session.next_update += period;
        if session.next_update <= now {
            session.next_update = now + period;
        }
        let s = snapshot(&ui, &session.view);
        session.view.set_metadata(
            format!(
                "{} — {} | {} BPM | {} | {}",
                s.title, s.artist, s.bpm, s.key, s.time
            )
            .into(),
        );
        session.display.try_submit(s);
        if let Some(frame) = session.display.latest() {
            if session
                .last_frame
                .as_ref()
                .is_none_or(|old| !Arc::ptr_eq(old, &frame))
            {
                let buffer = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                    &frame.pixels,
                    128,
                    frame.height as u32,
                );
                session.view.set_frame(slint::Image::from_rgb8(buffer));
                session.last_frame = Some(frame);
            }
        }
    });
    (timer, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ui() -> AppWindow {
        i_slint_backend_testing::init_no_event_loop();
        AppWindow::new().unwrap()
    }
    fn fake_pads() -> Pads {
        let mut p = Pads::new();
        p.worker = Some(MidiPads::fixture());
        p.epoch = 1;
        p.mapper = Some(
            PadState::new(PadProfile {
                channel: 0,
                first_note: 0,
                curve: VelocityCurve::Linear,
            })
            .unwrap(),
        );
        p
    }
    fn send(p: &Pads, epoch: u64, age: Duration) {
        assert!(p
            .worker
            .as_ref()
            .unwrap()
            .events
            .push(crate::remote_input::Packet {
                epoch,
                received: Instant::now() - age,
                bytes: [0x90, 0, 64],
                len: 3
            })
            .is_ok());
    }
    #[test]
    fn disconnect_overflow_and_old_epochs_cannot_leave_or_rearm_holds() {
        let ui = ui();
        for overflow in [false, true] {
            let view = RemoteDisplayWindow::new().unwrap();
            ui.set_audio_enabled(true);
            ui.set_left_loaded(true);
            let calls = Rc::new(RefCell::new(Vec::new()));
            let c = Rc::clone(&calls);
            ui.on_hold_hot_cue_left(move |_, held, _| c.borrow_mut().push(held));
            let mut p = fake_pads();
            send(&p, 0, Duration::ZERO);
            p.poll(&ui, &view);
            assert!(calls.borrow().is_empty());
            send(&p, 1, Duration::ZERO);
            p.poll(&ui, &view);
            assert_eq!(*calls.borrow(), vec![true]);
            if overflow {
                p.worker
                    .as_ref()
                    .unwrap()
                    .overflow
                    .store(true, Ordering::Release);
            } else {
                p.worker
                    .as_ref()
                    .unwrap()
                    .fixture_status(crate::remote_input::Status {
                        epoch: 1,
                        connected: false,
                        ..Default::default()
                    });
            }
            p.poll(&ui, &view);
            p.poll(&ui, &view);
            assert_eq!(*calls.borrow(), vec![true, false]);
        }
    }
    #[test]
    fn stale_packets_and_hold_timeout_stop_and_keep_explanation() {
        let ui = ui();
        for timeout in [false, true] {
            let view = RemoteDisplayWindow::new().unwrap();
            let mut p = fake_pads();
            if timeout {
                p.held_since[0] = Some(Instant::now() - Duration::from_secs(6));
            } else {
                send(&p, 1, Duration::from_millis(101));
            }
            p.poll(&ui, &view);
            assert!(p.mapper.is_none());
            let reason = view.get_pad_status();
            assert!(reason.contains(if timeout { "timeout" } else { "Stale" }));
            p.worker
                .as_ref()
                .unwrap()
                .fixture_status(crate::remote_input::Status {
                    epoch: 1,
                    connected: false,
                    message: "stopped".into(),
                    ..Default::default()
                });
            p.poll(&ui, &view);
            assert_eq!(view.get_pad_status(), reason);
        }
    }
    #[test]
    fn remote_snapshot_reads_actual_deck_and_widget_properties() {
        let ui = ui();
        let v = RemoteDisplayWindow::new().unwrap();
        ui.set_left_title("TITLE".into());
        ui.set_left_duration_seconds(120.);
        ui.set_left_position(25.);
        ui.set_left_loaded(true);
        ui.set_left_loop_active(true);
        ui.set_left_loop_range(crate::LoopRange {
            valid: true,
            start: 20.,
            end: 40.,
            ..Default::default()
        });
        let s = snapshot(&ui, &v);
        assert_eq!(s.title, "TITLE");
        assert_eq!(s.time, "00:30/02:00");
        assert_eq!(s.loop_range, Some((0.2, 0.4)));
        v.set_resolution(1);
        v.set_track(false);
        assert_eq!(snapshot(&ui, &v).config.resolution, Resolution::Half);
        assert!(!snapshot(&ui, &v).config.widgets.track);
        v.set_deck(1);
        ui.set_right_title("B".into());
        assert_eq!(snapshot(&ui, &v).title, "B");
    }
    #[test]
    fn remote_pad_routes_all_cues_with_separate_ownership_and_busy_guards() {
        let ui = ui();
        ui.set_audio_enabled(true);
        ui.set_left_loaded(true);
        ui.set_right_loaded(true);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let c = Rc::clone(&calls);
        ui.on_hold_hot_cue_left(move |slot, held, input| {
            c.borrow_mut().push((0, slot, held, input))
        });
        let c = Rc::clone(&calls);
        ui.on_hold_hot_cue_right(move |slot, held, input| {
            c.borrow_mut().push((1, slot, held, input))
        });
        for pad in 0..16 {
            route(&ui, PadEvent::Down { pad, velocity: 1 });
            route(&ui, PadEvent::Up { pad });
        }
        assert_eq!(calls.borrow().len(), 32);
        assert_eq!(calls.borrow()[30], (1, 7, true, 15));
        ui.set_loading(true);
        route(
            &ui,
            PadEvent::Down {
                pad: 0,
                velocity: 1,
            },
        );
        assert_eq!(calls.borrow().len(), 32);
        route(&ui, PadEvent::Up { pad: 0 });
        assert_eq!(calls.borrow().len(), 33);
    }
    #[test]
    fn release_clears_all_local_pad_holds_without_native_input() {
        let ui = ui();
        let mut pads = Pads::new();
        pads.mapper = Some(
            PadState::new(PadProfile {
                channel: 0,
                first_note: 0,
                curve: VelocityCurve::Linear,
            })
            .unwrap(),
        );
        pads.mapper.as_mut().unwrap().decode(&[0x90, 15, 127]);
        let count = Rc::new(Cell::new(0));
        let c = Rc::clone(&count);
        ui.on_hold_hot_cue_right(move |slot, held, input| {
            assert_eq!((slot, held, input), (7, false, 15));
            c.set(c.get() + 1);
        });
        pads.release(&ui);
        pads.release(&ui);
        assert_eq!(count.get(), 1);
    }
    #[test]
    fn remote_window_opens_closes_releases_and_reopens_without_native_input() {
        let ui = ui();
        ui.set_left_playing(true);
        ui.set_left_position(42.);
        let (_timer, state) = wire_state(&ui);
        assert!(state.borrow().is_none());
        ui.invoke_show_remote_display();
        {
            let current = state.borrow();
            let session = current.as_ref().unwrap();
            assert!(session.pads.borrow().worker.is_none());
            let mut pads = session.pads.borrow_mut();
            pads.mapper = Some(
                PadState::new(PadProfile {
                    channel: 0,
                    first_note: 0,
                    curve: VelocityCurve::Linear,
                })
                .unwrap(),
            );
            pads.mapper.as_mut().unwrap().decode(&[0x90, 0, 127]);
        }
        let count = Rc::new(Cell::new(0));
        let c = Rc::clone(&count);
        ui.on_hold_hot_cue_left(move |_, held, input| {
            assert!(!held);
            assert_eq!(input, 15);
            c.set(c.get() + 1);
        });
        state
            .borrow()
            .as_ref()
            .unwrap()
            .view
            .window()
            .dispatch_event(slint::platform::WindowEvent::CloseRequested);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(3));
        slint::platform::update_timers_and_animations();
        assert!(state.borrow().is_none());
        assert_eq!(count.get(), 1);
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 42.);
        ui.invoke_show_remote_display();
        assert!(state.borrow().is_some());
        state.borrow().as_ref().unwrap().view.invoke_dismiss();
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(3));
        slint::platform::update_timers_and_animations();
        assert!(state.borrow().is_none());
    }
}
