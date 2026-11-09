//! Audio Mastering Part 2 — Presets, LUFS, True Peak, Gain Staging, Chain Reordering.
//!
//! Extends the mastering engine (P10a) with metering (LUFS, True Peak,
//! gain reduction, spectrum analyzer), preset management, bypass toggling,
//! gain staging, and processing chain reordering.
//! Part of the Audio Mastering feature (P10b).
//!
//! ## Design principles
//!
//! * **Accurate** — Precise metering values.
//! * **Flexible** — Configurable processing chain.
//! * **Safe** — Gain staging prevents clipping.

use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// LUFS loudness range.
pub const LUFS_MIN: f64 = -23.0;
pub const LUFS_MAX: f64 = 0.0;

/// True peak threshold (dBFS).
pub const TRUE_PEAK_LIMIT: f64 = 0.0;

/// Clipping threshold (dBFS).
pub const CLIPPING_THRESHOLD: f64 = -0.1;

/// Gain reduction meter reading.
#[derive(Debug, Clone, Copy)]
pub struct GainReductionReading {
    /// Current gain reduction in dB (0 = no reduction).
    pub current_db: f64,
    /// Maximum gain reduction in dB.
    pub max_db: f64,
    /// Is active (reduction happening).
    pub active: bool,
}

impl GainReductionReading {
    /// Create new reading.
    pub fn new(current_db: f64, max_db: f64) -> Self {
        Self {
            current_db,
            max_db,
            active: current_db < 0.0,
        }
    }

    /// Create idle reading.
    pub fn idle() -> Self {
        Self {
            current_db: 0.0,
            max_db: 0.0,
            active: false,
        }
    }

    /// Check if reducing.
    pub fn is_reducing(&self) -> bool {
        self.current_db < -0.1
    }
}

impl Default for GainReductionReading {
    fn default() -> Self {
        Self::idle()
    }
}

/// LUFS reading.
#[derive(Debug, Clone, Copy)]
pub struct LUFSReading {
    /// Integrated LUFS.
    pub integrated: f64,
    /// Short-term LUFS (last 3 seconds).
    pub short_term: f64,
    /// Momentary LUFS (last 400ms).
    pub momentary: f64,
    /// Loudness range (LRA).
    pub lra: f64,
    /// Target LUFS (default -23).
    pub target: f64,
    /// Actual LUFS (overall).
    pub actual: f64,
    /// Loudness range low.
    pub lra_low: f64,
    /// Loudness range high.
    pub lra_high: f64,
}

impl LUFSReading {
    /// Create new reading.
    pub fn new(integrated: f64, short_term: f64, momentary: f64, lra: f64) -> Self {
        Self {
            integrated,
            short_term,
            momentary,
            lra,
            target: -23.0,
            actual: integrated,
            lra_low: integrated - lra,
            lra_high: integrated + lra,
        }
    }

    /// Create silent reading.
    pub fn silent() -> Self {
        Self {
            integrated: LUFS_MIN,
            short_term: LUFS_MIN,
            momentary: LUFS_MIN,
            lra: 0.0,
            target: -23.0,
            actual: LUFS_MIN,
            lra_low: LUFS_MIN,
            lra_high: LUFS_MIN,
        }
    }

    /// Check if within target.
    pub fn within_target(&self) -> bool {
        (self.integrated - self.target).abs() <= 1.0
    }

    /// Check if too loud.
    pub fn too_loud(&self) -> bool {
        self.integrated > self.target + 1.0
    }

    /// Check if too quiet.
    pub fn too_quiet(&self) -> bool {
        self.integrated < self.target - 1.0
    }

    /// Convert to bar level (0.0-1.0).
    pub fn to_bar_level(&self) -> f64 {
        ((self.integrated - LUFS_MIN) / (LUFS_MAX - LUFS_MIN)).clamp(0.0, 1.0)
    }

    /// Get color warning.
    pub fn color_warning(&self) -> &str {
        if self.too_loud() {
            "red"
        } else if self.too_quiet() {
            "yellow"
        } else {
            "green"
        }
    }
}

impl Default for LUFSReading {
    fn default() -> Self {
        Self::silent()
    }
}

/// True peak reading.
#[derive(Debug, Clone, Copy)]
pub struct TruePeakReading {
    /// Current true peak in dBFS.
    pub current_dbfs: f64,
    /// Maximum true peak in dBFS.
    pub max_dbfs: f64,
    /// Is overloaded (above threshold).
    pub overloaded: bool,
}

impl TruePeakReading {
    /// Create new reading.
    pub fn new(current_dbfs: f64, max_dbfs: f64) -> Self {
        Self {
            current_dbfs,
            max_dbfs,
            overloaded: current_dbfs > CLIPPING_THRESHOLD,
        }
    }

    /// Create idle reading.
    pub fn idle() -> Self {
        Self {
            current_dbfs: negative_infinity_db(),
            max_dbfs: negative_infinity_db(),
            overloaded: false,
        }
    }

    /// Check if overloaded.
    pub fn is_overloaded(&self) -> bool {
        self.overloaded || self.current_dbfs > CLIPPING_THRESHOLD
    }

    /// Get overload indicator.
    pub fn overload_indicator(&self) -> bool {
        self.is_overloaded()
    }
}

impl Default for TruePeakReading {
    fn default() -> Self {
        Self::idle()
    }
}

