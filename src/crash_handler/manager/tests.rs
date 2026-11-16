use super::*;

fn next(worker: &Worker) -> Event {
    worker.events.recv_timeout(Duration::from_secs(15)).unwrap()
}
fn request(worker: &Worker, id: u64, action: Action) -> Event {
    assert!(worker.send(Request { id, action }).is_ok());
    let reply = next(worker);
    assert_eq!(reply.id, id);
    reply
}

#[test]
fn startup_only_requests_previously_authorized_uploads_and_prompts_pending() {
    for (state, automatic, expected_upload, expected_prompt) in [
        (ReportStatus::Pending, false, false, true),
        (ReportStatus::Queued, false, true, false),
        (ReportStatus::Declined, true, false, false),
        (ReportStatus::Submitted, true, false, false),
        (ReportStatus::Pending, true, true, false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = CrashStore::open(temp.path().into()).unwrap();
        let report = store.insert("frame").unwrap();
        store.set_status(&report.id, state).unwrap();
        store
            .save_settings(&CrashSettings {
                auto_submit: automatic,
                server_url: "https://example.invalid/reports".into(),
            })
            .unwrap();
        let worker = Worker::spawn(store.clone()).unwrap();
        let event = next(&worker);
        assert_eq!(event.id, 0);
        assert_eq!(event.upload, expected_upload);
        assert_eq!(event.prompt, expected_prompt);
        assert_eq!(store.list().unwrap()[0].status, state); // Actor itself sends nothing.
    }
}

#[test]
fn explicit_actions_persist_consent_settings_and_delete_only_owned_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let store = CrashStore::open(temp.path().into()).unwrap();
    let report = store.insert("frame\n at /home/private/file.rs:4").unwrap();
    let dump = store.root().join(format!("{}.dmp", report.id));
    std::fs::write(&dump, b"local dump").unwrap();
    std::fs::write(store.root().join("unrelated.txt"), b"keep").unwrap();
    let worker = Worker::spawn(store.clone()).unwrap();
    let initial = next(&worker);
    assert!(initial.prompt);
    assert!(!initial.upload);
    assert!(!request(&worker, 1, Action::Send(report.id.clone())).upload);
    assert_eq!(store.list().unwrap()[0].status, ReportStatus::Pending);
    let settings = CrashSettings {
        auto_submit: false,
        server_url: "https://example.invalid/reports".into(),
    };
    assert!(request(&worker, 2, Action::Save(settings.clone())).settings_changed);
    assert_eq!(store.settings().unwrap(), settings);
    assert!(request(&worker, 3, Action::Send(report.id.clone())).upload);
    assert_eq!(store.list().unwrap()[0].status, ReportStatus::Queued);
    assert!(!request(&worker, 4, Action::KeepLocal(report.id.clone())).upload);
    assert_eq!(store.list().unwrap()[0].status, ReportStatus::Declined);
    assert_eq!(std::fs::read(&dump).unwrap(), b"local dump");
    assert!(
        !request(
            &worker,
            5,
            Action::Save(CrashSettings {
                auto_submit: true,
                ..settings
            })
        )
        .upload
    );
    assert!(request(&worker, 6, Action::Delete(report.id.clone()))
        .snapshot
        .unwrap()
        .reports
        .is_empty());
    assert!(!dump.exists());
    assert_eq!(
        std::fs::read(store.root().join("unrelated.txt")).unwrap(),
        b"keep"
    );
}

#[test]
fn invalid_and_corrupt_settings_fail_closed_and_never_fake_delivery() {
    let temp = tempfile::tempdir().unwrap();
    let store = CrashStore::open(temp.path().into()).unwrap();
    let report = store.insert("frame").unwrap();
    store.queue_report(&report.id).unwrap();
    std::fs::write(store.root().join("settings.json"), b"corrupt").unwrap();
    let worker = Worker::spawn(store.clone()).unwrap();
    let event = next(&worker);
    assert!(!event.upload);
    let data = event.snapshot.unwrap();
    assert!(!data.endpoint_ready);
    assert!(!data.settings.auto_submit);
    assert!(!data.settings_issue.is_empty());
    for (id, url) in [
        "",
        "http://localhost/",
        "https://user:pass@example.invalid/",
        "https://example.invalid/?secret=1",
    ]
    .into_iter()
    .enumerate()
    {
        let reply = request(
            &worker,
            id as u64 + 1,
            Action::Save(CrashSettings {
                auto_submit: true,
                server_url: url.into(),
            }),
        );
        assert!(!reply.upload);
        assert!(!reply.settings_changed);
        assert!(store.settings().is_err());
    }
    assert_eq!(store.list().unwrap()[0].status, ReportStatus::Queued);
    std::fs::write(store.root().join(format!("{}.json", report.id)), b"broken").unwrap();
    let failed = request(&worker, 9, Action::Send(report.id));
    assert!(!failed.message.contains("no data was sent"));
    assert!(!failed.upload);
}

#[test]
fn closed_result_channel_does_not_leave_filesystem_worker_alive() {
    let temp = tempfile::tempdir().unwrap();
    let store = CrashStore::open(temp.path().into()).unwrap();
    let worker = Worker::spawn(store).unwrap();
    let sender = worker.commands.clone();
    drop(worker);
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        match sender.try_send(Request {
            id: 1,
            action: Action::Retry,
        }) {
            Err(TrySendError::Disconnected(_)) => break,
            _ => {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
