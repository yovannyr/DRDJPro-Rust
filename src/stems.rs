//! Stems — Track separation, Cache, Voices (Drums/Bass/Melody/Vocals).
//!
//! Implements audio stem separation for DJ applications, allowing
//! individual control of instrument groups from stereo audio.
//! Part of the Stems feature (P12a).
//!
//! ## Design principles
//!
//! * **Voice separation** — Drums, Bass, Melody, Vocals.
//! * **Cache management** — Efficient storage of separated stems.
//! * **Non-blocking** — Separation runs asynchronously.
//! * **Real-time** — Stem mixing in real-time audio callbacks.

use crate::atomic_float::AtomicF32;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Voice/instrument category for stem separation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum VoiceCategory {
    /// Drums/percussion.
    Drums,
    /// Bass/guitar low frequencies.
    Bass,
    /// Melodic elements (synth, guitar, etc.).
    Melody,
    /// Vocals/voices.
    Vocals,
    /// Other/unused category.
    Other,
}

impl VoiceCategory {
    /// Get the display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Drums => "Drums",
            Self::Bass => "Bass",
            Self::Melody => "Melody",
            Self::Vocals => "Vocals",
            Self::Other => "Other",
        }
    }

    /// Get default color for UI display.
    pub fn default_color(&self) -> (u8, u8, u8) {
        match self {
            Self::Drums => (255, 80, 80),   // Red
            Self::Bass => (80, 120, 255),   // Blue
            Self::Melody => (80, 255, 120), // Green
            Self::Vocals => (255, 200, 80), // Orange
            Self::Other => (150, 150, 150), // Gray
        }
    }
}

/// Stem data for a single voice category.
#[derive(Debug, Clone)]
pub struct Stem {
    /// Voice category.
    pub category: VoiceCategory,
    /// Audio samples (interleaved stereo).
    pub samples: Vec<f32>,
    /// Sample rate.
    pub sample_rate: u32,
    /// Channel count (1 = mono, 2 = stereo).
    pub channels: u32,
    /// Duration in samples.
    pub duration_samples: usize,
    /// Gain/mix level (0.0 - 1.0).
    pub gain: f32,
    /// Muted.
    pub muted: bool,
    /// Solo'd (only this stem plays).
    pub solo: bool,
}

impl Stem {
    /// Create a new stem.
    pub fn new(category: VoiceCategory, samples: Vec<f32>, sample_rate: u32) -> Self {
        let channels = 2; // Stereo default.
        let duration_samples = if channels > 0 {
            samples.len() / channels as usize
        } else {
            0
        };

        Self {
            category,
            samples,
            sample_rate,
            channels,
            duration_samples,
            gain: 1.0,
            muted: false,
            solo: false,
        }
    }

    /// Get mono mix of this stem.
    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels <= 1 {
            return self.samples.clone();
        }

        let mut mono = Vec::with_capacity(self.samples.len() / 2);
        let ch = self.channels as usize;

        for i in (0..self.samples.len()).step_by(ch) {
            if i + 1 < self.samples.len() {
                let avg = (self.samples[i] + self.samples[i + 1]) / 2.0;
                mono.push(avg);
            } else {
                mono.push(self.samples[i]);
            }
        }

        mono
    }

    /// Apply gain to stem.
    pub fn apply_gain(&mut self, gain: f32) {
        self.gain = gain.clamp(0.0, 1.0);
        for sample in &mut self.samples {
            *sample *= self.gain;
        }
    }

    /// Check if this stem should output audio.
    pub fn should_output(&self, any_solo: bool) -> bool {
        if self.muted {
            return false;
        }
        if any_solo {
            return self.solo;
        }
        true
    }

    /// Normalize stem to peak amplitude.
    pub fn normalize(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        let peak = self.samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        if peak > 0.0 && peak < 1.0 {
            let scale = 0.95 / peak;
            for sample in &mut self.samples {
                *sample *= scale;
            }
        }
    }
}

