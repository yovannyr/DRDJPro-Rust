use drdj_usb::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

fn setup(request: u8) -> ControlSetup {
    ControlSetup {
        kind: ControlKind::Vendor,
        recipient: Recipient::Device,
        request,
        value: 0,
        index: 0,
    }
}
fn write(request: u8) -> IoCommand {
    IoCommand::ControlOut {
        session: SessionId(1),
        setup: setup(request),
        data: vec![request],
        timeout: Duration::from_millis(10),
    }
}
fn wait(ticket: &mut IoTicket) -> Result<IoReply, IoError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = ticket.poll() {
            return result;
        }
        assert!(Instant::now() < deadline, "USB ticket did not complete");
        thread::sleep(Duration::from_millis(2));
    }
}
type Calls = Arc<Mutex<Vec<(thread::ThreadId, IoCommand)>>>;
struct Echo(Calls);
impl Transport for Echo {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        self.0
            .lock()
            .unwrap()
            .push((thread::current().id(), command.clone()));
        match command {
            IoCommand::ControlOut { data, .. } => Ok(IoReply::Written(data.len())),
            IoCommand::BulkIn { length, .. } => Ok(IoReply::Data(vec![42; length.min(3)])),
            IoCommand::Close { .. } => Ok(IoReply::Closed),
            _ => Err(IoError::InvalidSession),
        }
    }
}

#[test]
fn accepted_commands_run_in_order_off_caller_thread_and_tickets_are_one_shot() {
    let caller = thread::current().id();
    let calls = Calls::default();
    let recorded = Arc::clone(&calls);
    let (created_tx, created_rx) = mpsc::channel();
    let worker = IoWorker::spawn(move || {
        created_tx.send(thread::current().id()).unwrap();
        Ok(Echo(Arc::clone(&recorded)))
    })
    .unwrap();
    let mut tickets: Vec<_> = (0..8)
        .map(|request| worker.submit(write(request)).unwrap())
        .collect();
    for ticket in &mut tickets {
        assert_eq!(wait(ticket).unwrap(), IoReply::Written(1));
        assert!(ticket.poll().is_none());
    }
    assert_ne!(
        created_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        caller
    );
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 8);
    for (index, (owner, command)) in calls.iter().enumerate() {
        assert_ne!(*owner, caller);
        assert_eq!(*command, write(index as u8));
    }
}

#[test]
fn invalid_requests_never_initialize_backend_or_enter_the_queue() {
    let creations = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&creations);
    let worker = IoWorker::spawn(move || {
        count.fetch_add(1, Ordering::SeqCst);
        Ok(Echo(Calls::default()))
    })
    .unwrap();
    let session = SessionId(1);
    for command in [
        IoCommand::ControlIn {
            session,
            setup: setup(1),
            length: MAX_TRANSFER_BYTES + 1,
            timeout: Duration::from_millis(10),
        },
        IoCommand::ControlOut {
            session,
            setup: setup(1),
            data: vec![0; MAX_TRANSFER_BYTES + 1],
            timeout: Duration::from_millis(10),
        },
        IoCommand::BulkIn {
            session,
            endpoint: 0x81,
            length: 1,
            timeout: Duration::ZERO,
        },
        IoCommand::BulkIn {
            session,
            endpoint: 0x81,
            length: 1,
            timeout: Duration::from_micros(999),
        },
        IoCommand::BulkIn {
            session,
            endpoint: 0x81,
            length: 1,
            timeout: Duration::from_millis(251),
        },
        IoCommand::BulkIn {
            session,
            endpoint: 0x01,
            length: 1,
            timeout: Duration::from_millis(10),
        },
        IoCommand::InterruptOut {
            session,
            endpoint: 0x81,
            data: vec![0],
            timeout: Duration::from_millis(10),
        },
        IoCommand::BulkIn {
            session,
            endpoint: 0x80,
            length: 1,
            timeout: Duration::from_millis(10),
        },
        IoCommand::BulkIn {
            session,
            endpoint: 0x91,
            length: 1,
            timeout: Duration::from_millis(10),
        },
    ] {
        assert!(matches!(
            worker.submit(command),
            Err(IoError::InvalidParameter(_))
        ));
    }
    assert_eq!(creations.load(Ordering::SeqCst), 0);
    // Zero-payload control commands (e.g. feature changes) remain legitimate.
    let mut ticket = worker
        .submit(IoCommand::ControlOut {
            session,
            setup: setup(1),
            data: Vec::new(),
            timeout: Duration::from_millis(1),
        })
        .unwrap();
    assert_eq!(wait(&mut ticket).unwrap(), IoReply::Written(0));
}

#[test]
fn short_reads_are_preserved_and_dropped_completion_does_not_block_next_request() {
    let worker = IoWorker::spawn(|| Ok(Echo(Calls::default()))).unwrap();
    drop(worker.submit(write(1)).unwrap());
    let mut ticket = worker
        .submit(IoCommand::BulkIn {
            session: SessionId(1),
            endpoint: 0x81,
            length: 128,
            timeout: Duration::from_millis(10),
        })
        .unwrap();
    assert_eq!(wait(&mut ticket).unwrap(), IoReply::Data(vec![42; 3]));
}

