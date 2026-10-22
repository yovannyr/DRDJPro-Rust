use slint::{ModelRc, Timer, TimerMode, VecModel};
use std::{rc::Rc, time::Duration};

mod transport;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    let waveform: Vec<f32> = (0..240)
        .map(|i| {
            let t = i as f32;
            let pulse = (t * 0.29).sin().abs();
            let texture = (t * 1.37).sin().abs() * 0.38;
            (0.12 + 0.62 * pulse + texture).min(1.0)
        })
        .collect();
    ui.set_waveform(ModelRc::new(Rc::new(VecModel::from(waveform))));

    wire_decks(&ui);

    let timer = Timer::default();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(33), move || {
        if let Some(ui) = weak.upgrade() {
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
            assert_eq!(controls.len(), 1, "expected one accessible control: {label}");
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
