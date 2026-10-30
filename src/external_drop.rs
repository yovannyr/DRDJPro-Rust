//! Windows file-manager drops share the controller's copy-only track loader.
use std::path::{Path, PathBuf};

use slint::LogicalPosition;

use crate::AppWindow;

#[derive(Default)]
pub(crate) struct FileGesture {
    count: usize,
    first: Option<PathBuf>,
    multiple: bool,
}

impl FileGesture {
    pub(crate) fn hover(&mut self, path: PathBuf) {
        if self.count == 0 {
            self.first = Some(path);
        } else {
            self.multiple = true;
        }
        self.count = self.count.saturating_add(1);
    }

    fn single(&self) -> Option<&Path> {
        (!self.multiple && self.count == 1)
            .then_some(self.first.as_deref())
            .flatten()
    }

    fn consume(&mut self, path: &Path) -> Result<(), &'static str> {
        let result = if self.multiple {
            Err("Drop exactly one track onto a deck")
        } else if self.single() == Some(path) {
            Ok(())
        } else {
            Err("File drop did not match a single hovered file; drag the file again")
        };
        self.count = self.count.saturating_sub(1);
        if self.count == 0 {
            self.clear();
        }
        result
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

fn transfer(path: &Path) -> slint::DataTransfer {
    let mut data = slint::DataTransfer::default();
    data.set_file_paths([path]);
    data
}

pub(crate) fn refresh_hover(
    ui: &AppWindow,
    gesture: &FileGesture,
    position: Option<LogicalPosition>,
) {
    let deck = gesture
        .single()
        .zip(position)
        .and_then(|(path, position)| {
            let deck = ui.invoke_external_drop_target(position.x, position.y);
            (deck >= 0 && ui.invoke_accepts_track_drop(deck, transfer(path))).then_some(deck)
        })
        .unwrap_or(-1);
    ui.set_external_drop_deck(deck);
}

pub(crate) fn drop_file(
    ui: &AppWindow,
    gesture: &mut FileGesture,
    path: &Path,
    position: Result<LogicalPosition, String>,
) -> bool {
    ui.set_external_drop_deck(-1);
    if let Err(error) = gesture.consume(path) {
        ui.set_status(error.into());
        return false;
    }
    let position = match position {
        Ok(position) => position,
        Err(error) => {
            ui.set_status(format!("Cannot locate file drop: {error}").into());
            return false;
        }
    };
    let deck = ui.invoke_external_drop_target(position.x, position.y);
    if deck < 0 {
        ui.set_status("Drop an audio file onto a deck with AUDIO, MON and FX panels closed".into());
        return false;
    }
    ui.invoke_drop_track(deck, transfer(path))
}

fn logical_position(
    cursor: (i32, i32),
    origin: (i32, i32),
    scale: f64,
) -> Result<LogicalPosition, String> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err("Invalid window DPI scale".into());
    }
    let x = (f64::from(cursor.0) - f64::from(origin.0)) / scale;
    let y = (f64::from(cursor.1) - f64::from(origin.1)) / scale;
    if !x.is_finite()
        || !y.is_finite()
        || x.abs() > f64::from(f32::MAX)
        || y.abs() > f64::from(f32::MAX)
    {
        return Err("Drop coordinates exceed the supported range".into());
    }
    Ok(LogicalPosition::new(x as f32, y as f32))
}

fn client_position(
    window: &slint::Window,
    cursor: (i32, i32),
    origin: (i32, i32),
) -> Result<LogicalPosition, String> {
    logical_position(cursor, origin, f64::from(window.scale_factor()))
}

#[cfg(target_os = "windows")]
pub(crate) fn install(ui: &AppWindow) -> slint::Timer {
    use std::{cell::RefCell, rc::Rc, time::Duration};

    use slint::{
        winit_030::{winit::event::WindowEvent, EventResult, WinitWindowAccessor},
        ComponentHandle, Timer, TimerMode,
    };

    let state = Rc::new(RefCell::new(FileGesture::default()));
    let weak = ui.as_weak();
    let events = Rc::clone(&state);
    ui.window().on_winit_window_event(move |window, event| {
        let Some(ui) = weak.upgrade() else {
            return EventResult::Propagate;
        };
        match event {
            WindowEvent::HoveredFile(path) => {
                events.borrow_mut().hover(path.clone());
                refresh_hover(&ui, &events.borrow(), cursor_position(window).ok());
            }
            WindowEvent::HoveredFileCancelled => {
                events.borrow_mut().clear();
                ui.set_external_drop_deck(-1);
            }
            WindowEvent::DroppedFile(path) => {
                drop_file(&ui, &mut events.borrow_mut(), path, cursor_position(window));
            }
            WindowEvent::Destroyed | WindowEvent::CloseRequested | WindowEvent::Occluded(true) => {
                events.borrow_mut().clear();
                ui.set_external_drop_deck(-1);
                return EventResult::Propagate;
            }
            _ => return EventResult::Propagate,
        }
        EventResult::PreventDefault
    });
    let timer = Timer::default();
    let weak = ui.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(33), move || {
        if let Some(ui) = weak.upgrade() {
            let state = state.borrow();
            if state.single().is_some() {
                refresh_hover(&ui, &state, cursor_position(ui.window()).ok());
            } else {
                ui.set_external_drop_deck(-1);
            }
        }
    });
    timer
}