/// Frequency band for spectrum analyzer.
#[derive(Debug, Clone)]
pub struct FrequencyBand {
    /// Center frequency in Hz.
    pub center_hz: f64,
    /// Lower frequency in Hz.
    pub lower_hz: f64,
    /// Upper frequency in Hz.
    pub upper_hz: f64,
    /// Band level in dB.
    pub level_db: f64,
}

impl FrequencyBand {
    /// Create new frequency band.
    pub fn new(center_hz: f64, level_db: f64) -> Self {
        let lower_hz = center_hz / 2.0_f64.sqrt();
        let upper_hz = center_hz * 2.0_f64.sqrt();
        Self {
            center_hz,
            lower_hz,
            upper_hz,
            level_db,
        }
    }
}

/// Spectrum data.
#[derive(Debug, Clone)]
pub struct SpectrumData {
    /// Frequency bands (20Hz-20kHz).
    pub bands: Vec<FrequencyBand>,
    /// Overall level in dB.
    pub overall_db: f64,
    /// Peak frequency.
    pub peak_frequency: f64,
    /// Spectral centroid.
    pub spectral_centroid: f64,
}

impl SpectrumData {
    /// Create spectrum data from levels.
    pub fn from_levels(levels: Vec<f64>, center_frequencies: Vec<f64>) -> Self {
        assert_eq!(
            levels.len(),
            center_frequencies.len(),
            "Levels and frequencies must match"
        );

        let bands: Vec<FrequencyBand> = center_frequencies
            .iter()
            .zip(levels.iter())
            .map(|(cf, level)| FrequencyBand::new(*cf, *level))
            .collect();

        let overall_db = if levels.is_empty() {
            negative_infinity_db()
        } else {
            // RMS of all bands.
            let sum: f64 = levels.iter().map(|level| 10.0_f64.powf(level / 10.0)).sum();
            10.0 * (sum / levels.len() as f64).log10()
        };

        let peak_frequency = if let Some((idx, _)) = levels
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        {
            center_frequencies[idx]
        } else {
            0.0
        };

        // Spectral centroid.
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        for (i, level) in levels.iter().enumerate() {
            let linear = 10.0_f64.powf(level / 10.0);
            weighted_sum += center_frequencies[i] * linear;
            total_weight += linear;
        }
        let spectral_centroid = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        Self {
            bands,
            overall_db,
            peak_frequency,
            spectral_centroid,
        }
    }

    /// Get peak band index.
    pub fn peak_band(&self) -> Option<usize> {
        self.bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.level_db.partial_cmp(&b.1.level_db).unwrap())
            .map(|(i, _)| i)
    }

    /// Check for bass emphasis relative to midrange.
    pub fn has_bass_emphasis(&self) -> bool {
        mean_band(self.bands.iter().filter(|b| b.center_hz < 250.0))
            > mean_band(
                self.bands
                    .iter()
                    .filter(|b| b.center_hz >= 250.0 && b.center_hz < 4000.0),
            )
    }
    /// Check for high-frequency emphasis relative to midrange.
    pub fn has_harshness(&self) -> bool {
        mean_band(self.bands.iter().filter(|b| b.center_hz > 6000.0))
            > mean_band(
                self.bands
                    .iter()
                    .filter(|b| b.center_hz >= 500.0 && b.center_hz <= 6000.0),
            )
    }
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            bands: Vec::new(),
            overall_db: negative_infinity_db(),
            peak_frequency: 0.0,
            spectral_centroid: 0.0,
        }
    }
}

/// Input/Output meter reading.
#[derive(Debug, Clone, Copy)]
pub struct LevelMeterReading {
    /// Current level in dBFS.
    pub current_dbfs: f64,
    /// Peak level in dBFS.
    pub peak_dbfs: f64,
    /// RMS level in dBFS.
    pub rms_dbfs: f64,
}

impl LevelMeterReading {
    /// Create new reading.
    pub fn new(current_dbfs: f64, peak_dbfs: f64, rms_dbfs: f64) -> Self {
        Self {
            current_dbfs,
            peak_dbfs,
            rms_dbfs,
        }
    }

    /// Create idle reading.
    pub fn idle() -> Self {
        Self {
            current_dbfs: negative_infinity_db(),
            peak_dbfs: negative_infinity_db(),
            rms_dbfs: negative_infinity_db(),
        }
    }

    /// Check for clipping.
    pub fn is_clipping(&self) -> bool {
        self.peak_dbfs > CLIPPING_THRESHOLD
    }

    /// Get bar level (0.0-1.0).
    pub fn bar_level(&self) -> f64 {
        ((self.current_dbfs + 60.0) / 60.0).clamp(0.0, 1.0)
    }
}

impl Default for LevelMeterReading {
    fn default() -> Self {
        Self::idle()
    }
}

/// Mastering preset.
#[derive(Debug, Clone)]
pub struct MasteringPreset {
    /// Preset name.
    pub name: String,
    /// Preset description.
    pub description: String,
    /// Compressor settings (saved as values).
    pub compressor_threshold: f64,
    pub compressor_ratio: f64,
    pub compressor_attack: Duration,
    pub compressor_release: Duration,
    /// EQ settings (saved as values).
    pub eq_low_gain: f64,
    pub eq_mid_gain: f64,
    pub eq_high_gain: f64,
    /// Limiter settings.
    pub limiter_threshold: f64,
    pub limiter_ceiling: f64,
    /// Exciter settings.
    pub exciter_enabled: bool,
    pub exciter_amount: f64,
    /// Bypass state.
    pub bypass: bool,
    /// Chain order.
    pub chain_order: Vec<ProcessingStage>,
    /// Is user-created.
    pub is_user_created: bool,
    /// Created timestamp.
    pub created_at: std::time::Instant,
}

