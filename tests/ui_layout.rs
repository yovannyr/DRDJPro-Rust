//! Production Slint contract tests, without devices or persistent user data.
use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};

use drdjpro_ui_spike::*;

fn click(ui: &AppWindow, label: &str) {
    let controls: Vec<_> = ElementHandle::find_by_accessible_label(ui, label)
        .filter(|item| {
            item.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)
        })
        .collect();
    assert_eq!(controls.len(), 1, "unique action: {label}");
    controls[0].invoke_accessible_default_action();
}

#[test]
fn layout_modes_preserve_deck_and_library_state_and_route_independently() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_left_title("A: supplied controller metadata".into());
    ui.set_right_title("B: supplied controller metadata".into());
    ui.set_left_position(23.5);
    ui.set_right_position(72.0);
    ui.set_left_pitch_percent(-3.0);
    ui.set_right_pitch_percent(6.0);
    ui.set_selected_track(17);
    ui.set_library_query("supplied query".into());
    ui.set_left_playing(true);
    ui.set_left_sync_active(true);
    ui.set_left_keylock(true);
    let a = Rc::new(Cell::new(0));
    let b = Rc::new(Cell::new(0));
    let count = a.clone();
    ui.on_toggle_left(move || count.set(count.get() + 1));
    let count = b.clone();
    ui.on_toggle_right(move || count.set(count.get() + 1));
    ui.show().unwrap();
    for _ in 0..3 {
        for (label, slots) in [("4 SLOTS", 4), ("2 DECKS", 2)] {
            click(&ui, label);
            assert_eq!(ui.get_visible_layout_slots(), slots);
            assert_eq!(ui.get_supported_audio_deck_count(), 2);
            assert_eq!(ui.get_left_position(), 23.5);
            assert_eq!(ui.get_right_position(), 72.0);
            assert_eq!(ui.get_selected_track(), 17);
            assert_eq!(ui.get_library_query(), "supplied query");
            assert_eq!(ui.get_left_pitch_percent(), -3.0);
            assert_eq!(ui.get_right_pitch_percent(), 6.0);
            assert!(ui.get_left_playing());
            assert!(ui.get_left_sync_active());
            assert!(ui.get_left_keylock());
            assert_eq!(ui.get_right_title(), "B: supplied controller metadata");
            assert!(ui.get_deck_c_data().title.is_empty());
            assert!(ui.get_deck_d_data().title.is_empty());
        }
    }
    click(&ui, "DECK A play/pause");
    assert_eq!((a.get(), b.get()), (1, 0));
    click(&ui, "DECK B play/pause");
    assert_eq!((a.get(), b.get()), (1, 1));
    // Controls dispatch requests; they never optimistically change engine state.
    assert!(ui.get_left_playing());
    assert!(!ui.get_right_playing());
}

#[test]
fn production_layout_geometry_keeps_transport_and_waveforms_reachable() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_mc4000_connected(true);
    ui.show().unwrap();
    for (four, width, height) in [
        (false, 1280., 860.),
        (false, 850., 860.),
        (true, 850., 1020.),
    ] {
        ui.set_four_decks(four);
        ui.window().set_size(slint::LogicalSize::new(width, height));
        ui.set_deck_view_index(1);
        for label in [
            "DECK A play/pause",
            "DECK B play/pause",
            "Shared waveform area",
            "Track library area",
        ] {
            let item = ElementHandle::find_by_accessible_label(&ui, label)
                .next()
                .unwrap();
            let pos = item.absolute_position();
            let size = item.size();
            assert!(size.width > 0. && size.height > 0., "{label}");
            assert!(pos.x >= 0. && pos.y >= 0., "{label}");
            assert!(pos.x + size.width <= width + 1., "{label} width");
            assert!(pos.y + size.height <= height + 1., "{label} height");
        }
        let wave = ElementHandle::find_by_accessible_label(&ui, "Shared waveform area")
            .next()
            .unwrap();
        for deck in ["DECK A", "DECK B"] {
            for suffix in [
                "virtual deck source time",
                "virtual deck BPM",
                "virtual deck pitch",
            ] {
                let label = format!("{deck} {suffix}");
                let item = ElementHandle::find_by_accessible_label(&ui, &label)
                    .next()
                    .unwrap();
                let pos = item.absolute_position();
                let size = item.size();
                assert!(
                    pos.y + size.height <= wave.absolute_position().y,
                    "{label} overlaps waveforms"
                );
            }
        }
    }
}

#[test]
fn unsupported_slots_have_no_transport_routes_and_editors_stay_reachable() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_four_decks(true);
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.show().unwrap();
    for deck in ["DECK C", "DECK D"] {
        assert!(
            ElementHandle::find_by_accessible_label(&ui, &format!("{deck} play/pause"))
                .next()
                .is_none()
        );
        assert!(
            ElementHandle::find_by_accessible_label(&ui, &format!("{deck} load track"))
                .next()
                .is_none()
        );
    }
    // Traverse the real tools action, including the new cue-editor page.
    for _ in 0..5 {
        click(&ui, "DECK A deck tools panel");
    }
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "DECK A edit hot cue")
            .next()
            .is_some()
    );
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "DECK A stutter")
            .next()
            .is_some()
    );
    ui.set_left_cue_editing(true);
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "DECK A cue name")
            .next()
            .is_some()
    );
    click(&ui, "2 DECKS");
    assert!(ui.get_left_cue_editing());
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "DECK A cue name")
            .next()
            .is_some()
    );
}

