use slint::{ModelRc, Timer, TimerMode, VecModel};
use std::{rc::Rc, time::Duration};

mod audio;
mod playback;
mod transport;

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
        if args.len() != 4 {
            eprintln!("Usage: drdjpro-ui-spike --gui <deck-a.mp3> <deck-b.mp3>");
            std::process::exit(2);
        }
        Some((args[2].clone(), args[3].clone()))
    } else {
        None
    };
    let audio_engine = if let Some((a, b)) = &gui_tracks {
        let engine = match playback::DualDeckEngine::open_default() {
            Ok(engine) => engine,
            Err(err) => {
                eprintln!("Cannot open default audio device: {err}");
                std::process::exit(1);
            }
        };
        if let Err(err) = engine.load_a(std::path::Path::new(a)) {
            eprintln!("Cannot load deck A: {err}");
            std::process::exit(1);
        }
        if let Err(err) = engine.load_b(std::path::Path::new(b)) {
            eprintln!("Cannot load deck B: {err}");
            std::process::exit(1);
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

    let waveform: Vec<f32> = (0..240)
        .map(|i| {
            let t = i as f32;
            let pulse = (t * 0.29).sin().abs();
            let texture = (t * 1.37).sin().abs() * 0.38;
            (0.12 + 0.62 * pulse + texture).min(1.0)
        })
        .collect();
    ui.set_waveform(ModelRc::new(Rc::new(VecModel::from(waveform))));

    if let Some(engine) = &audio_engine {
        wire_audio_decks(&ui, Rc::clone(engine));
    } else {
        wire_decks(&ui);
    }

    let timer = Timer::default();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(33), move || {
        if let Some(ui) = weak.upgrade() {
            if let Some(engine) = &audio_engine {
                ui.set_left_playing(!engine.is_a_paused());
                ui.set_right_playing(!engine.is_b_paused());
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

    ui.run()
}

fn wire_audio_decks(ui: &AppWindow, engine: Rc<playback::DualDeckEngine>) {
    let weak = ui.as_weak();
    let audio = Rc::clone(&engine);
    ui.on_toggle_left(move || {
        if let Some(ui) = weak.upgrade() {
            if audio.is_a_paused() {
                audio.play_a();
            } else {
                audio.pause_a();
            }
            ui.set_left_playing(!audio.is_a_paused());
        }
    });
    let weak = ui.as_weak();
    let audio = Rc::clone(&engine);
    ui.on_toggle_right(move || {
        if let Some(ui) = weak.upgrade() {
            if audio.is_b_paused() {
                audio.play_b();
            } else {
                audio.pause_b();
            }
            ui.set_right_playing(!audio.is_b_paused());
        }
    });
    let weak = ui.as_weak();
    let audio = Rc::clone(&engine);
    ui.on_reset_left(move || {
        if let Err(err) = audio.cue_a() {
            eprintln!("Deck A cue failed: {err}");
            return;
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_left_playing(false);
            ui.set_left_position(0.0);
        }
    });
    let weak = ui.as_weak();
    ui.on_reset_right(move || {
        if let Err(err) = engine.cue_b() {
            eprintln!("Deck B cue failed: {err}");
            return;
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_right_playing(false);
            ui.set_right_position(0.0);
        }
    });
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
