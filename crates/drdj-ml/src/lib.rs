//! A0 CPU backend experiment, optionally used by the application's import worker.
//!
//! Only the generated, trusted Conv/GRU fixture contract is supported: eight f32
//! inputs, twelve f32 outputs. This is neither a beat detector nor a realtime API.
//! Calls allocate and can block; use only on an owned, non-audio thread.

use std::{fmt, path::Path};

#[cfg(feature = "ort")]
pub mod beat_tracking;
#[cfg(feature = "ort")]
mod cache;
#[cfg(feature = "ort")]
mod ort_backend;
#[cfg(feature = "ort")]
mod registry;
#[cfg(feature = "ort")]
mod service;
#[cfg(feature = "ort")]
pub use registry::{ModelId, ModelRegistry, ModelSpec, ModelUse};
#[cfg(feature = "ort")]
pub use service::{
    InferenceJob, InferenceService, JobError, JobOutput, Priority, Ticket, MAX_CHUNKS, MAX_PENDING,
};
#[cfg(feature = "tract")]
mod tract_backend;

/// Fixed input contract of the synthetic A0 graph.
pub const INPUT_LEN: usize = 8;
/// Fixed flattened GRU sequence output, shape [6, 1, 1, 2].
pub const OUTPUT_LEN: usize = 12;

/// Optional CPU runtimes. No runtime is linked or loaded by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Explicitly loaded ONNX Runtime CPU library.
    Ort,
    /// Self-contained tract CPU backend.
    Tract,
}

/// Requested maximum parallelism for this small experiment.
#[derive(Debug, Clone, Copy)]
pub struct Threads(usize);

impl Threads {
    /// Accept only the experiment's one-, two- or four-thread configurations.
    pub fn new(value: usize) -> Result<Self, InferenceError> {
        match value {
            1 | 2 | 4 => Ok(Self(value)),
            _ => Err(InferenceError::InvalidThreads),
        }
    }

    /// Configured thread count; small graphs may execute serially.
    pub fn get(self) -> usize {
        self.0
    }
}

/// Explicit failures, never fabricated model output.
#[derive(Debug, PartialEq, Eq)]
pub enum InferenceError {
    /// Compile-time backend feature is not enabled.
    BackendUnavailable,
    /// A0 supports one, two or four threads.
    InvalidThreads,
    /// Input is the wrong size or contains NaN/infinity.
    InvalidInput,
    /// The trusted local model cannot be read or exceeds the spike size budget.
    ModelUnavailable,
    /// ORT requires an explicitly supplied library path.
    RuntimeUnavailable,
    /// Another ORT library is already owned by this experiment in this process.
    RuntimeConflict,
    /// Runtime initialization, graph loading or inference failed.
    Backend(String),
    /// Runtime output violates the fixture's type, shape or finiteness contract.
    InvalidOutput,
}

impl fmt::Display for InferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for InferenceError {}

/// Replaceable, worker-owned backend for the synthetic A0 graph only.
pub trait InferenceBackend {
    /// Run one complete fixture graph. State does not carry across calls.
    fn infer(&mut self, input: &[f32]) -> Result<Vec<f32>, InferenceError>;
}

/// Load a trusted, locally generated fixture; never downloads models or libraries.
///
/// This spike is not a sandbox for arbitrary ONNX graphs or external weight files.
/// `runtime_library` is required only for ORT and must remain consistent within
/// one process, because its dynamic library loader is process-wide.
pub fn load_backend(
    kind: BackendKind,
    model: &Path,
    threads: Threads,
    runtime_library: Option<&Path>,
) -> Result<Box<dyn InferenceBackend>, InferenceError> {
    let _ = (threads, runtime_library);
    // Fail before invoking a parser for missing/non-file/oversized local input.
    let metadata = model
        .metadata()
        .map_err(|_| InferenceError::ModelUnavailable)?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 65_536 {
        return Err(InferenceError::ModelUnavailable);
    }
    match kind {
        BackendKind::Ort => {
            #[cfg(feature = "ort")]
            {
                ort_backend::load(model, threads, runtime_library)
            }
            #[cfg(not(feature = "ort"))]
            {
                Err(InferenceError::BackendUnavailable)
            }
        }
        BackendKind::Tract => {
            #[cfg(feature = "tract")]
            {
                tract_backend::load(model, threads)
            }
            #[cfg(not(feature = "tract"))]
            {
                Err(InferenceError::BackendUnavailable)
            }
        }
    }
}

#[cfg(any(feature = "ort", feature = "tract", test))]
fn validate_input(input: &[f32]) -> Result<(), InferenceError> {
    if input.len() != INPUT_LEN || input.iter().any(|x| !x.is_finite()) {
        Err(InferenceError::InvalidInput)
    } else {
        Ok(())
    }
}

#[cfg(any(feature = "ort", feature = "tract", test))]
fn validate_output(output: Vec<f32>) -> Result<Vec<f32>, InferenceError> {
    if output.len() != OUTPUT_LEN || output.iter().any(|x| !x.is_finite()) {
        Err(InferenceError::InvalidOutput)
    } else {
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_threads_and_tensor_contracts() {
        for n in [1, 2, 4] {
            assert_eq!(Threads::new(n).unwrap().get(), n);
        }
        for n in [0, 3, usize::MAX] {
            assert_eq!(Threads::new(n).unwrap_err(), InferenceError::InvalidThreads);
        }
        assert!(validate_input(&[0.0; INPUT_LEN]).is_ok());
        assert_eq!(validate_input(&[]), Err(InferenceError::InvalidInput));
        assert_eq!(
            validate_input(&[f32::NAN; INPUT_LEN]),
            Err(InferenceError::InvalidInput)
        );
        assert_eq!(
            validate_input(&[f32::INFINITY; INPUT_LEN]),
            Err(InferenceError::InvalidInput)
        );
        assert!(validate_output(vec![0.0; OUTPUT_LEN]).is_ok());
        assert_eq!(validate_output(vec![]), Err(InferenceError::InvalidOutput));
        assert_eq!(
            validate_output(vec![f32::NAN; OUTPUT_LEN]),
            Err(InferenceError::InvalidOutput)
        );
    }

    #[test]
    fn missing_model_is_a_defined_error() {
        for kind in [BackendKind::Ort, BackendKind::Tract] {
            assert!(matches!(
                load_backend(
                    kind,
                    Path::new("absent-a0-model.onnx"),
                    Threads::new(1).unwrap(),
                    None
                ),
                Err(InferenceError::ModelUnavailable)
            ));
        }
    }

    #[cfg(not(all(feature = "ort", feature = "tract")))]
    #[test]
    fn disabled_backends_do_not_initialize_a_runtime() {
        let model = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/reference.json"));
        #[cfg(not(feature = "ort"))]
        assert!(matches!(
            load_backend(BackendKind::Ort, model, Threads::new(1).unwrap(), None),
            Err(InferenceError::BackendUnavailable)
        ));
        #[cfg(not(feature = "tract"))]
        assert!(matches!(
            load_backend(BackendKind::Tract, model, Threads::new(1).unwrap(), None),
            Err(InferenceError::BackendUnavailable)
        ));
    }
}
