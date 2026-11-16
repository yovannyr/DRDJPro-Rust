use super::*;
use std::fs;

fn root() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let store = super::super::CrashStore::open(temp.path().join("reports")).unwrap();
    (temp, store.root().into())
}
fn entries(root: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    for name in ["runtime.2.jsonl", "runtime.1.jsonl", "runtime.jsonl"] {
        if let Ok(bytes) = fs::read(root.join("runtime-logs").join(name)) {
            for line in bytes.split_inclusive(|b| *b == b'\n') {
                entries.push(serde_json::from_slice(line).unwrap());
            }
        }
    }
    entries
}

#[test]
fn rotation_caps_managed_data_and_preserves_unknown_files() {
    let (_temp, root) = root();
    let mut sink = sink::Sink::open(&root, 512).unwrap();
    let dir = root.join("runtime-logs");
    fs::write(dir.join("personal.txt"), b"keep this").unwrap();
    let entry = Entry {
        schema: 1,
        time_ms: 1,
        pid: 1,
        role: Role::App,
        event: Event::ReportStored,
        id: None,
        io: None,
        count: None,
    };
    for _ in 0..100 {
        sink.append(&[entry; 8]).unwrap();
    }
    let mut total = 0;
    for name in ["runtime.jsonl", "runtime.1.jsonl", "runtime.2.jsonl"] {
        let size = fs::metadata(dir.join(name)).unwrap().len();
        assert!(size <= 512);
        total += size;
    }
    assert!(total <= 3 * 512);
    assert!(!entries(&root).is_empty());
    assert_eq!(fs::read(dir.join("personal.txt")).unwrap(), b"keep this");
    assert!(fs::read_dir(dir).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("log-write-")));
}

#[test]
fn producer_is_allocation_free_and_overflow_is_observable_after_recovery() {
    let (_temp, root) = root();
    let lock = sink::Sink::open(&root, 1024).unwrap();
    let log = Guard::start(&root, Role::App, 4, 1024).unwrap();
    let allocations = crate::test_alloc::counted_allocations(|| {
        for _ in 0..100 {
            log.core.record(
                Event::UploadRetry,
                Some("https://SECRET/Users/private"),
                Some(io::ErrorKind::PermissionDenied),
            );
        }
    });
    assert_eq!(allocations, 0);
    assert!(log.core.dropped.load(Ordering::Relaxed) >= 96);
    assert!(!log.core.flush(Duration::from_millis(20))); // Held cross-process log lock.
    assert_eq!(log.core.timeouts.load(Ordering::Relaxed), 1);
    assert_eq!(log.core.write_errors.load(Ordering::Relaxed), 0);
    drop(lock);
    assert!(log.core.flush(Duration::from_secs(3)));
    let saved = entries(&root);
    assert!(saved
        .iter()
        .any(|r| r.event == Event::RecordsDropped && r.count.unwrap() > 0));
    assert!(saved.iter().all(|r| r.id.is_none()));
    let text = fs::read_to_string(root.join("runtime-logs/runtime.jsonl")).unwrap();
    for secret in ["SECRET", "https:", "Users", "private"] {
        assert!(!text.contains(secret));
    }
}

#[test]
fn unavailable_sink_fails_open_counts_loss_and_can_recover() {
    let (_temp, root) = root();
    let blocker = root.join("runtime-logs");
    fs::write(&blocker, b"unrelated blocker").unwrap();
    let log = Guard::start(&root, Role::App, 8, 1024).unwrap();
    log.core.record(
        Event::CaptureUnavailable,
        None,
        Some(io::ErrorKind::PermissionDenied),
    );
    assert!(!log.core.flush(Duration::from_secs(3)));
    assert_eq!(fs::read(&blocker).unwrap(), b"unrelated blocker");
    assert_eq!(log.core.health.load(Ordering::Acquire), 2);
    assert!(log.core.write_errors.load(Ordering::Relaxed) > 0);
    assert!(log.core.dropped.load(Ordering::Relaxed) > 0);
    // Removing the test's explicitly created blocker permits a later retry.
    fs::remove_file(&blocker).unwrap();
    log.core.record(Event::ReportStored, None, None);
    assert!(log.core.flush(Duration::from_secs(3)));
    assert_eq!(log.core.health.load(Ordering::Acquire), 1);
    assert!(entries(&root)
        .iter()
        .any(|e| e.event == Event::RecordsDropped));
}

