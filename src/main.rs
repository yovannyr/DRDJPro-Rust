use slint::{ModelRc, Timer, TimerMode, VecModel};
use std::{rc::Rc, time::Duration};

mod audio;
mod audio_devices;
mod audio_formats;
mod audio_identity;
mod beatgrid;
mod cancellation;
mod controller;
mod crates;
mod cue_bus;
mod deck_render;
mod delay_fx;
mod hot_cues;
mod legacy_catalog;
mod library;
mod library_store;
mod looping;
mod mc4000;
mod media_input;
mod mixer_dsp;
mod opus_audio;
mod output_preferences;
mod output_routing;
mod peak_meter;
mod phase_sync;
mod pitch_control;
mod playback;
mod playback_speed;
mod preferences;
mod render_bus;
mod saved_loops;
mod sync_control;
mod tempo;
#[cfg(test)]
mod test_support;
mod track_metadata;
mod transport;
mod waveform;
mod waveform_color;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    if std::env::args().nth(1).as_deref() == Some("--analyze") {
        let Some(path) = std::env::args().nth(2) else {
            eprintln!("Usage: drdjpro-ui-spike --analyze <music.mp3|music.wav>");
            std::process::exit(2);
        };
        match audio::analyze_file(std::path::Path::new(&path)) {
            Ok(info) => {
                println!(
                    "rate={} Hz, channels={}, frames={}, duration={:.3}s, peak={:.4}, rms={:.4}",
                    info.sample_rate,
                    info.channels,
                    info.frames,
                    info.duration_seconds(),
                    info.peak,
                    info.rms
                );
                return Ok(());
            }
            Err(err) => {
                eprintln!("Could not decode {path}: {err}");
                std::process::exit(1);
            }
        }
    }
    if std::env::args().nth(1).as_deref() == Some("--play") {
        let Some(path) = std::env::args().nth(2) else {
            eprintln!("Usage: drdjpro-ui-spike --play <music.mp3|music.wav>");
            std::process::exit(2);
        };
        if let Err(err) = playback::play_file(std::path::Path::new(&path)) {
            eprintln!("Audio playback failed: {err}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--mix") {
        let args: Vec<_> = std::env::args().collect();
        if args.len() != 4 {
            eprintln!("Usage: drdjpro-ui-spike --mix <deck-a.mp3> <deck-b.mp3>");
            std::process::exit(2);
        }
        if let Err(err) = playback::mix_files(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        ) {
            eprintln!("Audio mixing failed: {err}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let gui_tracks = if std::env::args().nth(1).as_deref() == Some("--gui") {
        let args: Vec<_> = std::env::args().collect();
        if args.len() != 2 && args.len() != 4 {
            eprintln!("Usage: drdjpro-ui-spike --gui [<deck-a.mp3> <deck-b.mp3>]");
            std::process::exit(2);
        }
        if args.len() == 4 {
            Some((args[2].clone(), args[3].clone()))
        } else {
            None
        }
    } else {
        None
    };
    let mut startup_audio_warning = String::new();
    let audio_engine = if std::env::args().nth(1).as_deref() != Some("--demo") {
        let startup = match library_store::default_path()
            .and_then(|path| library_store::read_startup_audio(&path))
        {
            Ok(value) => value,
            Err(error) => {
                startup_audio_warning =
                    format!("Startup preference unavailable: {error}; system default requested");
                Default::default()
            }
        };
        let opened =
            audio_devices::open_preferred(&startup, playback::DualDeckEngine::open_selected).map(
                |(engine, warning)| {
                    if let Some(warning) = warning {
                        startup_audio_warning = warning;
                    }
                    engine
                },
            );
        let engine = match opened {
            Ok(engine) => engine,
            Err(err) => {
                eprintln!("Cannot open default audio device: {err}");
                std::process::exit(1);
            }
        };
        if let Some((a, b)) = &gui_tracks {
            if let Err(err) = engine.load_a(std::path::Path::new(a)) {
                eprintln!("Cannot load deck A: {err}");
                std::process::exit(1);
            }
            if let Err(err) = engine.load_b(std::path::Path::new(b)) {
                eprintln!("Cannot load deck B: {err}");
                std::process::exit(1);
            }
        }
        engine.set_levels(playback::MixerLevels {
            crossfader: 0.5,
            channel_a: 1.0,
            channel_b: 1.0,
        });
        Some(Rc::new(engine))
    } else {
        None
    };
    let ui = AppWindow::new()?;
    ui.set_audio_enabled(audio_engine.is_some());
    ui.set_audio_startup_warning(startup_audio_warning.into());

    if audio_engine.is_none() {
        let waveform: Vec<f32> = (0..240)
            .map(|i| {
                let t = i as f32;
                let pulse = (t * 0.29).sin().abs();
                let texture = (t * 1.37).sin().abs() * 0.38;
                (0.12 + 0.62 * pulse + texture).min(1.0)
            })
            .collect();
        let model = ModelRc::new(Rc::new(VecModel::from(waveform)));
        ui.set_left_waveform(model.clone());
        ui.set_right_waveform(model);
    }

    let controller = if let Some(engine) = &audio_engine {
        let picker: controller::TrackPicker = std::sync::Arc::new(|| {
            rfd::FileDialog::new()
                .add_filter("Audio files", audio_formats::EXTENSIONS)
                .pick_file()
        });
        ui.set_status("Select a track for either deck".into());
        let controller = controller::AudioController::wire(
            &ui,
            Rc::clone(engine),
            picker,
            std::sync::Arc::new(|| {
                rfd::FileDialog::new()
                    .add_filter("Audio files", audio_formats::EXTENSIONS)
                    .pick_files()
            }),
        );
        let initial_paths = gui_tracks
            .as_ref()
            .map_or_else(Vec::new, |(left, right)| vec![left.into(), right.into()]);
        controller.restore_library(library_store::default_path(), initial_paths);
        Some(controller)
    } else {
        wire_decks(&ui);
        ui.set_status("Simulation: start without --demo for real audio".into());
        None
    };

    let audio_owner = controller
        .as_ref()
        .map(|controller| controller.audio_owner());
    drop(audio_engine);
    let mut midi = if controller.is_some() {
        match mc4000::Input::connect() {
            Ok(input) => {
                if input.is_some() {
                    ui.set_status("MC4000 connected: transport and mixer enabled".into());
                }
                input
            }
            Err(error) => {
                ui.set_status(format!("MC4000 input unavailable: {error}").into());
                None
            }
        }
    } else {
        None
    };
    let mut midi_discovery = std::time::Instant::now();
    let midi_trace = std::env::var_os("DRDJ_MIDI_TRACE").is_some_and(|v| v == "1");
    let timer = Timer::default();
    let closing_controller = controller.clone();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(33), move || {
        if let Some(ui) = weak.upgrade() {
            if let Some(controller) = &controller {
                let reconnect = ui.get_controller_reconnect_requested();
                ui.set_controller_reconnect_requested(false);
                mc4000::release_for_reconnect(&mut midi, reconnect);
                if reconnect || midi_discovery.elapsed() >= Duration::from_secs(2) {
                    midi_discovery = std::time::Instant::now();
                    match mc4000::present() {
                        Ok(false) => {
                            midi = None;
                            ui.set_controller_status("MC4000 disconnected".into());
                        }
                        Ok(true) if midi.is_none() => match mc4000::Input::connect() {
                            Ok(input) => {
                                midi = input;
                                if midi.is_some() {
                                    ui.set_status("MC4000 connected".into());
                                } else {
                                    ui.set_controller_status("MC4000 disconnected".into());
                                }
                            }
                            Err(error) => ui.set_controller_status(
                                format!("MC4000 connection failed: {error}").into(),
                            ),
                        },
                        Err(error) => {
                            midi = None;
                            ui.set_controller_status(
                                format!("MC4000 discovery failed: {error}").into(),
                            );
                        }
                        _ => (),
                    }
                    if let Some(input) = &mut midi {
                        let message = match input.refresh_output() {
                            Err(error) => format!("MC4000 input connected; LED retry: {error}"),
                            Ok(()) if input.leds_connected() => {
                                "MC4000 input + LEDs connected".into()
                            }
                            Ok(()) => "MC4000 input connected; LEDs unavailable (retrying)".into(),
                        };
                        ui.set_controller_status(message.into());
                    }
                }
                if let Some(midi) = &mut midi {
                    midi.discard_buttons_before(controller.transport_accept_after());
                    if midi.poll(|command| {
                        let before = (ui.get_left_playing(), ui.get_right_playing());
                        let transport = matches!(&command, mc4000::Command::Toggle(_));
                        if midi_trace && transport {
                            eprintln!("MC4000 dispatch {command:?} playing_before={before:?}");
                        }
                        mc4000::apply(&ui, command);
                        if midi_trace && transport {
                            eprintln!(
                                "MC4000 playing_after={:?}",
                                (ui.get_left_playing(), ui.get_right_playing())
                            );
                        }
                    }) {
                        ui.set_status(
                            "MC4000 input queue overflow; a button action was dropped".into(),
                        );
                    }
                }
                controller.tick();
                return;
            }
            ui.set_left_position(transport::advance_position(
                ui.get_left_position(),
                ui.get_left_playing(),
            ));
            ui.set_right_position(transport::advance_position(
                ui.get_right_position(),
                ui.get_right_playing(),
            ));
        }
    });

    let result = ui.run();
    lifecycle_trace("GUI loop exited");
    timer.stop();
    lifecycle_trace("timer stopped");
    if let Some(controller) = closing_controller {
        lifecycle_trace("storage flush begin");
        if let Err(error) = controller.close_library() {
            return Err(slint::PlatformError::Other(format!(
                "Library final save failed: {error}"
            )));
        }
        lifecycle_trace("storage flush complete");
    }
    lifecycle_trace("MIDI/timer release begin");
    drop(timer);
    lifecycle_trace("MIDI/timer release complete");
    drop(ui);
    lifecycle_trace("UI released");
    drop(audio_owner);
    lifecycle_trace("audio released");
    result
}

fn lifecycle_trace(stage: &str) {
    if std::env::var_os("DRDJ_LIFECYCLE_TRACE").is_some() {
        eprintln!("DrDJ lifecycle: {stage}");
    }
}

fn wire_decks(ui: &AppWindow) {
    let weak = ui.as_weak();
    ui.on_toggle_left(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_left_playing(transport::toggle_playing(ui.get_left_playing()));
        }
    });
    let weak = ui.as_weak();
    ui.on_toggle_right(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_right_playing(transport::toggle_playing(ui.get_right_playing()));
        }
    });
    let weak = ui.as_weak();
    ui.on_reset_left(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_left_position(transport::cue_position());
        }
    });
    let weak = ui.as_weak();
    ui.on_reset_right(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_right_position(transport::cue_position());
        }
    });
}

