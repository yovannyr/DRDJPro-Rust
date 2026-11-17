#![cfg(any(feature = "ort", feature = "tract"))]

use drdj_ml::{load_backend, BackendKind, InferenceError, Threads, INPUT_LEN};
use sha2::{Digest, Sha256};
use std::{env, path::PathBuf};

#[test]
fn generated_conv_gru_matches_independent_scalar_reference_and_resets_state() {
    let model = PathBuf::from(
        env::var_os("DRDJ_ML_FIXTURE")
            .expect("generate fixture with tools/ml_fixture.py and set DRDJ_ML_FIXTURE"),
    );
    let runtime = env::var_os("DRDJ_ORT_LIBRARY").map(PathBuf::from);
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("reference.json")).unwrap();
    let tolerance = reference["absolute_tolerance"].as_f64().unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(std::fs::read(&model).unwrap())),
        reference["model_sha256"].as_str().unwrap()
    );
    let kinds = [
        #[cfg(feature = "ort")]
        BackendKind::Ort,
        #[cfg(feature = "tract")]
        BackendKind::Tract,
    ];
    for kind in kinds {
        for threads in [1, 2, 4] {
            let mut backend = load_backend(
                kind,
                &model,
                Threads::new(threads).unwrap(),
                runtime.as_deref(),
            )
            .unwrap();
            for _ in 0..2 {
                for case in reference["cases"].as_array().unwrap() {
                    let input: Vec<_> = case["input"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_f64().unwrap() as f32)
                        .collect();
                    let expected = case["output"].as_array().unwrap();
                    let output = backend.infer(&input).unwrap();
                    assert_eq!(output.len(), expected.len());
                    for (actual, expected) in output.iter().zip(expected) {
                        assert!(
                            (f64::from(*actual) - expected.as_f64().unwrap()).abs() <= tolerance,
                            "{kind:?}/{threads}: {output:?}"
                        );
                    }
                }
            }
            assert_eq!(backend.infer(&[]), Err(InferenceError::InvalidInput));
            assert_eq!(
                backend.infer(&[f32::NAN; INPUT_LEN]),
                Err(InferenceError::InvalidInput)
            );
            assert_eq!(
                backend.infer(&[f32::INFINITY; INPUT_LEN]),
                Err(InferenceError::InvalidInput)
            );
        }
    }
}

#[test]
fn corrupt_or_oversized_graph_never_yields_a_backend() {
    let runtime = env::var_os("DRDJ_ORT_LIBRARY").map(PathBuf::from);
    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("corrupt.onnx");
    std::fs::write(&model, b"invalid graph").unwrap();
    for kind in [
        #[cfg(feature = "ort")]
        BackendKind::Ort,
        #[cfg(feature = "tract")]
        BackendKind::Tract,
    ] {
        assert!(load_backend(kind, &model, Threads::new(1).unwrap(), runtime.as_deref()).is_err());
    }
    std::fs::write(&model, vec![0u8; 65_537]).unwrap();
    assert!(matches!(
        load_backend(
            BackendKind::Ort,
            &model,
            Threads::new(1).unwrap(),
            runtime.as_deref()
        ),
        Err(InferenceError::ModelUnavailable)
    ));
}

#[cfg(feature = "ort")]
#[test]
fn ort_does_not_search_for_or_download_a_missing_runtime() {
    let model = PathBuf::from(env::var_os("DRDJ_ML_FIXTURE").unwrap());
    assert!(matches!(
        load_backend(BackendKind::Ort, &model, Threads::new(1).unwrap(), None),
        Err(InferenceError::RuntimeUnavailable)
    ));
}

#[cfg(feature = "ort")]
#[test]
fn ort_rejects_a_different_existing_library_after_initialization() {
    let model = PathBuf::from(env::var_os("DRDJ_ML_FIXTURE").unwrap());
    let library = PathBuf::from(env::var_os("DRDJ_ORT_LIBRARY").unwrap());
    let _first = load_backend(
        BackendKind::Ort,
        &model,
        Threads::new(1).unwrap(),
        Some(&library),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("different-library.dll");
    std::fs::write(&other, b"not a runtime").unwrap();
    assert!(matches!(
        load_backend(
            BackendKind::Ort,
            &model,
            Threads::new(1).unwrap(),
            Some(&other)
        ),
        Err(InferenceError::RuntimeConflict)
    ));
}
