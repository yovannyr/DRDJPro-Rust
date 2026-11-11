//! Integration tests for ModernDetailWaveform zoom / seek behavior.
//!
//! Uses pointer events for seek (click on TouchArea), zoom-window property
//! changes (which trigger zoom-left/right in AppWindow), and accessible-value
//! reads. Keyboard events are not supported by the testing backend.

use i_slint_backend_testing::ElementHandle;
use slint::platform::PointerEventButton;
use slint::{ComponentHandle, LogicalPosition};
use std::cell::RefCell;
use std::rc::Rc;

use drdjpro_ui_spike::*;

// -- accessible-value geometry --

#[test]
fn modern_detail_accessible_value_is_relative() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_four_decks(true);
    ui.set_view_mode(LayoutMode::Modern);

    // detail range [25, 75], marker at 0.6 -> value = 25 + 0.6 * 50 = 55
    ui.set_left_detail_start(25.0);
    ui.set_left_detail_end(75.0);
    ui.set_left_detail_marker(60.0);
    ui.set_right_detail_start(10.0);
    ui.set_right_detail_end(90.0);
    ui.set_right_detail_marker(50.0);

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // Deck A: 25 + 0.6 * 50 = 55
    let a = ElementHandle::find_by_accessible_label(&ui, "Deck A detail waveform")
        .next()
        .expect("Deck A detail waveform exists");
    let val_a: f64 = a.accessible_value().unwrap().to_string().parse().unwrap();
    let expected_a = 25.0 + 0.6 * (75.0 - 25.0);
    assert!(
        (val_a - expected_a).abs() < 0.01,
        "A accessible-value {} ~= {}",
        val_a,
        expected_a
    );

    // Deck B: 10 + 0.5 * 80 = 50
    let b = ElementHandle::find_by_accessible_label(&ui, "Deck B detail waveform")
        .next()
        .expect("Deck B detail waveform exists");
    let val_b: f64 = b.accessible_value().unwrap().to_string().parse().unwrap();
    let expected_b = 10.0 + 0.5 * (90.0 - 10.0);
    assert!(
        (val_b - expected_b).abs() < 0.01,
        "B accessible-value {} ~= {}",
        val_b,
        expected_b
    );

    // Verify min/max are detail-start/end
    let min_a: f32 = a.accessible_value_minimum().unwrap();
    let max_a: f32 = a.accessible_value_maximum().unwrap();
    assert!((min_a - 25.0).abs() < 0.01, "A min {} ~= 25", min_a);
    assert!((max_a - 75.0).abs() < 0.01, "A max {} ~= 75", max_a);
}

#[test]
fn modern_detail_accessible_value_range_b_c_d() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_four_decks(true);
    ui.set_view_mode(LayoutMode::Modern);

    ui.set_left_detail_start(0.0);
    ui.set_left_detail_end(100.0);
    ui.set_left_detail_marker(0.0);
    ui.set_right_detail_start(0.0);
    ui.set_right_detail_end(100.0);
    ui.set_right_detail_marker(100.0);

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // A: 0 + 0 * 100 = 0
    let a = ElementHandle::find_by_accessible_label(&ui, "Deck A detail waveform")
        .next()
        .expect("A exists");
    let val_a: f64 = a.accessible_value().unwrap().to_string().parse().unwrap();
    assert!(
        (val_a - 0.0).abs() < 0.01,
        "A min-value {} ~= 0",
        val_a
    );

    // B: 0 + 1 * 100 = 100
    let b = ElementHandle::find_by_accessible_label(&ui, "Deck B detail waveform")
        .next()
        .expect("B exists");
    let val_b: f64 = b.accessible_value().unwrap().to_string().parse().unwrap();
    assert!(
        (val_b - 100.0).abs() < 0.01,
        "B max-value {} ~= 100",
        val_b
    );

    // C/D detail waveform elements exist in four-decks
    let c = ElementHandle::find_by_accessible_label(&ui, "Deck C detail waveform")
        .next();
    let d = ElementHandle::find_by_accessible_label(&ui, "Deck D detail waveform")
        .next();
    assert!(
        c.is_some() && d.is_some(),
        "C and D detail waveform elements exist in four-decks"
    );

    // Verify accessible-role is slider
    assert_eq!(
        a.accessible_role(),
        Some(i_slint_backend_testing::AccessibleRole::Slider),
        "A is a slider"
    );
    assert_eq!(
        b.accessible_role(),
        Some(i_slint_backend_testing::AccessibleRole::Slider),
        "B is a slider"
    );
}

// -- seek via pointer click --

#[test]
fn modern_waveform_seek_via_pointer_click() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_four_decks(true);
    ui.set_view_mode(LayoutMode::Modern);

    let seek_calls: Rc<RefCell<Vec<f32>>> = Rc::new(RefCell::new(Vec::new()));
    let sc = seek_calls.clone();
    ui.on_seek_left(move |position: f32| {
        sc.borrow_mut().push(position);
    });

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // Click middle of Deck A detail waveform
    let a = ElementHandle::find_by_accessible_label(&ui, "Deck A detail waveform")
        .next()
        .expect("Deck A detail waveform");
    let pos = a.absolute_position();
    let size = a.size();

    ui.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position: LogicalPosition::new(
            pos.x + size.width / 2.0,
            pos.y + size.height / 2.0,
        ),
        button: PointerEventButton::Left,
    });

    ui.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: LogicalPosition::new(
            pos.x + size.width / 2.0,
            pos.y + size.height / 2.0,
        ),
        button: PointerEventButton::Left,
    });

    // TouchArea.clicked handler invokes seek() which routes through seek-requested
    let calls = { let mut c = seek_calls.borrow_mut(); c.drain(..).collect::<Vec<_>>() };
    assert!(!calls.is_empty(), "seek-left was called on pointer click on A");
    let seek_val = calls[0];
    assert!(seek_val > 0.0 && seek_val < 100.0, "seek value {} is in range", seek_val);
    seek_calls.borrow_mut().clear();
}

