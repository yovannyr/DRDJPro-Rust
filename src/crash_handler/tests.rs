use super::{privacy::sanitize_stack, *};
use std::{collections::HashSet, fs};

fn store() -> (tempfile::TempDir, CrashStore) {
    let temp = tempfile::tempdir().unwrap();
    let store = CrashStore::open(temp.path().join("crashes")).unwrap();
    (temp, store)
}

#[test]
fn consent_states_and_settings_survive_restart() {
    let (_temp, store) = store();
    assert!(!store.settings().unwrap().auto_submit);
    for state in [
        ReportStatus::Pending,
        ReportStatus::Declined,
        ReportStatus::Queued,
        ReportStatus::Submitted,
    ] {
        let report = store.insert("app::frame").unwrap();
        store.set_status(&report.id, state).unwrap();
        let reopened = CrashStore::open(store.root().to_owned()).unwrap();
        assert_eq!(
            reopened
                .list()
                .unwrap()
                .iter()
                .find(|r| r.id == report.id)
                .unwrap()
                .status,
            state
        );
    }
    let settings = CrashSettings {
        auto_submit: true,
        server_url: "https://example.invalid/reports".into(),
    };
    store.save_settings(&settings).unwrap();
    assert_eq!(
        CrashStore::open(store.root().into())
            .unwrap()
            .settings()
            .unwrap(),
        settings
    );
    assert_eq!(store.list().unwrap().len(), 4);
}

#[test]
fn captures_are_unique_under_concurrent_writers() {
    let (_temp, store) = store();
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let copy = store.clone();
            std::thread::spawn(move || copy.insert("app::frame").unwrap().id)
        })
        .collect();
    let ids: HashSet<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(ids.len(), 16);
    assert_eq!(store.list().unwrap().len(), 16);
    assert!(ids.iter().all(|id| id.len() == 32 && !id.contains(':')));
}

#[test]
fn source_paths_are_redacted_before_persisting_and_display() {
    let (_temp, store) = store();
    let input = "app::frame\n  at C:\\Users\\José Doe\\project\\src/main.rs:10:2\n  at /home/a b/project/src/main.rs:20:1\n  at \\\\server\\share\\private\\other.rs:3:4\npayload /home/private/a b.mp3";
    let report = store.insert(input).unwrap();
    let json = fs::read_to_string(store.root().join(format!("{}.json", report.id))).unwrap();
    for private in ["José", "a b", "Users", "server", "private"] {
        assert!(!json.contains(private));
    }
    assert!(report.stack.contains("app::frame\n"));
    assert!(report.stack.contains("main.rs:10:2"));
    assert!(report.stack.contains("main.rs:20:1"));
    assert!(report.stack.contains("other.rs:3:4"));
    assert_eq!(store.list().unwrap()[0].stack, report.stack);
    assert_eq!(
        sanitize_stack("function_without_path"),
        "function_without_path"
    );
}

#[test]
fn malformed_metadata_and_interrupted_temps_never_become_reports() {
    let (_temp, store) = store();
    let report = store.insert("good").unwrap();
    fs::write(
        store.root().join(format!("{}.json", "a".repeat(32))),
        "{broken",
    )
    .unwrap();
    fs::write(store.root().join("crash_queue.json"), "{}").unwrap();
    fs::write(store.root().join(".tmp-interrupted"), "{partial").unwrap();
    let reports = CrashStore::open(store.root().into())
        .unwrap()
        .list()
        .unwrap();
    assert_eq!(reports, vec![report]);
}

