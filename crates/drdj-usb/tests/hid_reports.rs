use drdj_usb::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

// Vendor-defined collection, 3 Input, 2 Output and 4 Feature payload bytes.
const SIMPLE: &[u8] = &[
    0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 3, 0x81, 2, 0x95, 2, 0x91, 2, 0x95, 4, 0xb1, 2,
    0xc0,
];
fn target() -> HidTarget {
    HidTarget {
        path: b"fixture-controller".to_vec(),
        vendor_id: 0x1234,
        product_id: 1,
        interface: 1,
        usage_page: 0xff00,
        usage: 1,
    }
}

#[test]
fn descriptor_separates_input_output_and_feature_lengths() {
    let layout = ReportLayout::parse(SIMPLE).unwrap();
    assert_eq!(layout.input_reports().get(&0), Some(&3));
    assert_eq!(layout.output_reports().get(&0), Some(&2));
    assert_eq!(layout.feature_reports().get(&0), Some(&4));
    assert!(!layout.is_numbered());
    assert!(!layout.contains_system_input());
    assert_eq!(layout.input_capacity(), Some(3));
    assert_eq!(layout.input_identity(&[1, 2, 3]).unwrap(), (0, 3));
    assert!(layout.input_identity(&[1, 2]).is_err());
    assert!(layout.input_identity(&[1, 2, 3, 4]).is_err());
}

#[test]
fn bit_fields_round_only_after_accumulation_and_numbered_padding_stays_explicit() {
    let descriptor = [
        0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x85, 1, 0x75, 1, 0x95, 3, 0x81, 2, 0x95, 5, 0x81, 3,
        0x85, 2, 0x75, 8, 0x95, 4, 0x81, 2, 0xc0,
    ];
    let layout = ReportLayout::parse(&descriptor).unwrap();
    assert_eq!(layout.input_reports().get(&1), Some(&1));
    assert_eq!(layout.input_reports().get(&2), Some(&4));
    assert_eq!(layout.input_capacity(), Some(5));
    assert_eq!(layout.input_identity(&[1, 0xff]).unwrap(), (1, 2));
    assert_eq!(layout.input_identity(&[1, 0xff, 0, 0, 0]).unwrap(), (1, 2));
    assert_eq!(layout.input_identity(&[2, 1, 2, 3, 4]).unwrap(), (2, 5));
    assert!(layout.input_identity(&[3, 0]).is_err());
    assert!(layout.input_identity(&[2, 1]).is_err());
}

#[test]
fn global_push_pop_restores_size_and_count() {
    let descriptor = [
        0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 2, 0xa4, 0x75, 1, 0x95, 8, 0x81, 2, 0xb4,
        0x81, 2, 0xc0,
    ];
    assert_eq!(
        ReportLayout::parse(&descriptor)
            .unwrap()
            .input_reports()
            .get(&0),
        Some(&3)
    );
}

#[test]
fn malformed_descriptors_and_extreme_sizes_never_turn_into_report_allocations() {
    for descriptor in [
        vec![],
        vec![0x06],
        vec![0xfe, 1, 0],
        vec![0xb4],
        vec![0xc0],
        vec![0x0c],
        vec![0x85, 0],
        vec![0x86, 0, 1],
    ] {
        assert!(ReportLayout::parse(&descriptor).is_err(), "{descriptor:?}");
    }
    for end in 0..SIMPLE.len() {
        assert!(
            ReportLayout::parse(&SIMPLE[..end]).is_err(),
            "accepted prefix {end}"
        );
    }
    let overflow = [
        0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x77, 255, 255, 255, 255, 0x95, 2, 0x81, 2, 0xc0,
    ];
    assert!(ReportLayout::parse(&overflow).is_err());
    let mixed = [
        0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 1, 0x81, 2, 0x85, 1, 0x81, 2, 0xc0,
    ];
    assert!(ReportLayout::parse(&mixed).is_err());
}

