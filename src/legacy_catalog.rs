//! Read-only migration reader for the published version-1 JSON catalog.
use crate::library::Catalog;
use std::{fs::File, io::Read, path::Path};
const MAX_BYTES: u64 = 32 * 1024 * 1024;
fn read(path: &Path) -> Result<Catalog, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Legacy library exceeds 32 MiB".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if let Some(version) = value.get("version").and_then(|v| v.as_u64()) {
        if version != 1 {
            return Err(format!("Unsupported legacy library version {version}"));
        }
    }
    let catalog: Catalog = serde_json::from_value(value).map_err(|e| e.to_string())?;
    catalog.validate()?;
    Ok(catalog)
}
/// Source and backup remain byte-for-byte unchanged, including on failure.
pub fn load(path: &Path) -> Result<(Catalog, &'static str), String> {
    let primary = path
        .try_exists()
        .map_err(|e| e.to_string())?
        .then(|| read(path));
    if let Some(Ok(catalog)) = primary.as_ref() {
        return Ok((catalog.clone(), "JSON library"));
    }
    if let Some(Err(error)) = primary.as_ref() {
        if error.starts_with("Unsupported legacy library version")
            || error.starts_with("Unsupported library version")
        {
            return Err(error.clone());
        }
    }
    let backup = crate::library_store::side_path(path, ".bak");
    if backup.try_exists().map_err(|e| e.to_string())? {
        return read(&backup).map(|catalog| (catalog, "JSON backup"));
    }
    if let Some(Err(error)) = primary {
        return Err(format!(
            "Legacy library damaged; original preserved: {error}"
        ));
    }
    Ok((
        Catalog {
            version: 1,
            crates: Default::default(),
            tracks: Vec::new(),
        },
        "New SQLite library",
    ))
}
