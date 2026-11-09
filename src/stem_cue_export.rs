//! Stems Part 2 — Stem Cue, Waveforms, Sync, Export, Cache Limits, Batch, Time Estimation.
//!
//! Extends the stem separation engine (P12a) with per-stem waveform display,
//! headphone cueing, synchronization guarantees, cache management,
//! stem export, quality indicators, batch extraction, and time estimation.
//! Part of the Stem Control feature (P12b).
//!
//! ## Design principles
//!
//! * **Synchronized** — All stems play in perfect sync.
//! * **Efficient** — Cache management with eviction policies.
//! * **Exportable** — Save individual stems as audio files.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::stems::VoiceCategory;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Stem waveform data.
#[derive(Debug, Clone)]
pub struct StemWaveform {
    /// Waveform samples (amplitude values).
    pub samples: Vec<f32>,
    /// Peak envelope.
    pub peaks: Vec<f32>,
    /// RMS envelope.
    pub rms: Vec<f32>,
    /// Duration of the stem.
    pub duration: Duration,
    /// Sample rate.
    pub sample_rate: u32,
    /// Voice category.
    pub category: VoiceCategory,
}

impl StemWaveform {
    /// Create new waveform from samples.
    pub fn new(
        samples: Vec<f32>,
        duration: Duration,
        sample_rate: u32,
        category: VoiceCategory,
    ) -> Self {
        let chunk_size = 100;
        let peaks: Vec<f32> = samples
            .chunks(chunk_size)
            .map(|chunk| chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max))
            .collect();

        let rms: Vec<f32> = samples
            .chunks(chunk_size)
            .map(|chunk| {
                let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
                (sum_sq / chunk.len() as f32).sqrt()
            })
            .collect();

        Self {
            samples,
            peaks,
            rms,
            duration,
            sample_rate,
            category,
        }
    }

    /// Create empty waveform.
    pub fn empty(category: VoiceCategory) -> Self {
        Self {
            samples: Vec::new(),
            peaks: Vec::new(),
            rms: Vec::new(),
            duration: Duration::ZERO,
            sample_rate: 44100,
            category,
        }
    }

    /// Get peak at position.
    pub fn peak_at(&self, position: f64) -> f32 {
        if self.peaks.is_empty() {
            return 0.0;
        }
        let idx = (position * self.peaks.len() as f64) as usize;
        self.peaks[idx.min(self.peaks.len() - 1)]
    }

    /// Get RMS at position.
    pub fn rms_at(&self, position: f64) -> f32 {
        if self.rms.is_empty() {
            return 0.0;
        }
        let idx = (position * self.rms.len() as f64) as usize;
        self.rms[idx.min(self.rms.len() - 1)]
    }

    /// Get waveform data for rendering (downsampled).
    pub fn render_data(&self, num_points: usize) -> Vec<f32> {
        if num_points == 0 {
            return Vec::new();
        }
        if self.peaks.is_empty() {
            return vec![0.0; num_points];
        }
        // Aggregate raw samples into exactly the requested number of display bins.
        // Reuse a sample for empty bins when the display exceeds source resolution.
        (0..num_points)
            .map(|point| {
                if self.samples.is_empty() {
                    return 0.0;
                }
                let start = point * self.samples.len() / num_points;
                let end = ((point + 1) * self.samples.len() / num_points)
                    .max(start + 1)
                    .min(self.samples.len());
                self.samples[start..end]
                    .iter()
                    .map(|value| value.abs())
                    .fold(0.0, f32::max)
            })
            .collect()
    }

    /// Get waveform position from time.
    pub fn position_from_time(&self, time: Duration) -> f64 {
        if self.duration.as_secs_f64() == 0.0 {
            return 0.0;
        }
        (time.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0)
    }

    /// Get sample at time position.
    pub fn sample_at(&self, position: f64) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let idx = (position * self.samples.len() as f64) as usize;
        self.samples[idx.min(self.samples.len() - 1)]
    }

    /// Get duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        self.duration.as_secs_f64()
    }

    /// Check if waveform is empty.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

impl Default for StemWaveform {
    fn default() -> Self {
        Self::empty(VoiceCategory::Drums)
    }
}

/// Stem cue point.
#[derive(Debug, Clone)]
pub struct StemCuePoint {
    /// Time position.
    pub time: Duration,
    /// Cue label.
    pub label: String,
    /// Is hot cue.
    pub is_hot_cue: bool,
    /// Cue color.
    pub color: (u8, u8, u8),
}

impl StemCuePoint {
    /// Create new cue point.
    pub fn new(time: Duration, label: &str, is_hot_cue: bool, color: (u8, u8, u8)) -> Self {
        Self {
            time,
            label: label.to_string(),
            is_hot_cue,
            color,
        }
    }

    /// Create hot cue.
    pub fn hot_cue(time: Duration, label: &str) -> Self {
        Self {
            time,
            label: label.to_string(),
            is_hot_cue: true,
            color: (255, 255, 0), // Yellow.
        }
    }

    /// Create regular cue point.
    pub fn cue_point(time: Duration, label: &str) -> Self {
        Self {
            time,
            label: label.to_string(),
            is_hot_cue: false,
            color: (0, 255, 0), // Green.
        }
    }
}

