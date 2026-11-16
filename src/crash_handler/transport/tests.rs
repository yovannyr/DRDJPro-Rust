use super::*;
use flate2::read::GzDecoder;
use std::{
    fs,
    io::Write,
    net::{TcpListener, TcpStream},
    sync::Mutex,
    thread::{self, JoinHandle},
};

struct Request {
    headers: String,
    body: Vec<u8>,
}

impl Request {
    fn json(&self) -> serde_json::Value {
        let mut json = Vec::new();
        GzDecoder::new(&self.body[..])
            .take(1_048_577)
            .read_to_end(&mut json)
            .unwrap();
        assert!(json.len() <= 1_048_576);
        serde_json::from_slice(&json).unwrap()
    }
}

struct Reply {
    status: u16,
    body: Vec<u8>,
    location: Option<String>,
}

fn acknowledged(request: &Request) -> Reply {
    Reply {
        status: 200,
        body: serde_json::to_vec(&serde_json::json!({
            "id": request.json()["id"], "accepted": true
        }))
        .unwrap(),
        location: None,
    }
}

struct Server {
    url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    fn start(handler: impl Fn(&Request) -> Reply + Send + 'static) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = format!(
            "https://localhost:{}/reports",
            listener.local_addr().unwrap().port()
        );
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let thread = thread::spawn(move || {
            // Public, deliberately non-production fixture; no trust-store install.
            let identity = native_tls::Identity::from_pkcs12(
                include_bytes!("fixtures/localhost.p12"),
                "test-only",
            )
            .unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            while !stopped.load(Ordering::Acquire) {
                let socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("Test listener: {error}"),
                };
                // Windows inherits the listener's nonblocking mode on accepted
                // sockets. TLS runs on this dedicated server thread, with deadlines.
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let Ok(mut stream) = acceptor.accept(socket) else {
                    continue; // Expected for untrusted-CA / wrong-host tests.
                };
                let Some(request) = read_request(&mut stream) else {
                    continue;
                };
                let reply = handler(&request);
                recorded.lock().unwrap().push(request);
                let mut headers = format!(
                    "HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                    reply.status, reply.body.len()
                );
                if let Some(location) = reply.location {
                    headers.push_str(&format!("Location: {location}\r\n"));
                }
                headers.push_str("\r\n");
                let _ = stream.write_all(headers.as_bytes());
                let _ = stream.write_all(&reply.body);
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_request(stream: &mut native_tls::TlsStream<TcpStream>) -> Option<Request> {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut byte = [0];
        if stream.read_exact(&mut byte).is_err() {
            return None;
        }
        bytes.push(byte[0]);
        assert!(bytes.len() < 8192);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let length: usize = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap();
    assert!(length <= 1_048_576);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    Some(Request { headers, body })
}

fn trusted() -> ClientBuilder {
    client_builder().no_proxy().add_root_certificate(
        reqwest::Certificate::from_pem(include_bytes!("fixtures/ca.pem")).unwrap(),
    )
}

fn store(server: &Server, auto_submit: bool) -> (tempfile::TempDir, CrashStore) {
    let temp = tempfile::tempdir().unwrap();
    let store = CrashStore::open(temp.path().join("reports")).unwrap();
    store
        .save_settings(&super::super::CrashSettings {
            server_url: server.url.clone(),
            auto_submit,
        })
        .unwrap();
    (temp, store)
}

fn complete(store: &CrashStore, builder: ClientBuilder) -> UploadSummary {
    let task = spawn_with(store.clone(), builder).unwrap();
    task.result
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap()
}

fn status(store: &CrashStore, id: &str) -> ReportStatus {
    store
        .list()
        .unwrap()
        .into_iter()
        .find(|r| r.id == id)
        .unwrap()
        .status
}

#[test]
fn real_https_gzip_ack_sends_only_redacted_text_and_preserves_dump() {
    let server = Server::start(|request| {
        let headers = request.headers.to_ascii_lowercase();
        assert!(headers.starts_with("post /reports http/1.1"));
        assert!(headers.contains("content-encoding: gzip"));
        assert!(headers.contains("content-type: application/json"));
        let json = request.json();
        assert!(headers.contains(&format!(
            "idempotency-key: {}",
            json["id"].as_str().unwrap()
        )));
        assert_eq!(json["schema_version"], 1);
        assert!(json.get("status").is_none());
        assert!(json.get("minidump").is_none());
        let text = json.to_string();
        assert!(!text.contains("PRIVATE_DUMP"));
        assert!(!text.contains("Users"));
        assert!(!text.contains("SensitiveName"));
        assert!(text.contains("frame.rs:42:1"));
        acknowledged(request)
    });
    let (_temp, store) = store(&server, false);
    let report = store
        .insert("app::frame\n at C:\\Users\\SensitiveName\\frame.rs:42:1")
        .unwrap();
    let dump = store.root().join(format!("{}.dmp", report.id));
    fs::write(&dump, b"PRIVATE_DUMP").unwrap();
    assert!(store.queue_report(&report.id).unwrap());
    assert_eq!(complete(&store, trusted()).submitted, 1);
    assert_eq!(status(&store, &report.id), ReportStatus::Submitted);
    assert_eq!(fs::read(dump).unwrap(), b"PRIVATE_DUMP");
    assert!(!store.queue_report(&report.id).unwrap());
    assert_eq!(complete(&store, trusted()).submitted, 0);
    assert_eq!(server.count(), 1);
    assert!(fs::read_dir(store.root()).unwrap().all(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_none_or(|ext| ext != "gz")
    }));
}

#[test]
fn only_explicit_consent_is_sent_and_auto_submit_never_overrides_decline() {
    let server = Server::start(acknowledged);
    let (_temp, store) = store(&server, false);
    let pending = store.insert("pending").unwrap();
    let declined = store.insert("declined").unwrap();
    store
        .set_status(&declined.id, ReportStatus::Declined)
        .unwrap();
    assert_eq!(
        complete(&store, trusted()),
        UploadSummary {
            skipped: 2,
            ..Default::default()
        }
    );
    assert_eq!(server.count(), 0);
    let mut settings = store.settings().unwrap();
    settings.auto_submit = true;
    store.save_settings(&settings).unwrap();
    assert_eq!(complete(&store, trusted()).submitted, 1);
    assert_eq!(status(&store, &pending.id), ReportStatus::Submitted);
    assert_eq!(status(&store, &declined.id), ReportStatus::Declined);
    assert!(store.queue_report(&declined.id).unwrap()); // Later explicit manual consent.
    assert_eq!(complete(&store, trusted()).submitted, 1);
    assert_eq!(server.count(), 2);
}

#[test]
fn failure_is_durable_and_next_worker_retries_without_resending_success() {
    let attempt = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&attempt);
    let server = Server::start(move |request| {
        if counter.fetch_add(1, Ordering::AcqRel) == 0 {
            Reply {
                status: 503,
                body: b"unavailable".to_vec(),
                location: None,
            }
        } else {
            acknowledged(request)
        }
    });
    let (_temp, store) = store(&server, false);
    let report = store.insert("frame").unwrap();
    store.queue_report(&report.id).unwrap();
    assert_eq!(complete(&store, trusted()).failed, 1);
    assert_eq!(status(&store, &report.id), ReportStatus::Queued);
    let reopened = CrashStore::open(store.root().into()).unwrap();
    assert_eq!(complete(&reopened, trusted()).submitted, 1);
    assert_eq!(complete(&reopened, trusted()).submitted, 0);
    assert_eq!(server.count(), 2);
}

#[test]
fn non_success_redirect_and_invalid_ack_never_mark_submitted() {
    for case in 0..8 {
        let server = Server::start(move |request| {
            let mut reply = acknowledged(request);
            match case {
                0 => reply.status = 403,
                1 => {
                    reply.status = 302;
                    reply.location = Some("http://127.0.0.1:1/leak".into());
                }
                2 => reply.body = br#"{"id":"wrong","accepted":true}"#.to_vec(),
                3 => reply.body = br#"{"id":"wrong","accepted":false}"#.to_vec(),
                4 => reply.body = b"not json".to_vec(),
                5 => reply.body = vec![b' '; MAX_ACK as usize + 1],
                6 => {
                    reply.status = 204;
                    reply.body.clear();
                }
                7 => {
                    reply.body = serde_json::to_vec(
                        &serde_json::json!({"id":request.json()["id"],"accepted":false}),
                    )
                    .unwrap()
                }
                _ => unreachable!(),
            }
            reply
        });
        let (_temp, store) = store(&server, false);
        let report = store.insert("frame").unwrap();
        store.queue_report(&report.id).unwrap();
        assert_eq!(complete(&store, trusted()).failed, 1, "case {case}");
        assert_eq!(status(&store, &report.id), ReportStatus::Queued);
        assert_eq!(server.count(), 1);
    }
}

#[test]
fn invalid_endpoint_is_rejected_before_network_or_authorizing_pending() {
    for raw in [
        "",
        "http://localhost/reports",
        "https://user:pass@localhost/",
        "https://localhost/?key=secret",
        "https://localhost/#fragment",
        "file:///tmp/report",
    ] {
        assert!(endpoint(raw).is_err());
    }
    let server = Server::start(acknowledged);
    let (_temp, store) = store(&server, true);
    let report = store.insert("frame").unwrap();
    let mut settings = store.settings().unwrap();
    settings.server_url = "http://localhost:1/reports".into();
    store.save_settings(&settings).unwrap();
    let task = spawn_with(store.clone(), trusted()).unwrap();
    assert!(task
        .result
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .is_err());
    assert_eq!(status(&store, &report.id), ReportStatus::Pending);
    assert_eq!(server.count(), 0);
}

#[test]
fn tls_rejects_untrusted_ca_and_wrong_hostname() {
    for wrong_host in [false, true] {
        let server = Server::start(acknowledged);
        let (_temp, store) = store(&server, false);
        let report = store.insert("frame").unwrap();
        store.queue_report(&report.id).unwrap();
        let builder = if wrong_host {
            let mut settings = store.settings().unwrap();
            settings.server_url = server.url.replace("localhost", "127.0.0.1");
            store.save_settings(&settings).unwrap();
            trusted()
        } else {
            client_builder().no_proxy()
        };
        assert_eq!(complete(&store, builder).failed, 1);
        assert_eq!(status(&store, &report.id), ReportStatus::Queued);
        assert_eq!(server.count(), 0);
    }
}

#[test]
fn network_wait_does_not_hold_store_lock_or_overwrite_decline() {
    let server_store = Arc::new(Mutex::new(None::<CrashStore>));
    let control = Arc::clone(&server_store);
    let server = Server::start(move |request| {
        let store = control.lock().unwrap().clone().unwrap();
        let id = request.json()["id"].as_str().unwrap().to_owned();
        // Would deadlock until the HTTP deadline if the worker held the store lock.
        store.set_status(&id, ReportStatus::Declined).unwrap();
        acknowledged(request)
    });
    let (_temp, store) = store(&server, false);
    *server_store.lock().unwrap() = Some(store.clone());
    let report = store.insert("frame").unwrap();
    store.queue_report(&report.id).unwrap();
    let summary = complete(&store, trusted());
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.submitted, 0);
    assert_eq!(summary.skipped, 1);
    assert_eq!(status(&store, &report.id), ReportStatus::Declined);
}