#[test]
fn modern_platter_accessible_data_binding() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_four_decks(true);
    ui.set_view_mode(LayoutMode::Modern);
    ui.show().unwrap();
    ui.set_left_platter_bpm("128.4".into());
    ui.set_right_platter_bpm("120.2".into());
    ui.set_left_platter_time("03:24".into());
    ui.set_right_platter_time("02:50".into());
    ui.set_left_platter_angle(45.0);
    ui.set_right_platter_angle(-30.0);
    ui.set_left_pitch_percent(5.5);
    ui.set_right_pitch_percent(-2.3);
    ui.set_left_position(25.0);
    ui.set_right_position(75.0);
    ui.set_left_playing(true);
    ui.set_right_playing(false);
    let a = ElementHandle::find_by_accessible_label(&ui, "Deck A modern platter").next();
    let b = ElementHandle::find_by_accessible_label(&ui, "Deck B modern platter").next();
    assert!(a.is_some(), "A");
    assert!(b.is_some(), "B");
    if let Some(ref item) = a {
        let desc = item.accessible_description().unwrap_or_default();
        let desc: &str = desc.as_str();
        assert!(desc.contains("128.4"), "A bpm");
        assert!(desc.contains("03:24"), "A time");
        assert!(desc.contains("pitch +5.5%"), "A pitch");
        assert!(desc.contains("25%"), "A pos");
        assert!(desc.contains("45"), "A angle");
        assert!(desc.contains("Playing"), "A play");
    }
    if let Some(ref item) = b {
        let desc = item.accessible_description().unwrap_or_default();
        let desc: &str = desc.as_str();
        assert!(desc.contains("120.2"), "B bpm");
        assert!(desc.contains("02:50"), "B time");
        assert!(desc.contains("-2.3%"), "B pitch");
        assert!(desc.contains("75%"), "B pos");
        assert!(desc.contains("-30"), "B angle");
        assert!(desc.contains("Paused"), "B paused");
    }
    ui.set_left_playing(false);
    if let Some(ref item) = a {
        let desc = item.accessible_description().unwrap_or_default();
        let desc: &str = desc.as_str();
        assert!(desc.contains("Paused"), "pause");
    }

    // C/D: platters exist in four-decks mode; in-only props, no Rust setters
    let c_item = ElementHandle::find_by_accessible_label(&ui, "Deck C modern platter").next();
    let d_item = ElementHandle::find_by_accessible_label(&ui, "Deck D modern platter").next();
    assert!(c_item.is_some(), "C platter exists in four-decks");
    assert!(d_item.is_some(), "D platter exists in four-decks");
}

#[test]
fn modern_scratch_releases_on_right_button_up_and_focus_loss() {
    use slint::platform::{PointerEventButton, WindowEvent};
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_view_mode(LayoutMode::Modern);
    ui.show().unwrap();
    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));

    let begin_count = Rc::new(Cell::new(0));
    let release_count = Rc::new(Cell::new(0));
    let begin_count_r = begin_count.clone();
    let release_count_r = release_count.clone();
    ui.on_virtual_platter_pointer(move |_deck: i32, action: i32, _x: f32, _y: f32| {
        if action == 0 {
            begin_count_r.set(begin_count_r.get() + 1);
        } else if action == 2 {
            release_count_r.set(release_count_r.get() + 1);
        }
    });

    let platter_a = ElementHandle::find_by_accessible_label(&ui, "Deck A modern platter")
        .next()
        .expect("Deck A modern platter exists");
    let origin = platter_a.absolute_position();
    let size = platter_a.size();
    let center =
        slint::LogicalPosition::new(origin.x + size.width / 2.0, origin.y + size.height / 2.0);
    assert_eq!(begin_count.get(), 0);
    assert_eq!(release_count.get(), 0);

    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: center,
        button: PointerEventButton::Left,
    });
    assert_eq!(begin_count.get(), 1, "begin on left-button down");

    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position: center,
        button: PointerEventButton::Right,
    });
    assert_eq!(release_count.get(), 1, "release on right-button up");
    assert_eq!(begin_count.get(), 1, "no double begin");

    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position: center,
        button: PointerEventButton::Left,
    });
    assert_eq!(begin_count.get(), 2, "second begin");
    assert_eq!(release_count.get(), 1, "still waiting for release");

    ui.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(false));
    assert_eq!(release_count.get(), 2, "release on focus loss");
    assert_eq!(begin_count.get(), 2, "no double begin on second attempt");
}