/// Stem playback state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StemPlaybackState {
    /// Stopped.
    #[default]
    Stopped,
    /// Playing.
    Playing,
    /// Paused.
    Paused,
    /// Cueing.
    Cueing,
}

/// Stem quality indicator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StemQuality {
    /// Excellent (high quality separation).
    Excellent,
    /// Good.
    Good,
    /// Fair (some artifacts).
    Fair,
    /// Poor (significant artifacts).
    Poor,
    /// Unknown (not separated yet).
    Unknown,
}

impl StemQuality {
    /// Get quality description.
    pub fn description(&self) -> &str {
        match self {
            Self::Excellent => "Excellent — Clean separation, minimal artifacts.",
            Self::Good => "Good — Minor artifacts, usable.",
            Self::Fair => "Fair — Noticeable artifacts.",
            Self::Poor => "Poor — Significant artifacts, limited usability.",
            Self::Unknown => "Unknown — Not yet separated.",
        }
    }

    /// Get quality score (0.0-1.0).
    pub fn score(&self) -> f32 {
        match self {
            Self::Excellent => 1.0,
            Self::Good => 0.8,
            Self::Fair => 0.5,
            Self::Poor => 0.2,
            Self::Unknown => 0.0,
        }
    }

    /// Get quality color.
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Excellent => (0, 255, 0),
            Self::Good => (100, 255, 0),
            Self::Fair => (255, 200, 0),
            Self::Poor => (255, 80, 80),
            Self::Unknown => (150, 150, 150),
        }
    }
}

/// Stem export format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StemExportFormat {
    /// WAV (uncompressed).
    Wav,
    /// FLAC (lossless compression).
    Flac,
    /// MP3 (compressed).
    MP3,
    /// AIFF (uncompressed).
    Aiff,
}

impl StemExportFormat {
    /// Get file extension.
    pub fn extension(&self) -> &str {
        match self {
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::MP3 => "mp3",
            Self::Aiff => "aiff",
        }
    }

    /// Get format name.
    pub fn name(&self) -> &str {
        match self {
            Self::Wav => "WAV",
            Self::Flac => "FLAC",
            Self::MP3 => "MP3",
            Self::Aiff => "AIFF",
        }
    }
}

/// Stem export job.
#[derive(Debug, Clone)]
pub struct StemExportJob {
    /// Track ID.
    pub track_id: String,
    /// Stem categories to export.
    pub categories: Vec<VoiceCategory>,
    /// Export format.
    pub format: StemExportFormat,
    /// Output directory.
    pub output_dir: String,
    /// Job status.
    pub status: ExportStatus,
    /// Started timestamp.
    pub started_at: Option<Instant>,
    /// Estimated completion.
    pub estimated_completion: Option<Duration>,
    /// Bytes exported.
    pub bytes_exported: u64,
    /// Total bytes.
    pub total_bytes: u64,
}

/// Export status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportStatus {
    /// Pending export.
    Pending,
    /// Exporting.
    Exporting,
    /// Export completed.
    Completed,
    /// Export failed.
    Failed { reason: String },
}

impl StemExportJob {
    /// Create new export job.
    pub fn new(
        track_id: &str,
        categories: Vec<VoiceCategory>,
        format: StemExportFormat,
        output_dir: &str,
    ) -> Self {
        Self {
            track_id: track_id.to_string(),
            categories,
            format,
            output_dir: output_dir.to_string(),
            status: ExportStatus::Pending,
            started_at: None,
            estimated_completion: None,
            bytes_exported: 0,
            total_bytes: 0,
        }
    }

    /// Start export.
    pub fn start(&mut self, estimated_duration: Duration) {
        self.status = ExportStatus::Exporting;
        self.started_at = Some(Instant::now());
        self.estimated_completion = Some(estimated_duration);
    }

    /// Complete export.
    pub fn complete(&mut self, bytes: u64) {
        self.status = ExportStatus::Completed;
        self.bytes_exported = bytes;
        self.total_bytes = bytes;
    }

    /// Fail export.
    pub fn fail(&mut self, reason: &str) {
        self.status = ExportStatus::Failed {
            reason: reason.to_string(),
        };
    }

    /// Check if job is complete.
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            ExportStatus::Completed | ExportStatus::Failed { .. }
        )
    }

    /// Get progress (0.0-1.0).
    pub fn progress(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        self.bytes_exported as f32 / self.total_bytes as f32
    }
}

/// Stem cache entry.
#[derive(Debug, Clone)]
pub struct StemCacheEntry {
    /// Track ID.
    pub track_id: String,
    /// Stem waveforms.
    pub waveforms: HashMap<VoiceCategory, StemWaveform>,
    /// Entry timestamp.
    pub cached_at: Instant,
    /// Entry size in bytes (approximate).
    pub size_bytes: usize,
    /// Access count.
    pub access_count: u32,
    /// Quality of separation.
    pub quality: StemQuality,
}

impl StemCacheEntry {
    /// Create new cache entry.
    pub fn new(track_id: &str, waveforms: HashMap<VoiceCategory, StemWaveform>) -> Self {
        let size_bytes = waveforms.values().map(|w| w.samples.len() * 4).sum(); // f32 = 4 bytes.

        Self {
            track_id: track_id.to_string(),
            waveforms,
            cached_at: Instant::now(),
            size_bytes,
            access_count: 0,
            quality: StemQuality::Unknown,
        }
    }

