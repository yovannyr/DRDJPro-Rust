use drdj_usb::*;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

fn target() -> HidTarget {
    HidTarget {
        path: b"capture-fixture".to_vec(),
        vendor_id: 0x1234,
        product_id: 1,
        interface: 1,
        usage_page: 0xff00,
        usage: 1,
    }
}
fn layout() -> ReportLayout {
    ReportLayout::parse(&[
        0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 4, 0x81, 2, 0xc0,
    ])
    .unwrap()
}
fn data(value: u32) -> IoReply {
    IoReply::Hid(HidReply::Input(Some(HidReport {
        report_id: 0,
        data: value.to_le_bytes().to_vec(),
        logical_length: 4,
    })))
}
fn connected() -> IoReply {
    IoReply::Hid(HidReply::Opened {
        session: HidSessionId(1),
        layout: layout(),
    })
}
fn wait_batch(
    capture: &HidCapture,
    mut predicate: impl FnMut(&CaptureBatch) -> bool,
) -> CaptureBatch {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(batch) = capture.try_drain(0) {
            if predicate(&batch) {
                return batch;
            }
        }
        assert!(
            Instant::now() < until,
            "capture did not reach expected state"
        );
        thread::sleep(Duration::from_millis(2));
    }
}
type Values = Arc<Mutex<VecDeque<u32>>>;
struct Packets {
    values: Values,
    idle: Option<mpsc::Sender<()>>,
    creator: mpsc::Sender<thread::ThreadId>,
}
impl Transport for Packets {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        match command {
            IoCommand::Hid(HidCommand::Open { .. }) => {
                self.creator.send(thread::current().id()).unwrap();
                Ok(connected())
            }
            IoCommand::Hid(HidCommand::ReadInputWait { timeout, .. }) => {
                let value = self.values.lock().unwrap().pop_front();
                if let Some(value) = value {
                    return Ok(data(value));
                }
                if let Some(idle) = self.idle.take() {
                    idle.send(()).unwrap();
                }
                thread::sleep(timeout);
                Ok(IoReply::Hid(HidReply::Input(None)))
            }
            _ => Err(IoError::NotSupported),
        }
    }
}
#[test]
fn one_second_worth_of_125hz_reports_is_preserved_in_fifo_without_loss() {
    let values = Values::new(Mutex::new((0..125).collect()));
    let (idle_tx, idle_rx) = mpsc::channel();
    let (creator_tx, creator_rx) = mpsc::channel();
    let capture = HidCapture::spawn(
        move || {
            Ok(Packets {
                values: Arc::clone(&values),
                idle: Some(idle_tx.clone()),
                creator: creator_tx.clone(),
            })
        },
        target(),
    )
    .unwrap();
    idle_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_ne!(
        creator_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        thread::current().id()
    );
    let batch = capture.try_drain(256).unwrap();
    assert_eq!(batch.reports.len(), 125);
    assert_eq!(batch.overflowed_total, 0);
    assert_eq!(batch.discarded_total, 0);
    for (index, frame) in batch.reports.iter().enumerate() {
        assert_eq!(frame.capture_id, capture.id());
        assert_eq!(frame.epoch, 1);
        assert_eq!(frame.sequence, index as u64 + 1);
        assert_eq!(frame.report.data, (index as u32).to_le_bytes());
    }
}
#[test]
fn overflow_is_bounded_explicit_and_keeps_the_newest_frames() {
    let count = HID_CAPTURE_CAPACITY as u32 + 37;
    let values = Values::new(Mutex::new((0..count).collect()));
    let (idle_tx, idle_rx) = mpsc::channel();
    let (creator_tx, _creator_rx) = mpsc::channel();
    let capture = HidCapture::spawn(
        move || {
            Ok(Packets {
                values: Arc::clone(&values),
                idle: Some(idle_tx.clone()),
                creator: creator_tx.clone(),
            })
        },
        target(),
    )
    .unwrap();
    idle_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let batch = capture.try_drain(usize::MAX).unwrap();
    assert_eq!(batch.reports.len(), HID_CAPTURE_CAPACITY);
    assert_eq!(batch.overflowed_total, 37);
    assert_eq!(batch.reports[0].report.data, 37u32.to_le_bytes());
    assert_eq!(
        batch.reports.last().unwrap().report.data,
        (count - 1).to_le_bytes()
    );
}

