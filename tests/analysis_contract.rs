//! Public analysis contracts consumed without desktop startup or audio devices.
use drdjpro_ui_spike::analysis::{
    corrections::KeyAnalysisEngine,
    energy,
    keys::{CamelotKey, HarmonieWheel, KeyCompatibility},
};

#[test]
fn key_clients_share_camelot_types_across_compatibility_and_corrections() {
    let c_major = CamelotKey::parse("8B").unwrap();
    let a_minor = CamelotKey::parse("8A").unwrap();
    let wheel = HarmonieWheel::new();
    assert_eq!(
        wheel.get_compatibility(c_major, a_minor).compatibility,
        KeyCompatibility::Relative
    );
    let engine = KeyAnalysisEngine::new();
    engine.analyze_key("track", c_major, 0.95);
    engine.apply_override("track", a_minor, None, true);
    let latest = CamelotKey::parse("12B").unwrap();
    engine.analyze_key("track", latest, 0.8);
    assert_eq!(engine.get_analysis("track").unwrap().key, a_minor);
    engine.remove_override("track");
    assert_eq!(engine.get_analysis("track").unwrap().key, latest);
    assert_eq!(latest.to_standard(), "E Major");
}

#[test]
fn pcm_clients_get_input_errors_and_distinguish_signal_from_silence() {
    let config = energy::Config {
        min_samples: 44100 * 10,
        sample_rate: 44100,
    };
    let silence = vec![0.0; 44100 * 11];
    let tone: Vec<_> = (0..silence.len())
        .map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / 44100.0).sin() * 0.5)
        .collect();
    let quiet = energy::analyze_energy(&silence, 1, 44100, &config).unwrap();
    let signal = energy::analyze_energy(&tone, 1, 44100, &config).unwrap();
    assert!(signal.integrated_loudness > quiet.integrated_loudness);
    assert!(signal.confidence > quiet.confidence);
    assert!(energy::analyze_energy(&tone, 0, 44100, &config).is_err());
}
