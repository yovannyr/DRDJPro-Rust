//! Harmony analysis module — key detection, confidence, and Camelot mapping.
//!
//! Uses chroma-based STFT analysis to detect the tonal center of a track.
//! Results include a confidence score (0.0–1.0) and support the Camelot
//! wheel notation used for DJ mixing.
//!
//! ## Algorithm overview
//!
//! 1. Decode the audio to PCM at the native sample rate.
//! 2. Compute overlapping STFT frames (window size 16384, hop 8192).
//! 3. Build a 12-bin chroma vector per frame via FFT bin accumulation.
//! 4. Average chroma vectors across all frames (silence-skipped).
//! 5. Correlate the averaged chroma against Krumhansl-Kessler profiles
//!    (major and minor) to find the best matching key.
//! 6. Return the key, confidence, and Camelot notation.

use rustfft::{num_complex::Complex, FftPlanner};
// --- Chroma analysis constants ---
const CHROMA_FFT_SIZE: usize = 16384;
const CHROMA_HOP: usize = 8192;
const CHROMA_MIN_HZ: f64 = 100.0;
const CHROMA_MAX_HZ: f64 = 3500.0;
const CHROMA_SILENCE_RMS: f32 = 1e-3;
const MIN_KEY_CORRELATION: f64 = 0.3;

// --- Krumhansl-Kessler profiles (tonic at index 0) ---
const KK_MAJOR: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KK_MINOR: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

// --- Note names ---
const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Musical key result with confidence score.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyAnalysis {
    /// Root note (e.g. "C", "D#", "G").
    pub root: String,
    /// Mode: "major" or "minor".
    pub mode: String,
    /// Confidence between 0.0 and 1.0.
    pub confidence: f32,
    /// Camelot notation (e.g. "8A" for B-flat major, "11B" for G minor).
    pub camelot: String,
}

impl KeyAnalysis {
    /// Validate the result — all fields must be consistent.
    pub fn validate(&self) -> bool {
        !self.root.is_empty()
            && self.root.len() <= 2
            && (self.mode == "major" || self.mode == "minor")
            && (0.0..=1.0).contains(&self.confidence)
            && (self.camelot.len() == 2)
            && self.camelot.chars().next().unwrap().is_ascii_digit()
            && matches!(self.camelot.chars().next_back().unwrap(), 'A' | 'B')
    }

    /// Return `true` if the key detection is considered reliable
    /// (confidence ≥ 0.5).
    pub fn is_reliable(&self) -> bool {
        self.confidence >= 0.5
    }
}

/// Analyze caller-supplied, finite, interleaved PCM outside real-time callbacks.
/// Decoding and the source buffer's memory limit belong to the caller.
pub fn analyze_pcm(samples: &[f32], channels: u16, rate: u32) -> Result<KeyAnalysis, String> {
    if channels == 0
        || rate == 0
        || !samples.len().is_multiple_of(usize::from(channels))
        || samples.iter().any(|sample| !sample.is_finite())
    {
        return Err("Invalid PCM format or non-finite samples".into());
    }
    let channels_count = usize::from(channels);
    let spec = rate;
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(CHROMA_FFT_SIZE);
    let mut chroma_accumulator = [0.0_f64; 12];
    let mut frame_count: u64 = 0;
    let window = CHROMA_FFT_SIZE * channels_count;
    if samples.len() >= window {
        for start in (0..=samples.len() - window).step_by(CHROMA_HOP * channels_count) {
            let chunk = &samples[start..start + window];
            let rms = (chunk.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>()
                / chunk.len() as f64)
                .sqrt();
            if rms < f64::from(CHROMA_SILENCE_RMS) {
                continue;
            }
            let mono: Vec<f64> = chunk
                .chunks_exact(channels_count)
                .map(|frame| {
                    frame.iter().map(|v| f64::from(*v)).sum::<f64>() / channels_count as f64
                })
                .collect();
            // FFT.
            let mut freq = mono
                .into_iter()
                .map(|re| Complex::new(re, 0.0))
                .collect::<Vec<_>>();
            fft.process(&mut freq);

            // Accumulate energy into 12 chroma bins.
            let bin_hz = spec as f64 / CHROMA_FFT_SIZE as f64;
            let mut frame_chroma = [0.0_f64; 12];
            for (bin_idx, bin_energy) in freq.iter().enumerate().take(CHROMA_FFT_SIZE / 2) {
                let freq_hz = bin_idx as f64 * bin_hz;
                if freq_hz < CHROMA_MIN_HZ || freq_hz > CHROMA_MAX_HZ {
                    continue;
                }

                // Map frequency to semitone (A4 = 440 Hz = index 57 in MIDI).
                let midi_note = 12.0 * (freq_hz / 440.0).log2() + 57.0;
                let semitone = midi_note.round().rem_euclid(12.0) as usize;
                frame_chroma[semitone] += bin_energy.norm_sqr();
            }

            // Normalize frame chroma by its energy.
            let energy: f64 = frame_chroma.iter().sum::<f64>();
            if energy > 0.0 {
                for bin in frame_chroma.iter_mut() {
                    *bin /= energy;
                }
            }

            // Accumulate.
            for i in 0..12 {
                chroma_accumulator[i] += frame_chroma[i];
            }
            frame_count += 1;
        }
    }
    if frame_count == 0 {
        return Ok(KeyAnalysis {
            root: "C".to_string(),
            mode: "major".to_string(),
            confidence: 0.0,
            camelot: "8A".to_string(),
        });
    }

    // Normalize accumulator.
    let frames = frame_count as f64;
    for bin in chroma_accumulator.iter_mut() {
        *bin /= frames;
    }

    // Correlate against KK profiles.
    let major_corr = correlation(&chroma_accumulator, &KK_MAJOR);
    let minor_corr = correlation(&chroma_accumulator, &KK_MINOR);

    let (best_corr, mode, best_profile) = if major_corr >= minor_corr {
        (major_corr, "major", &KK_MAJOR)
    } else {
        (minor_corr, "minor", &KK_MINOR)
    };

    // Find root note (highest correlation with shifted profile).
    let mut best_shift = 0usize;
    let mut best_shift_corr = -1.0_f64;
    for shift in 0..12 {
        let corr = shifted_correlation(&chroma_accumulator, best_profile, shift);
        if corr > best_shift_corr {
            best_shift_corr = corr;
            best_shift = shift;
        }
    }

    let confidence = (best_corr * best_shift_corr).max(0.0).min(1.0) as f32;

    let root = NOTE_NAMES[best_shift].to_string();
    let camelot = camelot_notation(best_shift, mode);

    Ok(KeyAnalysis {
        root,
        mode: mode.to_string(),
        confidence,
        camelot,
    })
}