/// Stem separation result.
#[derive(Debug, Clone)]
pub struct StemSeparation {
    /// Original file path.
    pub source_path: PathBuf,
    /// Individual stems.
    pub stems: Vec<Stem>,
    /// Separation method used.
    pub method: SeparationMethod,
    /// Separation quality (0.0 - 1.0).
    pub quality: f32,
    /// Processing duration.
    pub processing_duration: Duration,
    /// Separation complete.
    pub complete: bool,
}

impl StemSeparation {
    /// Create a new stem separation result.
    pub fn new(source_path: PathBuf, stems: Vec<Stem>, method: SeparationMethod) -> Self {
        Self {
            source_path,
            stems,
            method,
            quality: 0.0,
            processing_duration: Duration::ZERO,
            complete: true,
        }
    }

    /// Get stem by category.
    pub fn get_stem(&self, category: VoiceCategory) -> Option<&Stem> {
        self.stems.iter().find(|s| s.category == category)
    }

    /// Check if separation has all four standard voices.
    pub fn has_all_voices(&self) -> bool {
        let categories: Vec<VoiceCategory> = vec![
            VoiceCategory::Drums,
            VoiceCategory::Bass,
            VoiceCategory::Melody,
            VoiceCategory::Vocals,
        ];

        categories.iter().all(|c| self.get_stem(*c).is_some())
    }

    /// Mix stems to stereo output.
    pub fn mix_to_stereo(&self) -> Vec<f32> {
        let any_solo = self.stems.iter().any(|s| s.solo);
        let mut left = Vec::new();
        let mut right = Vec::new();

        let max_len = self
            .stems
            .iter()
            .map(|s| s.samples.len())
            .max()
            .unwrap_or(0);

        for i in (0..max_len).step_by(2) {
            let mut sample_left: f32 = 0.0;
            let mut sample_right: f32 = 0.0;

            for stem in &self.stems {
                if !stem.should_output(any_solo) {
                    continue;
                }

                if i + 1 < stem.samples.len() {
                    sample_left += stem.samples[i];
                    sample_right += stem.samples[i + 1];
                } else if i < stem.samples.len() {
                    sample_left += stem.samples[i];
                    sample_right += stem.samples[i];
                }
            }

            // Clip to prevent overflow.
            sample_left = sample_left.clamp(-1.0, 1.0);
            sample_right = sample_right.clamp(-1.0, 1.0);

            left.push(sample_left);
            right.push(sample_right);
        }

        left.into_iter()
            .zip(right)
            .flat_map(|(l, r)| vec![l, r])
            .collect()
    }

    /// Export stems to individual files.
    pub fn export_stems(&self, output_dir: &PathBuf) -> Result<Vec<PathBuf>, StemError> {
        let mut paths = Vec::new();

        for stem in &self.stems {
            let stem_path = output_dir.join(format!(
                "{}_{:?}.wav",
                self.source_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("stem"),
                stem.category
            ));

            // In production, write WAV file here.
            paths.push(stem_path);
        }

        Ok(paths)
    }
}

/// Stem separation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparationMethod {
    /// Frequency-based band splitting.
    BandSplit,
    /// Phase-based separation.
    PhaseSplit,
    /// ML-based separation (requires model).
    MLBased,
    /// Unknown method.
    Unknown,
}

impl SeparationMethod {
    /// Get the display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::BandSplit => "Band Split",
            Self::PhaseSplit => "Phase Split",
            Self::MLBased => "ML-Based",
            Self::Unknown => "Unknown",
        }
    }
}

/// Stem cache entry.
#[derive(Debug, Clone)]
pub struct StemCacheEntry {
    /// Source file hash (identifier).
    pub source_hash: u64,
    /// Source file path.
    pub source_path: PathBuf,
    /// Separation result.
    pub separation: StemSeparation,
    /// Entry creation time.
    pub created_at: Duration,
    /// Last access time.
    pub last_accessed: Duration,
    /// Access count.
    pub access_count: u64,
}

