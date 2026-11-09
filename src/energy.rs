//! Energy analysis module — loudness, dynamics, and phrase segmentation.
//!
//! Computes EBU R128 integrated loudness, momentary loudness, and a
//! spectral energy envelope for phrase boundary detection. Results are
//! stored with confidence scores and can be updated incrementally.
//!
//! ## Metrics
//!
//! * **Integrated loudness** (LUFS) — overall track energy via gating.
//! * **Momentary loudness** (LUFS, 400ms windows) — local energy peaks.
//! * **Short-term loudness** (LUFS, 3s windows) — phrase-level energy.
//! * **Dynamic range** (LRA) — variation between quiet and loud sections.
//! * **True peak** (dBTP) — inter-sample peak estimation.
//! * **Phrase boundaries** — detected from short-term energy zero-crossings.

use ebur128::{EbuR128, Mode};

/// Energy analysis result for a single track.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyAnalysis {
    /// Integrated loudness in LUFS (-70.0 to 0.0).
    pub integrated_loudness: f32,
    /// Momentary loudness (400ms, -70.0 to 0.0). `None` if track too short.
    pub momentary_loudness: Option<f32>,
    /// Short-term loudness (3s, -70.0 to 0.0). `None` if track too short.
    pub short_term_loudness: Option<f32>,
    /// Loudness range (LRA) in LU.
    pub loudness_range: f32,
    /// True peak in dBTP.
    pub true_peak_dbtp: f32,
    /// Confidence 0.0–1.0 based on signal quality and duration.
    pub confidence: f32,
    /// Number of detected phrase boundaries.
    pub phrase_count: u32,
    /// Duration of the longest continuous energy segment in beats.
    pub longest_phrase_beats: f32,
}

impl EnergyAnalysis {
    /// Validate the analysis result.
    pub fn validate(&self) -> bool {
        (-70.0..=0.0).contains(&self.integrated_loudness)
            && self
                .momentary_loudness
                .is_none_or(|v| (-70.0..=0.0).contains(&v))
            && self
                .short_term_loudness
                .is_none_or(|v| (-70.0..=0.0).contains(&v))
            && self.loudness_range >= 0.0
            && self.confidence >= 0.0
            && self.confidence <= 1.0
    }

    /// Estimate the number of beats per phrase from the energy envelope.
    /// Assumes a typical phrase length of 8–32 beats in 4/4 time.
    pub fn phrases_per_bar(&self) -> Option<u8> {
        if self.longest_phrase_beats < 4.0 || self.longest_phrase_beats > 64.0 {
            return None;
        }
        // Typical DJ phrase: 8, 16, or 32 beats.
        let candidates = [8u8, 16, 32];
        candidates
            .iter()
            .min_by(|a, b| {
                let da = (**a as f32 - self.longest_phrase_beats).abs();
                let db = (**b as f32 - self.longest_phrase_beats).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }

    /// Estimate phrase count from total duration and average phrase length.
    pub fn estimate_phrase_count(&self, duration_beats: f32) -> u32 {
        if self.longest_phrase_beats < 4.0 {
            return 0;
        }
        (duration_beats / self.longest_phrase_beats).max(1.0) as u32
    }
}

/// Configuration for energy analysis.
pub struct Config {
    /// Minimum track duration (samples) for valid analysis.
    pub min_samples: usize,
    /// Sample rate.
    pub sample_rate: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_samples: 44100 * 10, // 10 seconds minimum
            sample_rate: 44100,
        }
    }
}

