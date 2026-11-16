use super::super::{manager, CrashReport};
use super::*;
use slint::Model;
use std::sync::mpsc::{Receiver, SyncSender};

fn report(id: &str, state: ReportStatus) -> CrashReport {
    CrashReport {
        id: id.into(),
        created_ms: 1_000,
        version: "test".into(),
        os: "testOS".into(),
        arch: "testCPU".into(),
        stack: "app::frame\n  at <redacted>/frame.rs:42".into(),
        status: state,
    }
}

fn event(id: u64, reports: Vec<CrashReport>, prompt: bool) -> Event {
    Event {
        id,
        snapshot: Some(Snapshot {
            reports,
            settings: CrashSettings {
                auto_submit: false,
                server_url: "https://example.invalid/reports".into(),
            },
            settings_issue: String::new(),
            endpoint_ready: true,
        }),
        message: "Ready".into(),
        settings_changed: true,
        upload: false,
        prompt,
    }
}

fn setup(ui: &AppWindow) -> (CrashUiSession, Receiver<Request>, SyncSender<Event>) {
    let (worker, requests, events) = manager::testing_pair();
    let session = wire_with(
        ui,
        None,
        Some(worker),
        String::new(),
        Rc::new(|_| panic!("No upload authorized in this UI fixture")),
    );
    (session, requests, events)
}

#[test]
fn pending_post_crash_prompt_close_and_selected_consent_are_explicit() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.set_left_playing(true);
    ui.set_left_position(42.);
    let (session, requests, events) = setup(&ui);
    events
        .send(event(
            0,
            vec![
                report("first", ReportStatus::Pending),
                report("second", ReportStatus::Declined),
            ],
            true,
        ))
        .unwrap();
    tick(&session.state);
    let view = session.state.borrow().view.as_ref().unwrap().clone_strong();
    assert!(view.window().is_visible());
    assert!(view.get_post_crash());
    assert_eq!(view.get_rows().row_count(), 2);
    assert!(view.get_details().contains("frame.rs:42"));
    assert!(requests.try_recv().is_err());
    view.invoke_dismiss();
    assert!(!view.window().is_visible());
    assert!(requests.try_recv().is_err()); // Closing is neither opt-in nor decline.
    ui.invoke_show_crash_reports();
    let refresh = requests.try_recv().unwrap();
    events
        .send(event(
            refresh.id,
            vec![
                report("first", ReportStatus::Pending),
                report("second", ReportStatus::Declined),
            ],
            false,
        ))
        .unwrap();
    tick(&session.state);
    view.set_selected_row(1);
    view.invoke_selected();
    view.invoke_keep_local();
    let keep = requests.try_recv().unwrap();
    assert!(matches!(keep.action, Action::KeepLocal(ref id) if id == "second"));
    view.invoke_send_selected();
    assert!(requests.try_recv().is_err()); // Pending filesystem command blocks reentry.
    events
        .send(event(
            keep.id,
            vec![
                report("first", ReportStatus::Pending),
                report("second", ReportStatus::Declined),
            ],
            false,
        ))
        .unwrap();
    tick(&session.state);
    view.invoke_send_selected();
    let send = requests.try_recv().unwrap();
    assert!(matches!(send.action, Action::Send(ref id) if id == "second"));
    assert!(ui.get_left_playing());
    assert_eq!(ui.get_left_position(), 42.);
    session.shutdown();
    assert!(!view.window().is_visible());
    assert!(session.state.borrow().worker.is_none());
    ui.invoke_show_crash_reports();
    assert!(!view.window().is_visible());
}

#[test]
fn settings_are_explicit_selection_uses_ids_and_stale_replies_do_not_unlock() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    let (session, requests, events) = setup(&ui);
    events
        .send(event(
            0,
            vec![
                report("a", ReportStatus::Pending),
                report("b", ReportStatus::Pending),
            ],
            true,
        ))
        .unwrap();
    tick(&session.state);
    let view = session.state.borrow().view.as_ref().unwrap().clone_strong();
    view.set_selected_row(1);
    view.invoke_selected();
    view.set_endpoint("https://saved.invalid/new".into());
    view.set_automatic(true);
    assert!(requests.try_recv().is_err());
    view.invoke_save_settings();
    let save = requests.try_recv().unwrap();
    assert!(
        matches!(save.action, Action::Save(ref s) if s.auto_submit && s.server_url == "https://saved.invalid/new")
    );
    events.send(event(99, vec![], false)).unwrap();
    tick(&session.state);
    assert!(view.get_busy());
    assert_eq!(view.get_rows().row_count(), 2);
    let mut reply = event(
        save.id,
        vec![
            report("b", ReportStatus::Submitted),
            report("a", ReportStatus::Pending),
        ],
        false,
    );
    reply.snapshot.as_mut().unwrap().settings.server_url = "https://saved.invalid/new".into();
    reply.snapshot.as_mut().unwrap().settings.auto_submit = true;
    events.send(reply).unwrap();
    tick(&session.state);
    assert!(!view.get_busy());
    assert_eq!(view.get_selected_row(), 0);
    assert!(view.get_selected_submitted());
    view.invoke_send_selected();
    view.invoke_keep_local();
    assert!(requests.try_recv().is_err());
    view.invoke_delete_selected();
    assert!(matches!(requests.try_recv().unwrap().action, Action::Delete(ref id) if id == "b"));
}

#[test]
fn store_refresh_failure_preserves_known_delivery_and_disables_mutations() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    let (session, requests, events) = setup(&ui);
    events
        .send(event(0, vec![report("a", ReportStatus::Submitted)], false))
        .unwrap();
    tick(&session.state);
    ui.invoke_show_crash_reports();
    let refresh = requests.try_recv().unwrap();
    session.state.borrow_mut().delivery_status = "Server-confirmed: 1".into();
    events
        .send(Event {
            id: refresh.id,
            snapshot: None,
            message: "Crash report store unavailable; delivery state could not be refreshed".into(),
            settings_changed: false,
            upload: false,
            prompt: false,
        })
        .unwrap();
    tick(&session.state);
    let view = session.state.borrow().view.as_ref().unwrap().clone_strong();
    assert!(view.get_delivery_status().contains("Server-confirmed: 1"));
    assert!(!view.get_status().contains("no data was sent"));
    assert!(!view.get_available());
    view.invoke_delete_selected();
    assert!(requests.try_recv().is_err());
    view.invoke_refresh();
    assert!(matches!(
        requests.try_recv().unwrap().action,
        Action::Refresh(_)
    ));
}

#[test]
fn unavailable_startup_keeps_manager_accessible_without_sharing() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    let session = wire_with(
        &ui,
        None,
        None,
        "Crash capture unavailable".into(),
        Rc::new(|_| panic!("must not send")),
    );
    ui.invoke_show_crash_reports();
    let view = session.state.borrow().view.as_ref().unwrap().clone_strong();
    assert!(!view.get_available());
    assert!(!view.get_busy());
    assert!(view.get_capture_notice().contains("unavailable"));
    view.invoke_send_selected();
    view.invoke_save_settings();
    view.invoke_retry();
    ui.invoke_close_crash_reports();
    assert!(!view.window().is_visible());
}
