//! Deliberate faults run only in disposable subprocesses before UI/audio startup.
use drdjpro_ui_spike::crash_handler::{CrashStore, ReportStatus};
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn wait(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() >= timeout {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("Disposable crash probe exceeded {timeout:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn probe(
    mode: &str,
) -> (
    tempfile::TempDir,
    CrashStore,
    std::process::ExitStatus,
    Duration,
) {
    let temp = tempfile::tempdir().unwrap();
    let store = CrashStore::open(temp.path().join("reports")).unwrap();
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_drdjpro-ui-spike"))
        .arg("--crash-probe")
        .arg(store.root())
        .arg(mode)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let status = wait(&mut child, Duration::from_secs(20));
    let elapsed = start.elapsed();
    let pid: u32 = fs::read_to_string(store.root().join("probe-monitor.pid"))
        .unwrap()
        .parse()
        .unwrap();
    wait_for_monitor_exit(pid, Duration::from_secs(15));
    (temp, store, status, elapsed)
}

#[cfg(windows)]
fn wait_for_monitor_exit(pid: u32, timeout: Duration) {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_ACCESS_RIGHTS},
    };
    // SAFETY: opens a synchronize-only handle to the disposable monitor; no memory
    // access, forced exception or other process mutation is performed here.
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_ACCESS_RIGHTS(0x0010_0000), false, pid) {
            let result = WaitForSingleObject(handle, timeout.as_millis() as u32);
            let _ = CloseHandle(handle);
            assert_eq!(result, WAIT_OBJECT_0, "Monitor did not exit in time");
        }
    }
}

#[cfg(unix)]
fn wait_for_monitor_exit(pid: u32, timeout: Duration) {
    let start = Instant::now();
    // SAFETY: signal zero only queries existence of this disposable child's PID.
    while unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        // Linux may expose an exited child briefly as a zombie until reaped.
        #[cfg(target_os = "linux")]
        if fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| stat.contains(") Z")) {
            return;
        }
        assert!(start.elapsed() < timeout, "Monitor did not exit in time");
        thread::sleep(Duration::from_millis(20));
    }
}

fn assert_exception_stream(
    exception: Result<minidump::MinidumpException<'_>, minidump::Error>,
    panic_snapshot: bool,
) {
    if cfg!(target_os = "macos") && panic_snapshot {
        // macOS simulate_exception(None) captures a snapshot without exception
        // data. Reject malformed streams as well as an unexpected present stream.
        assert!(
            matches!(exception, Err(minidump::Error::StreamNotFound)),
            "macOS panic snapshot must omit the exception stream: {exception:?}"
        );
    } else {
        exception.expect("Native faults and non-macOS panics need an exception stream");
    }
}

fn assert_dump(database: &Path, expected_stack: Option<&str>) {
    let store = CrashStore::open(database.into()).unwrap();
    let reports = store.list().unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].status, ReportStatus::Pending);
    assert_eq!(reports[0].os, std::env::consts::OS);
    assert_eq!(reports[0].arch, std::env::consts::ARCH);
    if let Some(function) = expected_stack {
        assert!(
            reports[0].stack.contains(function),
            "Missing function: {}",
            reports[0].stack
        );
        assert!(!reports[0].stack.contains("Controlled crash probe"));
        assert!(!reports[0].stack.contains("Users\\"));
    }
    let path = database.join(format!("{}.dmp", reports[0].id));
    let dump = minidump::Minidump::read_path(&path).expect("Real minidump must parse");
    let threads = dump.get_stream::<minidump::MinidumpThreadList>().unwrap();
    assert!(!threads.threads.is_empty());
    assert_exception_stream(
        dump.get_stream::<minidump::MinidumpException>(),
        expected_stack.is_some(),
    );
    assert!(fs::metadata(path).unwrap().len() <= 10_000_000);
}

#[test]
fn main_thread_panic_generates_parseable_dump_and_redacted_stack() {
    let (_temp, store, status, elapsed) = probe("panic");
    assert!(!status.success());
    assert_dump(store.root(), Some("panic_probe"));
    eprintln!("CRASH_PROBE panic wall time: {elapsed:?}");
}

#[test]
fn worker_panic_captures_without_changing_unwind_semantics() {
    let (_temp, store, status, elapsed) = probe("thread");
    assert!(status.success());
    assert_dump(store.root(), Some("panic_probe"));
    eprintln!("CRASH_PROBE worker wall time: {elapsed:?}");
}

#[test]
fn native_access_violation_generates_parseable_dump() {
    let (_temp, store, status, elapsed) = probe("native");
    assert!(!status.success());
    assert_dump(store.root(), None);
    eprintln!("CRASH_PROBE native wall time: {elapsed:?}");
}

#[test]
fn normal_exit_stops_monitor_and_creates_no_false_report() {
    let (_temp, store, status, elapsed) = probe("normal");
    assert!(status.success());
    assert!(store.list().unwrap().is_empty());
    assert!(elapsed < Duration::from_secs(5));
}

#[test]
fn abrupt_exit_does_not_leave_monitor_running() {
    let (_temp, store, status, _) = probe("killed");
    assert_eq!(status.code(), Some(23));
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn monitor_without_client_times_out() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("ipc");
    let mut child = Command::new(env!("CARGO_BIN_EXE_drdjpro-ui-spike"))
        .arg("--crash-monitor")
        .arg(temp.path().join("reports"))
        .arg(socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(wait(&mut child, Duration::from_secs(8)).success());
}

#[test]
fn caught_outer_panic_can_drop_and_restart_the_monitor() {
    let (_temp, store, status, _) = probe("restart");
    assert!(status.success());
    assert_dump(store.root(), Some("panic_probe"));
}

#[test]
fn concurrent_native_and_rust_faults_do_not_cross_consume_replies() {
    let (_temp, store, status, _) = probe("concurrent");
    assert!(!status.success());
    let reports = store.list().unwrap();
    assert!(!reports.is_empty());
    for report in reports {
        let dump =
            minidump::Minidump::read_path(store.root().join(format!("{}.dmp", report.id))).unwrap();
        assert!(dump.get_stream::<minidump::MinidumpException>().is_ok());
        assert!(!dump
            .get_stream::<minidump::MinidumpThreadList>()
            .unwrap()
            .threads
            .is_empty());
    }
}

#[test]
fn two_worker_panics_then_native_fault_keep_the_same_monitor_usable() {
    let (_temp, store, status, _) = probe("sequential");
    assert!(!status.success());
    let reports = store.list().unwrap();
    assert_eq!(reports.len(), 3);
    // This probe joins both panic workers before the native fault. Their stack
    // messages are consumed and cleared per capture, so exactly two have stacks.
    assert_eq!(reports.iter().filter(|r| !r.stack.is_empty()).count(), 2);
    for report in reports {
        let panic_snapshot = !report.stack.is_empty();
        if panic_snapshot {
            assert!(report.stack.contains("panic_probe"));
        }
        let dump =
            minidump::Minidump::read_path(store.root().join(format!("{}.dmp", report.id))).unwrap();
        assert_exception_stream(
            dump.get_stream::<minidump::MinidumpException>(),
            panic_snapshot,
        );
    }
}