#[test]
fn recovery_cleans_only_owned_staging_and_corrupt_or_oversized_managed_logs() {
    let (_temp, root) = root();
    drop(sink::Sink::open(&root, 512).unwrap());
    let dir = root.join("runtime-logs");
    fs::write(
        dir.join("runtime.jsonl"),
        br#"{"schema":1,"event":"https://private/path"}"#,
    )
    .unwrap();
    fs::write(dir.join("runtime.1.jsonl"), vec![0; 4096]).unwrap();
    fs::write(dir.join("log-write-abcdefghijklmnop.tmp"), b"interrupted").unwrap();
    fs::write(dir.join("log-write-user.tmp"), b"keep").unwrap();
    drop(sink::Sink::open(&root, 512).unwrap());
    assert!(!dir.join("runtime.jsonl").exists());
    assert!(!dir.join("runtime.1.jsonl").exists());
    assert!(!dir.join("log-write-abcdefghijklmnop.tmp").exists());
    assert_eq!(fs::read(dir.join("log-write-user.tmp")).unwrap(), b"keep");
}

#[test]
fn atomic_replacement_does_not_append_into_an_external_hard_link() {
    let (temp, root) = root();
    drop(sink::Sink::open(&root, 1024).unwrap());
    let entry = Entry {
        schema: 1,
        time_ms: 1,
        pid: 1,
        role: Role::App,
        event: Event::ScopeStarted,
        id: None,
        io: None,
        count: None,
    };
    let mut original = serde_json::to_vec(&entry).unwrap();
    original.push(b'\n');
    let outside = temp.path().join("outside.txt");
    fs::write(&outside, &original).unwrap();
    fs::hard_link(&outside, root.join("runtime-logs/runtime.jsonl")).unwrap();
    let mut sink = sink::Sink::open(&root, 1024).unwrap();
    sink.append(&[entry]).unwrap();
    assert_eq!(fs::read(&outside).unwrap(), original);
    assert_eq!(entries(&root).len(), 2);
}

#[cfg(unix)]
#[test]
fn symlinked_directory_is_never_followed_and_private_modes_are_enforced() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (temp, root) = root();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("marker"), b"untouched").unwrap();
    symlink(&outside, root.join("runtime-logs")).unwrap();
    assert!(sink::Sink::open(&root, 1024).is_err());
    assert_eq!(fs::read(outside.join("marker")).unwrap(), b"untouched");
    fs::remove_file(root.join("runtime-logs")).unwrap();
    let log = Guard::start(&root, Role::App, 8, 1024).unwrap();
    assert!(log.core.flush(Duration::from_secs(3)));
    let dir = root.join("runtime-logs");
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(dir.join("runtime.jsonl"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn logger_reservation_preserves_retained_report_plus_log_budget() {
    let (_temp, root) = root();
    let store = super::super::CrashStore::open(root).unwrap();
    assert_eq!(PEAK_BYTES, 262_144);
    assert_eq!(store.max_bytes + PEAK_BYTES, 500_000_000);
    assert_eq!(store.clone().max_bytes, store.max_bytes);
}

#[test]
fn status_text_is_not_a_durability_or_success_claim() {
    let text = Status {
        enabled: true,
        health: 2,
        dropped: 12,
        errors: 3,
        timeouts: 1,
        recovered: 2,
    }
    .text();
    assert!(text.contains("unavailable"));
    assert!(text.contains("Unconfirmed/dropped events: 12"));
    assert!(text.contains("write failures: 3"));
    assert!(text.contains("flush timeouts: 1"));
    assert!(!Status::default().text().contains("active ("));
}
