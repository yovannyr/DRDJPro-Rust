#![cfg(feature = "ort")]
use drdj_ml::beat_tracking::Tracker;
use std::{env, fs, path::PathBuf};

#[test]
fn rejects_unverified_frontend_before_runtime_initialization() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("logmel.onnx"), b"wrong").unwrap();
    assert!(matches!(
        Tracker::load(tmp.path(), &tmp.path().join("absent.dll")),
        Err(drdj_ml::InferenceError::ModelUnavailable)
    ));
}

/// Use the externally supplied test assets. The first 36 s span a window seam.
#[test]
#[ignore = "requires pinned small0 models, ORT and Candombe 46 audio and beat annotations"]
fn annotated_performance_tracks_across_window_boundary() {
    let root = PathBuf::from(env::var_os("DRDJ_BEAT_TEST_DIR").unwrap());
    let mut model = Tracker::load(
        &root,
        &PathBuf::from(env::var_os("DRDJ_ORT_LIBRARY").unwrap()),
    )
    .unwrap();
    let output = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(root.join("46.flac"))
        .args(["-t", "36", "-ac", "1", "-ar", "22050", "-f", "f32le", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let e = model.track(output.stdout.as_slice()).unwrap();
    let annotations = fs::read_to_string(root.join("46.beats")).unwrap();
    for (predictions, downbeats) in [(&e.beats_ms, false), (&e.downbeats_ms, true)] {
        let mut times = Vec::new();
        for line in annotations.lines() {
            let (time, beat) = line.split_once('\t').unwrap();
            let t = time.parse::<f64>().unwrap();
            if t < 36.0 && (!downbeats || beat == "1") {
                times.push(t * 1000.0);
            }
        }
        assert!(times.last().unwrap() > &30000.0);
        let mut it = predictions.iter().copied().peekable();
        let mut matches = 0;
        for t in &times {
            while it.next_if(|p| f64::from(*p) < t - 70.0).is_some() {}
            matches += usize::from(it.next_if(|p| f64::from(*p) <= t + 70.0).is_some());
        }
        assert!(2.0 * matches as f64 / (times.len() + predictions.len()) as f64 > 0.95);
    }
    assert!(model.track(&[0u8; 3][..]).is_err());
    assert!(model.track(f32::NAN.to_le_bytes().as_slice()).is_err());
}