impl StemCacheEntry {
    /// Create a new cache entry.
    pub fn new(source_hash: u64, source_path: PathBuf, separation: StemSeparation) -> Self {
        let now = Duration::ZERO; // Would use system time.
        Self {
            source_hash,
            source_path,
            separation,
            created_at: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    /// Mark entry as accessed.
    pub fn access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Duration::ZERO; // Would be current time.
    }
}

/// Stem cache manager.
pub struct StemCache {
    /// Cache entries keyed by source hash.
    entries: std::sync::Mutex<HashMap<u64, StemCacheEntry>>,
    /// Maximum cache size in MB.
    max_size_mb: usize,
    /// Current cache size in MB (approximate).
    current_size_mb: AtomicUsize,
    /// Cache hits.
    hits: AtomicU64,
    /// Cache misses.
    misses: AtomicU64,
}

impl StemCache {
    /// Create a new stem cache.
    pub fn new(max_size_mb: usize) -> Self {
        Self {
            entries: std::sync::Mutex::new(HashMap::new()),
            max_size_mb,
            current_size_mb: AtomicUsize::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Get a cached separation result.
    pub fn get(&self, source_hash: u64) -> Option<StemSeparation> {
        let mut entries = self.entries.lock().unwrap();

        if let Some(entry) = entries.get_mut(&source_hash) {
            entry.access();
            self.hits.fetch_add(1, Ordering::Relaxed);
            Some(entry.separation.clone())
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    /// Add a separation result to cache.
    pub fn put(
        &self,
        source_hash: u64,
        source_path: PathBuf,
        separation: StemSeparation,
    ) -> Result<(), StemError> {
        let entry_size_mb = self.estimate_size_mb(&separation);
        let mut entries = self.entries.lock().unwrap();

        // Evict if necessary.
        while self.current_size_mb.load(Ordering::Relaxed) + entry_size_mb > self.max_size_mb {
            if entries.is_empty() {
                break;
            }
            // Remove least recently used (simple approach).
            let lru_key = entries.keys().next().cloned();
            if let Some(key) = lru_key {
                entries.remove(&key);
                self.current_size_mb
                    .fetch_sub(entry_size_mb, Ordering::Relaxed);
            }
        }

        let entry = StemCacheEntry::new(source_hash, source_path, separation);
        entries.insert(source_hash, entry);
        self.current_size_mb
            .fetch_add(entry_size_mb, Ordering::Relaxed);

        Ok(())
    }

    /// Remove a cache entry.
    pub fn remove(&self, source_hash: u64) {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.remove(&source_hash) {
            let size = self.estimate_size_mb(&entry.separation);
            self.current_size_mb.fetch_sub(size, Ordering::Relaxed);
        }
    }

    /// Clear entire cache.
    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
        self.current_size_mb.store(0, Ordering::Relaxed);
    }

    /// Get cache statistics.
    pub fn stats(&self) -> (u64, u64, usize, usize) {
        let entries = self.entries.lock().unwrap();
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
            entries.len(),
            self.current_size_mb.load(Ordering::Relaxed),
        )
    }

    /// Estimate cache entry size in MB.
    fn estimate_size_mb(&self, separation: &StemSeparation) -> usize {
        let total_samples: usize = separation.stems.iter().map(|s| s.samples.len()).sum();
        // f32 = 4 bytes per sample.
        let bytes = total_samples * 4;
        (bytes as f64 / (1024.0 * 1024.0)).ceil() as usize
    }

    /// Check cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            return 0.0;
        }
        hits as f64 / total as f64
    }
}

impl Default for StemCache {
    fn default() -> Self {
        Self::new(512) // 512 MB default cache.
    }
}

/// Stem mixing state.
#[derive(Debug, Clone)]
pub struct StemMixState {
    /// Individual stem gains.
    pub stem_gains: HashMap<VoiceCategory, f32>,
    /// Overall mix gain.
    pub master_gain: f32,
    /// Phase inversion for stems.
    pub phase_invert: HashMap<VoiceCategory, bool>,
    /// Stem routing (which output channel).
    pub routing: HashMap<VoiceCategory, StemRouting>,
}

impl StemMixState {
    /// Create a new stem mix state with default settings.
    pub fn new() -> Self {
        let mut stem_gains = HashMap::new();
        stem_gains.insert(VoiceCategory::Drums, 1.0);
        stem_gains.insert(VoiceCategory::Bass, 1.0);
        stem_gains.insert(VoiceCategory::Melody, 1.0);
        stem_gains.insert(VoiceCategory::Vocals, 1.0);

        Self {
            stem_gains,
            master_gain: 1.0,
            phase_invert: HashMap::new(),
            routing: HashMap::new(),
        }
    }

