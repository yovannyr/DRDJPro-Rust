//! Offline Beat This! small0 qualification. Input is mono, 22,050 Hz PCM.
//! Model files must be supplied explicitly. No graph is downloaded by this API.
use crate::{ort_backend, InferenceError as Error, Threads};
use ort::{environment::Environment, session::Session, value::Tensor};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

const HOP: usize = 441;
const BORDER: usize = 6;
const WINDOW: usize = 1500;
const BODY: usize = WINDOW - 2 * BORDER;
const MAX_FRAMES: usize = 50 * 3600;
const FRONT: (&str, usize, &str) = (
    "logmel.onnx",
    4467499,
    "140a676b72c056a447ecff773ea7e9079b1a8065d3bd05102947e0d9fa9b3c8b",
);
const MODEL: (&str, usize, &str) = (
    "beatthis_small0.onnx",
    10555794,
    "c2c0778c9243c8b3ea62c0d00750b555581c29b91591893d690a76b48d6496dd",
);

fn backend(e: impl std::fmt::Display) -> Error {
    Error::Backend(e.to_string())
}

/// Frame-centred event locations, in milliseconds from the first PCM sample.
#[derive(Debug)]
pub struct Events {
    pub beats_ms: Vec<u32>,
    pub downbeats_ms: Vec<u32>,
}

/// One single-thread CPU session per graph. Call only from an offline worker.
pub struct Tracker {
    front: Session,
    model: Session,
    _environment: Environment,
}

impl Tracker {
    /// Verify each graph before passing the same bytes to ORT.
    pub fn load(directory: &Path, runtime: &Path) -> Result<Self, Error> {
        let load = |(name, size, hash): (&str, usize, &str)| {
            let mut bytes = Vec::with_capacity(size + 1);
            File::open(directory.join(name))
                .and_then(|f| f.take(size as u64 + 1).read_to_end(&mut bytes))
                .map_err(|_| Error::ModelUnavailable)?;
            if bytes.len() != size || format!("{:x}", Sha256::digest(&bytes)) != hash {
                return Err(Error::ModelUnavailable);
            }
            let (builder, env) = ort_backend::builder(Threads::new(1)?, Some(runtime))?;
            let session = builder
                .with_intra_op_spinning(false)
                .map_err(backend)?
                .with_inter_op_spinning(false)
                .map_err(backend)?
                .commit_from_memory(&bytes)
                .map_err(backend)?;
            Ok((session, env))
        };
        let (front, env) = load(FRONT)?;
        let (model, _) = load(MODEL)?;
        Ok(Self {
            front,
            model,
            _environment: env,
        })
    }

