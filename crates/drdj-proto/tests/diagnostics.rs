use drdj_proto::*;
use std::sync::Mutex;

struct Capture(Mutex<Vec<String>>);
static LOGS: Capture = Capture(Mutex::new(Vec::new()));
impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target() == "drdj_proto"
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            self.0.lock().unwrap().push(record.args().to_string());
        }
    }
    fn flush(&self) {}
}

#[test]
fn diagnostics_identify_fields_and_deprecation_without_echoing_private_values() {
    log::set_logger(&LOGS).unwrap();
    log::set_max_level(log::LevelFilter::Warn);
    let t = TrackMessage {
        file_path: "private-file-path".into(),
        analysis_hash: "private-hash".into(),
        ..Default::default()
    };
    encode_checked(&t).unwrap();
    let invalid = TrackMessage { bpm: f64::NAN, ..t };
    assert!(encode_checked(&invalid).is_err());
    assert!(decode_checked::<TrackMessage>(&[]).is_err());
    assert!(from_json::<TrackMessage>(r#"{"bpm":"private-invalid-number"}"#).is_err());
    let logs = LOGS.0.lock().unwrap().join("\n");
    assert!(logs.contains("deprecated field TrackMessage.analysis_hash"));
    assert!(logs.contains("bpm"));
    assert!(logs.contains("required field missing"));
    assert!(!logs.contains("private-"), "{logs}");
}