impl MasteringPreset {
    /// Create new preset.
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            compressor_threshold: -18.0,
            compressor_ratio: 2.0,
            compressor_attack: Duration::from_millis(10),
            compressor_release: Duration::from_millis(100),
            eq_low_gain: 0.0,
            eq_mid_gain: 0.0,
            eq_high_gain: 0.0,
            limiter_threshold: -1.0,
            limiter_ceiling: -0.3,
            exciter_enabled: false,
            exciter_amount: 0.0,
            bypass: false,
            chain_order: vec![
                ProcessingStage::Compressor,
                ProcessingStage::EQ,
                ProcessingStage::Exciter,
                ProcessingStage::Limiter,
            ],
            is_user_created: true,
            created_at: std::time::Instant::now(),
        }
    }

    /// Create built-in preset.
    pub fn built_in(name: &str, description: &str, chain_order: Vec<ProcessingStage>) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            compressor_threshold: -18.0,
            compressor_ratio: 2.0,
            compressor_attack: Duration::from_millis(10),
            compressor_release: Duration::from_millis(100),
            eq_low_gain: 0.0,
            eq_mid_gain: 0.0,
            eq_high_gain: 0.0,
            limiter_threshold: -1.0,
            limiter_ceiling: -0.3,
            exciter_enabled: false,
            exciter_amount: 0.0,
            bypass: false,
            chain_order,
            is_user_created: false,
            created_at: std::time::Instant::now(),
        }
    }

    /// Get preset name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get preset description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Check if user created.
    pub fn is_user_created(&self) -> bool {
        self.is_user_created
    }
}

/// Processing stage in the chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProcessingStage {
    /// Compressor.
    Compressor,
    /// EQ.
    EQ,
    /// Exciter.
    Exciter,
    /// Limiter.
    Limiter,
}

impl ProcessingStage {
    /// Get stage name.
    pub fn name(&self) -> &str {
        match self {
            Self::Compressor => "Compressor",
            Self::EQ => "EQ",
            Self::Exciter => "Exciter",
            Self::Limiter => "Limiter",
        }
    }

    /// Get stage display name.
    pub fn display_name(&self) -> &str {
        self.name()
    }
}

/// Mastering chain configuration.
pub struct MasteringChain {
    /// Processing stages in order.
    pub stages: Vec<ProcessingStage>,
    /// Input gain in dB.
    pub input_gain_db: f64,
    /// Output gain in dB.
    pub output_gain_db: f64,
    /// Master bypass.
    pub bypass: bool,
    /// Gain staging mode.
    pub gain_staging: GainStagingMode,
}

impl MasteringChain {
    /// Create new chain.
    pub fn new() -> Self {
        Self {
            stages: vec![
                ProcessingStage::Compressor,
                ProcessingStage::EQ,
                ProcessingStage::Exciter,
                ProcessingStage::Limiter,
            ],
            input_gain_db: 0.0,
            output_gain_db: 0.0,
            bypass: false,
            gain_staging: GainStagingMode::Automatic,
        }
    }

    /// Add stage at end.
    pub fn add_stage(&mut self, stage: ProcessingStage) {
        if !self.stages.contains(&stage) {
            self.stages.push(stage);
        }
    }

    /// Remove stage.
    pub fn remove_stage(&mut self, stage: &ProcessingStage) -> bool {
        let pos = self.stages.iter().position(|s| s == stage);
        if let Some(idx) = pos {
            self.stages.remove(idx);
            true
        } else {
            false
        }
    }