    /// Increment access count.
    pub fn increment_access(&mut self) {
        self.access_count += 1;
    }

    /// Check if entry is stale (older than 1 hour).
    pub fn is_stale(&self) -> bool {
        Instant::now().duration_since(self.cached_at) > Duration::from_secs(3600)
    }

    /// Get total size.
    pub fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    /// Get all categories.
    pub fn categories(&self) -> Vec<VoiceCategory> {
        self.waveforms.keys().copied().collect()
    }

    /// Get quality.
    pub fn quality(&self) -> &StemQuality {
        &self.quality
    }

    /// Set quality.
    pub fn set_quality(&mut self, quality: StemQuality) {
        self.quality = quality;
    }
}

/// Stem cache manager.
pub struct StemCacheManager {
    /// Cache entries.
    entries: HashMap<String, StemCacheEntry>,
    /// Maximum cache size in bytes.
    max_size_bytes: usize,
    /// Maximum number of entries.
    max_entries: usize,
    /// Total cache size.
    total_size: usize,
}

impl StemCacheManager {
    /// Create new cache manager.
    pub fn new(max_size_bytes: usize, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size_bytes,
            max_entries,
            total_size: 0,
        }
    }

    /// Insert or update cache entry.
    pub fn insert(&mut self, entry: StemCacheEntry) {
        // Evict if full.
        while (self.entries.len() >= self.max_entries
            || self.total_size + entry.size_bytes() > self.max_size_bytes)
            && !self.entries.is_empty()
        {
            self.evict_lru();
        }

        let track_id = entry.track_id.clone();
        self.total_size += entry.size_bytes();
        self.entries.insert(track_id, entry);
    }

    /// Get cache entry.
    pub fn get(&mut self, track_id: &str) -> Option<&mut StemCacheEntry> {
        let entry = self.entries.get_mut(track_id)?;
        entry.increment_access();
        Some(entry)
    }

    /// Remove cache entry.
    pub fn remove(&mut self, track_id: &str) -> Option<StemCacheEntry> {
        let entry = self.entries.remove(track_id);
        if let Some(ref e) = entry {
            self.total_size -= e.size_bytes();
        }
        entry
    }

    /// Check if track is in cache.
    pub fn contains(&self, track_id: &str) -> bool {
        self.entries.contains_key(track_id)
    }

    /// Get cache entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total cache size.
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Get maximum cache size.
    pub fn max_size(&self) -> usize {
        self.max_size_bytes
    }

    /// Get cache usage percentage.
    pub fn usage_percent(&self) -> f32 {
        if self.max_size_bytes == 0 {
            return 0.0;
        }
        (self.total_size as f32 / self.max_size_bytes as f32) * 100.0
    }

    /// Clear stale entries.
    pub fn clear_stale(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, entry| !entry.is_stale());
        let cleared = before - self.entries.len();

        // Recalculate size.
        self.total_size = self.entries.values().map(|e| e.size_bytes()).sum();

        cleared
    }

    /// Clear all entries.
    pub fn clear_all(&mut self) {
        self.entries.clear();
        self.total_size = 0;
    }

    /// Evict least recently used entry.
    fn evict_lru(&mut self) {
        let lru_key = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.access_count)
            .map(|(k, _)| k.clone());

        if let Some(key) = lru_key {
            if let Some(entry) = self.entries.remove(&key) {
                self.total_size -= entry.size_bytes();
            }
        }
    }

    /// Get cache info.
    pub fn info(&self) -> (usize, usize, usize, f32) {
        (
            self.entries.len(),
            self.total_size,
            self.max_size_bytes,
            self.usage_percent(),
        )
    }
}

impl Default for StemCacheManager {
    fn default() -> Self {
        Self::new(1024 * 1024 * 500, 100) // 500 MB max.
    }
}

/// Batch extraction job.
#[derive(Debug, Clone)]
pub struct BatchExtractionJob {
    /// Track IDs to separate.
    pub track_ids: Vec<String>,
    /// Target categories.
    pub categories: Vec<VoiceCategory>,
    /// Estimated total time.
    pub estimated_time: Duration,
    /// Current progress.
    pub progress: f32,
    /// Jobs completed.
    pub completed: u32,
    /// Jobs in progress.
    pub in_progress: u32,
    /// Jobs failed.
    pub failed: u32,
    /// Job status.
    pub status: BatchStatus,
}

/// Batch status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchStatus {
    /// Not started.
    Pending,
    /// Processing.
    Processing,
    /// Completed.
    Completed,
    /// Failed.
    Failed { reason: String },
}

impl BatchExtractionJob {
    /// Create new batch job.
    pub fn new(
        track_ids: Vec<String>,
        categories: Vec<VoiceCategory>,
        estimated_time: Duration,
    ) -> Self {
        Self {
            track_ids,
            categories,
            estimated_time,
            progress: 0.0,
            completed: 0,
            in_progress: 0,
            failed: 0,
            status: BatchStatus::Pending,
        }
    }

    /// Start batch job.
    pub fn start(&mut self) {
        self.status = BatchStatus::Processing;
        self.in_progress = 1;
    }

    /// Advance one job.
    pub fn advance_job(&mut self, success: bool) {
        self.in_progress = self.in_progress.saturating_sub(1);
        if success {
            self.completed += 1;
        } else {
            self.failed += 1;
        }
        self.progress = self.completed as f32 / self.track_ids.len().max(1) as f32;

        if self.in_progress == 0 && self.status == BatchStatus::Processing && self.failed == 0 {
            self.status = BatchStatus::Completed;
        }
    }