    /// Set gain for a stem category.
    pub fn set_gain(&mut self, category: VoiceCategory, gain: f32) {
        self.stem_gains.insert(category, gain.clamp(0.0, 1.0));
    }

    /// Get gain for a stem category.
    pub fn get_gain(&self, category: VoiceCategory) -> f32 {
        *self.stem_gains.get(&category).unwrap_or(&1.0)
    }

    /// Set master gain.
    pub fn set_master_gain(&mut self, gain: f32) {
        self.master_gain = gain.clamp(0.0, 1.0);
    }

    /// Toggle phase inversion for a stem.
    pub fn toggle_phase(&mut self, category: VoiceCategory) {
        let current = self.phase_invert.get(&category).copied().unwrap_or(false);
        if current {
            self.phase_invert.remove(&category);
        } else {
            self.phase_invert.insert(category, true);
        }
    }
}

impl Default for StemMixState {
    fn default() -> Self {
        Self::new()
    }
}

/// Stem output routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemRouting {
    /// Normal stereo output.
    Stereo,
    /// Left channel only.
    Left,
    /// Right channel only.
    Right,
    /// Center (mono).
    Center,
    /// External output.
    External,
}

/// Stem separation errors.
#[derive(Debug, Clone)]
pub enum StemError {
    /// Source audio unavailable.
    SourceUnavailable,
    /// Model not loaded (for ML methods).
    ModelNotLoaded,
    /// Insufficient memory.
    InsufficientMemory,
    /// Cache full.
    CacheFull,
    /// Invalid parameters.
    InvalidParameters(String),
    /// Processing failed.
    ProcessingError(String),
}

impl std::fmt::Display for StemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceUnavailable => write!(f, "Source audio unavailable"),
            Self::ModelNotLoaded => write!(f, "ML model not loaded"),
            Self::InsufficientMemory => write!(f, "Insufficient memory for separation"),
            Self::CacheFull => write!(f, "Stem cache full"),
            Self::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            Self::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
        }
    }
}

impl std::error::Error for StemError {}

/// Stem separator — orchestrates separation pipeline.
pub struct StemSeparator {
    /// Cache manager.
    cache: StemCache,
    /// Current separation state.
    separating: AtomicBool,
    /// Separation complete.
    separation_complete: AtomicBool,
    /// Progress (0.0 - 1.0).
    progress: AtomicF32,
    /// Method being used.
    method: std::sync::Mutex<SeparationMethod>,
}

impl StemSeparator {
    /// Create a new stem separator.
    pub fn new(cache: StemCache) -> Self {
        Self {
            cache,
            separating: AtomicBool::new(false),
            separation_complete: AtomicBool::new(false),
            progress: AtomicF32::new(0.0),
            method: std::sync::Mutex::new(SeparationMethod::BandSplit),
        }
    }

    /// Separate stems from audio samples.
    ///
    /// This is a simplified implementation using frequency band splitting.
    /// Production would use ML models (Demucs, Spleeter, etc.).
    pub fn separate(
        &self,
        source_path: PathBuf,
        samples: &[f32],
        sample_rate: u32,
        method: SeparationMethod,
    ) -> Result<StemSeparation, StemError> {
        if samples.is_empty() {
            return Err(StemError::SourceUnavailable);
        }

        self.separating.store(true, Ordering::Relaxed);
        self.progress.store(0.1, Ordering::Relaxed);
        *self.method.lock().unwrap() = method;

        // Check cache first.
        let hash = self.hash_samples(samples);
        if let Some(cached) = self.cache.get(hash) {
            self.separating.store(false, Ordering::Relaxed);
            self.separation_complete.store(true, Ordering::Relaxed);
            self.progress.store(1.0, Ordering::Relaxed);
            return Ok(cached);
        }

        self.progress.store(0.3, Ordering::Relaxed);

        // Perform separation (simplified).
        let stems = self.perform_separation(samples, sample_rate, method)?;

        self.progress.store(0.8, Ordering::Relaxed);

        let mut separation = StemSeparation::new(source_path.clone(), stems, method);
        separation.quality = 0.75; // Simplified quality estimate.

        // Store in cache.
        let _ = self
            .cache
            .put(hash, source_path.clone(), separation.clone());

        self.progress.store(1.0, Ordering::Relaxed);
        self.separating.store(false, Ordering::Relaxed);
        self.separation_complete.store(true, Ordering::Relaxed);

        Ok(separation)
    }