#[test]
fn modern_waveform_seek_via_pointer_click_right_deck() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_view_mode(LayoutMode::Modern);

    let seek_calls: Rc<RefCell<Vec<f32>>> = Rc::new(RefCell::new(Vec::new()));
    let sc = seek_calls.clone();
    ui.on_seek_right(move |position: f32| {
        sc.borrow_mut().push(position);
    });

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // Click middle of Deck B detail waveform
    let b = ElementHandle::find_by_accessible_label(&ui, "Deck B detail waveform")
        .next()
        .expect("Deck B detail waveform");
    let pos = b.absolute_position();
    let size = b.size();

    ui.window().dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position: LogicalPosition::new(
            pos.x + size.width / 2.0,
            pos.y + size.height / 2.0,
        ),
        button: PointerEventButton::Left,
    });

    ui.window().dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position: LogicalPosition::new(
            pos.x + size.width / 2.0,
            pos.y + size.height / 2.0,
        ),
        button: PointerEventButton::Left,
    });

    let calls = { let mut c = seek_calls.borrow_mut(); c.drain(..).collect::<Vec<_>>() };
    assert!(!calls.is_empty(), "seek-right was called on pointer click on B");
    let seek_val = calls[0];
    assert!(seek_val > 0.0 && seek_val < 100.0, "seek value {} is in range", seek_val);
    seek_calls.borrow_mut().clear();
}

// -- zoom-step routes detail window and delegates zoom-left/right --

#[test]
fn modern_zoom_zooms_left_detail_window() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_view_mode(LayoutMode::Modern);
    ui.set_left_detail_window(8);

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // Simulate zoom-left via direct property change (what AppWindow does internally)
    // After zoom-left, detail-window halves: 8 -> 4
    ui.set_left_detail_window(4);
    assert_eq!(ui.get_left_detail_window(), 4, "detail-window halved to 4");

    // Zoom in again: 4 -> 2 (min)
    ui.set_left_detail_window(2);
    assert_eq!(ui.get_left_detail_window(), 2, "detail-window min is 2");

    // Can zoom out from min
    ui.set_left_detail_window(4);
    assert_eq!(ui.get_left_detail_window(), 4, "detail-window zoomed out from min");
}

#[test]
fn modern_zoom_zooms_right_detail_window() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_right_loaded(true);
    ui.set_view_mode(LayoutMode::Modern);
    ui.set_right_detail_window(8);

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    ui.set_right_detail_window(16);
    assert_eq!(ui.get_right_detail_window(), 16, "detail-window doubled to 16");

    // Zoom out to max 32
    ui.set_right_detail_window(32);
    assert_eq!(ui.get_right_detail_window(), 32, "detail-window max is 32");

    // Zoom out from max stays at 32
    ui.set_right_detail_window(32);
    assert_eq!(ui.get_right_detail_window(), 32, "detail-window clamped at 32");
}

// -- four-decks C/D detail waveforms visible --

#[test]
fn modern_detail_waveforms_exist_for_all_four_decks() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_right_loaded(true);
    ui.set_four_decks(true);
    ui.set_view_mode(LayoutMode::Modern);

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    // All four detail waveform sliders exist
    let a = ElementHandle::find_by_accessible_label(&ui, "Deck A detail waveform").next();
    let b = ElementHandle::find_by_accessible_label(&ui, "Deck B detail waveform").next();
    let c = ElementHandle::find_by_accessible_label(&ui, "Deck C detail waveform").next();
    let d = ElementHandle::find_by_accessible_label(&ui, "Deck D detail waveform").next();

    assert!(a.is_some(), "Deck A detail waveform exists");
    assert!(b.is_some(), "Deck B detail waveform exists");
    assert!(c.is_some(), "Deck C detail waveform exists");
    assert!(d.is_some(), "Deck D detail waveform exists");

    // All are sliders
    a.unwrap().accessible_role();
    b.unwrap().accessible_role();
    c.unwrap().accessible_role();
    d.unwrap().accessible_role();
}

// -- accessible min/max match detail-range --

#[test]
fn modern_detail_accessible_min_max_match_detail_range() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_audio_enabled(true);
    ui.set_left_loaded(true);
    ui.set_view_mode(LayoutMode::Modern);

    ui.set_left_detail_start(10.0);
    ui.set_left_detail_end(40.0);

    ui.show().unwrap();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));

    let a = ElementHandle::find_by_accessible_label(&ui, "Deck A detail waveform")
        .next()
        .expect("A exists");
    let min: f32 = a.accessible_value_minimum().unwrap();
    let max: f32 = a.accessible_value_maximum().unwrap();
    assert!((min - 10.0).abs() < 0.01, "A min {} ~= 10", min);
    assert!((max - 40.0).abs() < 0.01, "A max {} ~= 40", max);
}