    /// Check if job is complete.
    pub fn is_complete(&self) -> bool {
        matches!(self.status, BatchStatus::Completed)
    }

    /// Get progress percentage.
    pub fn progress_percent(&self) -> f32 {
        self.progress * 100.0
    }

    /// Get remaining estimate.
    pub fn remaining_estimate(&self) -> Duration {
        if self.progress == 0.0 {
            return self.estimated_time;
        }
        let per_job = self.estimated_time / self.track_ids.len().max(1) as u32;
        let remaining =
            (self.track_ids.len() as f32 * self.progress - self.completed as f32) as u64;
        Duration::from_secs(per_job.as_secs() * remaining.max(1))
    }
}

/// Stem cue and export engine — manages waveforms, cueing, export, caching, and batch extraction.
pub struct StemCueExportEngine {
    /// Track waveforms (track_id -> stem waveforms).
    waveforms: HashMap<String, HashMap<VoiceCategory, StemWaveform>>,
    /// Cue points per track.
    cue_points: HashMap<String, Vec<StemCuePoint>>,
    /// Stem cache.
    cache: StemCacheManager,
    /// Current playback positions per track.
    playback_positions: HashMap<String, Duration>,
    /// Is synchronized.
    is_synced: bool,
    /// Active export jobs.
    export_jobs: Vec<StemExportJob>,
    /// Active batch jobs.
    batch_jobs: Vec<BatchExtractionJob>,
    /// Current batch job.
    current_batch: Option<usize>,
    /// Separation time estimates.
    separation_estimates: HashMap<String, Duration>,
}

impl StemCueExportEngine {
    /// Create new stem cue export engine.
    pub fn new() -> Self {
        Self {
            waveforms: HashMap::new(),
            cue_points: HashMap::new(),
            cache: StemCacheManager::default(),
            playback_positions: HashMap::new(),
            is_synced: true,
            export_jobs: Vec::new(),
            batch_jobs: Vec::new(),
            current_batch: None,
            separation_estimates: HashMap::new(),
        }
    }

    /// Register stem waveforms for a track.
    pub fn register_waveforms(
        &mut self,
        track_id: &str,
        waveforms: HashMap<VoiceCategory, StemWaveform>,
    ) {
        self.waveforms.insert(track_id.to_string(), waveforms);
    }

    /// Get stem waveform for a track and category.
    pub fn get_stem_waveform(
        &self,
        track_id: &str,
        category: VoiceCategory,
    ) -> Option<&StemWaveform> {
        self.waveforms.get(track_id).and_then(|w| w.get(&category))
    }

    /// Get all stem waveforms for a track.
    pub fn get_track_waveforms(
        &self,
        track_id: &str,
    ) -> Option<&HashMap<VoiceCategory, StemWaveform>> {
        self.waveforms.get(track_id)
    }

    /// Add cue point.
    pub fn add_cue_point(&mut self, track_id: &str, cue: StemCuePoint) {
        self.cue_points
            .entry(track_id.to_string())
            .or_default()
            .push(cue);
    }

