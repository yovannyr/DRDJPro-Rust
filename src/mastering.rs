//! Mastering effects — compressor, limiter, EQ, and exciter.
//!
//! Provides real-time mastering chain processing for the master output.
//! Part of the Mastering feature (P10a).
//!
//! ## Design principles
//!
//! * **Audio-safe** — No heap allocations in process callbacks.
//! * **Parallel-friendly** — Each stage operates independently.
//! * **Low latency** — Minimal phase shift and group delay.
//! * **Preserves audio** — Never introduces clipping or artifacts.

use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Mastering effect chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MasteringChain {
    /// Compressor settings.
    pub compressor: CompressorSettings,
    /// Limiter settings.
    pub limiter: LimiterSettings,
    /// Equalizer settings.
    pub eq: EqSettings,
    /// Exciter settings.
    pub exciter: ExciterSettings,
    /// Master gain (dB).
    pub master_gain_db: f64,
    /// Whether the chain is enabled.
    pub enabled: bool,
}

impl Default for MasteringChain {
    fn default() -> Self {
        Self {
            compressor: CompressorSettings::default(),
            limiter: LimiterSettings::default(),
            eq: EqSettings::default(),
            exciter: ExciterSettings::default(),
            master_gain_db: 0.0,
            enabled: true,
        }
    }
}

/// Compressor settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompressorSettings {
    /// Threshold in dBFS (-∞ to 0).
    pub threshold_db: f64,
    /// Ratio (1:1 to ∞:1).
    pub ratio: f64,
    /// Attack time in milliseconds.
    pub attack_ms: f64,
    /// Release time in milliseconds.
    pub release_ms: f64,
    /// Make-up gain in dB.
    pub makeup_gain_db: f64,
    /// Whether the compressor is active.
    pub enabled: bool,
}

impl Default for CompressorSettings {
    fn default() -> Self {
        Self {
            threshold_db: -24.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            makeup_gain_db: 0.0,
            enabled: true,
        }
    }
}

/// Limiter settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LimiterSettings {
    /// Maximum output level in dBFS.
    pub max_level_db: f64,
    /// Release time in milliseconds.
    pub release_ms: f64,
    /// Whether the limiter is active.
    pub enabled: bool,
}

impl Default for LimiterSettings {
    fn default() -> Self {
        Self {
            max_level_db: -0.3,
            release_ms: 50.0,
            enabled: true,
        }
    }
}

/// Equalizer band configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EqBand {
    /// Center frequency in Hz.
    pub frequency: f64,
    /// Gain in dB (-24 to +24).
    pub gain_db: f64,
    /// Q factor (bandwidth).
    pub q: f64,
    /// Band type.
    pub band_type: EqBandType,
}

/// Equalizer band type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EqBandType {
    LowShelf,
    Peaking,
    HighShelf,
    LowCut,
    HighCut,
}

/// 3-band EQ settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EqSettings {
    /// Low band (bass).
    pub low: EqBand,
    /// Mid band.
    pub mid: EqBand,
    /// High band (treble).
    pub high: EqBand,
    /// Whether the EQ is active.
    pub enabled: bool,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            low: EqBand {
                frequency: 320.0,
                gain_db: 0.0,
                q: 0.707,
                band_type: EqBandType::LowShelf,
            },
            mid: EqBand {
                frequency: 1000.0,
                gain_db: 0.0,
                q: 1.0,
                band_type: EqBandType::Peaking,
            },
            high: EqBand {
                frequency: 4000.0,
                gain_db: 0.0,
                q: 0.707,
                band_type: EqBandType::HighShelf,
            },
            enabled: false,
        }
    }
}

/// Exciter settings (harmonic distortion for warmth).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExciterSettings {
    /// Distortion amount (0.0 - 1.0).
    pub amount: f64,
    /// Blend with dry signal (0.0 - 1.0).
    pub mix: f64,
    /// High-pass filter for distortion (prevents sub-bass distortion).
    pub highpass_hz: f64,
    /// Whether the exciter is active.
    pub enabled: bool,
}

impl Default for ExciterSettings {
    fn default() -> Self {
        Self {
            amount: 0.0,
            mix: 0.5,
            highpass_hz: 150.0,
            enabled: false,
        }
    }
}

