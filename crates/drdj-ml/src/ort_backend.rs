use super::*;
use ort::{environment::Environment, session::Session, value::Tensor};
use std::{path::PathBuf, sync::Mutex};

// ORT's library loader is process-wide. This isolated crate must own ORT
// initialization; other direct ort clients must not initialize a different DLL.
// This ort release permits only one Environment per process; sessions share it.
static LOADED_LIBRARY: Mutex<Option<(PathBuf, Option<Environment>)>> = Mutex::new(None);

struct OrtBackend {
    session: Session,
    _environment: Environment,
}

fn error(error: impl fmt::Display) -> InferenceError {
    InferenceError::Backend(error.to_string())
}

pub(super) fn load(
    model: &Path,
    threads: Threads,
    runtime_library: Option<&Path>,
) -> Result<Box<dyn InferenceBackend>, InferenceError> {
    let library = runtime_library
        .filter(|p| p.is_file())
        .ok_or(InferenceError::RuntimeUnavailable)?;
    let library = library
        .canonicalize()
        .map_err(|_| InferenceError::RuntimeUnavailable)?;
    let mut loaded = LOADED_LIBRARY
        .lock()
        .map_err(|_| InferenceError::RuntimeUnavailable)?;
    if loaded
        .as_ref()
        .is_some_and(|(previous, _)| previous != &library)
    {
        return Err(InferenceError::RuntimeConflict);
    }
    let environment = match loaded
        .as_ref()
        .and_then(|(_, environment)| environment.as_ref())
    {
        Some(environment) => environment.clone(),
        None => {
            let builder = ort::init_from(&library)
                .map_err(error)?
                .with_telemetry(false);
            // init_from installed the DLL even if Environment creation fails.
            *loaded = Some((library.clone(), None));
            let environment = builder.build().map_err(error)?;
            *loaded = Some((library, Some(environment.clone())));
            environment
        }
    };
    drop(loaded);
    let session = Session::builder(&environment)
        .map_err(error)?
        .with_intra_threads(threads.get())
        .map_err(error)?
        .with_inter_threads(1)
        .map_err(error)?
        .with_parallel_execution(false)
        .map_err(error)?
        .commit_from_file(model)
        .map_err(error)?;
    Ok(Box::new(OrtBackend {
        session,
        _environment: environment,
    }))
}

impl InferenceBackend for OrtBackend {
    fn infer(&mut self, input: &[f32]) -> Result<Vec<f32>, InferenceError> {
        validate_input(input)?;
        let input = Tensor::from_array(([1usize, 1, INPUT_LEN], input.to_vec())).map_err(error)?;
        let outputs = self.session.run(ort::inputs![input]).map_err(error)?;
        if outputs.len() != 1 {
            return Err(InferenceError::InvalidOutput);
        }
        let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(error)?;
        if shape.as_ref() != [6, 1, 1, 2] {
            return Err(InferenceError::InvalidOutput);
        }
        validate_output(values.to_vec())
    }
}