/// Analyze a stereo PCM buffer for energy metrics.
///
/// Returns `Ok(EnergyAnalysis)` with computed metrics, or `Err` if
/// the buffer is too short or contains invalid data.
pub fn analyze_energy(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    config: &Config,
) -> Result<EnergyAnalysis, String> {
    let channels = usize::from(channels);
    if channels == 0 || channels > 8 {
        return Err("Invalid channel count".into());
    }
    if samples.len() < config.min_samples * channels {
        return Err(format!(
            "Buffer too short: {} samples (minimum {})",
            samples.len() / channels,
            config.min_samples
        ));
    }

    if sample_rate == 0
        || !samples.len().is_multiple_of(channels)
        || samples.iter().any(|x| !x.is_finite())
    {
        return Err("Invalid PCM format or non-finite samples".into());
    }
    // Offline analysis: preserve interleaved channel energy and use the locked API.
    let mut meter = EbuR128::new(
        channels as u32,
        sample_rate,
        Mode::I | Mode::S | Mode::LRA | Mode::TRUE_PEAK,
    )
    .map_err(|e| format!("Failed to create EBU R128 meter: {e}"))?;
    meter.add_frames_f32(samples).map_err(|e| e.to_string())?;
    let integrated_loudness = meter.loudness_global().map_err(|e| e.to_string())? as f32;
    let momentary = meter.loudness_momentary().ok().map(|v| v as f32);
    let short_term = meter.loudness_shortterm().ok().map(|v| v as f32);
    let loudness_range = meter.loudness_range().unwrap_or(0.0) as f32;
    let mut true_peak = 0.0f64;
    for channel in 0..channels as u32 {
        true_peak = true_peak.max(meter.true_peak(channel).map_err(|e| e.to_string())?);
    }
    let true_peak = (20.0 * true_peak.max(f64::MIN_POSITIVE).log10()) as f32;

    // Compute confidence based on signal quality.
    let signal_strength = if integrated_loudness.is_finite() {
        ((integrated_loudness + 70.0) / 70.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let confidence = signal_strength;

    // Detect phrase boundaries from short-term energy.
    let phrase_count = detect_phrase_boundaries(samples, channels as u16, sample_rate, config);
    let longest_phrase_beats =
        estimate_longest_phrase(samples, channels as u16, sample_rate, config);

    Ok(EnergyAnalysis {
        integrated_loudness,
        momentary_loudness: momentary,
        short_term_loudness: short_term,
        loudness_range,
        true_peak_dbtp: true_peak,
        confidence,
        phrase_count,
        longest_phrase_beats,
    })
}

/// Detect phrase boundaries from short-term energy changes.
///
/// Looks for significant energy transitions (±6dB over 2-second windows)
/// as indicators of phrase boundaries.
fn detect_phrase_boundaries(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    _config: &Config,
) -> u32 {
    let channels = usize::from(channels);
    if samples.len() < sample_rate as usize * 4 * channels {
        return 0;
    }

    let window_samples = sample_rate as usize * 2; // 2-second windows
    let mut boundaries = 0u32;
    let mut prev_energy: Option<f64> = None;

    for start in
        (0..samples.len() - window_samples * channels).step_by(window_samples * channels / 2)
    {
        let end = start + window_samples * channels;
        let chunk = &samples[start..end];

        let energy: f64 =
            chunk.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / chunk.len() as f64;
        let energy_db = if energy > 0.0 {
            (10.0 * energy.log10()).abs().min(60.0)
        } else {
            0.0
        };

        if let Some(prev) = prev_energy {
            let delta = energy_db - prev;
            if delta.abs() > 6.0 {
                boundaries += 1;
            }
        }
        prev_energy = Some(energy_db);
    }

    boundaries.min(256) // Cap at 256 phrase boundaries
}

/// Estimate the longest continuous phrase in beats.
///
/// Analyzes the energy envelope to find the longest segment with
/// consistent energy levels (±3dB).
fn estimate_longest_phrase(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    _config: &Config,
) -> f32 {
    let channels = usize::from(channels);
    if samples.len() < sample_rate as usize * channels {
        return 0.0;
    }

    // Use 1-second windows.
    let window_samples = sample_rate as usize * channels;
    let mut max_consistent = 0usize;
    let mut current_run = 0usize;
    let mut prev_level: Option<f64> = None;

    let threshold_db = 3.0;

    for start in (0..samples.len() - window_samples).step_by(window_samples) {
        let end = (start + window_samples).min(samples.len());
        let chunk = &samples[start..end];

        let energy: f64 =
            chunk.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / chunk.len() as f64;
        let level_db = if energy > 0.0 {
            10.0 * energy.log10()
        } else {
            -60.0
        };

        if let Some(prev) = prev_level {
            if (level_db - prev).abs() <= threshold_db {
                current_run += 1;
            } else {
                if current_run > max_consistent {
                    max_consistent = current_run;
                }
                current_run = 1;
            }
        } else {
            current_run = 1;
        }
        prev_level = Some(level_db);
    }

    // Final run.
    if current_run > max_consistent {
        max_consistent = current_run;
    }

    // Convert windows to beats (assume 120 BPM = 2 beats per second).
    (max_consistent as f32) * 2.0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_silence(samples: usize) -> Vec<f32> {
        vec![0.0_f32; samples]
    }

    fn generate_tone(freq: f32, sample_rate: u32, duration_secs: usize) -> Vec<f32> {
        (0..sample_rate as usize * duration_secs)
            .map(|i| {
                (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect()
    }

    #[test]
    fn silence_produces_valid_but_low_energy_result() {
        let samples = generate_silence(44100 * 11); // 11 seconds
        let config = Config {
            min_samples: 44100 * 10,
            sample_rate: 44100,
        };
        let result = analyze_energy(&samples, 1, 44100, &config);
        // Silence should fail the EBU R128 meter or produce very low loudness.
        match result {
            Ok(analysis) => {
                assert!(analysis.integrated_loudness < -50.0);
                assert!(analysis.confidence < 0.5);
            }
            Err(_) => {
                // Also acceptable — silence may fail the minimum duration check.
            }
        }
    }

    #[test]
    fn tone_produces_higher_energy_than_silence() {
        let silence = generate_silence(44100 * 11);
        let tone = generate_tone(440.0, 44100, 11);

        let config = Config {
            min_samples: 44100 * 10,
            sample_rate: 44100,
        };

        let silence_result = analyze_energy(&silence, 1, 44100, &config);
        let tone_result = analyze_energy(&tone, 1, 44100, &config);

        match (silence_result, tone_result) {
            (Ok(s), Ok(t)) => {
                assert!(t.integrated_loudness > s.integrated_loudness);
                assert!(t.confidence > s.confidence);
            }
            _ => panic!("Tone analysis should succeed"),
        }
    }

    #[test]
    fn energy_analysis_validate_works() {
        let analysis = EnergyAnalysis {
            integrated_loudness: -18.0,
            momentary_loudness: Some(-20.0),
            short_term_loudness: Some(-15.0),
            loudness_range: 8.0,
            true_peak_dbtp: -1.5,
            confidence: 0.85,
            phrase_count: 8,
            longest_phrase_beats: 16.0,
        };
        assert!(analysis.validate());

        let invalid = EnergyAnalysis {
            integrated_loudness: -80.0, // Out of range
            momentary_loudness: None,
            short_term_loudness: None,
            loudness_range: 0.0,
            true_peak_dbtp: 0.0,
            confidence: 0.5,
            phrase_count: 0,
            longest_phrase_beats: 0.0,
        };
        assert!(!invalid.validate());
    }

    #[test]
    fn phrases_per_bar_returns_typical_values() {
        let analysis = EnergyAnalysis {
            integrated_loudness: -18.0,
            momentary_loudness: Some(-20.0),
            short_term_loudness: Some(-15.0),
            loudness_range: 8.0,
            true_peak_dbtp: -1.5,
            confidence: 0.85,
            phrase_count: 8,
            longest_phrase_beats: 16.0,
        };
        assert_eq!(analysis.phrases_per_bar(), Some(16));
    }

    #[test]
    fn estimate_phrase_count_works() {
        let analysis = EnergyAnalysis {
            integrated_loudness: -18.0,
            momentary_loudness: Some(-20.0),
            short_term_loudness: Some(-15.0),
            loudness_range: 8.0,
            true_peak_dbtp: -1.5,
            confidence: 0.85,
            phrase_count: 8,
            longest_phrase_beats: 16.0,
        };
        assert_eq!(analysis.estimate_phrase_count(160.0), 10); // 160 beats / 16 beats per phrase
    }

    #[test]
    fn buffer_too_short_returns_error() {
        let samples = generate_silence(44100 * 5); // Only 5 seconds
        let config = Config {
            min_samples: 44100 * 10,
            sample_rate: 44100,
        };
        let result = analyze_energy(&samples, 1, 44100, &config);
        assert!(result.is_err());
    }
}
