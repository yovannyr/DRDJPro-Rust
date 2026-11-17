use super::*;
use std::sync::Arc;
use tract_linalg::multithread::{multithread_tract_scope, Executor};
use tract_onnx::prelude::*;

struct TractBackend {
    model: Arc<TypedRunnableModel>,
    executor: Executor,
}

fn error(error: impl fmt::Display) -> InferenceError {
    InferenceError::Backend(error.to_string())
}

pub(super) fn load(
    model: &Path,
    threads: Threads,
) -> Result<Box<dyn InferenceBackend>, InferenceError> {
    let executor = if threads.get() == 1 {
        Executor::SingleThread
    } else {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .build()
            .map_err(error)?;
        Executor::MultiThread(Arc::new(pool))
    };
    let model = multithread_tract_scope(executor.clone(), || {
        tract_onnx::onnx()
            .model_for_path(model)?
            .into_optimized()?
            .into_runnable()
    })
    .map_err(error)?;
    Ok(Box::new(TractBackend { model, executor }))
}

impl InferenceBackend for TractBackend {
    fn infer(&mut self, input: &[f32]) -> Result<Vec<f32>, InferenceError> {
        validate_input(input)?;
        let input = Tensor::from_shape(&[1, 1, INPUT_LEN], input).map_err(error)?;
        let outputs = multithread_tract_scope(self.executor.clone(), || {
            self.model.run(tvec!(input.into()))
        })
        .map_err(error)?;
        if outputs.len() != 1 || outputs[0].shape() != [6, 1, 1, 2] {
            return Err(InferenceError::InvalidOutput);
        }
        validate_output(
            outputs[0]
                .to_plain_array_view::<f32>()
                .map_err(error)?
                .iter()
                .copied()
                .collect(),
        )
    }
}