#[test]
fn protected_system_usages_are_detected_in_direct_and_ranged_forms() {
    let mouse = [0x05, 1, 0x09, 2, 0xa1, 1, 0x75, 8, 0x95, 3, 0x81, 2, 0xc0];
    let keyboard = [0x05, 7, 0x09, 1, 0xa1, 1, 0x75, 8, 0x95, 8, 0x81, 2, 0xc0];
    let range = [
        0x05, 1, 0x19, 1, 0x29, 8, 0xa1, 1, 0x75, 8, 0x95, 1, 0x81, 2, 0xc0,
    ];
    for descriptor in [&mouse[..], &keyboard[..], &range[..]] {
        assert!(ReportLayout::parse(descriptor)
            .unwrap()
            .contains_system_input());
    }
}

struct Fake(Arc<AtomicUsize>);
impl Transport for Fake {
    fn execute(&mut self, command: IoCommand) -> Result<IoReply, IoError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let IoCommand::Hid(command) = command else {
            return Err(IoError::NotSupported);
        };
        Ok(IoReply::Hid(match command {
            HidCommand::Enumerate => HidReply::Devices(vec![HidInfo {
                target: target(),
                bus: HidBus::Usb,
                manufacturer: Some("Test".into()),
                product: Some("Fixture".into()),
                serial: None,
                string_warning: false,
            }]),
            HidCommand::Open { .. } => HidReply::Opened {
                session: HidSessionId(1),
                layout: ReportLayout::parse(SIMPLE)?,
            },
            HidCommand::ReadInput { .. } => HidReply::Input(Some(HidReport {
                report_id: 0,
                data: vec![1, 2, 3],
                logical_length: 3,
            })),
            HidCommand::Close { .. } => HidReply::Closed,
            _ => return Err(IoError::NotSupported),
        }))
    }
}
fn wait(ticket: &mut HidTicket) -> Result<HidReply, IoError> {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(reply) = ticket.poll() {
            return reply;
        }
        assert!(Instant::now() < until, "HID completion missing");
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn hid_actor_enumerates_without_opening_and_only_captures_after_explicit_open_and_read() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let worker = HidWorker::spawn(move || Ok(Fake(Arc::clone(&count)))).unwrap();
    let devices = wait(&mut worker.submit(HidCommand::Enumerate).unwrap()).unwrap();
    assert!(matches!(devices, HidReply::Devices(ref devices) if devices.len() == 1));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let opened = wait(
        &mut worker
            .submit(HidCommand::Open { target: target() })
            .unwrap(),
    )
    .unwrap();
    let HidReply::Opened { session, .. } = opened else {
        panic!("not opened")
    };
    let report = wait(&mut worker.submit(HidCommand::ReadInput { session }).unwrap()).unwrap();
    assert!(matches!(report, HidReply::Input(Some(HidReport { data, .. })) if data == vec![1,2,3]));
    assert_eq!(
        wait(&mut worker.submit(HidCommand::Close { session }).unwrap()).unwrap(),
        HidReply::Closed
    );
}

#[test]
fn invalid_and_system_input_requests_are_rejected_before_backend_initialization() {
    let creations = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&creations);
    let worker = HidWorker::spawn(move || {
        count.fetch_add(1, Ordering::SeqCst);
        Ok(Fake(Arc::new(AtomicUsize::new(0))))
    })
    .unwrap();
    for (page, usage) in [(1, 2), (1, 6), (1, 7), (1, 0x80), (7, 1)] {
        let selected = HidTarget {
            usage_page: page,
            usage,
            ..target()
        };
        assert!(matches!(
            worker.submit(HidCommand::Open { target: selected }),
            Err(IoError::ProtectedInput)
        ));
    }
    let invalid = HidTarget {
        path: vec![b'a', 0, b'b'],
        ..target()
    };
    assert!(worker.submit(HidCommand::Open { target: invalid }).is_err());
    assert!(worker
        .submit(HidCommand::WriteOutput {
            session: HidSessionId(1),
            report: Vec::new()
        })
        .is_err());
    assert!(worker
        .submit(HidCommand::SendFeature {
            session: HidSessionId(1),
            report: vec![0; MAX_HID_REPORT_BYTES + 1]
        })
        .is_err());
    assert_eq!(creations.load(Ordering::SeqCst), 0);
}