#[test]
fn deleted_during_request_is_not_recreated_after_ack() {
    let server_store = Arc::new(Mutex::new(None::<CrashStore>));
    let control = Arc::clone(&server_store);
    let server = Server::start(move |request| {
        control
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .delete(request.json()["id"].as_str().unwrap())
            .unwrap();
        acknowledged(request)
    });
    let (_temp, store) = store(&server, false);
    *server_store.lock().unwrap() = Some(store.clone());
    let report = store.insert("frame").unwrap();
    store.queue_report(&report.id).unwrap();
    assert_eq!(complete(&store, trusted()).skipped, 1);
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn exclusive_upload_lock_and_cancel_before_send_preserve_queue() {
    let server = Server::start(acknowledged);
    let (_temp, store) = store(&server, false);
    let report = store.insert("frame").unwrap();
    store.queue_report(&report.id).unwrap();
    let lock = store.try_upload_lock().unwrap();
    assert!(store.try_upload_lock().is_err());
    let task = spawn_with(store.clone(), trusted()).unwrap();
    assert!(task
        .result
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .is_err());
    drop(lock);
    let cancel = AtomicBool::new(true);
    assert_eq!(
        upload(
            &trusted().build().unwrap(),
            &endpoint(&server.url).unwrap(),
            &report,
            &cancel
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(status(&store, &report.id), ReportStatus::Queued);
    assert_eq!(server.count(), 0);
    assert_eq!(complete(&store, trusted()).submitted, 1);
}

#[test]
fn timeout_retains_queue_and_reports_failure() {
    let server = Server::start(|request| {
        thread::sleep(Duration::from_secs(2));
        acknowledged(request)
    });
    let (_temp, store) = store(&server, false);
    let report = store.insert("frame").unwrap();
    store.queue_report(&report.id).unwrap();
    assert_eq!(
        complete(&store, trusted().timeout(Duration::from_secs(1))).failed,
        1
    );
    assert_eq!(status(&store, &report.id), ReportStatus::Queued);
}

#[test]
fn normal_ui_startup_retries_only_consented_reports_over_real_https() {
    i_slint_backend_testing::init_no_event_loop();
    let server = Server::start(acknowledged);
    let (_temp, store) = store(&server, false);
    let queued = store.insert("authorized frame").unwrap();
    store.queue_report(&queued.id).unwrap();
    let pending = store.insert("needs consent").unwrap();
    let declined = store.insert("local only").unwrap();
    store
        .set_status(&declined.id, ReportStatus::Declined)
        .unwrap();
    let app = crate::AppWindow::new().unwrap();
    app.set_left_playing(true);
    app.set_left_position(42.);
    let session = super::super::ui::testing_wire(
        &app,
        store.clone(),
        std::rc::Rc::new(|store| spawn_with(store, trusted())),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while status(&store, &queued.id) != ReportStatus::Submitted {
        session.testing_tick();
        assert!(
            std::time::Instant::now() < deadline,
            "Startup retry did not finish"
        );
        thread::sleep(Duration::from_millis(2));
    }
    while !session
        .testing_delivery_status()
        .contains("Server-confirmed: 1")
        || !session.testing_idle()
    {
        session.testing_tick();
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(server.count(), 1);
    assert_eq!(status(&store, &pending.id), ReportStatus::Pending);
    assert_eq!(status(&store, &declined.id), ReportStatus::Declined);
    assert!(app.get_left_playing());
    assert_eq!(app.get_left_position(), 42.);
    // Refresh failure after a real ACK must retain the known delivery result.
    // Resolve both paths within this test's explicitly owned temporary directory.
    let parent = std::fs::canonicalize(_temp.path()).unwrap();
    assert_eq!(store.root().parent(), Some(parent.as_path()));
    let moved = parent.join("unavailable");
    assert!(moved.starts_with(&parent));
    fs::rename(store.root(), moved).unwrap();
    app.invoke_show_crash_reports();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while !session.testing_store_failed() {
        session.testing_tick();
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    assert!(session
        .testing_delivery_status()
        .contains("Server-confirmed: 1"));
    session.shutdown();
}

#[test]
fn main_window_shutdown_cancels_upload_while_filesystem_actor_is_blocked() {
    i_slint_backend_testing::init_no_event_loop();
    let (entered, received) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let server = Server::start(move |request| {
        if calls.fetch_add(1, Ordering::AcqRel) == 0 {
            entered.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(15)).unwrap();
        }
        acknowledged(request)
    });
    let (_temp, store) = store(&server, false);
    for _ in 0..2 {
        let report = store.insert("frame").unwrap();
        store.queue_report(&report.id).unwrap();
    }
    let task = spawn_with(store.clone(), trusted()).unwrap();
    let cancelled = Arc::clone(&task.cancel);
    received.recv_timeout(Duration::from_secs(15)).unwrap();
    let lock = fs::OpenOptions::new()
        .write(true)
        .open(store.root().join(".lock"))
        .unwrap();
    lock.lock().unwrap();
    let watchdog_lock = lock.try_clone().unwrap();
    let (unlock, wait_unlock) = mpsc::sync_channel(1);
    let timed_out = Arc::new(AtomicBool::new(false));
    let timeout = Arc::clone(&timed_out);
    let watchdog = thread::spawn(move || {
        if wait_unlock.recv_timeout(Duration::from_secs(15)).is_err() {
            timeout.store(true, Ordering::Release);
        }
        watchdog_lock.unlock().unwrap();
    });
    let app = crate::AppWindow::new().unwrap();
    // The actor starts with an initial FS scan, which cannot acquire the held lock.
    let session = super::super::ui::testing_own_upload(&app, store.clone(), task);
    app.invoke_close_crash_reports();
    assert!(cancelled.load(Ordering::Acquire));
    unlock.send(()).unwrap();
    watchdog.join().unwrap();
    assert!(
        !timed_out.load(Ordering::Acquire),
        "Shutdown waited for the FS actor"
    );
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(guard) = store.try_upload_lock() {
            drop(guard);
            break;
        }
        assert!(std::time::Instant::now() < deadline, "Upload did not stop");
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        server.count(),
        1,
        "No second queued report may start after shutdown"
    );
    assert_eq!(
        store
            .list()
            .unwrap()
            .iter()
            .filter(|r| r.status == ReportStatus::Queued)
            .count(),
        1
    );
    session.shutdown();
}
