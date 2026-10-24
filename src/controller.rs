//! GUI control plane with injectable file selection and one bounded load worker.
use crate::{
    playback::{DeckId, DualDeckEngine, MixerLevels, PreparedTrack},
    AppWindow,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
};

type LoadResult = Option<Result<PreparedTrack, String>>;
struct DetailCache {
    source: Arc<crate::waveform::DetailPeaks>,
    range: crate::waveform::DetailRange,
}
pub type TrackPicker = Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>;
pub struct AudioController {
    ui: slint::Weak<AppWindow>,
    engine: Rc<DualDeckEngine>,
    picker: TrackPicker,
    pending: RefCell<Option<(DeckId, Receiver<LoadResult>)>>,
    displayed_overviews: RefCell<[Option<Arc<[f32]>>; 2]>,
    displayed_details: RefCell<[Option<DetailCache>; 2]>,
}
impl AudioController {
    pub fn wire(ui: &AppWindow, engine: Rc<DualDeckEngine>, picker: TrackPicker) -> Rc<Self> {
        let controller = Rc::new(Self {
            ui: ui.as_weak(),
            engine,
            picker,
            pending: RefCell::new(None),
            displayed_overviews: RefCell::new([None, None]),
            displayed_details: RefCell::new([None, None]),
        });
        ui.set_audio_enabled(true);
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(&controller);
            let toggle = move || {
                if let Some(c) = weak.upgrade() {
                    if c.engine.snapshot(id).playing {
                        c.engine.pause(id);
                    } else {
                        c.engine.play(id);
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_toggle_left(toggle),
                DeckId::B => ui.on_toggle_right(toggle),
            }
            let weak = Rc::downgrade(&controller);
            let cue = move || {
                if let Some(c) = weak.upgrade() {
                    c.engine.cue(id);
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_reset_left(cue),
                DeckId::B => ui.on_reset_right(cue),
            }
            let weak = Rc::downgrade(&controller);
            let load = move || {
                if let Some(c) = weak.upgrade() {
                    c.request_load(id);
                }
            };
            match id {
                DeckId::A => ui.on_load_left(load),
                DeckId::B => ui.on_load_right(load),
            }
            let weak = Rc::downgrade(&controller);
            let seek = move |percent| {
                if let Some(c) = weak.upgrade() {
                    if let Err(err) = c.engine.seek_percent(id, percent) {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(format!("Deck {id:?}: {err}").into());
                        }
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_seek_left(seek),
                DeckId::B => ui.on_seek_right(seek),
            }
            let weak = Rc::downgrade(&controller);
            let zoom = move || {
                if let Some(c) = weak.upgrade() {
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_zoom_left(zoom),
                DeckId::B => ui.on_zoom_right(zoom),
            }
        }
        let weak = Rc::downgrade(&controller);
        ui.on_mixer_changed(move || {
            if let Some(c) = weak.upgrade() {
                c.apply_levels();
            }
        });
        controller.apply_levels();
        controller.refresh();
        controller
    }
    fn apply_levels(&self) {
        if let Some(ui) = self.ui.upgrade() {
            self.engine.set_levels(MixerLevels {
                crossfader: ui.get_crossfader(),
                channel_a: ui.get_channel_a(),
                channel_b: ui.get_channel_b(),
            });
        }
    }
    fn request_load(&self, id: DeckId) {
        if self.pending.borrow().is_some() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        ui.set_loading(true);
        ui.set_status("Select MP3/WAV; decoding runs in the background".into());
        let (tx, rx) = mpsc::sync_channel(1);
        let picker = Arc::clone(&self.picker);
        match std::thread::Builder::new()
            .name("track-loader".into())
            .spawn(move || {
                let result = picker().map(|path| PreparedTrack::open(&path));
                let _ = tx.send(result);
            }) {
            Ok(_) => {
                *self.pending.borrow_mut() = Some((id, rx));
            }
            Err(err) => {
                ui.set_loading(false);
                ui.set_status(format!("Cannot start loader: {err}").into());
            }
        }
    }
    fn complete_load(&self, id: DeckId, result: LoadResult) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        ui.set_loading(false);
        match result {
            Some(Ok(track)) => {
                self.engine.install(id, track);
                ui.set_status(format!("Deck {id:?} loaded and paused").into());
            }
            Some(Err(err)) => ui.set_status(format!("Deck {id:?}: {err}").into()),
            None => ui.set_status("Selection cancelled".into()),
        }
        self.refresh();
    }
    pub fn tick(&self) {
        let result = self
            .pending
            .borrow()
            .as_ref()
            .and_then(|(id, rx)| match rx.try_recv() {
                Ok(result) => Some((*id, result)),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some((*id, Some(Err("Track loader stopped unexpectedly".into()))))
                }
            });
        if let Some((id, result)) = result {
            self.pending.borrow_mut().take();
            self.complete_load(id, result);
        }
        self.refresh();
    }
    fn refresh(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        for id in [DeckId::A, DeckId::B] {
            let s = self.engine.snapshot(id);
            let index = if id == DeckId::A { 0 } else { 1 };
            let mut displayed = self.displayed_overviews.borrow_mut();
            let changed = match (&displayed[index], &s.overview) {
                (Some(previous), Some(current)) => !Arc::ptr_eq(previous, current),
                (None, None) => false,
                _ => true,
            };
            if changed {
                let peaks = s.overview.as_ref().map_or_else(Vec::new, |v| v.to_vec());
                let model = ModelRc::new(Rc::new(VecModel::from(peaks)));
                let colors = s
                    .detail
                    .as_ref()
                    .map_or_else(ModelRc::default, |d| color_model(d.overview_colors()));
                match id {
                    DeckId::A => {
                        ui.set_left_waveform(model);
                        ui.set_left_waveform_colors(colors);
                    }
                    DeckId::B => {
                        ui.set_right_waveform(model);
                        ui.set_right_waveform_colors(colors);
                    }
                }
                displayed[index] = s.overview.clone();
            }
            drop(displayed);
            let percent = s.percent();
            if let Some(detail) = &s.detail {
                let seconds = match id {
                    DeckId::A => ui.get_left_detail_window(),
                    DeckId::B => ui.get_right_detail_window(),
                }
                .clamp(2, 32) as u32;
                let range = detail.range(s.position, seconds);
                let start = detail.percent(range.start_frame);
                let end = detail.percent(range.end_frame);
                let marker = if end > start {
                    (100.0 * (percent - start) / (end - start)).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                let mut cache = self.displayed_details.borrow_mut();
                let changed = cache[index].as_ref().is_none_or(|previous| {
                    !Arc::ptr_eq(&previous.source, detail) || previous.range != range
                });
                if changed {
                    let model = ModelRc::new(Rc::new(VecModel::from(detail.render(range))));
                    let colors = color_model(&detail.render_colors(range));
                    match id {
                        DeckId::A => {
                            ui.set_left_detail(model);
                            ui.set_left_detail_colors(colors);
                        }
                        DeckId::B => {
                            ui.set_right_detail(model);
                            ui.set_right_detail_colors(colors);
                        }
                    }
                    cache[index] = Some(DetailCache {
                        source: Arc::clone(detail),
                        range,
                    });
                }
                match id {
                    DeckId::A => {
                        ui.set_left_detail_start(start);
                        ui.set_left_detail_end(end);
                        ui.set_left_detail_marker(marker);
                    }
                    DeckId::B => {
                        ui.set_right_detail_start(start);
                        ui.set_right_detail_end(end);
                        ui.set_right_detail_marker(marker);
                    }
                }
            }
            let time = format!("{} / {}", clock(s.position), clock(s.duration));
            match id {
                DeckId::A => {
                    ui.set_left_title(s.title.into());
                    ui.set_left_loaded(s.loaded);
                    ui.set_left_playing(s.playing);
                    ui.set_left_position(percent);
                    ui.set_left_time(time.into());
                }
                DeckId::B => {
                    ui.set_right_title(s.title.into());
                    ui.set_right_loaded(s.loaded);
                    ui.set_right_playing(s.playing);
                    ui.set_right_position(percent);
                    ui.set_right_time(time.into());
                }
            }
        }
    }
}
fn color_model(colors: &[crate::waveform_color::Rgb]) -> ModelRc<slint::Color> {
    let colors: Vec<_> = colors
        .iter()
        .map(|c| slint::Color::from_rgb_u8(c.0, c.1, c.2))
        .collect();
    ModelRc::new(Rc::new(VecModel::from(colors)))
}
fn clock(time: std::time::Duration) -> String {
    let secs = time.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fixture, wav_file, TempFile};
    use i_slint_backend_testing::ElementHandle;
    use std::{collections::VecDeque, sync::Mutex, time::Duration};
    fn control(ui: &AppWindow, label: &str) -> ElementHandle {
        let controls: Vec<_> = ElementHandle::find_by_accessible_label(ui, label).collect();
        assert_eq!(controls.len(), 1, "accessible control {label}");
        controls.into_iter().next().unwrap()
    }
    fn finish(c: &AudioController) {
        assert!(c.pending.borrow().is_some(), "GUI started a load worker");
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while c.pending.borrow().is_some() {
            assert!(std::time::Instant::now() < deadline, "worker completed");
            // Exercise the same completion path as the application's GUI timer.
            c.tick();
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn detail_zoom_and_clicks_follow_local_time_without_changing_other_deck() {
        use slint::platform::{PointerEventButton, WindowEvent};
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
        let c = AudioController::wire(&ui, Rc::clone(&engine), Arc::new(|| None));
        ui.show().unwrap();
        assert_eq!(
            control(&ui, "DECK A zoom in").accessible_enabled(),
            Some(false)
        );
        let wav = wav_file(8192, 48_000 * 30);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(&fixture()).unwrap();
        c.tick();
        assert_eq!(
            ui.get_left_detail().row_count(),
            crate::waveform::OVERVIEW_BINS
        );
        let overview = ui.get_left_waveform();
        let right_model = ui.get_right_detail();
        for _ in 0..2 {
            control(&ui, "DECK A zoom in").invoke_accessible_default_action();
        }
        assert_eq!(ui.get_left_detail_window(), 2);
        assert_eq!(
            control(&ui, "DECK A zoom in").accessible_enabled(),
            Some(false)
        );
        for _ in 0..4 {
            control(&ui, "DECK A zoom out").invoke_accessible_default_action();
        }
        assert_eq!(ui.get_left_detail_window(), 32);
        assert_eq!(
            control(&ui, "DECK A zoom out").accessible_enabled(),
            Some(false)
        );
        for _ in 0..2 {
            control(&ui, "DECK A zoom in").invoke_accessible_default_action();
        }
        assert_eq!(ui.get_left_detail_window(), 8);
        engine.play(DeckId::A);
        for width in [1200.0, 850.0] {
            ui.window().set_size(slint::LogicalSize::new(width, 740.0));
            engine.seek_percent(DeckId::A, 50.0).unwrap();
            c.tick();
            assert!((ui.get_left_detail_start() - 100.0 * 11.0 / 30.0).abs() < 0.001);
            assert!((ui.get_left_detail_end() - 100.0 * 19.0 / 30.0).abs() < 0.001);
            let view = control(&ui, "DECK A detailed waveform");
            let origin = view.absolute_position();
            let size = view.size();
            let position = slint::LogicalPosition::new(
                origin.x + size.width * 0.25,
                origin.y + size.height / 2.0,
            );
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            // Quarter of the 11..19s view is 13s, not quarter of the whole track.
            assert!((engine.snapshot(DeckId::A).position.as_secs_f64() - 13.0).abs() < 0.001);
            assert!(ui.get_left_playing());
            assert!((ui.get_left_detail_marker() - 50.0).abs() < 0.01);
            let marker = control(&ui, "DECK A detail playhead");
            assert!(
                (marker.absolute_position().x
                    - origin.x
                    - (size.width - marker.size().width) / 2.0)
                    .abs()
                    < 0.1
            );
        }
        output.by_ref().take(4800).for_each(drop);
        c.tick();
        assert!(ui.get_left_detail_start() > 30.0);
        assert_eq!(ui.get_right_position(), 0.0);
        assert_eq!(ui.get_right_detail_window(), 8);
        assert_eq!(ui.get_right_detail(), right_model);
        engine.pause(DeckId::A);
        c.tick();
        let paused = ui.get_left_detail();
        let paused_colors = ui.get_left_detail_colors();
        c.tick();
        assert_eq!(ui.get_left_detail(), paused);
        assert_eq!(ui.get_left_detail_colors(), paused_colors);
        engine.seek_percent(DeckId::A, 100.0).unwrap();
        c.tick();
        assert!((ui.get_left_detail_marker() - 100.0).abs() < 0.001);
        assert!(!ui.get_left_playing());
        engine.cue(DeckId::A);
        c.tick();
        assert_eq!(ui.get_left_detail_marker(), 0.0);
        assert_eq!(ui.get_left_detail_start(), 0.0);
        assert_eq!(ui.get_left_waveform(), overview);
        ui.hide().unwrap();
    }
    #[test]
    fn headless_overview_clicks_map_to_track_time_and_playhead_at_different_widths() {
        use slint::platform::{PointerEventButton, WindowEvent};
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
        let c = AudioController::wire(&ui, Rc::clone(&engine), Arc::new(|| None));
        ui.show().unwrap(); // Testing backend only: no native window or visual acceptance.
        assert_eq!(
            control(&ui, "DECK A waveform overview").accessible_enabled(),
            Some(false)
        );
        assert_eq!(ui.get_left_waveform().row_count(), 0);
        let wav = wav_file(8192, 48_000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(&fixture()).unwrap();
        c.tick();
        let model = ui.get_left_waveform();
        assert_eq!(model.row_count(), crate::waveform::OVERVIEW_BINS);
        assert!((0..model.row_count()).all(|i| model.row_data(i) == Some(0.25)));
        let right_model = ui.get_right_waveform();
        for width in [1200.0, 850.0] {
            ui.window().set_size(slint::LogicalSize::new(width, 740.0));
            for (deck, fraction) in [("A", 0.25), ("B", 0.75), ("A", 0.5)] {
                let overview = control(&ui, &format!("DECK {deck} waveform overview"));
                let origin = overview.absolute_position();
                let size = overview.size();
                assert!(size.width > 0.0 && size.height > 0.0);
                let position = slint::LogicalPosition::new(
                    origin.x + size.width * fraction,
                    origin.y + size.height / 2.0,
                );
                ui.window()
                    .dispatch_event(WindowEvent::PointerMoved { position });
                ui.window().dispatch_event(WindowEvent::PointerPressed {
                    position,
                    button: PointerEventButton::Left,
                });
                ui.window().dispatch_event(WindowEvent::PointerReleased {
                    position,
                    button: PointerEventButton::Left,
                });
                let id = if deck == "A" { DeckId::A } else { DeckId::B };
                assert!((engine.snapshot(id).percent() - fraction * 100.0).abs() < 0.01);
                let marker = control(&ui, &format!("DECK {deck} waveform playhead"));
                let expected_x = origin.x + fraction * (size.width - marker.size().width);
                assert!((marker.absolute_position().x - expected_x).abs() < 0.1);
                assert!(!engine.snapshot(id).playing);
            }
        }
        engine.play(DeckId::A);
        c.tick();
        control(&ui, "DECK A waveform overview").set_accessible_value("60");
        assert!(ui.get_left_playing());
        assert!((ui.get_left_position() - 60.0).abs() < 0.01);
        let right = ui.get_right_position();
        for (value, fraction) in [("0", 0.0), ("100", 1.0)] {
            control(&ui, "DECK A waveform overview").set_accessible_value(value);
            let overview = control(&ui, "DECK A waveform overview");
            let marker = control(&ui, "DECK A waveform playhead");
            let expected = overview.absolute_position().x
                + fraction * (overview.size().width - marker.size().width);
            assert!((marker.absolute_position().x - expected).abs() < 0.1);
        }
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_right_position(), right);
        engine.cue(DeckId::A);
        c.tick();
        assert_eq!(ui.get_left_waveform(), model); // No model rebuild on timer, seek or Cue.
        assert_eq!(ui.get_right_waveform(), right_model);
        ui.hide().unwrap();
    }
    #[test]
    fn headless_position_sliders_seek_both_decks_without_changing_transport_or_other_deck() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
        let c = AudioController::wire(&ui, Rc::clone(&engine), Arc::new(|| None));
        assert_eq!(
            control(&ui, "DECK A position").accessible_enabled(),
            Some(false)
        );
        let wav = wav_file(8192, 48_000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(&fixture()).unwrap();
        c.tick();
        control(&ui, "DECK A position").set_accessible_value("25");
        c.tick();
        assert_eq!(ui.get_left_position(), 25.0);
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_right_position(), 0.0);
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        control(&ui, "DECK A position").set_accessible_value("75");
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 75.0);
        output.by_ref().take(2400).for_each(drop);
        c.tick();
        assert!(ui.get_left_position() > 75.0);
        let left = ui.get_left_position();
        control(&ui, "DECK B position").set_accessible_value("80");
        assert!((ui.get_right_position() - 80.0).abs() < 0.001);
        assert!(!ui.get_right_playing());
        assert_eq!(ui.get_left_position(), left);
        control(&ui, "DECK A position").set_accessible_value("100");
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 100.0);
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 0.0);
        control(&ui, "DECK A cue").invoke_accessible_default_action();
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 0.0);
        assert!((ui.get_right_position() - 80.0).abs() < 0.001);
    }
    #[test]
    fn headless_gui_load_transport_faders_errors_cancel_and_position() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
        let wav = wav_file(8192, 48_000);
        let bad = TempFile::new("mp3", b"bad music");
        let paths = Arc::new(Mutex::new(VecDeque::from([
            Some(wav.path().to_owned()),
            Some(fixture()),
            Some(bad.path().to_owned()),
            None,
            Some(wav.path().to_owned()),
        ])));
        let picker: TrackPicker = Arc::new(move || paths.lock().unwrap().pop_front().unwrap());
        let c = AudioController::wire(&ui, Rc::clone(&engine), picker);
        assert!(!ui.get_left_loaded());
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        assert!(ui.get_loading());
        c.request_load(DeckId::B); // A second request cannot overtake the first.
        finish(&c);
        assert!(ui.get_left_loaded());
        let left_waveform = ui.get_left_waveform();
        let left_detail = ui.get_left_detail();
        let left_colors = ui.get_left_waveform_colors();
        assert_eq!(left_colors.row_count(), left_waveform.row_count());
        let bass = left_colors.row_data(left_colors.row_count() - 1).unwrap();
        assert!(bass.red() > bass.green() && bass.red() > bass.blue());
        assert_eq!(
            ui.get_left_detail_colors().row_count(),
            left_detail.row_count()
        );
        assert!(!ui.get_right_loaded());
        assert!(!ui.get_left_playing());
        control(&ui, "DECK B load track").invoke_accessible_default_action();
        finish(&c);
        assert!(ui.get_right_loaded());
        let right_waveform = ui.get_right_waveform();
        let right_colors = ui.get_right_waveform_colors();
        assert_eq!(right_colors.row_count(), right_waveform.row_count());
        assert!((0..right_colors.row_count()).any(|i| {
            let color = right_colors.row_data(i).unwrap();
            color.green() > color.red() && color.green() > color.blue()
        }));
        let right_detail = ui.get_right_detail();
        assert!(ui.get_right_title().ends_with("tone-9s.mp3"));
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(!engine.snapshot(DeckId::B).playing);
        // Use actual slider accessibility actions, not just the public callback.
        control(&ui, "Channel A fader").set_accessible_value("0.4");
        control(&ui, "Channel B fader").set_accessible_value("0.8");
        control(&ui, "Crossfader").set_accessible_value("0.25");
        output.by_ref().take(2400).for_each(drop);
        c.tick();
        assert!(ui.get_left_position() > 0.0);
        assert_eq!(ui.get_right_position(), 0.0);
        let samples: Vec<_> = output.by_ref().take(2400).collect();
        assert!(samples.iter().all(|s| (s - 0.075).abs() < 1e-5));
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        assert!(!engine.snapshot(DeckId::A).playing);
        control(&ui, "DECK B play/pause").invoke_accessible_default_action();
        control(&ui, "DECK A cue").invoke_accessible_default_action();
        assert_eq!(ui.get_left_position(), 0.0);
        assert!(ui.get_right_playing());
        let title = ui.get_left_title();
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        finish(&c);
        assert_eq!(ui.get_left_title(), title);
        assert_eq!(ui.get_left_waveform(), left_waveform);
        assert!(ui.get_status().starts_with("Deck A:"));
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        finish(&c);
        assert_eq!(ui.get_left_title(), title);
        assert_eq!(ui.get_status(), "Selection cancelled");
        assert_eq!(ui.get_left_waveform_colors(), left_colors);
        assert_eq!(ui.get_right_waveform_colors(), right_colors);
        assert_eq!(ui.get_left_waveform(), left_waveform);
        assert_eq!(ui.get_right_waveform(), right_waveform);
        assert_eq!(ui.get_left_detail(), left_detail);
        assert_eq!(ui.get_right_detail(), right_detail);
        control(&ui, "DECK B load track").invoke_accessible_default_action();
        finish(&c);
        assert!(!ui.get_right_playing());
        assert_eq!(ui.get_right_position(), 0.0);
        assert_eq!(ui.get_left_title(), title);
        assert_ne!(ui.get_right_waveform(), right_waveform);
        assert_ne!(ui.get_right_waveform_colors(), right_colors);
        assert_eq!(ui.get_left_waveform_colors(), left_colors);
        assert_eq!(ui.get_left_waveform(), left_waveform);
        assert_ne!(ui.get_right_detail(), right_detail);
        assert_eq!(ui.get_left_detail(), left_detail);
    }
}
