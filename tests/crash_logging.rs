//! Real process-lifetime hubs, cross-process locks and helper diagnostics.
//! Every child uses an explicitly owned temporary store, never the user's logs.
use drdjpro_ui_spike::crash_handler::CrashStore;
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn child(root: &Path, flag: &str, mode: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_drdjpro-ui-spike"))
        .arg(flag)
        .arg(root)
        .arg(mode)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}
fn wait(child: &mut Child) -> std::process::ExitStatus {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Disposable log probe timed out");
        }
        thread::sleep(Duration::from_millis(5));
    }
}
fn logs(root: &Path) -> Vec<serde_json::Value> {
    let dir = root.join("runtime-logs");
    let mut result = Vec::new();
    let mut total = 0;
    for name in ["runtime.2.jsonl", "runtime.1.jsonl", "runtime.jsonl"] {
        if let Ok(bytes) = fs::read(dir.join(name)) {
            assert!(bytes.len() <= 65536);
            total += bytes.len();
            for line in bytes.split_inclusive(|b| *b == b'\n') {
                assert!(line.len() <= 512);
                let record: serde_json::Value = serde_json::from_slice(line).unwrap();
                assert_eq!(record["schema"], 1);
                assert!(record.as_object().unwrap().keys().all(|key| [
                    "schema", "time_ms", "pid", "role", "event", "id", "io", "count"
                ]
                .contains(&key.as_str())));
                result.push(record);
            }
        }
    }
    assert!(total <= 3 * 65536);
    result
}

#[test]
fn nested_and_restarted_owners_keep_the_same_runtime_hub_usable() {
    let temp = tempfile::tempdir().unwrap();
    let mut process = child(temp.path(), "--crash-log-probe", "owners");
    assert!(wait(&mut process).success());
    let entries = logs(temp.path());
    for event in ["report_stored", "report_deleted", "sharing_declined"] {
        assert!(
            entries.iter().any(|entry| entry["event"] == event),
            "Missing {event}"
        );
    }
}

#[test]
fn existing_host_logger_is_preserved_and_its_private_text_is_not_collected() {
    let temp = tempfile::tempdir().unwrap();
    let mut process = child(temp.path(), "--crash-log-probe", "host");
    assert!(wait(&mut process).success());
    let entries = logs(temp.path());
    assert!(entries.iter().any(|e| e["event"] == "report_stored"));
    let text = serde_json::to_string(&entries).unwrap();
    for secret in ["SECRET", "https:", "credentials", "private"] {
        assert!(!text.contains(secret));
    }
}

#[test]
fn two_processes_share_one_bounded_rotation_without_torn_records() {
    let temp = tempfile::tempdir().unwrap();
    let mut first = child(temp.path(), "--crash-log-probe", "small");
    let mut second = child(temp.path(), "--crash-log-probe", "small");
    let pids: BTreeSet<_> = [first.id() as u64, second.id() as u64]
        .into_iter()
        .collect();
    assert!(wait(&mut first).success());
    assert!(wait(&mut second).success());
    let entries = logs(temp.path());
    let saved: BTreeSet<_> = entries.iter().filter_map(|e| e["pid"].as_u64()).collect();
    assert_eq!(saved, pids);
    let mut rotation_a = child(temp.path(), "--crash-log-probe", "rotate");
    let mut rotation_b = child(temp.path(), "--crash-log-probe", "rotate");
    assert!(wait(&mut rotation_a).success());
    assert!(wait(&mut rotation_b).success());
    assert!(!logs(temp.path()).is_empty());
    let dir = temp.path().join("runtime-logs");
    assert!(dir.join("runtime.2.jsonl").exists());
    assert!(fs::read_dir(dir).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("log-write-")));
}

#[test]
fn app_and_monitor_emit_real_lifecycle_and_capture_events() {
    let temp = tempfile::tempdir().unwrap();
    let mut process = child(temp.path(), "--crash-probe", "thread");
    assert!(wait(&mut process).success());
    let store = CrashStore::open(temp.path().into()).unwrap();
    let report = store.list().unwrap().pop().unwrap();
    let entries = logs(temp.path());
    assert!(entries
        .iter()
        .any(|e| e["role"] == "app" && e["event"] == "monitor_started"));
    assert!(entries
        .iter()
        .any(|e| e["role"] == "monitor" && e["event"] == "capture_requested"));
    assert!(entries.iter().any(|e| e["role"] == "monitor"
        && e["event"] == "capture_finalized"
        && e["id"] == report.id));
    let text = serde_json::to_string(&entries).unwrap();
    assert!(!text.contains("panic_probe"));
    assert!(!text.contains("Controlled crash probe"));
    assert!(!text.contains("Users"));
    assert!(!text.contains("/home/"));
}

#[test]
fn unavailable_log_directory_never_aborts_normal_application_startup() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("runtime-logs"), b"owned test blocker").unwrap();
    let mut process = child(temp.path(), "--crash-startup-probe", "normal");
    assert!(wait(&mut process).success());
    assert_eq!(
        fs::read(temp.path().join("runtime-logs")).unwrap(),
        b"owned test blocker"
    );
    assert!(CrashStore::open(temp.path().into())
        .unwrap()
        .list()
        .unwrap()
        .is_empty());
}