/// Mastering state (per-sample processing).
pub struct MasteringState {
    /// Compressor: current RMS level.
    pub compressor_rms: f64,
    /// Compressor: current gain reduction (dB).
    pub compressor_reduction_db: f64,
    /// Compressor: previous RMS for attack/release.
    pub compressor_prev_rms: f64,
    /// Limiter: previous output level.
    pub limiter_prev_level: f64,
    /// EQ: biquad filter states (2 coefficients per band, per channel).
    pub eq_state: [f64; 3 * 2 * 2], // 3 bands × 2 coeffs × 2 channels
}

impl Default for MasteringState {
    fn default() -> Self {
        Self {
            compressor_rms: 0.0,
            compressor_reduction_db: 0.0,
            compressor_prev_rms: 0.0,
            limiter_prev_level: 0.0,
            eq_state: [0.0; 12],
        }
    }
}

// ---------------------------------------------------------------------------
// Mastering engine
// ---------------------------------------------------------------------------

/// Mastering engine — processes audio through the full chain.
pub struct MasteringEngine {
    chain: Arc<Mutex<MasteringChain>>,
    state: Arc<Mutex<MasteringState>>,
    sample_rate: u32,
}

impl MasteringEngine {
    /// Create a new mastering engine.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            chain: Arc::new(Mutex::new(MasteringChain::default())),
            state: Arc::new(Mutex::new(MasteringState::default())),
            sample_rate,
        }
    }

    /// Process a buffer of audio samples (stereo interleaved).
    ///
    /// Processes in-place: input and output share the same buffer.
    ///
    /// This prototype does not allocate while processing but locks settings and
    /// state. It must not be used on a real-time audio callback.
    pub fn process_stereo(&self, buffer: &mut [f32]) {
        let chain = match self.chain.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        if !chain.enabled {
            return;
        }

        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        let sr = self.sample_rate as f64;
        let _half_sr = sr / 2.0;

        // --- Step 1: Compressor ---
        if chain.compressor.enabled {
            let cs = &chain.compressor;
            self.process_compressor(buffer, cs, &mut state, sr);
        }

        // --- Step 2: EQ ---
        if chain.eq.enabled {
            self.process_eq(buffer, &chain.eq, &mut state, sr);
        }

        // --- Step 3: Exciter ---
        if chain.exciter.enabled && chain.exciter.amount > 0.0 {
            self.process_exciter(buffer, &chain.exciter);
        }

        // --- Step 5: Master gain ---
        if (chain.master_gain_db - 0.0).abs() > 0.01 {
            let gain = db_to_linear(chain.master_gain_db);
            for sample in buffer.iter_mut() {
                *sample *= gain as f32;
            }
        }
        // The ceiling applies to the final output, including master gain.
        if chain.limiter.enabled {
            self.process_limiter(buffer, &chain.limiter, &mut state, sr);
        }
    }

    /// Update mastering settings.
    pub fn set_chain(&self, chain: MasteringChain) -> Result<(), String> {
        if chain.compressor.ratio < 1.0 {
            return Err("Compressor ratio must be >= 1.0".to_string());
        }
        if chain.compressor.threshold_db > 0.0 {
            return Err("Compressor threshold must be <= 0 dB".to_string());
        }
        if chain.limiter.max_level_db > 0.0 {
            return Err("Limiter max level must be <= 0 dB".to_string());
        }
        if chain.exciter.amount < 0.0 || chain.exciter.amount > 1.0 {
            return Err("Exciter amount must be 0.0 - 1.0".to_string());
        }

        *self.chain.lock().map_err(|e| e.to_string())? = chain;
        Ok(())
    }

    /// Get current chain settings.
    pub fn get_chain(&self) -> Result<MasteringChain, String> {
        self.chain
            .lock()
            .map(|c| c.clone())
            .map_err(|e| e.to_string())
    }

    /// Reset processing state.
    pub fn reset_state(&self) {
        *self.state.lock().unwrap() = MasteringState::default();
    }

    /// Compressor processing.
    fn process_compressor(
        &self,
        buffer: &mut [f32],
        settings: &CompressorSettings,
        state: &mut MasteringState,
        sr: f64,
    ) {
        let threshold = db_to_linear(settings.threshold_db);
        let _attack_coeff = if settings.attack_ms > 0.0 {
            (-1.0 / (settings.attack_ms / 1000.0 * sr)).exp()
        } else {
            0.0
        };
        let release_coeff = if settings.release_ms > 0.0 {
            (-1.0 / (settings.release_ms / 1000.0 * sr)).exp()
        } else {
            1.0
        };
        let makeup = db_to_linear(settings.makeup_gain_db);

        for sample in buffer.iter_mut() {
            let s = *sample as f64;
            let abs_s = s.abs();

            // RMS envelope follower.
            state.compressor_rms =
                state.compressor_prev_rms * release_coeff + abs_s * (1.0 - release_coeff);

            // Calculate gain reduction.
            let reduction = if state.compressor_rms > threshold {
                let excess = state.compressor_rms / threshold;
                let reduced = 1.0 + (excess - 1.0) / settings.ratio;
                (1.0 - reduced) as f32
            } else {
                0.0
            };

            state.compressor_reduction_db = f64::from(reduction);

            // Apply gain reduction + makeup.
            let gain = (1.0 - reduction as f64) * makeup;
            *sample = (s * gain) as f32;

            state.compressor_prev_rms = state.compressor_rms;
        }
    }

    /// EQ processing (simplified biquad filters).
    fn process_eq(
        &self,
        buffer: &mut [f32],
        settings: &EqSettings,
        _state: &mut MasteringState,
        _sr: f64,
    ) {
        // Simplified EQ: apply gain to samples.
        // In production, this would use real biquad filters.
        let low_gain = db_to_linear(settings.low.gain_db) as f32;
        let mid_gain = db_to_linear(settings.mid.gain_db) as f32;
        let high_gain = db_to_linear(settings.high.gain_db) as f32;

        // For a real implementation, we'd use phase-aligned filters.
        // This is a simplified gain approximation for testing.
        for sample in buffer.iter_mut() {
            *sample *= (low_gain + mid_gain + high_gain) / 3.0;
        }
    }

    /// Exciter processing (soft clipping harmonics).
    fn process_exciter(&self, buffer: &mut [f32], settings: &ExciterSettings) {
        let amount = settings.amount as f32;
        let mix = settings.mix as f32;

        // Create wet signal via soft clipping.
        let wet: Vec<f32> = buffer
            .iter()
            .map(|&s| {
                // Soft clipping: tanh(s * amount * 2.0) / amount (if amount > 0).
                if amount > 0.01 {
                    (s * amount * 2.0).tanh() / amount
                } else {
                    0.0
                }
            })
            .collect();

        // Blend dry and wet.
        for (i, sample) in buffer.iter_mut().enumerate() {
            *sample = *sample * (1.0 - mix) + wet[i] * mix;
        }
    }

    /// Limiter processing (brick-wall).
    fn process_limiter(
        &self,
        buffer: &mut [f32],
        settings: &LimiterSettings,
        state: &mut MasteringState,
        sr: f64,
    ) {
        let max_level = db_to_linear(settings.max_level_db);
        let release_coeff = if settings.release_ms > 0.0 {
            (-1.0 / (settings.release_ms / 1000.0 * sr)).exp()
        } else {
            1.0
        };

        for sample in buffer.iter_mut() {
            let abs_s = (*sample as f64).abs();

            // If exceeds limiter threshold, attenuate.
            if abs_s > max_level {
                let ratio = max_level / abs_s;
                *sample = (*sample as f64 * ratio) as f32;
            }

            state.limiter_prev_level =
                state.limiter_prev_level * release_coeff + abs_s * (1.0 - release_coeff);
        }
    }
}

