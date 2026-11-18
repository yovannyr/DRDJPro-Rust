//! Background load for the opt-in headless playback soak.
#[cfg(feature = "ort")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use drdj_ml::{
        InferenceJob, InferenceService, JobError, ModelId, ModelRegistry, ModelSpec, ModelUse,
        Priority, INPUT_LEN, MAX_CHUNKS,
    };
    use sha2::{Digest, Sha256};
    use std::{
        env, fs,
        path::PathBuf,
        thread,
        time::{Duration, Instant},
    };
    let root = PathBuf::from(
        env::args_os()
            .nth(1)
            .ok_or("expected isolated soak directory")?,
    );
    let model = PathBuf::from(env::var_os("DRDJ_ML_FIXTURE").ok_or("DRDJ_ML_FIXTURE missing")?);
    let runtime = PathBuf::from(env::var_os("DRDJ_ORT_LIBRARY").ok_or("DRDJ_ORT_LIBRARY missing")?);
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../tests/reference.json"))?;
    let digest = Sha256::digest(fs::read(&model)?);
    if Some(format!("{digest:x}").as_str()) != reference["model_sha256"].as_str() {
        return Err("unexpected fixture hash".into());
    }
    let id = ModelId {
        name: "conv-gru".into(),
        version: 1,
    };
    let mut registry = ModelRegistry::default();
    registry.register(ModelSpec {
        id: id.clone(),
        path: model,
        sha256: digest.into(),
        license: "MIT".into(),
        allowed_use: ModelUse::SyntheticTest,
        minimum_ram_bytes: 64 * 1024 * 1024,
    })?;
    let service = InferenceService::start(registry, root.join("cache"), runtime, 64 * 1024 * 1024)?;
    let mut completed = 0u64;
    let stop = root.join("stop");
    let ready = root.join("ready");
    let deadline = Instant::now() + Duration::from_secs(900);
    while !stop.exists() && Instant::now() < deadline {
        let mut hash = [0u8; 32];
        hash[..8].copy_from_slice(&completed.to_le_bytes());
        let mut ticket = match service.submit(InferenceJob {
            model: id.clone(),
            audio_sha256: hash,
            chunks: vec![[0.25; INPUT_LEN]; MAX_CHUNKS],
            priority: Priority::Normal,
        }) {
            Ok(ticket) => ticket,
            Err(JobError::QueueBusy) => {
                thread::yield_now();
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        let timeout = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(result) = ticket.try_result() {
                if result?.from_cache {
                    return Err("load must execute inference".into());
                }
                break;
            }
            if Instant::now() >= timeout {
                return Err("worker stalled".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        completed += 1;
        if completed == 1 {
            fs::write(&ready, [])?;
        }
    }
    fs::write(root.join("completed"), completed.to_string())?;
    Ok(())
}

#[cfg(not(feature = "ort"))]
fn main() {
    eprintln!("a1_load requires the ort feature");
    std::process::exit(2);
}