struct Reconnect {
    first: bool,
    delivered: bool,
    entered: mpsc::Sender<()>,
    gate: Option<mpsc::Receiver<()>>,
}
impl Transport for Reconnect {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        match command {
            IoCommand::Hid(HidCommand::Open { .. }) => Ok(connected()), // intentionally reuse ID to test epochs
            IoCommand::Hid(HidCommand::ReadInputWait { timeout, .. }) => {
                if !self.delivered {
                    self.delivered = true;
                    return Ok(data(if self.first { 1 } else { 2 }));
                }
                if let Some(gate) = self.gate.take() {
                    self.entered.send(()).unwrap();
                    gate.recv().unwrap();
                    return Err(IoError::Disconnected);
                }
                thread::sleep(timeout);
                Ok(IoReply::Hid(HidReply::Input(None)))
            }
            _ => Err(IoError::NotSupported),
        }
    }
}
#[test]
fn disconnect_discards_old_packets_and_reopen_changes_epoch_even_with_reused_backend_id() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut gate = Some(release_rx);
    let mut first = true;
    let capture = HidCapture::spawn(
        move || {
            let backend = Reconnect {
                first,
                delivered: false,
                entered: entered_tx.clone(),
                gate: gate.take(),
            };
            first = false;
            Ok(backend)
        },
        target(),
    )
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        wait_batch(&capture, |batch| matches!(
            batch.status,
            CaptureStatus::Connected { epoch: 1, .. }
        ))
        .status,
        CaptureStatus::Connected { .. }
    ));
    release_tx.send(()).unwrap();
    let failed = wait_batch(&capture, |batch| {
        matches!(batch.status, CaptureStatus::Reconnecting { .. })
    });
    assert_eq!(failed.discarded_total, 1);
    let status = wait_batch(&capture, |batch| {
        matches!(batch.status, CaptureStatus::Connected { epoch: 2, .. })
    });
    assert!(matches!(
        status.status,
        CaptureStatus::Connected {
            session: HidSessionId(1),
            ..
        }
    ));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(batch) = capture.try_drain(256) {
            if !batch.reports.is_empty() {
                assert_eq!(batch.reports.len(), 1);
                assert_eq!(batch.reports[0].epoch, 2);
                assert_eq!(batch.reports[0].report.data, 2u32.to_le_bytes());
                break;
            }
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
}