#[cfg(target_os = "windows")]
fn cursor_position(window: &slint::Window) -> Result<LogicalPosition, String> {
    use slint::winit_030::WinitWindowAccessor;
    use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

    window
        .with_winit_window(|native| {
            let mut cursor = POINT::default();
            // Slint's effective scale may override native DPI via SLINT_SCALE_FACTOR.
            unsafe { GetCursorPos(&mut cursor) }.map_err(|error| error.to_string())?;
            let origin = native.inner_position().map_err(|error| error.to_string())?;
            client_position(window, (cursor.x, cursor.y), (origin.x, origin.y))
        })
        .ok_or_else(|| "Native Winit window unavailable".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::ComponentHandle;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn gestures_reject_every_file_of_multiple_drop_and_recover() {
        let a = PathBuf::from(r"C:\music\a.wav");
        let b = PathBuf::from(r"C:\music\b.wav");
        let mut state = FileGesture::default();
        assert!(state.consume(&a).is_err());
        state.hover(a.clone());
        assert_eq!(state.single(), Some(a.as_path()));
        assert!(state.consume(&b).is_err());
        assert!(state.single().is_none());
        for paths in [[&a, &b], [&a, &a]] {
            for path in paths {
                state.hover(path.clone());
            }
            assert!(state.single().is_none());
            assert!(state.consume(paths[0]).is_err());
            assert!(state.consume(paths[1]).is_err());
            assert_eq!(state.count, 0);
        }
        state.hover(a.clone());
        state.hover(b.clone());
        state.clear();
        state.hover(b.clone());
        assert!(state.consume(&b).is_ok());
        state.hover(a.clone());
        assert!(state.consume(&a).is_ok());
    }

    #[test]
    fn coordinates_use_client_origin_and_current_dpi_without_integer_overflow() {
        for scale in [1.0, 1.5, 2.0] {
            let pos = logical_position((-900, 600), (-1200, 150), scale).unwrap();
            assert_eq!(pos.x, (300.0 / scale) as f32);
            assert_eq!(pos.y, (450.0 / scale) as f32);
        }
        assert!(logical_position((i32::MAX, 0), (i32::MIN, 0), 1.0).is_ok());
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MIN_POSITIVE] {
            assert!(logical_position((100, 100), (0, 0), scale).is_err());
        }
    }

    #[test]
    fn client_coordinates_use_slint_scale_even_when_native_dpi_differs() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        ui.set_audio_enabled(true);
        ui.show().unwrap();
        for scale in [1.0, 1.5, 2.0] {
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale,
                });
            ui.window().set_size(slint::LogicalSize::new(1280., 860.));
            let button = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "DECK B load track",
            )
            .next()
            .unwrap();
            let target = button.absolute_position();
            let origin = (-1500, 100);
            let cursor = (
                origin.0 + ((target.x + 5.) * scale) as i32,
                origin.1 + ((target.y + 5.) * scale) as i32,
            );
            let logical = client_position(ui.window(), cursor, origin).unwrap();
            assert!((logical.x - target.x - 5.).abs() <= 1.);
            assert!((logical.y - target.y - 5.).abs() <= 1.);
            assert_eq!(ui.invoke_external_drop_target(logical.x, logical.y), 1);
        }
        ui.hide().unwrap();
    }

    #[test]
    fn deck_geometry_and_external_dispatch_respect_panels_and_revalidate() {
        i_slint_backend_testing::init_no_event_loop();
        for (width, height) in [(850., 740.), (1280., 860.)] {
            let ui = AppWindow::new().unwrap();
            ui.window().set_size(slint::LogicalSize::new(width, height));
            ui.set_audio_enabled(true);
            let received = Rc::new(RefCell::new(Vec::new()));
            ui.on_accepts_track_drop(|_, _| true);
            ui.show().unwrap();
            let path = PathBuf::from(r"C:\music\Grüße #1.wav");
            let mut gesture = FileGesture::default();
            for (deck, label) in [(0, "DECK A load track"), (1, "DECK B load track")] {
                let sink = Rc::clone(&received);
                ui.on_drop_track(move |deck, data| {
                    sink.borrow_mut()
                        .push((deck, data.file_paths().unwrap().next().unwrap().to_owned()));
                    true
                });
                let button =
                    i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, label)
                        .next()
                        .unwrap();
                let origin = button.absolute_position();
                let position = LogicalPosition::new(origin.x + 5., origin.y + 5.);
                assert_eq!(ui.invoke_external_drop_target(position.x, position.y), deck);
                gesture.hover(path.clone());
                refresh_hover(&ui, &gesture, Some(position));
                assert_eq!(ui.get_external_drop_deck(), deck);
                assert!(drop_file(&ui, &mut gesture, &path, Ok(position)));
                assert_eq!(ui.get_external_drop_deck(), -1);
                assert_eq!(received.borrow().last(), Some(&(deck, path.clone())));
                for panel in 0..3 {
                    ui.set_audio_device_open(panel == 0);
                    ui.set_monitor_open(panel == 1);
                    ui.set_fx_open(panel == 2);
                    assert_eq!(ui.invoke_external_drop_target(position.x, position.y), -1);
                }
                ui.set_fx_open(false);
                gesture.hover(path.clone());
                ui.on_accepts_track_drop(|_, _| false);
                refresh_hover(&ui, &gesture, Some(position));
                assert_eq!(ui.get_external_drop_deck(), -1);
                // A previously accepted hover must not bypass the actual drop callback.
                ui.on_drop_track(|_, _| false);
                assert!(!drop_file(&ui, &mut gesture, &path, Ok(position)));
                ui.on_accepts_track_drop(|_, _| true);
            }
            for (x, y) in [
                (-1., 100.),
                (20., 15.),
                (20., height - 60.),
                (width + 1., 100.),
            ] {
                assert_eq!(ui.invoke_external_drop_target(x, y), -1);
            }
            gesture.hover(path.clone());
            assert!(!drop_file(
                &ui,
                &mut gesture,
                &path,
                Err("Injected cursor failure".into())
            ));
            assert!(ui.get_status().contains("Injected cursor failure"));
            ui.hide().unwrap();
        }
    }
}