struct FailOnce {
    calls: Arc<AtomicUsize>,
}
impl Transport for FailOnce {
    fn execute(&mut self, _: IoCommand) -> Result<IoReply, IoError> {
        match self.calls.fetch_add(1, Ordering::SeqCst) {
            0 => Err(IoError::Timeout),
            _ => Ok(IoReply::Written(1)),
        }
    }
}
#[test]
fn initialization_can_recover_and_timed_out_writes_are_never_automatically_replayed() {
    let creations = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&creations);
    let calls = Arc::new(AtomicUsize::new(0));
    let operations = Arc::clone(&calls);
    let worker = IoWorker::spawn(move || {
        if count.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(IoError::AccessDenied);
        }
        Ok(FailOnce {
            calls: Arc::clone(&operations),
        })
    })
    .unwrap();
    let mut first = worker.submit(write(1)).unwrap();
    assert_eq!(wait(&mut first), Err(IoError::AccessDenied));
    let mut second = worker.submit(write(1)).unwrap();
    assert_eq!(wait(&mut second), Err(IoError::Timeout));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let mut explicit_retry = worker.submit(write(1)).unwrap();
    assert_eq!(wait(&mut explicit_retry).unwrap(), IoReply::Written(1));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(creations.load(Ordering::SeqCst), 2);
}

struct Blocked {
    entered: Option<mpsc::Sender<()>>,
    release: mpsc::Receiver<()>,
    closed: mpsc::Sender<thread::ThreadId>,
    calls: Arc<AtomicUsize>,
}
impl Transport for Blocked {
    fn execute(&mut self, _: IoCommand) -> Result<IoReply, IoError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.take().unwrap().send(()).unwrap();
        self.release.recv().unwrap();
        Ok(IoReply::Written(1))
    }
}
impl Drop for Blocked {
    fn drop(&mut self) {
        let _ = self.closed.send(thread::current().id());
    }
}

#[test]
fn full_queue_reports_backpressure_and_shutdown_cancels_queued_work_without_waiting() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (closed_tx, closed_rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut backend = Some(Blocked {
        entered: Some(entered_tx),
        release: release_rx,
        closed: closed_tx,
        calls: Arc::clone(&calls),
    });
    let worker = IoWorker::spawn(move || Ok(backend.take().unwrap())).unwrap();
    let mut current = worker.submit(write(1)).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut pending: Vec<_> = (0..IO_QUEUE_CAPACITY)
        .map(|_| worker.submit(write(2)).unwrap())
        .collect();
    assert!(matches!(worker.submit(write(3)), Err(IoError::QueueFull)));
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let dropper = thread::spawn(move || {
        drop(worker);
        dropped_tx.send(()).unwrap();
    });
    let stopped_promptly = dropped_rx.recv_timeout(Duration::from_millis(200));
    release_tx.send(()).unwrap();
    dropper.join().unwrap();
    assert!(
        stopped_promptly.is_ok(),
        "worker Drop blocked on native I/O"
    );
    assert_eq!(wait(&mut current).unwrap(), IoReply::Written(1));
    for ticket in &mut pending {
        assert_eq!(wait(ticket), Err(IoError::Stopped));
    }
    assert_ne!(
        closed_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        thread::current().id()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn raw_standard_state_changes_cannot_bypass_managed_interface_policy() {
    let creations = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&creations);
    let worker = IoWorker::spawn(move || {
        count.fetch_add(1, Ordering::SeqCst);
        Ok(Echo(Calls::default()))
    })
    .unwrap();
    for request in [1, 3, 5, 7, 9, 11] {
        let mut packet = setup(request);
        packet.kind = ControlKind::Standard;
        assert!(matches!(
            worker.submit(IoCommand::ControlOut {
                session: SessionId(1),
                setup: packet,
                data: Vec::new(),
                timeout: Duration::from_millis(10)
            }),
            Err(IoError::InvalidParameter(_))
        ));
        assert!(IoCommand::ControlIn {
            session: SessionId(1),
            setup: packet,
            length: 1,
            timeout: Duration::from_millis(10)
        }
        .validate()
        .is_err());
    }
    assert_eq!(creations.load(Ordering::SeqCst), 0);
    let mut descriptor = setup(6);
    descriptor.kind = ControlKind::Standard;
    assert!(IoCommand::ControlIn {
        session: SessionId(1),
        setup: descriptor,
        length: 18,
        timeout: Duration::from_millis(10)
    }
    .validate()
    .is_ok());
    // Request number 9 is valid in a vendor protocol: do not confuse request
    // numbers across different bmRequestType namespaces.
    let mut vendor = worker.submit(write(9)).unwrap();
    assert_eq!(wait(&mut vendor).unwrap(), IoReply::Written(1));
}

#[test]
fn managed_string_descriptors_and_halt_commands_enforce_their_own_limits() {
    let target = DeviceTarget {
        id: DeviceId { bus: 1, address: 1 },
        vendor_id: 0x1234,
        product_id: 1,
    };
    let value = IoCommand::SetString {
        target,
        index: 1,
        language: 0x409,
        value: "🎵".repeat(63),
        timeout: Duration::from_millis(10),
    };
    assert!(value.validate().is_ok());
    for command in [
        IoCommand::SetString {
            target,
            index: 1,
            language: 0x409,
            value: "a".repeat(127),
            timeout: Duration::from_millis(10),
        },
        IoCommand::SetString {
            target,
            index: 1,
            language: 0x409,
            value: "🎵".repeat(64),
            timeout: Duration::from_millis(10),
        },
        IoCommand::SetString {
            target,
            index: 0,
            language: 0x409,
            value: "DJ".into(),
            timeout: Duration::from_millis(10),
        },
        IoCommand::SetString {
            target,
            index: 1,
            language: 0,
            value: "DJ".into(),
            timeout: Duration::from_millis(10),
        },
        IoCommand::ClearHalt {
            session: SessionId(1),
            endpoint: 0x80,
        },
        IoCommand::ClearHalt {
            session: SessionId(1),
            endpoint: 0x91,
        },
    ] {
        assert!(matches!(
            command.validate(),
            Err(IoError::InvalidParameter(_))
        ));
    }
    assert!(IoCommand::ClearHalt {
        session: SessionId(1),
        endpoint: 0x81
    }
    .validate()
    .is_ok());
}