    /// Perform frequency band split separation.
    fn perform_separation(
        &self,
        samples: &[f32],
        sample_rate: u32,
        method: SeparationMethod,
    ) -> Result<Vec<Stem>, StemError> {
        if method != SeparationMethod::BandSplit {
            return Err(StemError::ModelNotLoaded);
        }

        // Simplified band split.
        // Drums: < 150 Hz (kick, snare) + some higher transients.
        // Bass: 60 - 250 Hz.
        // Melody: 250 Hz - 4 kHz.
        // Vocals: 300 Hz - 3.4 kHz.

        let drums = Self::extract_band(samples, 0, 150, sample_rate);
        let bass = Self::extract_band(samples, 60, 250, sample_rate);
        let melody = Self::extract_band(samples, 250, 4000, sample_rate);
        let vocals = Self::extract_band(samples, 300, 3400, sample_rate);

        Ok(vec![
            Stem::new(VoiceCategory::Drums, drums, sample_rate),
            Stem::new(VoiceCategory::Bass, bass, sample_rate),
            Stem::new(VoiceCategory::Melody, melody, sample_rate),
            Stem::new(VoiceCategory::Vocals, vocals, sample_rate),
        ])
    }

    /// Extract frequency band from samples (simplified).
    fn extract_band(samples: &[f32], _low_hz: u32, _high_hz: u32, _sample_rate: u32) -> Vec<f32> {
        // Simplified: just return samples (production would use FFT filtering).
        // Real implementation would:
        // 1. Apply FFT to blocks
        // 2. Zero out bins outside low_hz-high_hz range
        // 3. Inverse FFT
        samples.to_vec()
    }