    /// Get cue points for track.
    pub fn get_cue_points(&self, track_id: &str) -> &[StemCuePoint] {
        self.cue_points
            .get(track_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Remove cue point by index.
    pub fn remove_cue_point(&mut self, track_id: &str, index: usize) -> bool {
        if let Some(points) = self.cue_points.get_mut(track_id) {
            if index < points.len() {
                points.remove(index);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Set playback position.
    pub fn set_playback_position(&mut self, track_id: &str, position: Duration) {
        self.playback_positions
            .insert(track_id.to_string(), position);
    }

    /// Get playback position.
    pub fn playback_position(&self, track_id: &str) -> Duration {
        self.playback_positions
            .get(track_id)
            .copied()
            .unwrap_or(Duration::ZERO)
    }

    /// Check if all stems are synchronized.
    pub fn is_synchronized(&self) -> bool {
        self.is_synced
    }

    /// Set synchronization state.
    pub fn set_synchronized(&mut self, synced: bool) {
        self.is_synced = synced;
    }

    /// Check if cueing is available for a stem.
    pub fn is_cueing_available(&self, track_id: &str, category: VoiceCategory) -> bool {
        self.get_stem_waveform(track_id, category)
            .map(|w| !w.is_empty())
            .unwrap_or(false)
    }

    /// Cache stem entry.
    pub fn cache_entry(&mut self, entry: StemCacheEntry) {
        self.cache.insert(entry);
    }

    /// Get cached entry.
    pub fn get_cached_entry(&mut self, track_id: &str) -> Option<&mut StemCacheEntry> {
        self.cache.get(track_id)
    }

    /// Check if track is in cache.
    pub fn is_cached(&self, track_id: &str) -> bool {
        self.cache.contains(track_id)
    }

    /// Get cache info.
    pub fn cache_info(&self) -> (usize, usize, usize, f32) {
        self.cache.info()
    }

    /// Clear stale cache entries.
    pub fn clear_stale_cache(&mut self) -> usize {
        self.cache.clear_stale()
    }

    /// Clear all cache.
    pub fn clear_all_cache(&mut self) {
        self.cache.clear_all();
    }

    /// Create export job.
    pub fn create_export_job(
        &mut self,
        track_id: &str,
        categories: Vec<VoiceCategory>,
        format: StemExportFormat,
        output_dir: &str,
    ) {
        let estimated_duration = Duration::from_secs(30 * categories.len() as u64); // Approximate.

        let mut job = StemExportJob::new(track_id, categories.clone(), format, output_dir);

        // Estimate total bytes.
        let total_bytes = categories
            .iter()
            .filter_map(|cat| self.get_stem_waveform(track_id, *cat))
            .map(|w| (w.samples.len() as u64) * 4)
            .sum();

        job.total_bytes = total_bytes;
        job.estimated_completion = Some(estimated_duration);

        self.export_jobs.push(job);
    }

    /// Start export job.
    pub fn start_export_job(&mut self, index: usize) -> bool {
        if index < self.export_jobs.len() {
            let estimated = self.export_jobs[index]
                .estimated_completion
                .unwrap_or(Duration::from_secs(30));
            self.export_jobs[index].start(estimated);
            true
        } else {
            false
        }
    }

    /// Complete export job.
    pub fn complete_export_job(&mut self, index: usize, bytes: u64) -> bool {
        if index < self.export_jobs.len() {
            self.export_jobs[index].complete(bytes);
            true
        } else {
            false
        }
    }

    /// Get export jobs.
    pub fn export_jobs(&self) -> &[StemExportJob] {
        &self.export_jobs
    }

    /// Create batch extraction job.
    pub fn create_batch_job(
        &mut self,
        track_ids: Vec<String>,
        categories: Vec<VoiceCategory>,
        estimated_time: Duration,
    ) {
        let job = BatchExtractionJob::new(track_ids, categories, estimated_time);
        self.batch_jobs.push(job);
        self.current_batch = Some(self.batch_jobs.len() - 1);
    }

    /// Start current batch job.
    pub fn start_batch_job(&mut self) -> bool {
        if let Some(idx) = self.current_batch {
            if idx < self.batch_jobs.len() {
                self.batch_jobs[idx].start();
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get current batch job.
    pub fn current_batch_job(&self) -> Option<&BatchExtractionJob> {
        self.current_batch.and_then(|idx| self.batch_jobs.get(idx))
    }

    /// Advance batch job by one track.
    pub fn advance_batch_job(&mut self, success: bool) -> bool {
        if let Some(idx) = self.current_batch {
            if idx < self.batch_jobs.len() && self.batch_jobs[idx].status == BatchStatus::Processing
            {
                self.batch_jobs[idx].advance_job(success);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get batch jobs.
    pub fn batch_jobs(&self) -> &[BatchExtractionJob] {
        &self.batch_jobs
    }

    /// Set separation time estimate.
    pub fn set_separation_estimate(&mut self, track_id: &str, duration: Duration) {
        self.separation_estimates
            .insert(track_id.to_string(), duration);
    }

    /// Get separation time estimate.
    pub fn separation_estimate(&self, track_id: &str) -> Option<Duration> {
        self.separation_estimates.get(track_id).copied()
    }

    /// Get waveform count for track.
    pub fn waveform_count(&self, track_id: &str) -> usize {
        self.waveforms.get(track_id).map(|w| w.len()).unwrap_or(0)
    }

    /// Get all track IDs.
    pub fn track_ids(&self) -> Vec<String> {
        self.waveforms.keys().cloned().collect()
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        format!(
            "Stems: {} tracks | Sync: {} | Cache: {:.1}% | Exports: {} | Batch: {}",
            self.waveforms.len(),
            if self.is_synced { "Yes" } else { "No" },
            self.cache.usage_percent(),
            self.export_jobs.iter().filter(|j| !j.is_complete()).count(),
            self.current_batch
                .map_or("None".to_string(), |_| "Active".to_string()),
        )
    }

    /// Get available export formats.
    pub fn available_formats() -> &'static [StemExportFormat] {
        &[
            StemExportFormat::Wav,
            StemExportFormat::Flac,
            StemExportFormat::MP3,
            StemExportFormat::Aiff,
        ]
    }

    /// Get available categories.
    pub fn available_categories() -> &'static [VoiceCategory] {
        &[
            VoiceCategory::Drums,
            VoiceCategory::Bass,
            VoiceCategory::Melody,
            VoiceCategory::Vocals,
        ]
    }
}

impl Default for StemCueExportEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Stem export error.
#[derive(Debug, Clone)]
pub enum StemExportError {
    /// Waveform not found.
    WaveformNotFound,
    /// Cache full.
    CacheFull,
    /// Export failed.
    ExportFailed(String),
    /// Invalid format.
    InvalidFormat,
    /// No tracks in batch.
    NoTracksInBatch,
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for StemExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WaveformNotFound => write!(f, "Waveform not found"),
            Self::CacheFull => write!(f, "Stem cache is full"),
            Self::ExportFailed(msg) => write!(f, "Export failed: {}", msg),
            Self::InvalidFormat => write!(f, "Invalid export format"),
            Self::NoTracksInBatch => write!(f, "No tracks in batch"),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for StemExportError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_waveform_creation() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let waveform = StemWaveform::new(
            samples.clone(),
            Duration::from_secs(3),
            44100,
            VoiceCategory::Drums,
        );
        assert_eq!(waveform.duration_secs(), 3.0);
        assert_eq!(waveform.sample_rate, 44100);
        assert_eq!(waveform.category, VoiceCategory::Drums);
    }

    #[test]
    fn stem_waveform_peak_at() {
        let samples: Vec<f32> = vec![0.0, 1.0, 0.5, -0.5, -1.0, 0.0];
        let waveform =
            StemWaveform::new(samples, Duration::from_secs(1), 44100, VoiceCategory::Drums);
        let peak = waveform.peak_at(0.5);
        assert!(peak >= 0.0);
    }

    #[test]
    fn stem_waveform_render_data() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let waveform =
            StemWaveform::new(samples, Duration::from_secs(1), 44100, VoiceCategory::Drums);
        let render = waveform.render_data(50);
        assert_eq!(render.len(), 50);
    }

    #[test]
    fn stem_waveform_empty() {
        let waveform = StemWaveform::empty(VoiceCategory::Vocals);
        assert!(waveform.is_empty());
        assert_eq!(waveform.peak_at(0.5), 0.0);
    }

    #[test]
    fn stem_cue_point_hot_cue() {
        let cue = StemCuePoint::hot_cue(Duration::from_millis(500), "Hot 1");
        assert!(cue.is_hot_cue);
        assert_eq!(cue.label, "Hot 1");
        assert_eq!(cue.color, (255, 255, 0));
    }

    #[test]
    fn stem_cue_point_regular() {
        let cue = StemCuePoint::cue_point(Duration::from_secs(30), "Intro");
        assert!(!cue.is_hot_cue);
        assert_eq!(cue.label, "Intro");
        assert_eq!(cue.color, (0, 255, 0));
    }

    #[test]
    fn stem_quality_scores() {
        assert_eq!(StemQuality::Excellent.score(), 1.0);
        assert_eq!(StemQuality::Good.score(), 0.8);
        assert_eq!(StemQuality::Fair.score(), 0.5);
        assert_eq!(StemQuality::Poor.score(), 0.2);
        assert_eq!(StemQuality::Unknown.score(), 0.0);
    }

    #[test]
    fn stem_quality_colors() {
        assert_eq!(StemQuality::Excellent.color(), (0, 255, 0));
        assert_eq!(StemQuality::Poor.color(), (255, 80, 80));
    }

    #[test]
    fn stem_export_format() {
        assert_eq!(StemExportFormat::Wav.extension(), "wav");
        assert_eq!(StemExportFormat::Flac.extension(), "flac");
        assert_eq!(StemExportFormat::MP3.extension(), "mp3");
        assert_eq!(StemExportFormat::Aiff.extension(), "aiff");
    }

    #[test]
    fn stem_export_job_creation() {
        let job = StemExportJob::new(
            "t1",
            vec![VoiceCategory::Drums],
            StemExportFormat::Wav,
            "/tmp",
        );
        assert!(matches!(job.status, ExportStatus::Pending));
        assert!(!job.is_complete());
    }

    #[test]
    fn stem_export_job_start_complete() {
        let mut job = StemExportJob::new(
            "t1",
            vec![VoiceCategory::Drums],
            StemExportFormat::Wav,
            "/tmp",
        );
        job.start(Duration::from_secs(30));
        assert!(matches!(job.status, ExportStatus::Exporting));

        job.complete(1024);
        assert!(job.is_complete());
        assert_eq!(job.bytes_exported, 1024);
    }

    #[test]
    fn stem_export_job_progress() {
        let mut job = StemExportJob::new(
            "t1",
            vec![VoiceCategory::Drums],
            StemExportFormat::Wav,
            "/tmp",
        );
        job.total_bytes = 1000;
        job.bytes_exported = 500;
        assert_eq!(job.progress(), 0.5);
    }

    #[test]
    fn stem_cache_manager_insert_get() {
        let mut manager = StemCacheManager::new(1024 * 1024, 10);
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Drums,
            StemWaveform::empty(VoiceCategory::Drums),
        );
        let entry = StemCacheEntry::new("t1", waveforms);

        manager.insert(entry);
        assert!(manager.contains("t1"));
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn stem_cache_manager_remove() {
        let mut manager = StemCacheManager::new(1024 * 1024, 10);
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Bass,
            StemWaveform::empty(VoiceCategory::Bass),
        );
        let entry = StemCacheEntry::new("t1", waveforms);

        manager.insert(entry);
        let removed = manager.remove("t1");
        assert!(removed.is_some());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn stem_cache_manager_eviction() {
        let mut manager = StemCacheManager::new(1024 * 1024 * 10, 3);

        for i in 0..5 {
            let mut waveforms = HashMap::new();
            waveforms.insert(
                VoiceCategory::Drums,
                StemWaveform::empty(VoiceCategory::Drums),
            );
            let entry = StemCacheEntry::new(&format!("t{}", i), waveforms);
            manager.insert(entry);
        }

        assert!(manager.len() <= 3);
    }

    #[test]
    fn stem_cache_manager_access_count() {
        let mut manager = StemCacheManager::new(1024 * 1024, 10);
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Melody,
            StemWaveform::empty(VoiceCategory::Melody),
        );
        let entry = StemCacheEntry::new("t1", waveforms);

        manager.insert(entry);

        // Access multiple times.
        for _ in 0..5 {
            manager.get("t1");
        }

        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn stem_cache_manager_info() {
        let manager = StemCacheManager::new(1024 * 1024, 100);
        let (count, size, max_size, usage) = manager.info();
        assert_eq!(count, 0);
        assert_eq!(size, 0);
        assert_eq!(max_size, 1024 * 1024);
        assert_eq!(usage, 0.0);
    }

    #[test]
    fn stem_cache_manager_clear_stale() {
        let mut manager = StemCacheManager::new(1024 * 1024, 10);
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Vocals,
            StemWaveform::empty(VoiceCategory::Vocals),
        );
        let entry = StemCacheEntry::new("t1", waveforms);

        // Create entry with stale timestamp.
        // Note: is_stale() checks if entry is > 1 hour old.
        // For testing, we insert and then verify no stale entries exist initially.
        manager.insert(entry);
        let cleared = manager.clear_stale();
        assert_eq!(cleared, 0); // Not stale yet.
    }

    #[test]
    fn stem_cache_manager_clear_all() {
        let mut manager = StemCacheManager::new(1024 * 1024, 10);
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Drums,
            StemWaveform::empty(VoiceCategory::Drums),
        );
        manager.insert(StemCacheEntry::new("t1", waveforms));

        manager.clear_all();
        assert!(manager.is_empty());
        assert_eq!(manager.total_size(), 0);
    }

    #[test]
    fn stem_cache_manager_usage_percent() {
        let manager = StemCacheManager::new(1024 * 1024, 100);
        assert_eq!(manager.usage_percent(), 0.0);
    }

    #[test]
    fn stem_cue_export_engine_creation() {
        let engine = StemCueExportEngine::new();
        assert!(engine.track_ids().is_empty());
        assert!(engine.is_synchronized());
    }

    #[test]
    fn stem_cue_export_engine_register_waveforms() {
        let mut engine = StemCueExportEngine::new();
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Drums,
            StemWaveform::empty(VoiceCategory::Drums),
        );
        engine.register_waveforms("t1", waveforms);

        assert_eq!(engine.waveform_count("t1"), 1);
    }

    #[test]
    fn stem_cue_export_engine_add_cue_point() {
        let mut engine = StemCueExportEngine::new();
        engine.add_cue_point(
            "t1",
            StemCuePoint::hot_cue(Duration::from_millis(100), "Hot 1"),
        );

        let points = engine.get_cue_points("t1");
        assert_eq!(points.len(), 1);
    }

    #[test]
    fn stem_cue_export_engine_remove_cue_point() {
        let mut engine = StemCueExportEngine::new();
        engine.add_cue_point(
            "t1",
            StemCuePoint::hot_cue(Duration::from_millis(100), "Hot 1"),
        );
        engine.add_cue_point(
            "t1",
            StemCuePoint::cue_point(Duration::from_secs(30), "Intro"),
        );

        engine.remove_cue_point("t1", 0);
        assert_eq!(engine.get_cue_points("t1").len(), 1);
    }

    #[test]
    fn stem_cue_export_engine_playback_position() {
        let mut engine = StemCueExportEngine::new();
        engine.set_playback_position("t1", Duration::from_secs(60));
        assert_eq!(engine.playback_position("t1"), Duration::from_secs(60));
    }

    #[test]
    fn stem_cue_export_engine_sync_state() {
        let mut engine = StemCueExportEngine::new();
        assert!(engine.is_synchronized());

        engine.set_synchronized(false);
        assert!(!engine.is_synchronized());
    }

    #[test]
    fn stem_cue_export_engine_create_export_job() {
        let mut engine = StemCueExportEngine::new();
        engine.create_export_job(
            "t1",
            vec![VoiceCategory::Drums],
            StemExportFormat::Wav,
            "/tmp",
        );

        assert_eq!(engine.export_jobs().len(), 1);
    }

    #[test]
    fn stem_cue_export_engine_start_export() {
        let mut engine = StemCueExportEngine::new();
        engine.create_export_job(
            "t1",
            vec![VoiceCategory::Drums],
            StemExportFormat::Wav,
            "/tmp",
        );
        engine.start_export_job(0);

        assert!(matches!(
            engine.export_jobs()[0].status,
            ExportStatus::Exporting
        ));
    }

    #[test]
    fn stem_cue_export_engine_complete_export() {
        let mut engine = StemCueExportEngine::new();
        engine.create_export_job(
            "t1",
            vec![VoiceCategory::Drums],
            StemExportFormat::Wav,
            "/tmp",
        );
        engine.start_export_job(0);
        engine.complete_export_job(0, 1024);

        assert!(engine.export_jobs()[0].is_complete());
    }

    #[test]
    fn stem_cue_export_engine_batch_job() {
        let mut engine = StemCueExportEngine::new();
        engine.create_batch_job(
            vec!["t1".to_string(), "t2".to_string()],
            vec![VoiceCategory::Drums],
            Duration::from_secs(60),
        );

        assert!(engine.start_batch_job());
        assert!(engine.current_batch_job().is_some());
    }

    #[test]
    fn stem_cue_export_engine_advance_batch() {
        let mut engine = StemCueExportEngine::new();
        engine.create_batch_job(
            vec!["t1".to_string(), "t2".to_string()],
            vec![VoiceCategory::Drums],
            Duration::from_secs(60),
        );
        engine.start_batch_job();
        engine.advance_batch_job(true);

        let job = engine.current_batch_job().unwrap();
        assert_eq!(job.completed, 1);
    }

    #[test]
    fn stem_cue_export_engine_separation_estimate() {
        let mut engine = StemCueExportEngine::new();
        engine.set_separation_estimate("t1", Duration::from_secs(30));
        assert_eq!(
            engine.separation_estimate("t1"),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn stem_cue_export_engine_status_string() {
        let engine = StemCueExportEngine::new();
        let status = engine.status_string();
        assert!(status.contains("Stems:"));
    }

    #[test]
    fn stem_cue_export_engine_available_formats() {
        let formats = StemCueExportEngine::available_formats();
        assert!(formats.len() >= 3);
    }

    #[test]
    fn stem_cue_export_engine_available_categories() {
        let categories = StemCueExportEngine::available_categories();
        assert!(categories.contains(&VoiceCategory::Drums));
        assert!(categories.contains(&VoiceCategory::Bass));
    }

    #[test]
    fn stem_cue_export_engine_cueing_available() {
        let mut engine = StemCueExportEngine::new();
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Drums,
            StemWaveform::new(
                vec![0.5; 100],
                Duration::from_secs(1),
                44100,
                VoiceCategory::Drums,
            ),
        );
        engine.register_waveforms("t1", waveforms);

        assert!(engine.is_cueing_available("t1", VoiceCategory::Drums));
        assert!(!engine.is_cueing_available("t1", VoiceCategory::Bass));
    }

    #[test]
    fn stem_cue_export_engine_cache_integration() {
        let mut engine = StemCueExportEngine::new();
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Drums,
            StemWaveform::empty(VoiceCategory::Drums),
        );
        let mut entry = StemCacheEntry::new("t1", waveforms);
        entry.set_quality(StemQuality::Good);

        engine.cache_entry(entry);
        assert!(engine.is_cached("t1"));

        let info = engine.cache_info();
        assert_eq!(info.0, 1);
    }

    #[test]
    fn stem_cue_export_engine_export_job_not_complete_pending() {
        let engine = StemCueExportEngine::new();
        assert!(!engine.export_jobs().iter().any(|j| j.is_complete()));
    }

    #[test]
    fn stem_export_error_display() {
        assert_eq!(
            format!("{}", StemExportError::WaveformNotFound),
            "Waveform not found"
        );
        assert_eq!(
            format!("{}", StemExportError::CacheFull),
            "Stem cache is full"
        );
    }

    #[test]
    fn batch_extraction_job_progress() {
        let mut job = BatchExtractionJob::new(
            vec!["t1".to_string(), "t2".to_string(), "t3".to_string()],
            vec![VoiceCategory::Drums],
            Duration::from_secs(90),
        );

        job.start();
        job.advance_job(true);
        job.advance_job(true);
        assert_eq!(job.completed, 2);
        assert_eq!(job.progress_percent(), (2.0 / 3.0) * 100.0);
    }

    #[test]
    fn batch_extraction_job_failed() {
        let mut job = BatchExtractionJob::new(
            vec!["t1".to_string()],
            vec![VoiceCategory::Drums],
            Duration::from_secs(30),
        );

        job.start();
        job.advance_job(false);
        assert_eq!(job.failed, 1);
    }

    #[test]
    fn batch_extraction_job_remaining_estimate() {
        let mut job = BatchExtractionJob::new(
            vec!["t1".to_string(), "t2".to_string()],
            vec![VoiceCategory::Drums],
            Duration::from_secs(60),
        );

        job.start();
        job.advance_job(true);

        let remaining = job.remaining_estimate();
        assert!(remaining <= Duration::from_secs(30));
    }

    #[test]
    fn stem_waveform_position_from_time() {
        let samples: Vec<f32> = vec![0.5; 100];
        let waveform = StemWaveform::new(
            samples,
            Duration::from_secs(10),
            44100,
            VoiceCategory::Drums,
        );
        assert_eq!(waveform.position_from_time(Duration::from_secs(5)), 0.5);
        assert_eq!(waveform.position_from_time(Duration::from_secs(15)), 1.0); // Clamped.
    }

    #[test]
    fn stem_waveform_sample_at() {
        let samples: Vec<f32> = vec![0.0, 0.5, 1.0, 0.5, 0.0];
        let waveform =
            StemWaveform::new(samples, Duration::from_secs(1), 44100, VoiceCategory::Drums);
        let sample = waveform.sample_at(0.5);
        assert!(sample >= 0.0);
    }

    #[test]
    fn stem_cache_entry_categories() {
        let mut waveforms = HashMap::new();
        waveforms.insert(
            VoiceCategory::Drums,
            StemWaveform::empty(VoiceCategory::Drums),
        );
        waveforms.insert(
            VoiceCategory::Bass,
            StemWaveform::empty(VoiceCategory::Bass),
        );
        let entry = StemCacheEntry::new("t1", waveforms);

        let categories = entry.categories();
        assert_eq!(categories.len(), 2);
    }

    #[test]
    fn stem_waveform_rms_at() {
        let samples: Vec<f32> = vec![0.5; 100];
        let waveform =
            StemWaveform::new(samples, Duration::from_secs(1), 44100, VoiceCategory::Drums);
        let rms = waveform.rms_at(0.5);
        assert!(rms >= 0.0);
    }
}