/// Compute Pearson correlation between two vectors.
fn correlation(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let mut sum_a = 0.0;
    let mut sum_b = 0.0;
    for i in 0..12 {
        sum_a += a[i];
        sum_b += b[i];
    }
    let mean_a = sum_a / 12.0;
    let mean_b = sum_b / 12.0;

    let mut numerator = 0.0;
    let mut denom_a = 0.0;
    let mut denom_b = 0.0;
    for i in 0..12 {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        numerator += da * db;
        denom_a += da * da;
        denom_b += db * db;
    }

    let denominator = (denom_a * denom_b).sqrt();
    if denominator < 1e-12 {
        return 0.0;
    }
    numerator / denominator
}

/// Correlate with a shifted profile (circular rotation for different keys).
fn shifted_correlation(chroma: &[f64; 12], profile: &[f64; 12], shift: usize) -> f64 {
    let mut shifted = [0.0_f64; 12];
    for i in 0..12 {
        shifted[(i + shift) % 12] = profile[i];
    }
    correlation(chroma, &shifted)
}

/// Convert root index and mode to Camelot notation.
fn camelot_notation(root_index: usize, mode: &str) -> String {
    const MAJOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
    const MINOR: [u8; 12] = [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];
    if mode == "minor" {
        format!("{}A", MINOR[root_index % 12])
    } else {
        format!("{}B", MAJOR[root_index % 12])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camelot_major_c_returns_8b() {
        let result = camelot_notation(0, "major");
        assert_eq!(result, "8B");
    }

    #[test]
    fn camelot_minor_c_returns_5a() {
        let result = camelot_notation(0, "minor");
        assert_eq!(result, "5A");
    }

    #[test]
    fn key_analysis_validate_gives_consistent_results() {
        let analysis = KeyAnalysis {
            root: "G".to_string(),
            mode: "major".to_string(),
            confidence: 0.75,
            camelot: "9A".to_string(),
        };
        assert!(analysis.validate());
        assert!(analysis.is_reliable());

        let weak = KeyAnalysis {
            root: "D".to_string(),
            mode: "minor".to_string(),
            confidence: 0.2,
            camelot: "4B".to_string(),
        };
        assert!(weak.validate());
        assert!(!weak.is_reliable());
    }

    #[test]
    fn correlation_with_identical_vectors_is_one() {
        let a = [
            1.0_f64, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
        ];
        let corr = correlation(&a, &a);
        assert!((corr - 1.0).abs() < 1e-10);
    }

    #[test]
    fn shifted_correlation_detects_key_rotation() {
        // A profile shifted by 4 semitones should correlate best at shift=4.
        let profile = KK_MAJOR;
        let mut shifted = [0.0_f64; 12];
        for i in 0..12 {
            shifted[(i + 4) % 12] = profile[i];
        }
        let corr0 = shifted_correlation(&shifted, &profile, 0);
        let corr4 = shifted_correlation(&shifted, &profile, 4);
        assert!(
            corr4 > corr0,
            "shift=4 should correlate better than shift=0"
        );
    }
}