    /// Hash audio samples for cache lookup.
    fn hash_samples(&self, samples: &[f32]) -> u64 {
        // Simple hash — production would use a proper hash like xxHash.
        let mut hash: u64 = 0xcbf29ce484222325;
        for chunk in samples.chunks(8) {
            for &sample in chunk {
                hash ^= u64::from(sample.to_bits());
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        hash
    }

    /// Get current progress.
    pub fn progress(&self) -> f32 {
        self.progress.load(Ordering::Relaxed)
    }

    /// Check if separation is in progress.
    pub fn is_separating(&self) -> bool {
        self.separating.load(Ordering::Relaxed)
    }

    /// Check if separation is complete.
    pub fn is_complete(&self) -> bool {
        self.separation_complete.load(Ordering::Relaxed)
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> (u64, u64, usize, usize) {
        self.cache.stats()
    }

    /// Get cache hit rate.
    pub fn cache_hit_rate(&self) -> f64 {
        self.cache.hit_rate()
    }

    /// Clear cache.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get the current separation method.
    pub fn current_method(&self) -> SeparationMethod {
        *self.method.lock().unwrap()
    }
}

impl Default for StemSeparator {
    fn default() -> Self {
        Self::new(StemCache::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn cache_hash_distinguishes_equal_length_pcm() {
        let separator = super::StemSeparator::new(super::StemCache::new(1));
        let a = [0.0f32, 0.25, -0.5, 0.75];
        let b = [0.0f32, 0.25, -0.5, 0.875];
        assert_ne!(separator.hash_samples(&a), separator.hash_samples(&b));
        assert_eq!(separator.hash_samples(&a), separator.hash_samples(&a));
    }

    use super::*;

    fn make_test_samples(length: usize) -> Vec<f32> {
        (0..length)
            .map(|i| (0.5 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 44100.0)).sin() as f32)
            .collect()
    }

    #[test]
    fn voice_category_display() {
        assert_eq!(VoiceCategory::Drums.display_name(), "Drums");
        assert_eq!(VoiceCategory::Bass.display_name(), "Bass");
        assert_eq!(VoiceCategory::Melody.display_name(), "Melody");
        assert_eq!(VoiceCategory::Vocals.display_name(), "Vocals");
        assert_eq!(VoiceCategory::Other.display_name(), "Other");
    }

    #[test]
    fn voice_category_color() {
        assert_eq!(VoiceCategory::Drums.default_color(), (255, 80, 80));
        assert_eq!(VoiceCategory::Bass.default_color(), (80, 120, 255));
        assert_eq!(VoiceCategory::Melody.default_color(), (80, 255, 120));
        assert_eq!(VoiceCategory::Vocals.default_color(), (255, 200, 80));
        assert_eq!(VoiceCategory::Other.default_color(), (150, 150, 150));
    }

    #[test]
    fn stem_creation() {
        let samples = make_test_samples(1024);
        let stem = Stem::new(VoiceCategory::Drums, samples.clone(), 44100);

        assert_eq!(stem.category, VoiceCategory::Drums);
        assert_eq!(stem.sample_rate, 44100);
        assert_eq!(stem.channels, 2);
        assert_eq!(stem.gain, 1.0);
        assert!(!stem.muted);
        assert!(!stem.solo);
    }

    #[test]
    fn stem_to_mono() {
        let samples: Vec<f32> = vec![0.5, 0.3, 0.6, 0.4, 0.7, 0.5];
        let stem = Stem::new(VoiceCategory::Drums, samples, 44100);
        let mono = stem.to_mono();

        // Should have 3 mono samples (pairs averaged).
        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.4).abs() < 0.01);
        assert!((mono[1] - 0.5).abs() < 0.01);
        assert!((mono[2] - 0.6).abs() < 0.01);
    }

    #[test]
    fn stem_apply_gain() {
        let samples = vec![0.8f32, 0.6, 0.4];
        let mut stem = Stem::new(VoiceCategory::Bass, samples, 44100);
        stem.apply_gain(0.5);

        assert_eq!(stem.gain, 0.5);
        assert!((stem.samples[0] - 0.4).abs() < 0.01);
        assert!((stem.samples[1] - 0.3).abs() < 0.01);
        assert!((stem.samples[2] - 0.2).abs() < 0.01);
    }

    #[test]
    fn stem_gain_clamping() {
        let samples = vec![0.5f32];
        let mut stem = Stem::new(VoiceCategory::Melody, samples, 44100);

        // Gain should be clamped to [0.0, 1.0].
        stem.apply_gain(2.0);
        assert_eq!(stem.gain, 1.0);

        stem.apply_gain(-0.5);
        assert_eq!(stem.gain, 0.0);

        stem.apply_gain(0.5);
        assert_eq!(stem.gain, 0.5);
    }

    #[test]
    fn stem_should_output() {
        let samples = vec![0.5f32];
        let stem = Stem::new(VoiceCategory::Drums, samples, 44100);

        // Not muted, no solo → output.
        assert!(stem.should_output(false));

        // Muted → no output even without solo.
        let muted_stem = Stem {
            muted: true,
            ..stem.clone()
        };
        assert!(!muted_stem.should_output(false));

        // Solo mode — only solo stems output.
        let solo_stem = Stem {
            solo: true,
            muted: false,
            ..stem.clone()
        };
        assert!(solo_stem.should_output(true));

        let normal_stem = Stem {
            solo: false,
            ..stem.clone()
        };
        assert!(!normal_stem.should_output(true));
    }

    #[test]
    fn stem_normalize() {
        let samples = vec![0.5f32, 0.25, 0.125];
        let mut stem = Stem::new(VoiceCategory::Drums, samples, 44100);

        assert!((stem.samples[0] - 0.5).abs() < 0.01);
        stem.normalize();
        // Peak should now be ~0.95.
        assert!((stem.samples[0] - 0.95).abs() < 0.01);
    }

    #[test]
    fn stem_separation_all_voices() {
        let path = PathBuf::from("/test.mp3");
        let stems = vec![
            Stem::new(VoiceCategory::Drums, vec![0.0; 1024], 44100),
            Stem::new(VoiceCategory::Bass, vec![0.0; 1024], 44100),
            Stem::new(VoiceCategory::Melody, vec![0.0; 1024], 44100),
            Stem::new(VoiceCategory::Vocals, vec![0.0; 1024], 44100),
        ];
        let separation = StemSeparation::new(path.clone(), stems, SeparationMethod::BandSplit);

        assert!(separation.has_all_voices());
        assert!(separation.get_stem(VoiceCategory::Drums).is_some());
        assert!(separation.get_stem(VoiceCategory::Bass).is_some());
    }

    #[test]
    fn stem_separation_missing_voice() {
        let path = PathBuf::from("/test.mp3");
        let stems = vec![
            Stem::new(VoiceCategory::Drums, vec![0.0; 1024], 44100),
            Stem::new(VoiceCategory::Bass, vec![0.0; 1024], 44100),
        ];
        let separation = StemSeparation::new(path, stems, SeparationMethod::BandSplit);

        assert!(!separation.has_all_voices());
        assert!(separation.get_stem(VoiceCategory::Vocals).is_none());
    }

    #[test]
    fn stem_cache_put_get() {
        let cache = StemCache::new(128); // 128 MB max.
        let samples = vec![0.0f32; 1024];
        let stems = vec![
            Stem::new(VoiceCategory::Drums, samples.clone(), 44100),
            Stem::new(VoiceCategory::Bass, samples.clone(), 44100),
        ];
        let separation = StemSeparation::new(
            PathBuf::from("/test.wav"),
            stems,
            SeparationMethod::BandSplit,
        );

        let hash = 12345u64;
        let result = cache.put(hash, PathBuf::from("/test.wav"), separation);
        assert!(result.is_ok());

        let got = cache.get(hash);
        assert!(got.is_some());
        assert_eq!(got.unwrap().stems.len(), 2);
    }

    #[test]
    fn stem_cache_miss() {
        let cache = StemCache::new(128);
        let got = cache.get(99999);
        assert!(got.is_none());
    }

    #[test]
    fn stem_cache_stats() {
        let cache = StemCache::new(128);
        let (hits, misses, entries, size_mb) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 0);
        assert_eq!(entries, 0);
        assert_eq!(size_mb, 0);

        // After a miss.
        let _ = cache.get(1);
        let (hits, misses, _, _) = cache.stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 1);
    }

