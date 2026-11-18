#![cfg(feature = "ort")]

use drdj_ml::{
    InferenceJob, InferenceService, JobError, ModelId, ModelRegistry, ModelSpec, ModelUse, Priority,
};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

#[test]
fn ort_service_matches_fixture_and_persists_only_complete_results() {
    let source = PathBuf::from(env::var_os("DRDJ_ML_FIXTURE").unwrap());
    let runtime = PathBuf::from(env::var_os("DRDJ_ORT_LIBRARY").unwrap());
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("model.onnx");
    fs::copy(source, &path).unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("reference.json")).unwrap();
    let id = ModelId {
        name: "conv-gru".into(),
        version: 1,
    };
    let mut registry = ModelRegistry::default();
    let digest = Sha256::digest(fs::read(&path).unwrap());
    assert_eq!(
        format!("{digest:x}"),
        reference["model_sha256"].as_str().unwrap()
    );
    registry
        .register(ModelSpec {
            id: id.clone(),
            path: path.clone(),
            sha256: digest.into(),
            license: "MIT".into(),
            allowed_use: ModelUse::SyntheticTest,
            minimum_ram_bytes: 64 * 1024 * 1024,
        })
        .unwrap();
    let root = temp.path().join("cache");
    let service = InferenceService::start(registry, root, runtime, 64 * 1024 * 1024).unwrap();
    let cases = reference["cases"].as_array().unwrap();
    for from_cache in [false, true] {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut ticket = loop {
            let chunks = cases
                .iter()
                .map(|case| std::array::from_fn(|i| case["input"][i].as_f64().unwrap() as f32))
                .collect();
            match service.submit(InferenceJob {
                model: id.clone(),
                audio_sha256: [4; 32],
                chunks,
                priority: Priority::Normal,
            }) {
                Ok(ticket) => break ticket,
                Err(JobError::QueueBusy) if Instant::now() < deadline => thread::yield_now(),
                Err(error) => panic!("{error}"),
            }
        };
        let output = loop {
            if let Some(result) = ticket.try_result() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(ticket.progress(), (cases.len(), cases.len()));
        assert_eq!(output.from_cache, from_cache);
        assert_eq!(output.chunks.len(), cases.len());
        for (values, case) in output.chunks.iter().zip(cases) {
            for (actual, expected) in values.iter().zip(case["output"].as_array().unwrap()) {
                assert!((f64::from(*actual) - expected.as_f64().unwrap()).abs() < 1e-5);
            }
        }
        if !from_cache {
            fs::remove_file(&path).unwrap();
        }
    }
}