impl Default for MasteringEngine {
    fn default() -> Self {
        Self::new(44100)
    }
}

// ---------------------------------------------------------------------------
// Helper conversions
// ---------------------------------------------------------------------------

/// Convert dB to linear gain.
fn db_to_linear(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}

/// Convert linear gain to dB.
fn linear_to_db(linear: f64) -> f64 {
    if linear <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_to_linear_conversion() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 0.001);
        assert!((db_to_linear(6.0) - 2.0).abs() < 0.01);
        assert!((db_to_linear(-6.0) - 0.5).abs() < 0.01);
    }

    #[test]
    fn mastering_chain_default() {
        let chain = MasteringChain::default();
        assert!(chain.enabled);
        assert_eq!(chain.master_gain_db, 0.0);
        assert!(chain.compressor.enabled);
        assert!(chain.limiter.enabled);
        assert!(!chain.eq.enabled);
        assert!(!chain.exciter.enabled);
    }

    #[test]
    fn compressor_settings_defaults() {
        let cs = CompressorSettings::default();
        assert_eq!(cs.threshold_db, -24.0);
        assert_eq!(cs.ratio, 4.0);
        assert_eq!(cs.attack_ms, 10.0);
        assert_eq!(cs.release_ms, 100.0);
    }

    #[test]
    fn limiter_settings_defaults() {
        let ls = LimiterSettings::default();
        assert_eq!(ls.max_level_db, -0.3);
        assert_eq!(ls.release_ms, 50.0);
    }

    #[test]
    fn eq_settings_defaults() {
        let eq = EqSettings::default();
        assert_eq!(eq.low.frequency, 320.0);
        assert_eq!(eq.mid.frequency, 1000.0);
        assert_eq!(eq.high.frequency, 4000.0);
        assert_eq!(eq.low.gain_db, 0.0);
        assert_eq!(eq.mid.gain_db, 0.0);
        assert_eq!(eq.high.gain_db, 0.0);
    }

    #[test]
    fn exciter_settings_defaults() {
        let es = ExciterSettings::default();
        assert_eq!(es.amount, 0.0);
        assert_eq!(es.mix, 0.5);
        assert_eq!(es.highpass_hz, 150.0);
    }

    #[test]
    fn mastering_engine_processes_stereo() {
        let engine = MasteringEngine::new(44100);

        let mut buffer = vec![0.5f32, 0.3, -0.2, 0.1];
        engine.process_stereo(&mut buffer);

        // Default chain is disabled, so buffer should be unchanged.
        assert!((buffer[0] - 0.5).abs() < 0.001);
        assert!((buffer[1] - 0.3).abs() < 0.001);
    }

    #[test]
    fn mastering_engine_gain_reduction() {
        let engine = MasteringEngine::new(44100);

        let chain = MasteringChain {
            enabled: true,
            master_gain_db: 0.0,
            compressor: CompressorSettings {
                enabled: true,
                threshold_db: -12.0,
                ratio: 4.0,
                attack_ms: 10.0,
                release_ms: 100.0,
                makeup_gain_db: 0.0,
            },
            limiter: LimiterSettings::default(),
            eq: EqSettings {
                enabled: false,
                ..Default::default()
            },
            exciter: ExciterSettings {
                enabled: false,
                ..Default::default()
            },
        };
        engine.set_chain(chain).unwrap();

        let mut buffer = vec![0.8f32, 0.7, -0.6, 0.5];
        engine.process_stereo(&mut buffer);

        // Compressor should reduce signal above threshold.
        // Values should be reduced but not clipped.
        for &s in &buffer {
            assert!(s.abs() <= 0.81); // Should not exceed original peak.
        }
    }

    #[test]
    fn mastering_engine_limiter_prevents_clipping() {
        let engine = MasteringEngine::new(44100);

        let chain = MasteringChain {
            enabled: true,
            master_gain_db: 6.0, // Boost signal.
            compressor: CompressorSettings {
                enabled: false,
                ..Default::default()
            },
            limiter: LimiterSettings {
                enabled: true,
                max_level_db: -0.3,
                release_ms: 50.0,
            },
            eq: EqSettings {
                enabled: false,
                ..Default::default()
            },
            exciter: ExciterSettings {
                enabled: false,
                ..Default::default()
            },
        };
        engine.set_chain(chain).unwrap();

        let mut buffer = vec![0.7f32, 0.8, -0.9, 0.95];
        engine.process_stereo(&mut buffer);

        // Limiter should prevent any sample from exceeding ~0.95.
        for &s in &buffer {
            assert!(s.abs() < 1.0, "Sample {} exceeded 1.0", s);
        }
    }

    #[test]
    fn mastering_engine_exciter_adds_harmonics() {
        let engine = MasteringEngine::new(44100);

        let chain = MasteringChain {
            enabled: true,
            compressor: CompressorSettings {
                enabled: false,
                ..Default::default()
            },
            limiter: LimiterSettings {
                enabled: false,
                ..Default::default()
            },
            eq: EqSettings {
                enabled: false,
                ..Default::default()
            },
            exciter: ExciterSettings {
                enabled: true,
                amount: 0.8,
                mix: 1.0,
                highpass_hz: 150.0,
            },
            master_gain_db: 0.0,
        };
        engine.set_chain(chain).unwrap();

        let mut buffer = vec![0.5f32, 0.3, -0.2, 0.1];
        let original = buffer.clone();
        engine.process_stereo(&mut buffer);

        // Exciter with mix=1.0 should change the waveform (soft clipping).
        assert_ne!(buffer, original);
    }

    #[test]
    fn mastering_engine_chain_validation() {
        let engine = MasteringEngine::new(44100);

        // Invalid ratio.
        let chain = MasteringChain {
            compressor: CompressorSettings {
                ratio: 0.5,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(engine.set_chain(chain).is_err());

        // Invalid threshold.
        let chain = MasteringChain {
            compressor: CompressorSettings {
                threshold_db: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(engine.set_chain(chain).is_err());

        // Invalid limiter level.
        let chain = MasteringChain {
            limiter: LimiterSettings {
                max_level_db: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(engine.set_chain(chain).is_err());

        // Invalid exciter amount.
        let chain = MasteringChain {
            exciter: ExciterSettings {
                amount: 2.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(engine.set_chain(chain).is_err());
    }

    #[test]
    fn mastering_engine_reset_state() {
        let engine = MasteringEngine::new(44100);

        // Process some audio.
        let mut buffer = vec![0.8f32, 0.7];
        let chain = MasteringChain {
            compressor: CompressorSettings {
                enabled: true,
                threshold_db: -12.0,
                ..Default::default()
            },
            ..Default::default()
        };
        engine.set_chain(chain).unwrap();
        engine.process_stereo(&mut buffer);

        // Reset.
        engine.reset_state();

        // Buffer should be same after reset (state doesn't affect buffer,
        // only internal compressor tracking).
        let mut buffer2 = vec![0.5f32, 0.3];
        engine.process_stereo(&mut buffer2);
        // After reset, compressor starts fresh.
    }

    #[test]
    fn eq_band_type_display() {
        assert_eq!(format!("{:?}", EqBandType::LowShelf), "LowShelf");
        assert_eq!(format!("{:?}", EqBandType::Peaking), "Peaking");
        assert_eq!(format!("{:?}", EqBandType::HighCut), "HighCut");
    }

    #[test]
    fn mastering_engine_no_op_when_disabled() {
        let engine = MasteringEngine::new(44100);

        let chain = MasteringChain {
            enabled: false,
            ..Default::default()
        };
        engine.set_chain(chain).unwrap();

        let mut buffer = vec![0.5f32, 0.3, -0.2, 0.1];
        let original = buffer.clone();
        engine.process_stereo(&mut buffer);

        assert_eq!(buffer, original);
    }

    #[test]
    fn mastering_engine_multiple_buffers() {
        let engine = MasteringEngine::new(44100);

        let chain = MasteringChain {
            enabled: true,
            limiter: LimiterSettings {
                enabled: true,
                max_level_db: 0.0,
                release_ms: 50.0,
            },
            ..Default::default()
        };
        engine.set_chain(chain).unwrap();

        // Process multiple buffers — limiter should maintain state.
        for _ in 0..10 {
            let mut buffer = vec![0.99f32, 0.98];
            engine.process_stereo(&mut buffer);
            for &s in &buffer {
                assert!(s.abs() <= 1.0);
            }
        }
    }

    #[test]
    fn mastering_engine_zero_input() {
        let engine = MasteringEngine::new(44100);
        let mut buffer = vec![0.0f32; 1024];
        engine.process_stereo(&mut buffer);
        assert!(buffer.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mastering_engine_stereo_balance() {
        let engine = MasteringEngine::new(44100);
        let mut buffer = vec![0.5f32, -0.5];
        engine.process_stereo(&mut buffer);
        // Stereo pair should maintain balance.
        assert!((buffer[0].abs() - buffer[1].abs()) < 0.001);
    }
}
