//! Offline CPU Spleeter 4stems. Stream stereo 44.1 kHz f32le into the model.
//! Output frames contain L/R pairs for vocals, drums, bass and other, in order.
use crate::{ort_backend, Threads};
use ort::{environment::Environment, session::Session, value::Tensor};
use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use sha2::{Digest, Sha256};
use std::{error::Error, fs::File, io::Read, path::Path, sync::Arc};

const FFT: usize = 4096;
const HOP: usize = 1024;
const FRAMES: usize = 512;
const BINS: usize = 1024;
const LEFT: usize = 32 * HOP;
/// Maximum number of source frames emitted by one inference window.
pub const BLOCK: usize = 448 * HOP;
const WINDOW: usize = (FRAMES - 1) * HOP + FFT;
const GRAPHS: [(&str, &str); 4] = [
    (
        "vocals",
        "db47148ab1c52709ce694893f532c91abfe3edc4d46238939570e036a22878ca",
    ),
    (
        "drums",
        "7ae4002e5633634674f74dc3356d5875b0da894d59ce0f60e844bb8f9cb8aa92",
    ),
    (
        "bass",
        "ba4c4949a27222492cca49859901a873b4b71461dc48c7c5a51f93d31eb11f55",
    ),
    (
        "other",
        "3cc59116cb7195946ab9596d8ca25984d09c0f8a70db8cf85d063132f97bc61d",
    ),
];
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Single-threaded graphs and preplanned transforms, owned by one import worker.
pub struct Separator {
    sessions: Vec<Session>,
    _environment: Environment,
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    hann: Vec<f32>,
}
impl Separator {
    /// Load only checksum-pinned graphs from an explicit model directory.
    pub fn load(dir: &Path, runtime: &Path) -> Result<Self> {
        let mut sessions = Vec::new();
        let mut environment = None;
        for (name, digest) in GRAPHS {
            let mut bytes = Vec::new();
            File::open(dir.join(format!("{name}.fp16.onnx")))?
                .take(20_000_001)
                .read_to_end(&mut bytes)?;
            if format!("{:x}", Sha256::digest(&bytes)) != digest {
                return Err("invalid Spleeter model digest".into());
            }
            let (builder, env) = ort_backend::builder(Threads::new(1)?, Some(runtime))?;
            sessions.push(
                builder
                    .with_intra_op_spinning(false)?
                    .with_inter_op_spinning(false)?
                    .commit_from_memory(&bytes)?,
            );
            environment = Some(env);
        }
        let mut planner = FftPlanner::new();
        Ok(Self {
            sessions,
            _environment: environment.expect("four graphs"),
            forward: planner.plan_fft_forward(FFT),
            inverse: planner.plan_fft_inverse(FFT),
            hann: (0..FFT)
                .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT as f32).cos())
                .collect(),
        })
    }

    /// Discard the context margins on each model window. Weighted overlap-add
    /// reconstructs the center under the periodic Hann window. No track-sized PCM.
    pub fn stream(
        &mut self,
        mut pcm: impl Read,
        mut emit: impl FnMut(&[[f32; 8]]) -> Result<()>,
        cancelled: impl Fn() -> bool,
    ) -> Result<()> {
        let mut bytes = vec![0; LEFT * 8];
        loop {
            if cancelled() {
                return Err("separation cancelled".into());
            }
            let need = WINDOW * 8 - bytes.len();
            (&mut pcm).take(need as u64).read_to_end(&mut bytes)?;
            if !bytes.len().is_multiple_of(8) {
                return Err("partial stereo PCM frame".into());
            }
            let available = bytes.len() / 8 - LEFT;
            if available == 0 {
                break;
            }
            let count = available.min(BLOCK);
            let block = self.window(&bytes, count, &cancelled)?;
            emit(&block)?;
            if available <= BLOCK {
                break;
            }
            bytes.drain(..BLOCK * 8);
        }
        Ok(())
    }

    fn window(
        &mut self,
        bytes: &[u8],
        count: usize,
        cancelled: &impl Fn() -> bool,
    ) -> Result<Vec<[f32; 8]>> {
        let mut pcm = Vec::with_capacity(WINDOW * 2);
        for raw in bytes.as_chunks::<4>().0 {
            let sample = f32::from_le_bytes(*raw);
            if !sample.is_finite() {
                return Err("non-finite PCM".into());
            }
            pcm.push(sample);
        }
        pcm.resize(WINDOW * 2, 0.);
        let mut spec = Vec::with_capacity(2 * FRAMES * (FFT / 2 + 1));
        let mut mags = Vec::with_capacity(2 * FRAMES * BINS);
        let mut frame = vec![Complex32::default(); FFT];
        let mut scratch = vec![
            Complex32::default();
            self.forward
                .get_inplace_scratch_len()
                .max(self.inverse.get_inplace_scratch_len())
        ];
        for channel in 0..2 {
            for t in 0..FRAMES {
                for (i, x) in frame.iter_mut().enumerate() {
                    *x = Complex32::new(pcm[(t * HOP + i) * 2 + channel] * self.hann[i], 0.);
                }
                self.forward.process_with_scratch(&mut frame, &mut scratch);
                spec.extend_from_slice(&frame[..=FFT / 2]);
                mags.extend(frame[..BINS].iter().map(|c| c.norm()));
            }
        }
        let mut total = vec![1e-10f32; mags.len()];
        let input = Tensor::from_array(([2usize, 1, FRAMES, BINS], mags))?;
        let mut powers = Vec::new();
        for session in &mut self.sessions {
            if cancelled() {
                return Err("separation cancelled".into());
            }
            let output = session.run(ort::inputs![&input])?;
            let (shape, y) = output[0].try_extract_tensor::<f32>()?;
            if shape.as_ref() != [2, 1, 512, 1024] || y.iter().any(|v| !v.is_finite()) {
                return Err("invalid stem estimate".into());
            }
            let power: Vec<_> = y.iter().map(|v| v * v).collect();
            for (sum, p) in total.iter_mut().zip(&power) {
                *sum += p;
            }
            powers.push(power);
        }
        let mut result = vec![[0.; 8]; count];
        for (stem, power) in powers.into_iter().enumerate() {
            for (n, (spectrum, est)) in spec
                .as_chunks::<{ FFT / 2 + 1 }>()
                .0
                .iter()
                .zip(power.as_chunks::<BINS>().0.iter())
                .enumerate()
            {
                let channel = n / FRAMES;
                let offset = (n % FRAMES) * HOP;
                if offset + FFT <= LEFT || offset >= LEFT + count {
                    continue;
                }
                let mut mean = 0.;
                for k in 0..BINS {
                    let mask = (est[k] + 2.5e-11) / total[n * BINS + k];
                    mean += mask;
                    frame[k] = spectrum[k] * mask;
                }
                for k in BINS..=FFT / 2 {
                    frame[k] = spectrum[k] * (mean / BINS as f32);
                }
                for k in 1..FFT / 2 {
                    frame[FFT - k] = frame[k].conj();
                }
                self.inverse.process_with_scratch(&mut frame, &mut scratch);
                for i in LEFT.saturating_sub(offset)..FFT.min(LEFT + count - offset) {
                    // Four overlapping periodic Hann windows sum to 1.5 in power.
                    result[offset + i - LEFT][2 * stem + channel] +=
                        frame[i].re * self.hann[i] / (1.5 * FFT as f32);
                }
            }
        }
        Ok(result)
    }
}