struct Fail {
    malformed: bool,
}
impl Transport for Fail {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        match command {
            IoCommand::Hid(HidCommand::Open { .. }) if !self.malformed => {
                Err(IoError::ProtectedInput)
            }
            IoCommand::Hid(HidCommand::Open { .. }) => Ok(connected()),
            IoCommand::Hid(HidCommand::ReadInputWait { .. }) => {
                Ok(IoReply::Hid(HidReply::Input(Some(HidReport {
                    report_id: 0,
                    data: vec![1],
                    logical_length: 1,
                }))))
            }
            _ => Err(IoError::NotSupported),
        }
    }
}
struct Maker {
    calls: Arc<AtomicUsize>,
    exited: mpsc::Sender<()>,
    malformed: bool,
}
impl Maker {
    fn make(&mut self) -> Result<Fail, IoError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Fail {
            malformed: self.malformed,
        })
    }
}
impl Drop for Maker {
    fn drop(&mut self) {
        let _ = self.exited.send(());
    }
}
#[test]
fn policy_and_malformed_packet_failures_stop_without_reopening_the_device() {
    for malformed in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let (exited_tx, exited_rx) = mpsc::channel();
        let mut maker = Maker {
            calls: Arc::clone(&calls),
            exited: exited_tx,
            malformed,
        };
        let capture = HidCapture::spawn(move || maker.make(), target()).unwrap();
        exited_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let batch = capture.try_drain(256).unwrap();
        let expected = if malformed {
            IoError::InvalidHidReport
        } else {
            IoError::ProtectedInput
        };
        assert!(matches!(batch.status, CaptureStatus::Failed { error, .. } if error == expected));
        assert!(batch.reports.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

struct Blocked {
    entered: Option<mpsc::Sender<()>>,
    gate: mpsc::Receiver<()>,
    closed: mpsc::Sender<thread::ThreadId>,
}
impl Transport for Blocked {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        match command {
            IoCommand::Hid(HidCommand::Open { .. }) => Ok(connected()),
            IoCommand::Hid(HidCommand::ReadInputWait { .. }) => {
                self.entered.take().unwrap().send(()).unwrap();
                self.gate.recv().unwrap();
                Ok(data(99))
            }
            _ => Err(IoError::NotSupported),
        }
    }
}
impl Drop for Blocked {
    fn drop(&mut self) {
        let _ = self.closed.send(thread::current().id());
    }
}
#[test]
fn stop_never_waits_for_an_uncooperative_driver_and_resources_close_off_caller_thread() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (closed_tx, closed_rx) = mpsc::channel();
    let mut backend = Some(Blocked {
        entered: Some(entered_tx),
        gate: release_rx,
        closed: closed_tx,
    });
    let capture = HidCapture::spawn(move || Ok(backend.take().unwrap()), target()).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (dropped_tx, dropped_rx) = mpsc::channel();
    let dropper = thread::spawn(move || {
        drop(capture);
        dropped_tx.send(()).unwrap();
    });
    let dropped = dropped_rx.recv_timeout(Duration::from_millis(200));
    release_tx.send(()).unwrap();
    dropper.join().unwrap();
    assert!(dropped.is_ok(), "capture Drop waited for driver");
    assert_ne!(
        closed_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        thread::current().id()
    );
}

#[test]
fn protected_target_and_unbounded_input_wait_are_rejected_before_capture_start() {
    let selected = HidTarget {
        usage_page: 1,
        usage: 6,
        ..target()
    };
    let factory = || -> Result<Fail, IoError> {
        panic!("protected target must not initialize native backend")
    };
    assert!(HidCapture::spawn(factory, selected).is_err());
    for timeout in [
        Duration::ZERO,
        Duration::from_micros(999),
        Duration::from_millis(9),
    ] {
        assert!(HidCommand::ReadInputWait {
            session: HidSessionId(1),
            timeout
        }
        .validate()
        .is_err());
    }
    assert!(HidCommand::ReadInputWait {
        session: HidSessionId(1),
        timeout: Duration::from_millis(5)
    }
    .validate()
    .is_ok());
}

struct Closing {
    delivered: bool,
    entered: mpsc::Sender<()>,
    gate: mpsc::Receiver<()>,
}
impl Transport for Closing {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        match command {
            IoCommand::Hid(HidCommand::Open { .. }) => Ok(connected()),
            IoCommand::Hid(HidCommand::ReadInputWait { .. }) if !self.delivered => {
                self.delivered = true;
                Ok(data(1))
            }
            _ => Err(IoError::Disconnected),
        }
    }
}
impl Drop for Closing {
    fn drop(&mut self) {
        let _ = self.entered.send(());
        let _ = self.gate.recv();
    }
}
#[test]
fn connection_loss_and_queue_invalidation_precede_blocking_native_cleanup() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut backend = Some(Closing {
        delivered: false,
        entered: entered_tx,
        gate: release_rx,
    });
    let capture = HidCapture::spawn(move || Ok(backend.take().unwrap()), target()).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    // Read the status while native resource destruction is still blocked.
    let batch = capture.try_drain(256);
    drop(capture);
    release_tx.send(()).unwrap();
    let batch = batch.unwrap();
    assert!(matches!(
        batch.status,
        CaptureStatus::Reconnecting {
            epoch: 1,
            error: IoError::Disconnected,
            ..
        }
    ));
    assert!(batch.reports.is_empty());
    assert_eq!(batch.discarded_total, 1);
}