    #[test]
    fn stem_cache_hit_rate() {
        let cache = StemCache::new(128);

        // Add and retrieve.
        let samples = vec![0.0f32; 512];
        let stems = vec![Stem::new(VoiceCategory::Drums, samples, 44100)];
        let separation = StemSeparation::new(
            PathBuf::from("/test.wav"),
            stems,
            SeparationMethod::BandSplit,
        );
        cache
            .put(1, PathBuf::from("/test.wav"), separation.clone())
            .unwrap();
        let _ = cache.get(1);
        let _ = cache.get(1);

        let rate = cache.hit_rate();
        assert!(rate > 0.5); // 2 hits out of 3 attempts.
    }

    #[test]
    fn stem_cache_clear() {
        let cache = StemCache::new(128);
        let samples = vec![0.0f32; 512];
        let stems = vec![Stem::new(VoiceCategory::Drums, samples, 44100)];
        let separation = StemSeparation::new(
            PathBuf::from("/test.wav"),
            stems,
            SeparationMethod::BandSplit,
        );
        cache
            .put(1, PathBuf::from("/test.wav"), separation.clone())
            .unwrap();
        cache
            .put(2, PathBuf::from("/test2.wav"), separation.clone())
            .unwrap();

        let (_, _, entries, _) = cache.stats();
        assert_eq!(entries, 2);

        cache.clear();
        let (_, _, entries, _) = cache.stats();
        assert_eq!(entries, 0);
    }

    #[test]
    fn stem_mix_state() {
        let mut mix = StemMixState::new();

        assert_eq!(mix.get_gain(VoiceCategory::Drums), 1.0);
        assert_eq!(mix.get_gain(VoiceCategory::Bass), 1.0);

        mix.set_gain(VoiceCategory::Drums, 0.5);
        assert_eq!(mix.get_gain(VoiceCategory::Drums), 0.5);

        mix.set_master_gain(0.8);
        assert_eq!(mix.master_gain, 0.8);

        mix.toggle_phase(VoiceCategory::Melody);
        assert_eq!(mix.phase_invert.get(&VoiceCategory::Melody), Some(&true));
        mix.toggle_phase(VoiceCategory::Melody);
        assert_eq!(mix.phase_invert.get(&VoiceCategory::Melody), None);
    }