    /// Move stage to position.
    pub fn move_stage(&mut self, stage: &ProcessingStage, new_position: usize) -> bool {
        let pos = self.stages.iter().position(|s| s == stage);
        if let Some(idx) = pos {
            if new_position < self.stages.len() {
                self.stages.remove(idx);
                self.stages.insert(new_position, stage.clone());
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Swap two stages.
    pub fn swap_stages(&mut self, stage_a: &ProcessingStage, stage_b: &ProcessingStage) -> bool {
        let pos_a = self.stages.iter().position(|s| s == stage_a);
        let pos_b = self.stages.iter().position(|s| s == stage_b);

        if let (Some(a), Some(b)) = (pos_a, pos_b) {
            self.stages.swap(a, b);
            true
        } else {
            false
        }
    }

    /// Get chain order as string.
    pub fn chain_order_string(&self) -> String {
        self.stages
            .iter()
            .map(|s| s.display_name())
            .collect::<Vec<_>>()
            .join(" → ")
    }

    /// Set input gain.
    pub fn set_input_gain(&mut self, gain_db: f64) {
        self.input_gain_db = gain_db;
    }

    /// Set output gain.
    pub fn set_output_gain(&mut self, gain_db: f64) {
        self.output_gain_db = gain_db;
    }

    /// Bypass chain.
    pub fn bypass(&mut self) {
        self.bypass = true;
    }

    /// Unbypass chain.
    pub fn unbypass(&mut self) {
        self.bypass = false;
    }

    /// Toggle bypass.
    pub fn toggle_bypass(&mut self) {
        self.bypass = !self.bypass;
    }

    /// Check if bypassed.
    pub fn is_bypassed(&self) -> bool {
        self.bypass
    }

    /// Set gain staging mode.
    pub fn set_gain_staging(&mut self, mode: GainStagingMode) {
        self.gain_staging = mode;
    }

    /// Get gain staging mode.
    pub fn gain_staging(&self) -> &GainStagingMode {
        &self.gain_staging
    }

    /// Check for clipping risk.
    pub fn has_clipping_risk(&self) -> bool {
        self.output_gain_db > 0.0 && self.gain_staging == GainStagingMode::Manual
    }
}

impl Default for MasteringChain {
    fn default() -> Self {
        Self::new()
    }
}

/// Gain staging mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GainStagingMode {
    /// Automatic gain staging.
    #[default]
    Automatic,
    /// Manual gain staging.
    Manual,
}

/// Metering data (all meters together).
#[derive(Debug, Clone)]
pub struct MeteringData {
    /// Gain reduction reading.
    pub gain_reduction: GainReductionReading,
    /// LUFS reading.
    pub lufs: LUFSReading,
    /// True peak reading.
    pub true_peak: TruePeakReading,
    /// Input level.
    pub input_level: LevelMeterReading,
    /// Output level.
    pub output_level: LevelMeterReading,
    /// Spectrum data.
    pub spectrum: SpectrumData,
}

impl MeteringData {
    /// Create new metering data.
    pub fn new(
        gain_reduction: GainReductionReading,
        lufs: LUFSReading,
        true_peak: TruePeakReading,
        input_level: LevelMeterReading,
        output_level: LevelMeterReading,
        spectrum: SpectrumData,
    ) -> Self {
        Self {
            gain_reduction,
            lufs,
            true_peak,
            input_level,
            output_level,
            spectrum,
        }
    }

    /// Create idle metering data.
    pub fn idle() -> Self {
        Self {
            gain_reduction: GainReductionReading::idle(),
            lufs: LUFSReading::silent(),
            true_peak: TruePeakReading::idle(),
            input_level: LevelMeterReading::idle(),
            output_level: LevelMeterReading::idle(),
            spectrum: SpectrumData::default(),
        }
    }

    /// Check if any meter is overloaded.
    pub fn is_overloaded(&self) -> bool {
        self.true_peak.is_overloaded() || self.output_level.is_clipping()
    }

    /// Get overall warning status.
    pub fn warning_status(&self) -> &str {
        if self.is_overloaded() {
            "overload"
        } else if self.lufs.too_loud() {
            "too_loud"
        } else if self.lufs.too_quiet() {
            "too_quiet"
        } else if self.gain_reduction.is_reducing() {
            "compressing"
        } else {
            "nominal"
        }
    }
}

impl Default for MeteringData {
    fn default() -> Self {
        Self::idle()
    }
}

/// Mastering preset manager and metering engine.
pub struct MasteringEngineV2 {
    /// Presets (built-in and custom).
    presets: HashMap<String, MasteringPreset>,
    /// Currently active preset.
    active_preset: Option<String>,
    /// Mastering chain.
    chain: MasteringChain,
    /// Current metering data.
    metering: MeteringData,
    /// A/B bypass state.
    ab_bypass: bool,
    /// A preset.
    pub preset_a: Option<MasteringPreset>,
    /// B preset.
    pub preset_b: Option<MasteringPreset>,
}

impl MasteringEngineV2 {
    /// Create new mastering engine.
    pub fn new() -> Self {
        let mut engine = Self {
            presets: HashMap::new(),
            active_preset: None,
            chain: MasteringChain::new(),
            metering: MeteringData::idle(),
            ab_bypass: false,
            preset_a: None,
            preset_b: None,
        };

        // Register built-in presets.
        engine.register_builtin_presets();

        engine
    }

    /// Register built-in presets.
    fn register_builtin_presets(&mut self) {
        self.presets.insert(
            "Default".to_string(),
            MasteringPreset::built_in(
                "Default",
                "Standard mastering chain with compressor, EQ, and limiter.",
                vec![
                    ProcessingStage::Compressor,
                    ProcessingStage::EQ,
                    ProcessingStage::Exciter,
                    ProcessingStage::Limiter,
                ],
            ),
        );

        self.presets.insert(
            "Radio Ready".to_string(),
            MasteringPreset::built_in(
                "Radio Ready",
                "Optimized for radio broadcast loudness and clarity.",
                vec![
                    ProcessingStage::Compressor,
                    ProcessingStage::EQ,
                    ProcessingStage::Limiter,
                ],
            ),
        );

        self.presets.insert(
            "Club Mix".to_string(),
            MasteringPreset::built_in(
                "Club Mix",
                "Maximum loudness for club systems with strong low-end.",
                vec![
                    ProcessingStage::Compressor,
                    ProcessingStage::EQ,
                    ProcessingStage::Limiter,
                ],
            ),
        );

        self.presets.insert(
            "Streaming".to_string(),
            MasteringPreset::built_in(
                "Streaming",
                "Loudness normalized for streaming platforms (-14 LUFS).",
                vec![
                    ProcessingStage::Compressor,
                    ProcessingStage::EQ,
                    ProcessingStage::Limiter,
                ],
            ),
        );
    }

    /// Add custom preset.
    pub fn add_preset(&mut self, preset: MasteringPreset) {
        self.presets.insert(preset.name.clone(), preset);
    }

    /// Remove custom preset.
    pub fn remove_preset(&mut self, name: &str) -> bool {
        if let Some(preset) = self.presets.get(name) {
            if preset.is_user_created() {
                self.presets.remove(name);
                true
            } else {
                false // Can't remove built-in presets.
            }
        } else {
            false
        }
    }

    /// Get preset.
    pub fn get_preset(&self, name: &str) -> Option<&MasteringPreset> {
        self.presets.get(name)
    }

    /// Get all preset names.
    pub fn preset_names(&self) -> Vec<String> {
        self.presets.keys().cloned().collect()
    }

    /// Get preset count.
    pub fn preset_count(&self) -> usize {
        self.presets.len()
    }

    /// Get user preset count.
    pub fn user_preset_count(&self) -> usize {
        self.presets
            .values()
            .filter(|p| p.is_user_created())
            .count()
    }

    /// Get built-in preset count.
    pub fn builtin_preset_count(&self) -> usize {
        self.presets
            .values()
            .filter(|p| !p.is_user_created())
            .count()
    }

    /// Load preset.
    pub fn load_preset(&mut self, name: &str) -> Result<(), MasteringError> {
        if let Some(preset) = self.presets.get(name) {
            self.active_preset = Some(name.to_string());
            self.chain.bypass = preset.bypass;
            self.chain.stages = preset.chain_order.clone();
            Ok(())
        } else {
            Err(MasteringError::PresetNotFound(name.to_string()))
        }
    }

    /// Get active preset.
    pub fn active_preset(&self) -> Option<&str> {
        self.active_preset.as_deref()
    }

    /// Set active preset.
    pub fn set_active_preset(&mut self, name: &str) {
        self.active_preset = Some(name.to_string());
    }

    /// Toggle bypass (A/B comparison).
    pub fn toggle_bypass(&mut self) {
        self.ab_bypass = !self.ab_bypass;
    }

    /// Check if bypassed (A/B).
    pub fn is_bypassed(&self) -> bool {
        self.ab_bypass
    }

    /// Set A preset.
    pub fn set_preset_a(&mut self, preset: MasteringPreset) {
        self.preset_a = Some(preset);
    }

    /// Set B preset.
    pub fn set_preset_b(&mut self, preset: MasteringPreset) {
        self.preset_b = Some(preset);
    }

    /// Check if A/B comparison is active.
    pub fn is_ab_comparison_active(&self) -> bool {
        self.preset_a.is_some() && self.preset_b.is_some()
    }

    /// Get chain.
    pub fn chain(&self) -> &MasteringChain {
        &self.chain
    }

    /// Get chain mutably.
    pub fn chain_mut(&mut self) -> &mut MasteringChain {
        &mut self.chain
    }

    /// Add stage to chain.
    pub fn add_stage(&mut self, stage: ProcessingStage) {
        self.chain.add_stage(stage);
    }

    /// Remove stage from chain.
    pub fn remove_stage(&mut self, stage: &ProcessingStage) {
        self.chain.remove_stage(stage);
    }

    /// Move stage in chain.
    pub fn move_stage(&mut self, stage: &ProcessingStage, new_position: usize) {
        self.chain.move_stage(stage, new_position);
    }

    /// Swap stages in chain.
    pub fn swap_stages(&mut self, stage_a: &ProcessingStage, stage_b: &ProcessingStage) {
        self.chain.swap_stages(stage_a, stage_b);
    }

    /// Get chain order string.
    pub fn chain_order_string(&self) -> String {
        self.chain.chain_order_string()
    }

    /// Update metering data.
    pub fn update_metering(&mut self, data: MeteringData) {
        self.metering = data;
    }

    /// Get current metering.
    pub fn metering(&self) -> &MeteringData {
        &self.metering
    }

    /// Get gain reduction reading.
    pub fn gain_reduction(&self) -> &GainReductionReading {
        &self.metering.gain_reduction
    }

    /// Get LUFS reading.
    pub fn lufs_reading(&self) -> &LUFSReading {
        &self.metering.lufs
    }

    /// Get true peak reading.
    pub fn true_peak(&self) -> &TruePeakReading {
        &self.metering.true_peak
    }

    /// Get spectrum data.
    pub fn spectrum(&self) -> &SpectrumData {
        &self.metering.spectrum
    }

    /// Get input level.
    pub fn input_level(&self) -> &LevelMeterReading {
        &self.metering.input_level
    }

    /// Get output level.
    pub fn output_level(&self) -> &LevelMeterReading {
        &self.metering.output_level
    }

    /// Check for overload.
    pub fn is_overloaded(&self) -> bool {
        self.metering.is_overloaded()
    }

    /// Get warning status.
    pub fn warning_status(&self) -> &str {
        self.metering.warning_status()
    }

    /// Get metering status summary.
    pub fn metering_summary(&self) -> String {
        format!(
            "LUFS: {:.1} | True Peak: {:.2} dBFS | GR: {:.1} dB | Status: {}",
            self.metering.lufs.integrated,
            self.metering.true_peak.current_dbfs,
            self.metering.gain_reduction.current_db,
            self.metering.warning_status(),
        )
    }

    /// Get chain clipping risk.
    pub fn has_clipping_risk(&self) -> bool {
        self.chain.has_clipping_risk()
    }
}

impl Default for MasteringEngineV2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Mastering engine error.
#[derive(Debug, Clone)]
pub enum MasteringError {
    /// Preset not found.
    PresetNotFound(String),
    /// Invalid preset name.
    InvalidPresetName,
    /// Invalid stage.
    InvalidStage,
    /// Chain error.
    ChainError(String),
    /// Metering error.
    MeteringError(String),
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for MasteringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PresetNotFound(name) => write!(f, "Preset not found: {}", name),
            Self::InvalidPresetName => write!(f, "Invalid preset name"),
            Self::InvalidStage => write!(f, "Invalid processing stage"),
            Self::ChainError(msg) => write!(f, "Chain error: {}", msg),
            Self::MeteringError(msg) => write!(f, "Metering error: {}", msg),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for MasteringError {}

/// Helper to get negative infinity in dB.
fn negative_infinity_db() -> f64 {
    -120.0
}

fn mean_band<'a>(bands: impl Iterator<Item = &'a FrequencyBand>) -> f64 {
    let (sum, count) = bands.fold((0.0, 0usize), |(sum, count), b| {
        (sum + b.level_db, count + 1)
    });
    if count == 0 {
        -120.0
    } else {
        sum / count as f64
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mastering_engine_v2_creation() {
        let engine = MasteringEngineV2::new();
        assert_eq!(engine.preset_count(), 4); // Built-in presets.
        assert!(engine.active_preset().is_none());
        assert!(!engine.is_bypassed());
    }

    #[test]
    fn mastering_engine_v2_builtin_presets() {
        let engine = MasteringEngineV2::new();
        assert_eq!(engine.preset_count(), 4);
        assert_eq!(engine.builtin_preset_count(), 4);
        assert_eq!(engine.user_preset_count(), 0);
    }

    #[test]
    fn mastering_engine_v2_load_preset() {
        let mut engine = MasteringEngineV2::new();
        let result = engine.load_preset("Default");
        assert!(result.is_ok());
        assert_eq!(engine.active_preset(), Some("Default"));
    }

    #[test]
    fn mastering_engine_v2_load_nonexistent_preset() {
        let mut engine = MasteringEngineV2::new();
        let result = engine.load_preset("Nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn mastering_engine_v2_add_custom_preset() {
        let mut engine = MasteringEngineV2::new();
        let preset = MasteringPreset::new("My Preset", "My custom mastering preset");
        engine.add_preset(preset);

        assert_eq!(engine.preset_count(), 5);
        assert_eq!(engine.user_preset_count(), 1);
    }

    #[test]
    fn mastering_engine_v2_remove_custom_preset() {
        let mut engine = MasteringEngineV2::new();
        let preset = MasteringPreset::new("My Preset", "My custom mastering preset");
        engine.add_preset(preset);

        let result = engine.remove_preset("My Preset");
        assert!(result);
        assert_eq!(engine.user_preset_count(), 0);
    }

    #[test]
    fn mastering_engine_v2_cannot_remove_builtin() {
        let mut engine = MasteringEngineV2::new();
        let result = engine.remove_preset("Default");
        assert!(!result);
    }

    #[test]
    fn mastering_engine_v2_toggle_bypass() {
        let mut engine = MasteringEngineV2::new();
        assert!(!engine.is_bypassed());

        engine.toggle_bypass();
        assert!(engine.is_bypassed());

        engine.toggle_bypass();
        assert!(!engine.is_bypassed());
    }

    #[test]
    fn mastering_engine_v2_chain_stages() {
        let engine = MasteringEngineV2::new();
        let chain_str = engine.chain_order_string();
        assert!(chain_str.contains("Compressor"));
        assert!(chain_str.contains("Limiter"));
    }

    #[test]
    fn mastering_engine_v2_chain_swap_stages() {
        let mut engine = MasteringEngineV2::new();
        engine.swap_stages(&ProcessingStage::Compressor, &ProcessingStage::Limiter);

        let chain_str = engine.chain_order_string();
        assert!(chain_str.starts_with("Limiter"));
    }

    #[test]
    fn mastering_engine_v2_metering_data() {
        let mut engine = MasteringEngineV2::new();

        let metering = MeteringData::new(
            GainReductionReading::new(-3.0, -5.0),
            LUFSReading::new(-23.0, -24.0, -25.0, 6.0),
            TruePeakReading::new(-1.5, -0.5),
            LevelMeterReading::new(-6.0, -3.0, -8.0),
            LevelMeterReading::new(-5.0, -2.0, -7.0),
            SpectrumData::default(),
        );

        engine.update_metering(metering);
        assert!(!engine.is_overloaded());
        assert_eq!(engine.warning_status(), "compressing");
    }

    #[test]
    fn mastering_engine_v2_metering_overload() {
        let mut engine = MasteringEngineV2::new();

        let metering = MeteringData::new(
            GainReductionReading::idle(),
            LUFSReading::new(-18.0, -19.0, -20.0, 6.0),
            TruePeakReading::new(0.2, 0.5), // Overloaded.
            LevelMeterReading::idle(),
            LevelMeterReading::idle(),
            SpectrumData::default(),
        );

        engine.update_metering(metering);
        assert!(engine.is_overloaded());
        assert_eq!(engine.warning_status(), "overload");
    }

    #[test]
    fn mastering_engine_v2_metering_summary() {
        let mut engine = MasteringEngineV2::new();

        let metering = MeteringData::new(
            GainReductionReading::new(-3.0, -5.0),
            LUFSReading::new(-18.0, -19.0, -20.0, 6.0),
            TruePeakReading::new(-1.5, -0.5),
            LevelMeterReading::idle(),
            LevelMeterReading::idle(),
            SpectrumData::default(),
        );

        engine.update_metering(metering);
        let summary = engine.metering_summary();
        assert!(summary.contains("-18.0"));
        assert!(summary.contains("-1.5"));
    }

    #[test]
    fn mastering_engine_v2_ab_comparison() {
        let mut engine = MasteringEngineV2::new();

        assert!(!engine.is_ab_comparison_active());

        engine.set_preset_a(MasteringPreset::new("Preset A", "A"));
        engine.set_preset_b(MasteringPreset::new("Preset B", "B"));

        assert!(engine.is_ab_comparison_active());
    }

    #[test]
    fn gain_reduction_reading() {
        let reading = GainReductionReading::new(-3.0, -5.0);
        assert!(reading.is_reducing());
        assert!(reading.active);

        let idle = GainReductionReading::idle();
        assert!(!idle.is_reducing());
        assert!(!idle.active);
    }

    #[test]
    fn lufs_reading_within_target() {
        let reading = LUFSReading::new(-22.0, -23.0, -24.0, 4.0);
        assert!(reading.within_target());
        assert_eq!(reading.color_warning(), "green");
    }

    #[test]
    fn lufs_reading_too_loud() {
        let reading = LUFSReading::new(-20.0, -21.0, -22.0, 4.0);
        assert!(reading.too_loud());
        assert_eq!(reading.color_warning(), "red");
    }

    #[test]
    fn lufs_reading_too_quiet() {
        let reading = LUFSReading::new(-26.0, -25.0, -24.0, 4.0);
        assert!(reading.too_quiet());
        assert_eq!(reading.color_warning(), "yellow");
    }

    #[test]
    fn lufs_reading_bar_level() {
        let reading = LUFSReading::new(-23.0, -24.0, -25.0, 4.0);
        assert_eq!(reading.to_bar_level(), 0.0);

        let reading_max = LUFSReading::new(0.0, -1.0, -2.0, 4.0);
        assert_eq!(reading_max.to_bar_level(), 1.0);
    }

    #[test]
    fn true_peak_reading_overload() {
        let reading = TruePeakReading::new(0.2, 0.5);
        assert!(reading.is_overloaded());
        assert!(reading.overload_indicator());
    }

    #[test]
    fn true_peak_reading_normal() {
        let reading = TruePeakReading::new(-3.0, -1.0);
        assert!(!reading.is_overloaded());
    }

    #[test]
    fn spectrum_data_creation() {
        let levels = vec![-20.0, -15.0, -10.0, -12.0];
        let freqs = vec![100.0, 1000.0, 5000.0, 10000.0];
        let spectrum = SpectrumData::from_levels(levels, freqs);

        assert_eq!(spectrum.bands.len(), 4);
        assert!(spectrum.peak_frequency > 0.0);
    }

    #[test]
    fn level_meter_reading_clipping() {
        let reading = LevelMeterReading::new(-6.0, -0.05, -8.0);
        assert!(reading.is_clipping());
    }

    #[test]
    fn level_meter_reading_no_clipping() {
        let reading = LevelMeterReading::new(-6.0, -3.0, -8.0);
        assert!(!reading.is_clipping());
    }

    #[test]
    fn level_meter_bar_level() {
        let reading = LevelMeterReading::new(-30.0, -20.0, -35.0);
        let level = reading.bar_level();
        assert!((0.0..=1.0).contains(&level));
    }

    #[test]
    fn mastering_chain_new() {
        let chain = MasteringChain::new();
        assert_eq!(chain.stages.len(), 4);
        assert!(!chain.is_bypassed());
    }

    #[test]
    fn mastering_chain_add_stage() {
        let mut chain = MasteringChain::new();
        chain.add_stage(ProcessingStage::Compressor); // Already exists, should not duplicate.
        assert_eq!(
            chain
                .stages
                .iter()
                .filter(|s| **s == ProcessingStage::Compressor)
                .count(),
            1
        );
    }

    #[test]
    fn mastering_chain_remove_stage() {
        let mut chain = MasteringChain::new();
        let before = chain.stages.len();
        chain.remove_stage(&ProcessingStage::Exciter);
        assert_eq!(chain.stages.len(), before - 1);
    }

    #[test]
    fn mastering_chain_swap_stages() {
        let mut chain = MasteringChain::new();
        chain.swap_stages(&ProcessingStage::Compressor, &ProcessingStage::Limiter);

        assert_eq!(chain.stages[0], ProcessingStage::Limiter);
        assert_eq!(chain.stages[3], ProcessingStage::Compressor);
    }

    #[test]
    fn mastering_chain_bypass() {
        let mut chain = MasteringChain::new();
        assert!(!chain.is_bypassed());

        chain.bypass();
        assert!(chain.is_bypassed());

        chain.unbypass();
        assert!(!chain.is_bypassed());
    }

    #[test]
    fn mastering_chain_clipping_risk() {
        let mut chain = MasteringChain::new();
        chain.set_input_gain(0.0);
        chain.set_output_gain(1.0);
        chain.set_gain_staging(GainStagingMode::Manual);

        assert!(chain.has_clipping_risk());
    }

    #[test]
    fn mastering_chain_no_clipping_risk_auto() {
        let mut chain = MasteringChain::new();
        chain.set_input_gain(0.0);
        chain.set_output_gain(1.0);
        chain.set_gain_staging(GainStagingMode::Automatic);

        assert!(!chain.has_clipping_risk());
    }

    #[test]
    fn mastering_preset_creation() {
        let preset = MasteringPreset::new("Test", "Test preset");
        assert_eq!(preset.name(), "Test");
        assert_eq!(preset.description(), "Test preset");
        assert!(preset.is_user_created());
    }

    #[test]
    fn mastering_preset_built_in() {
        let preset = MasteringPreset::built_in(
            "Radio",
            "Radio optimized",
            vec![ProcessingStage::Compressor, ProcessingStage::Limiter],
        );
        assert!(!preset.is_user_created());
    }

    #[test]
    fn processing_stage_names() {
        assert_eq!(ProcessingStage::Compressor.name(), "Compressor");
        assert_eq!(ProcessingStage::EQ.name(), "EQ");
        assert_eq!(ProcessingStage::Exciter.name(), "Exciter");
        assert_eq!(ProcessingStage::Limiter.name(), "Limiter");
    }

    #[test]
    fn mastering_error_display() {
        assert_eq!(
            format!("{}", MasteringError::PresetNotFound("Test".to_string())),
            "Preset not found: Test"
        );
    }

    #[test]
    fn spectrum_overall_db() {
        let levels = vec![-20.0, -20.0, -20.0];
        let freqs = vec![100.0, 1000.0, 10000.0];
        let spectrum = SpectrumData::from_levels(levels, freqs);
        assert!(spectrum.overall_db < 0.0);
    }

    #[test]
    fn spectrum_data_peak_band() {
        let levels = vec![-30.0, -10.0, -20.0, -40.0];
        let freqs = vec![100.0, 1000.0, 5000.0, 10000.0];
        let spectrum = SpectrumData::from_levels(levels, freqs);

        let peak = spectrum.peak_band().unwrap();
        assert_eq!(peak, 1); // -10 dB is highest.
    }

    #[test]
    fn mastering_engine_v2_metering_idle() {
        let engine = MasteringEngineV2::new();
        let metering = engine.metering();
        assert_eq!(metering.lufs.integrated, LUFS_MIN);
    }

    #[test]
    fn mastering_engine_v2_warning_nominal() {
        let mut engine = MasteringEngineV2::new();
        engine.update_metering(MeteringData::idle());
        assert_eq!(engine.warning_status(), "nominal");
    }

    #[test]
    fn mastering_engine_v2_warning_too_loud() {
        let mut engine = MasteringEngineV2::new();
        let metering = MeteringData::new(
            GainReductionReading::idle(),
            LUFSReading::new(-20.0, -21.0, -22.0, 4.0),
            TruePeakReading::idle(),
            LevelMeterReading::idle(),
            LevelMeterReading::idle(),
            SpectrumData::default(),
        );
        engine.update_metering(metering);
        assert_eq!(engine.warning_status(), "too_loud");
    }

    #[test]
    fn mastering_engine_v2_warning_too_quiet() {
        let mut engine = MasteringEngineV2::new();
        let metering = MeteringData::new(
            GainReductionReading::idle(),
            LUFSReading::new(-28.0, -27.0, -26.0, 4.0),
            TruePeakReading::idle(),
            LevelMeterReading::idle(),
            LevelMeterReading::idle(),
            SpectrumData::default(),
        );
        engine.update_metering(metering);
        assert_eq!(engine.warning_status(), "too_quiet");
    }

    #[test]
    fn mastering_chain_move_stage() {
        let mut chain = MasteringChain::new();
        chain.move_stage(&ProcessingStage::Limiter, 0);

        assert_eq!(chain.stages[0], ProcessingStage::Limiter);
    }

    #[test]
    fn mastering_chain_set_gains() {
        let mut chain = MasteringChain::new();
        chain.set_input_gain(3.0);
        chain.set_output_gain(-1.0);

        assert_eq!(chain.input_gain_db, 3.0);
        assert_eq!(chain.output_gain_db, -1.0);
    }

    #[test]
    fn mastering_engine_v2_load_presets_preserve_bypass() {
        let mut engine = MasteringEngineV2::new();
        engine.load_preset("Default").unwrap();
        assert_eq!(engine.active_preset(), Some("Default"));
    }

    #[test]
    fn mastering_engine_v2_chain_order_string_format() {
        let engine = MasteringEngineV2::new();
        let order = engine.chain_order_string();
        assert!(order.contains("→"));
    }

    #[test]
    fn mastering_chain_swap_nonexistent() {
        let mut chain = MasteringChain::new();
        let result = chain.swap_stages(&ProcessingStage::Compressor, &ProcessingStage::Compressor);
        // Same stage, positions exist.
        assert!(result);
    }

    #[test]
    fn mastering_chain_remove_nonexistent() {
        let mut chain = MasteringChain::new();
        let result = chain.remove_stage(&ProcessingStage::Compressor);
        assert!(result); // Actually exists.
    }

    #[test]
    fn mastering_engine_preset_names_sorted() {
        let engine = MasteringEngineV2::new();
        let names = engine.preset_names();
        assert!(names.contains(&"Default".to_string()));
        assert!(names.contains(&"Radio Ready".to_string()));
    }

    #[test]
    fn mastering_chain_chain_order_with_stages() {
        let mut chain = MasteringChain::new();
        chain.add_stage(ProcessingStage::Compressor);
        chain.add_stage(ProcessingStage::Compressor);
        // Should not add duplicates.
        assert_eq!(
            chain
                .stages
                .iter()
                .filter(|s| **s == ProcessingStage::Compressor)
                .count(),
            1
        );
    }
}