    /// Read mono little-endian f32 PCM. Retain at most two windows of PCM
    /// for look-back and one hour of two scalar probabilities per frame.
    pub fn track(&mut self, mut pcm: impl Read) -> Result<Events, Error> {
        let mut scores = Vec::new();
        let mut window = Vec::new();
        let mut base = 0; // absolute frame at the start of retained PCM
        loop {
            let emitted = scores.len();
            let target = (emitted + BODY + BORDER - base) * HOP * 4;
            pcm.by_ref()
                .take((target - window.len()) as u64)
                .read_to_end(&mut window)
                .map_err(backend)?;
            if window.len() % 4 != 0 {
                return Err(Error::InvalidInput);
            }
            let end = base + (window.len() / 4).div_ceil(HOP);
            if end > MAX_FRAMES {
                return Err(Error::InvalidInput);
            }
            let last = end - emitted <= BODY;
            let keep = (end - emitted).min(BODY);
            let start = if last && emitted != 0 {
                end.saturating_sub(WINDOW - BORDER)
            } else {
                emitted.saturating_sub(BORDER)
            };
            let pad_left = if emitted == 0 { BORDER } else { 0 };
            let left = emitted - start + pad_left;
            if keep > 0 {
                let mut audio = vec![0.; 512];
                audio.extend(
                    window[(start - base) * HOP * 4..]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .copied()
                        .map(f32::from_le_bytes),
                );
                if audio.iter().any(|x| !x.is_finite()) {
                    return Err(Error::InvalidInput);
                }
                // The pinned mel graph uses a valid (uncentred) 1024-sample STFT.
                // Zero-pad half a window so output frame k is centred at k * HOP.
                audio.resize(audio.len() + 512, 0.);
                let len = audio.len();
                let tensor = Tensor::from_array(([len], audio)).map_err(backend)?;
                let spec = self.front.run(ort::inputs![tensor]).map_err(backend)?;
                let (shape, mel) = spec[0].try_extract_tensor::<f32>().map_err(backend)?;
                let frames = end - start;
                if shape.len() != 2 || shape[1] != 128 || shape[0] < frames as i64 {
                    return Err(Error::InvalidOutput);
                }
                let model_frames = (pad_left + frames + BORDER).min(WINDOW);
                let mut input = vec![0.; model_frames * 128];
                input[pad_left * 128..(pad_left + frames) * 128]
                    .copy_from_slice(&mel[..frames * 128]);
                let tensor =
                    Tensor::from_array(([1, model_frames, 128], input)).map_err(backend)?;
                let out = self.model.run(ort::inputs![tensor]).map_err(backend)?;
                let extract = |name: &str| {
                    let (s, v) = out
                        .get(name)
                        .ok_or(Error::InvalidOutput)?
                        .try_extract_tensor::<f32>()
                        .map_err(backend)?;
                    if s.as_ref() != [1, model_frames as i64]
                        || v.iter().any(|x| !(0.0..=1.0).contains(x))
                    {
                        return Err(Error::InvalidOutput);
                    }
                    Ok(&v[left..left + keep])
                };
                scores.extend(
                    extract("beat")?
                        .iter()
                        .copied()
                        .zip(extract("downbeat")?.iter().copied()),
                );
            }
            if last {
                break;
            }
            // Retain enough history to shift a short final window back to full length.
            let next_base = scores.len().saturating_sub(WINDOW - BORDER);
            window.drain(..(next_base - base) * HOP * 4);
            base = next_base;
        }
        Ok(events(&scores))
    }
}

fn peaks(scores: &[(f32, f32)], value: impl Fn((f32, f32)) -> f32) -> Vec<u32> {
    let mut peaks = Vec::new();
    let mut group = None;
    for i in 0..=scores.len() {
        let selected = i < scores.len()
            && value(scores[i]) > 0.5
            && scores[i.saturating_sub(3)..(i + 4).min(scores.len())]
                .iter()
                .all(|s| value(*s) <= value(scores[i]));
        if selected {
            group.get_or_insert(i);
        } else if let Some(start) = group.take() {
            peaks.push(((start + i - 1) * 10) as u32);
        }
    }
    peaks
}

fn events(scores: &[(f32, f32)]) -> Events {
    let beats_ms = peaks(scores, |s| s.0);
    let mut downbeats_ms = Vec::new();
    for d in peaks(scores, |s| s.1) {
        let i = beats_ms.partition_point(|b| *b < d);
        if let Some(&b) = beats_ms[i.saturating_sub(1)..(i + 1).min(beats_ms.len())]
            .iter()
            .min_by_key(|b| b.abs_diff(d))
        {
            if downbeats_ms.last() != Some(&b) {
                downbeats_ms.push(b);
            }
        }
    }
    Events {
        beats_ms,
        downbeats_ms,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plateaus_align_once_and_silence_has_no_downbeats() {
        let mut plateau = vec![(0.8, 0.); 10];
        plateau.extend([(0., 0.), (0., 0.), (0.9, 0.)]);
        assert_eq!(peaks(&plateau, |s| s.0), [80, 240]);
        let mut s = vec![(0., 0.); 25];
        s[4].0 = 0.9;
        s[5].0 = 0.9;
        s[6].1 = 0.8;
        s[19] = (0.7, 0.9);
        let e = events(&s);
        assert_eq!(e.beats_ms, [90, 380]);
        assert_eq!(e.downbeats_ms, e.beats_ms);
        assert!(events(&[(0., 0.9)]).downbeats_ms.is_empty());
        assert!(events(&[]).beats_ms.is_empty());
    }
}