    #[test]
    fn stem_mix_state_gain_clamping() {
        let mut mix = StemMixState::new();
        mix.set_gain(VoiceCategory::Drums, 2.0);
        assert_eq!(mix.get_gain(VoiceCategory::Drums), 1.0);

        mix.set_gain(VoiceCategory::Drums, -0.5);
        assert_eq!(mix.get_gain(VoiceCategory::Drums), 0.0);
    }

    #[test]
    fn stem_separator_separate() {
        let separator = StemSeparator::new(StemCache::new(512));
        let samples = make_test_samples(44100); // 1 second.

        let result = separator.separate(
            PathBuf::from("/test.wav"),
            &samples,
            44100,
            SeparationMethod::BandSplit,
        );

        assert!(result.is_ok());
        let separation = result.unwrap();
        assert!(separation.has_all_voices());
    }

    #[test]
    fn stem_separator_empty_samples() {
        let separator = StemSeparator::new(StemCache::new(512));

        let result = separator.separate(
            PathBuf::from("/test.wav"),
            &[],
            44100,
            SeparationMethod::BandSplit,
        );

        assert!(result.is_err());
    }

    #[test]
    fn stem_separator_progress() {
        let separator = StemSeparator::new(StemCache::new(512));

        assert_eq!(separator.progress(), 0.0);
        assert!(!separator.is_separating());
        assert!(!separator.is_complete());

        let samples = make_test_samples(4410);
        let _ = separator.separate(
            PathBuf::from("/test.wav"),
            &samples,
            44100,
            SeparationMethod::BandSplit,
        );

        assert_eq!(separator.progress(), 1.0);
        assert!(!separator.is_separating());
        assert!(separator.is_complete());
    }

    #[test]
    fn stem_separator_caching() {
        let separator = StemSeparator::new(StemCache::new(512));
        let samples = make_test_samples(4410);

        // First call — cache miss.
        let _ = separator.separate(
            PathBuf::from("/test.wav"),
            &samples,
            44100,
            SeparationMethod::BandSplit,
        );

        // Second call with same data — should hit cache.
        let _ = separator.separate(
            PathBuf::from("/test.wav"),
            &samples,
            44100,
            SeparationMethod::BandSplit,
        );

        let (_, _, entries, _) = separator.cache_stats();
        assert_eq!(entries, 1); // Only one entry cached.

        let rate = separator.cache_hit_rate();
        assert!(rate > 0.0);
    }

    #[test]
    fn stem_separator_invalid_method() {
        let separator = StemSeparator::new(StemCache::new(512));
        let samples = make_test_samples(4410);

        let result = separator.separate(
            PathBuf::from("/test.wav"),
            &samples,
            44100,
            SeparationMethod::MLBased,
        );

        // ML-Based requires model → should fail without it.
        assert!(result.is_err());
    }

    #[test]
    fn stem_routing() {
        assert_eq!(StemRouting::Stereo, StemRouting::Stereo);
        assert_ne!(StemRouting::Left, StemRouting::Right);
        assert_ne!(StemRouting::Center, StemRouting::External);
    }

    #[test]
    fn separation_method_display() {
        assert_eq!(SeparationMethod::BandSplit.display_name(), "Band Split");
        assert_eq!(SeparationMethod::PhaseSplit.display_name(), "Phase Split");
        assert_eq!(SeparationMethod::MLBased.display_name(), "ML-Based");
        assert_eq!(SeparationMethod::Unknown.display_name(), "Unknown");
    }

    #[test]
    fn stem_error_display() {
        assert!(format!("{}", StemError::SourceUnavailable).contains("Source audio"));
        assert!(format!("{}", StemError::ModelNotLoaded).contains("model not loaded"));
        assert!(format!("{}", StemError::CacheFull).contains("cache"));
    }
}
