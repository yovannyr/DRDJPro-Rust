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