#[test]
fn mismatched_ids_and_oversized_metadata_are_not_trusted() {
    let (_temp, store) = store();
    let report = store.insert("frame").unwrap();
    let other = store.root().join(format!("{}.json", "a".repeat(32)));
    fs::copy(store.root().join(format!("{}.json", report.id)), &other).unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
    assert!(store
        .set_status(&"a".repeat(32), ReportStatus::Queued)
        .is_err());
    fs::write(other, vec![b' '; 1_048_577]).unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn traversal_and_absolute_paths_cannot_delete_external_files() {
    let (temp, store) = store();
    let outside = temp.path().join("outside.txt");
    fs::write(&outside, b"preserve").unwrap();
    for id in ["../outside", "", "C:\\outside", "/outside", "settings"] {
        assert!(store.delete(id).is_err());
        assert!(store.set_status(id, ReportStatus::Queued).is_err());
    }
    assert_eq!(fs::read(&outside).unwrap(), b"preserve");
    assert!(CrashStore::open(std::path::PathBuf::new()).is_err());
}

#[test]
fn count_retention_removes_owned_files_but_preserves_unmanaged_files() {
    let (_temp, mut store) = store();
    store.max_reports = 2;
    let first = store.insert("first").unwrap();
    fs::write(store.root().join(format!("{}.dmp", first.id)), b"dump").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(store.root().join("keep.json"), "keep").unwrap();
    store.insert("second").unwrap();
    store.insert("third").unwrap();
    assert_eq!(store.list().unwrap().len(), 2);
    assert!(!store.root().join(format!("{}.dmp", first.id)).exists());
    assert!(!store.root().join(format!("{}.json", first.id)).exists());
    assert_eq!(
        fs::read_to_string(store.root().join("keep.json")).unwrap(),
        "keep"
    );
}

#[test]
fn byte_retention_counts_orphan_and_oversized_dumps() {
    let (_temp, mut store) = store();
    store.max_bytes = 1024;
    let orphan = store.root().join(format!("{}.dmp", "b".repeat(32)));
    fs::write(&orphan, vec![0; 1025]).unwrap();
    store.enforce_limits().unwrap();
    assert!(!orphan.exists());
    let report = store.insert("frame").unwrap();
    let path = store.root().join(format!("{}.dmp", report.id));
    fs::write(&path, [0; 1025]).unwrap();
    store.enforce_limits().unwrap();
    assert!(!path.exists());
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn failed_dump_deletion_preserves_report_and_surfaces_error() {
    let (_temp, store) = store();
    let report = store.insert("frame").unwrap();
    fs::create_dir(store.root().join(format!("{}.dmp", report.id))).unwrap();
    assert!(store.delete(&report.id).is_err());
    assert_eq!(store.list().unwrap(), vec![report]);
}

#[test]
fn no_dump_is_faked_on_text_capture() {
    let (_temp, store) = store();
    let report = store.insert("frame").unwrap();
    assert!(!store.root().join(format!("{}.dmp", report.id)).exists());
    store.delete(&report.id).unwrap();
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn interrupted_owned_temps_are_recovered_and_unknown_files_preserved() {
    let (_temp, store) = store();
    let report = store.insert("frame").unwrap();
    for index in 0..8 {
        fs::write(
            store.root().join(format!("crash-write-{index:016}.tmp")),
            [0; 1024],
        )
        .unwrap();
    }
    let unknown = store.root().join("other-application.tmp");
    fs::write(&unknown, b"preserve").unwrap();
    let reopened = CrashStore::open(store.root().into()).unwrap();
    assert_eq!(reopened.list().unwrap(), vec![report]);
    assert_eq!(fs::read(unknown).unwrap(), b"preserve");
    assert!(!fs::read_dir(store.root()).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("crash-write-")));
}

#[test]
fn corrupt_consent_fails_closed() {
    let (_temp, store) = store();
    fs::write(store.root().join("settings.json"), "{broken").unwrap();
    assert!(store.settings().is_err());
    let location = default_database_dir().unwrap();
    assert!(location.is_absolute());
    assert!(location.ends_with("DRDJPro-Rust/crashes"));
}

#[test]
fn capture_transaction_blocks_other_writers_and_cleans_up_before_unlock() {
    use std::io::Write;
    let (_temp, store) = store();
    let capture = store.begin_capture("frame").unwrap();
    let (mut writer, path) = capture.writer().unwrap();
    writer.write_all(b"partial").unwrap();
    drop(writer);
    let copy = store.clone();
    let (started, ready) = std::sync::mpsc::channel();
    let (done, received) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        started.send(()).unwrap();
        copy.insert("another frame").unwrap();
        done.send(()).unwrap();
    });
    ready.recv().unwrap();
    assert!(received
        .recv_timeout(std::time::Duration::from_millis(40))
        .is_err());
    drop(capture);
    received
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    thread.join().unwrap();
    assert!(!path.exists());
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn oversized_capture_is_not_finalized() {
    let (_temp, store) = store();
    let capture = store.begin_capture("frame").unwrap();
    let (writer, path) = capture.writer().unwrap();
    writer.set_len(10_000_001).unwrap();
    drop(writer);
    assert!(capture.commit().is_err());
    assert!(store.list().unwrap().is_empty());
    assert!(!path.exists());
}

#[cfg(unix)]
#[test]
fn symlink_root_and_attachments_are_never_followed() {
    use std::os::unix::fs::symlink;
    let (temp, store) = store();
    let outside = temp.path().join("outside");
    fs::write(&outside, "preserve").unwrap();
    let link_root = temp.path().join("link");
    symlink(store.root(), &link_root).unwrap();
    assert!(CrashStore::open(link_root).is_err());
    let report = store.insert("frame").unwrap();
    symlink(&outside, store.root().join(format!("{}.dmp", report.id))).unwrap();
    assert!(store.delete(&report.id).is_err());
    assert!(store.enforce_limits().is_err());
    assert_eq!(fs::read_to_string(outside).unwrap(), "preserve");
}