#[cfg(test)]
mod ui_smoke_tests {
    use super::*;
    use i_slint_backend_testing::ElementHandle;

    #[test]
    fn play_pause_and_cue_are_connected_to_each_deck() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().expect("headless UI creation");
        wire_decks(&ui);

        fn activate(ui: &AppWindow, label: &str) {
            let controls: Vec<_> = ElementHandle::find_by_accessible_label(ui, label).collect();
            assert_eq!(
                controls.len(),
                1,
                "expected one accessible control: {label}"
            );
            controls[0].invoke_accessible_default_action();
        }

        assert!(!ui.get_left_playing());
        assert!(!ui.get_right_playing());
        activate(&ui, "DECK A play/pause");
        assert!(ui.get_left_playing());
        assert!(!ui.get_right_playing());
        activate(&ui, "DECK B play/pause");
        assert!(ui.get_right_playing());
        activate(&ui, "DECK A play/pause");
        assert!(!ui.get_left_playing());
        assert!(ui.get_right_playing());

        ui.set_left_position(55.0);
        ui.set_right_position(72.0);
        activate(&ui, "DECK A cue");
        assert_eq!(ui.get_left_position(), 0.0);
        assert_eq!(ui.get_right_position(), 72.0);
        activate(&ui, "DECK B cue");
        assert_eq!(ui.get_right_position(), 0.0);
    }
}
