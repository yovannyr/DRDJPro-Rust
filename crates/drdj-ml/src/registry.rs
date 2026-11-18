use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::PathBuf};

const MAX_MODEL_BYTES: u64 = 65_536;

/// Versioned identity of a locally registered model.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModelId {
    pub name: String,
    pub version: u32,
}

/// A1 admits only local synthetic test models.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelUse {
    SyntheticTest,
}

/// Metadata supplied by the owner of a trusted local model.
#[derive(Clone, Debug)]
pub struct ModelSpec {
    pub id: ModelId,
    pub path: PathBuf,
    pub sha256: [u8; 32],
    pub license: String,
    pub allowed_use: ModelUse,
    pub minimum_ram_bytes: u64,
}

/// Setup-time registry, moved into the inference worker.
#[derive(Default)]
pub struct ModelRegistry(BTreeMap<ModelId, ModelSpec>);

impl ModelRegistry {
    /// Register one version. Duplicate identities cannot replace earlier entries.
    pub fn register(&mut self, model: ModelSpec) -> Result<(), crate::JobError> {
        if model.id.name.is_empty()
            || model.id.name.len() > 128
            || model.id.version == 0
            || model.license.trim().is_empty()
            || model.minimum_ram_bytes == 0
            || self.0.contains_key(&model.id)
        {
            return Err(crate::JobError::InvalidModel);
        }
        self.0.insert(model.id.clone(), model);
        Ok(())
    }

    pub(crate) fn get(&self, id: &ModelId) -> Result<&ModelSpec, crate::JobError> {
        self.0.get(id).ok_or(crate::JobError::ModelNotLoaded)
    }
}

impl ModelSpec {
    pub(crate) fn verified_bytes(&self) -> Result<Vec<u8>, crate::JobError> {
        use crate::JobError;
        let file = File::open(&self.path).map_err(|_| JobError::ModelNotLoaded)?;
        if !file
            .metadata()
            .map_err(|_| JobError::ModelNotLoaded)?
            .is_file()
        {
            return Err(JobError::ModelNotLoaded);
        }
        let mut bytes = Vec::new();
        file.take(MAX_MODEL_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| JobError::ModelNotLoaded)?;
        if bytes.is_empty()
            || bytes.len() as u64 > MAX_MODEL_BYTES
            || <[u8; 32]>::from(Sha256::digest(&bytes)) != self.sha256
        {
            return Err(JobError::ModelCorrupt);
        }
        Ok(bytes)
    }
}
