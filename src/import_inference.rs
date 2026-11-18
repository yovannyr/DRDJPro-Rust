//! Optional synthetic inference on the library worker. No audio device dependencies.
use crate::cancellation::Cancellation;
use drdj_ml::{
    InferenceJob, InferenceService, JobError, ModelId, ModelRegistry, ModelSpec, ModelUse,
    Priority, INPUT_LEN, MAX_CHUNKS,
};
use sha2::{Digest, Sha256};
use std::{env, path::PathBuf, sync::LazyLock, time::Duration};
use symphonia::core::audio::SignalSpec;

fn model_id() -> ModelId {
    ModelId {
        name: "conv-gru".into(),
        version: 1,
    }
}

pub(crate) fn configured() -> Option<&'static InferenceService> {
    static SERVICE: LazyLock<Option<InferenceService>> = LazyLock::new(|| {
        let cache = PathBuf::from(env::var_os("DRDJ_ML_IMPORT_CACHE")?);
        let start = || {
            let absolute = |key| {
                env::var_os(key)
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .ok_or(JobError::InvalidModel)
            };
            if !cache.is_absolute() {
                return Err(JobError::InvalidModel);
            }
            let mut registry = ModelRegistry::default();
            registry.register(ModelSpec {
                id: model_id(),
                path: absolute("DRDJ_ML_FIXTURE")?,
                sha256: [
                    0x33, 0xe4, 0xa9, 0xe3, 0xae, 0x47, 0xe5, 0xe7, 0x10, 0x41, 0x4c, 0xa9, 0xa5,
                    0x53, 0xac, 0x00, 0xc4, 0x71, 0xbb, 0x79, 0xf8, 0x7c, 0x27, 0xdf, 0xcd, 0x8b,
                    0x60, 0x70, 0x1d, 0xc3, 0x13, 0x88,
                ],
                license: "MIT".into(),
                allowed_use: ModelUse::SyntheticTest,
                minimum_ram_bytes: 64 << 20,
            })?;
            InferenceService::start(registry, cache, absolute("DRDJ_ORT_LIBRARY")?, 64 << 20)
        };
        start()
            .inspect_err(|e| log::warn!("Optional import inference unavailable: {e}"))
            .ok()
    });
    SERVICE.as_ref()
}

pub(crate) struct Pass<'a> {
    service: &'a InferenceService,
    digest: Sha256,
}
impl<'a> Pass<'a> {
    pub fn new(service: &'a InferenceService) -> Self {
        Self {
            service,
            digest: Sha256::new_with_prefix(b"drdj-import-peak-v1"),
        }
    }
    pub fn samples(&mut self, samples: &[f32]) {
        self.digest.update(bytemuck::cast_slice::<f32, u8>(samples));
    }
    pub fn finish(
        mut self,
        spec: SignalSpec,
        peaks: &[u8],
        cancel: &Cancellation,
    ) -> Result<(), JobError> {
        self.digest.update(spec.rate.to_le_bytes());
        self.digest.update(spec.channels.bits().to_le_bytes());
        let bins = INPUT_LEN * MAX_CHUNKS;
        let stride = peaks.len().div_ceil(bins).max(1);
        let mut chunks = vec![[0.; INPUT_LEN]; peaks.len().div_ceil(stride).div_ceil(INPUT_LEN)];
        for (index, window) in peaks.chunks(stride).enumerate() {
            chunks[index / INPUT_LEN][index % INPUT_LEN] =
                f32::from(*window.iter().max().unwrap()) / 255.;
        }
        if cancel.is_cancelled() {
            return Err(JobError::Cancelled);
        }
        let mut ticket = self.service.submit(InferenceJob {
            model: model_id(),
            audio_sha256: self.digest.finalize().into(),
            chunks,
            priority: Priority::Low,
        })?;
        loop {
            if cancel.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            if let Some(result) = ticket.try_result() {
                return result.map(|_| ());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{audio_analyzer::AudioAnalyzer, cancellation::Cancellation, library};
    use std::{env, fs, path::PathBuf, process::Command};

    #[test]
    #[ignore = "requires generated DRDJ_ML_FIXTURE and DRDJ_ORT_LIBRARY"]
    fn import_and_analysis_use_optional_inference() {
        const CHILD: &str = "DRDJ_A2_TEST_MODE";
        let audio =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/format-flac.flac");
        if let Ok(mode) = env::var(CHILD) {
            let cancel = Cancellation::default();
            let batch = library::inspect_paths_cancellable(vec![audio.clone()], &cancel).unwrap();
            let imported = &batch.metadata[0].1.analysis;
            if mode == "disabled" {
                assert!(imported.is_none());
                return;
            }
            assert!(imported.as_ref().unwrap().validate());
            let cache = PathBuf::from(env::var_os("DRDJ_ML_IMPORT_CACHE").unwrap());
            let count = || fs::read_dir(&cache).map_or(0, |entries| entries.count());
            assert_eq!(count(), usize::from(mode == "enabled"));
            let other = fs::canonicalize(audio.with_file_name("format-opus-mono.opus")).unwrap();
            let keys = || {
                let mut keys = Vec::new();
                if let Ok(entries) = fs::read_dir(&cache) {
                    for entry in entries {
                        keys.push(fs::read(entry.unwrap().path()).unwrap()[..32].to_vec());
                    }
                }
                keys.sort();
                keys
            };
            let before = keys();
            let analyzed = AudioAnalyzer::analyze_cancellable(&other, &cancel).unwrap();
            assert!(analyzed.validate());
            if mode == "enabled" {
                // Slot collisions are legal; a different cache key must still be published.
                assert_ne!(before, keys());
                let folder = tempfile::tempdir().unwrap();
                let copy = folder.path().join("copy.opus");
                fs::copy(other, &copy).unwrap();
                let before = keys();
                let batch = library::inspect_directory_cancellable(folder.path(), &cancel).unwrap();
                assert_eq!(before, keys());
                let mut catalog = library::Library::default();
                assert_eq!(catalog.insert(batch.paths), 1);
                for (path, metadata) in batch.metadata {
                    catalog.set_metadata(&path, metadata);
                }
                let restored = library::Library::restore(catalog.catalog()).unwrap();
                let mut expected = analyzed;
                expected.file_path = fs::canonicalize(copy)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                assert_eq!(
                    restored.track_by_id(1).unwrap().metadata.analysis.as_ref(),
                    Some(&expected)
                );
            } else {
                assert_eq!(count(), 0);
            }
            cancel.request();
            assert!(library::inspect_paths_cancellable(vec![audio], &cancel).is_err());
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        for mode in ["disabled", "missing", "enabled"] {
            let cache = dir.path().join(mode);
            let mut child = Command::new(env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "import_inference::tests::import_and_analysis_use_optional_inference",
                    "--ignored",
                ])
                .env(CHILD, mode)
                .env_remove("DRDJ_ML_IMPORT_CACHE");
            if mode != "disabled" {
                child.env("DRDJ_ML_IMPORT_CACHE", cache);
            }
            if mode == "missing" {
                child.env("DRDJ_ML_FIXTURE", dir.path().join("missing.onnx"));
            }
            assert!(child.status().unwrap().success(), "{mode}");
        }
    }
}
