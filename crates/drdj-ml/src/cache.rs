use crate::{JobError, ModelSpec, OUTPUT_LEN};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const SLOTS: u8 = 32;

/// Identity includes the audio fingerprint and exact inputs/parameters.
pub(crate) fn key(
    model: &ModelSpec,
    audio: &[u8; 32],
    chunks: &[[f32; crate::INPUT_LEN]],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    // Change the domain when the output layout or inference semantics change.
    digest.update(b"drdj-a1-ort-cpu-v1");
    digest.update(audio);
    digest.update((model.id.name.len() as u32).to_le_bytes());
    digest.update(model.id.name.as_bytes());
    digest.update(model.id.version.to_le_bytes());
    digest.update(model.sha256);
    for chunk in chunks {
        for value in chunk {
            digest.update(value.to_le_bytes());
        }
    }
    digest.finalize().into()
}

fn slot(root: &Path, key: &[u8; 32]) -> PathBuf {
    root.join(format!("a1-{:02}.bin", key[0] % SLOTS))
}

pub(crate) fn read(root: &Path, key: &[u8; 32], count: usize) -> Option<Vec<[f32; OUTPUT_LEN]>> {
    let length = 64 + count * OUTPUT_LEN * 4;
    let mut bytes = Vec::new();
    File::open(slot(root, key))
        .ok()?
        .take(length as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() != length
        || &bytes[..32] != key
        || Sha256::digest(&bytes[64..])[..] != bytes[32..64]
    {
        return None;
    }
    bytes[64..]
        .as_chunks::<{ OUTPUT_LEN * 4 }>()
        .0
        .iter()
        .map(|chunk| {
            let output = std::array::from_fn(|index| {
                let offset = index * 4;
                f32::from_le_bytes(
                    chunk[offset..offset + 4]
                        .try_into()
                        .expect("fixed output layout"),
                )
            });
            output
                .iter()
                .all(|value| value.is_finite())
                .then_some(output)
        })
        .collect()
}

/// Complete files are published atomically. A slot collision is ordinary eviction.
pub(crate) fn write(
    root: &Path,
    key: &[u8; 32],
    output: &[[f32; OUTPUT_LEN]],
) -> Result<(), JobError> {
    std::fs::create_dir_all(root).map_err(JobError::Cache)?;
    let bytes: Vec<u8> = output
        .iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let mut staged = tempfile::NamedTempFile::new_in(root).map_err(JobError::Cache)?;
    staged.write_all(key).map_err(JobError::Cache)?;
    staged
        .write_all(&Sha256::digest(&bytes))
        .map_err(JobError::Cache)?;
    staged.write_all(&bytes).map_err(JobError::Cache)?;
    staged.as_file().sync_all().map_err(JobError::Cache)?;
    publish(staged, &slot(root, key)).map_err(JobError::Cache)?;
    Ok(())
}

fn publish(mut staged: tempfile::NamedTempFile, target: &Path) -> std::io::Result<()> {
    let mut delays = [5, 15, 40, 80].into_iter();
    loop {
        match staged.persist(target) {
            Ok(_) => return Ok(()),
            Err(error) => {
                // Windows scanners may briefly hold a destination without delete sharing.
                if !cfg!(windows) || !matches!(error.error.raw_os_error(), Some(5 | 32 | 33)) {
                    return Err(error.error);
                }
                let Some(ms) = delays.next() else {
                    return Err(error.error);
                };
                staged = error.file;
                std::thread::sleep(std::time::Duration::from_millis(ms));
            }
        }
    }
}

#[cfg(all(test, windows))]
#[test]
fn sharing_violation_preserves_previous_result_and_can_be_retried() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let key = [0; 32];
    let first = [[1.0; OUTPUT_LEN]];
    let second = [[2.0; OUTPUT_LEN]];
    write(dir.path(), &key, &first).unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(slot(dir.path(), &key))
        .unwrap();
    // A permanent denial exhausts the bounded retries without replacing the slot.
    assert!(matches!(
        write(dir.path(), &key, &second),
        Err(JobError::Cache(_))
    ));
    drop(held);
    assert_eq!(read(dir.path(), &key, 1).unwrap(), first);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    write(dir.path(), &key, &second).unwrap();
    assert_eq!(read(dir.path(), &key, 1).unwrap(), second);
}
