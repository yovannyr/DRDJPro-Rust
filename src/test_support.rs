//! Self-contained fixtures; no network or sound device is needed by tests.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct TempFile(PathBuf);
impl TempFile {
    pub fn new(ext: &str, bytes: &[u8]) -> Self {
        let path = std::env::temp_dir().join(format!(
            "drdj-{}-{}.{ext}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        fs::remove_file(&self.0).expect("fixture file released");
    }
}
pub fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-9s.mp3")
}
pub fn wav_file(value: i16, frames: u32) -> TempFile {
    wav_samples(&vec![value; frames as usize], 48_000, 1)
}
pub fn wav_samples(samples: &[i16], rate: u32, channels: u16) -> TempFile {
    let len = (samples.len() * 2) as u32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    wav.extend_from_slice(&(channels * 2).to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    TempFile::new("wav", &wav)
}

pub fn tagged_mp3(title: &str, artist: &str, album: &str) -> TempFile {
    fn size(value: usize) -> [u8; 4] {
        [
            (value >> 21 & 127) as u8,
            (value >> 14 & 127) as u8,
            (value >> 7 & 127) as u8,
            (value & 127) as u8,
        ]
    }
    let mut tags = Vec::new();
    for (key, value) in [(b"TIT2", title), (b"TPE1", artist), (b"TALB", album)] {
        let mut payload = vec![3];
        payload.extend_from_slice(value.as_bytes());
        tags.extend_from_slice(key);
        tags.extend_from_slice(&size(payload.len()));
        tags.extend_from_slice(&[0, 0]);
        tags.extend_from_slice(&payload);
    }
    let mut bytes = b"ID3\x04\x00\x00".to_vec();
    bytes.extend_from_slice(&size(tags.len()));
    bytes.extend_from_slice(&tags);
    bytes.extend_from_slice(&fs::read(fixture()).unwrap());
    TempFile::new("mp3", &bytes)
}
pub fn tagged_wav(title: &str, artist: &str, album: &str) -> TempFile {
    let wav = wav_file(8192, 48_000);
    let raw = fs::read(wav.path()).unwrap();
    let mut info = b"INFO".to_vec();
    for (key, value) in [(b"INAM", title), (b"IART", artist), (b"IPRD", album)] {
        let mut payload = value.as_bytes().to_vec();
        payload.push(0);
        info.extend_from_slice(key);
        info.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        info.extend_from_slice(&payload);
        if payload.len() % 2 != 0 {
            info.push(0);
        }
    }
    let mut bytes = raw[..36].to_vec();
    bytes.extend_from_slice(b"LIST");
    bytes.extend_from_slice(&(info.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&info);
    bytes.extend_from_slice(&raw[36..]);
    let len = bytes.len() as u32 - 8;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    TempFile::new("wav", &bytes)
}
