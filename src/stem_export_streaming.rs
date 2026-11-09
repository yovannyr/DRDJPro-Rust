//! Stem export and streaming extension for
//! live stem mixing and pre-export stem configuration.
//! Extends P12a (stem separation, cache, voices).
//!
//! ## Design principles
//!
//! * **Export-ready** — Stems can be exported as multi-channel WAV.
//! * **Live streaming** — Stems stream over UDP for multi-deck sync.
//! * **Snapshots** — User stem configurations are saved/restorable.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::stems::{Stem, StemError, StemSeparation, VoiceCategory};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Audio export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// WAV (PCM).
    WavPcm16,
    /// WAV (float32).
    WavFloat32,
    /// FLAC (lossless).
    Flac,
    /// MP3 (stereo mix only).
    MP3,
}

/// Export configuration.
#[derive(Debug, Clone)]
pub struct StemExportConfig {
    /// Format.
    pub format: ExportFormat,
    /// Target sample rate (44100, 48000, 96000).
    pub sample_rate: u32,
    /// Bit depth for WAV.
    pub bit_depth: u8,
    /// Include all stems or mix.
    pub output_mode: OutputMode,
    /// Destination file path prefix.
    pub destination: String,
}

impl Default for StemExportConfig {
    fn default() -> Self {
        Self {
            format: ExportFormat::WavPcm16,
            sample_rate: 44100,
            bit_depth: 16,
            output_mode: OutputMode::SeparateFiles,
            destination: "./exports".to_string(),
        }
    }
}

/// Output mode for stem export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Each stem as a separate file.
    SeparateFiles,
    /// All stems in one multi-channel file.
    SingleMultichannel,
    /// Stereo mix of selected stems.
    StereoMix,
}

/// Export result.
#[derive(Debug, Clone)]
pub struct StemExportResult {
    /// Number of files written.
    pub files_written: usize,
    /// Total size in bytes.
    pub total_size_bytes: u64,
    /// Duration of export (ms).
    pub duration_ms: u64,
    /// Format used.
    pub format: ExportFormat,
    /// Any errors.
    pub errors: Vec<String>,
}

/// Stem bundle for export or streaming.
#[derive(Debug, Clone)]
pub struct StemBundle {
    /// Source track name.
    pub source_name: String,
    /// Separated stems.
    pub stems: Vec<Stem>,
    /// Stem gains at bundling time.
    pub stem_gains: HashMap<VoiceCategory, f32>,
    /// Stem mute states at bundling time.
    pub stem_muted: HashMap<VoiceCategory, bool>,
}

impl StemBundle {
    /// Create from a separation result with current gains.
    pub fn from_separation(sep: &StemSeparation, source_name: &str) -> Self {
        let mut stem_gains = HashMap::new();
        let mut stem_muted = HashMap::new();

        for stem in &sep.stems {
            stem_gains.insert(stem.category, stem.gain);
            stem_muted.insert(stem.category, stem.muted);
        }

        Self {
            source_name: source_name.to_string(),
            stems: sep.stems.clone(),
            stem_gains,
            stem_muted,
        }
    }

    /// Apply gain overrides to stems.
    pub fn apply_gains(&mut self) {
        for stem in &mut self.stems {
            if let Some(&gain) = self.stem_gains.get(&stem.category) {
                stem.gain = gain;
            }
        }
    }

    /// Apply mute states to stems.
    pub fn apply_mutes(&mut self) {
        for stem in &mut self.stems {
            if let Some(&muted) = self.stem_muted.get(&stem.category) {
                stem.muted = muted;
            }
        }
    }

    /// Get stems that are not muted.
    pub fn active_stems(&self) -> Vec<&Stem> {
        self.stems
            .iter()
            .filter(|s| !self.stem_muted.get(&s.category).copied().unwrap_or(false))
            .collect()
    }

    /// Get a stereo mix of all active stems at their gains.
    pub fn stereo_mix(&self) -> Result<(Vec<f32>, Vec<f32>), StemError> {
        let active = self.active_stems();
        if active.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }

        let total_samples = active[0].samples.len();
        let mut left = vec![0.0_f32; total_samples / 2];
        let mut right = vec![0.0_f32; total_samples / 2];

        for stem in &active {
            if stem.samples.len() != total_samples {
                continue;
            }
            let gain = stem.gain;
            for i in (0..total_samples).step_by(2) {
                if i + 1 < stem.samples.len() {
                    left[i / 2] += stem.samples[i] * gain;
                    right[i / 2] += stem.samples[i + 1] * gain;
                }
            }
        }

        // Soft-clip to prevent overflow.
        let max_val = left
            .iter()
            .chain(right.iter())
            .map(|value| value.abs())
            .max_by(f32::total_cmp);
        if let Some(max) = max_val {
            if max > 1.0 {
                for v in left.iter_mut() {
                    *v /= max;
                }
                for v in right.iter_mut() {
                    *v /= max;
                }
            }
        }

        Ok((left, right))
    }
}

/// Stem export manager.
pub struct StemExportManager {
    config: StemExportConfig,
    export_count: AtomicU64,
    total_bytes_exported: AtomicU64,
}

impl StemExportManager {
    pub fn new(config: StemExportConfig) -> Self {
        Self {
            config,
            export_count: AtomicU64::new(0),
            total_bytes_exported: AtomicU64::new(0),
        }
    }

    /// Update export configuration.
    pub fn update_config(&mut self, config: StemExportConfig) {
        self.config = config;
    }

    /// Get current config.
    pub fn config(&self) -> &StemExportConfig {
        &self.config
    }

    /// Export stems to files (simulation).
    pub fn export_stems(&self, bundle: &StemBundle) -> StemExportResult {
        let start = std::time::Instant::now();

        let mut files_written = 0usize;
        let mut total_size = 0u64;
        let errors = Vec::new();

        match self.config.output_mode {
            OutputMode::SeparateFiles => {
                for stem in &bundle.stems {
                    if stem.muted {
                        continue;
                    }
                    let size = self.estimate_file_size(stem);
                    total_size += size;
                    files_written += 1;
                }
            }
            OutputMode::SingleMultichannel => {
                total_size = bundle
                    .stems
                    .iter()
                    .map(|s| self.estimate_file_size(s))
                    .sum();
                files_written = 1;
            }
            OutputMode::StereoMix => {
                total_size = self.estimate_stereo_mix_size(bundle);
                files_written = 1;
            }
        }

        let duration = start.elapsed().as_millis() as u64;
        self.export_count
            .fetch_add(files_written as u64, Ordering::Relaxed);
        self.total_bytes_exported
            .fetch_add(total_size, Ordering::Relaxed);

        StemExportResult {
            files_written,
            total_size_bytes: total_size,
            duration_ms: duration,
            format: self.config.format,
            errors,
        }
    }

    /// Estimate file size for a stem.
    fn estimate_file_size(&self, stem: &Stem) -> u64 {
        let bytes_per_sample = match self.config.format {
            ExportFormat::WavPcm16 => 2,
            ExportFormat::WavFloat32 => 4,
            ExportFormat::Flac => 2, // approx.
            ExportFormat::MP3 => 4,  // approx.
        };
        // Stereo = 2 channels.
        let samples = stem.samples.len() / stem.channels as usize;
        (samples as u64 * bytes_per_sample as u64 * 2) + 44 // WAV header.
    }

    /// Estimate stereo mix file size.
    fn estimate_stereo_mix_size(&self, bundle: &StemBundle) -> u64 {
        if let Some(stem) = bundle.stems.first() {
            self.estimate_file_size(stem)
        } else {
            0
        }
    }

    /// Get export statistics.
    pub fn statistics(&self) -> (u64, u64) {
        (
            self.export_count.load(Ordering::Relaxed),
            self.total_bytes_exported.load(Ordering::Relaxed),
        )
    }
}

/// Stem streaming session for live multi-deck sync.
#[derive(Debug)]
pub struct StemStreamingSession {
    /// Session ID.
    pub session_id: u64,
    /// Remote destination.
    pub destination: SocketAddr,
    /// Whether streaming is active.
    pub active: bool,
    /// Packets sent.
    pub packets_sent: AtomicU64,
    /// Bytes sent.
    pub bytes_sent: AtomicU64,
    /// Source stems (for encoding).
    pub source_stems: Vec<VoiceCategory>,
}

impl StemStreamingSession {
    /// Create a new streaming session.
    pub fn new(destination: SocketAddr, stems: Vec<VoiceCategory>) -> Self {
        Self {
            session_id: 0, // assigned on start.
            destination,
            active: false,
            packets_sent: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            source_stems: stems,
        }
    }

    /// Start the session.
    pub fn start(&mut self, session_id: u64) {
        self.session_id = session_id;
        self.active = true;
    }

    /// Stop the session.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Record a packet send.
    pub fn record_packet(&self, size_bytes: u64) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(size_bytes, Ordering::Relaxed);
    }

    /// Get session statistics.
    pub fn statistics(&self) -> (u64, u64, bool) {
        (
            self.packets_sent.load(Ordering::Relaxed),
            self.bytes_sent.load(Ordering::Relaxed),
            self.active,
        )
    }
}

/// Stem streaming manager.
pub struct StemStreamingManager {
    sessions: std::sync::Mutex<HashMap<u64, StemStreamingSession>>,
    next_session_id: AtomicU64,
}

impl StemStreamingManager {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Mutex::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
        }
    }

    /// Start a new streaming session.
    pub fn start_session(&self, destination: SocketAddr, stems: Vec<VoiceCategory>) -> u64 {
        let id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
        let mut session = StemStreamingSession::new(destination, stems);
        session.start(id);
        self.sessions.lock().unwrap().insert(id, session);
        id
    }

    /// Stop a streaming session.
    pub fn stop_session(&self, session_id: u64) {
        if let Some(mut sess) = self.sessions.lock().unwrap().remove(&session_id) {
            sess.stop();
        }
    }

    /// Get session stats.
    pub fn session_stats(&self, session_id: u64) -> Option<(u64, u64, bool)> {
        self.sessions
            .lock()
            .unwrap()
            .get(&session_id)
            .map(|s| s.statistics())
    }

    /// Get active session count.
    pub fn active_count(&self) -> usize {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.active)
            .count()
    }
}

/// Stem mix snapshot — user stem configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StemMixSnapshot {
    /// Snapshot name.
    pub name: String,
    /// Per-stem gains.
    pub gains: HashMap<VoiceCategory, f32>,
    /// Per-stem mute states.
    pub mutes: HashMap<VoiceCategory, bool>,
    /// Per-stem solo states.
    pub solos: HashMap<VoiceCategory, bool>,
    /// Source track name.
    pub source_track: String,
    /// Timestamp.
    pub timestamp: std::time::SystemTime,
}

impl StemMixSnapshot {
    /// Create a new snapshot from stems.
    pub fn from_stems(stems: &[Stem], source_track: &str) -> Self {
        let mut gains = HashMap::new();
        let mut mutes = HashMap::new();
        let mut solos = HashMap::new();

        for stem in stems {
            gains.insert(stem.category, stem.gain);
            mutes.insert(stem.category, stem.muted);
            solos.insert(stem.category, stem.solo);
        }

        Self {
            name: String::new(),
            gains,
            mutes,
            solos,
            source_track: source_track.to_string(),
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Apply snapshot to stems.
    pub fn apply_to_stems(&self, stems: &mut [Stem]) {
        for stem in stems {
            if let Some(&gain) = self.gains.get(&stem.category) {
                stem.gain = gain;
            }
            if let Some(&muted) = self.mutes.get(&stem.category) {
                stem.muted = muted;
            }
            if let Some(&solo) = self.solos.get(&stem.category) {
                stem.solo = solo;
            }
        }
    }
}

/// Stem mix template — reusable stem configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StemMixTemplate {
    /// Template name.
    pub name: String,
    /// Template description.
    pub description: String,
    /// Default gains per category.
    pub gains: HashMap<VoiceCategory, f32>,
    /// Default mutes.
    pub default_mutes: HashMap<VoiceCategory, bool>,
    /// Is a built-in template.
    pub is_builtin: bool,
}

impl StemMixTemplate {
    /// Create a new template.
    pub fn new(name: &str, gains: HashMap<VoiceCategory, f32>) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            gains,
            default_mutes: HashMap::new(),
            is_builtin: false,
        }
    }

    /// Build a stem mix from a template and source stems.
    pub fn build_mixer(&self, source_stems: &[Stem]) -> Vec<Stem> {
        source_stems
            .iter()
            .cloned()
            .map(|mut stem| {
                if let Some(&gain) = self.gains.get(&stem.category) {
                    stem.gain = gain;
                }
                if let Some(&muted) = self.default_mutes.get(&stem.category) {
                    stem.muted = muted;
                }
                stem
            })
            .collect()
    }
}

/// Stem manager with templates.
pub struct StemMixManager {
    snapshots: std::sync::Mutex<Vec<StemMixSnapshot>>,
    templates: std::sync::Mutex<Vec<StemMixTemplate>>,
}

impl StemMixManager {
    pub fn new() -> Self {
        Self {
            snapshots: std::sync::Mutex::new(Vec::new()),
            templates: std::sync::Mutex::new(Self::builtin_templates()),
        }
    }

    /// Save a snapshot.
    pub fn save_snapshot(&self, snapshot: StemMixSnapshot) -> Result<(), String> {
        if snapshot.name.is_empty() {
            return Err("Snapshot name cannot be empty".to_string());
        }
        self.snapshots.lock().unwrap().push(snapshot);
        Ok(())
    }

    /// Get all snapshots.
    pub fn snapshots(&self) -> Vec<StemMixSnapshot> {
        self.snapshots.lock().unwrap().clone()
    }

    /// Delete a snapshot by index.
    pub fn delete_snapshot(&self, index: usize) -> bool {
        self.snapshots.lock().unwrap().remove(index);
        true
    }

    /// Get all templates.
    pub fn templates(&self) -> Vec<StemMixTemplate> {
        self.templates.lock().unwrap().clone()
    }

    /// Apply a template to stems.
    pub fn apply_template(&self, template_name: &str, stems: &mut [Stem]) -> bool {
        if let Some(template) = self
            .templates
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.name == template_name)
        {
            for stem in stems {
                if let Some(&gain) = template.gains.get(&stem.category) {
                    stem.gain = gain;
                }
                if let Some(&muted) = template.default_mutes.get(&stem.category) {
                    stem.muted = muted;
                }
            }
            true
        } else {
            false
        }
    }

    /// Built-in templates.
    fn builtin_templates() -> Vec<StemMixTemplate> {
        let mut no_vocals = HashMap::new();
        no_vocals.insert(VoiceCategory::Vocals, 0.0);
        no_vocals.insert(VoiceCategory::Melody, 1.0);
        no_vocals.insert(VoiceCategory::Drums, 1.0);
        no_vocals.insert(VoiceCategory::Bass, 1.0);

        let mut karaoke = HashMap::new();
        karaoke.insert(VoiceCategory::Vocals, 0.0);
        karaoke.insert(VoiceCategory::Melody, 1.0);
        karaoke.insert(VoiceCategory::Drums, 1.0);
        karaoke.insert(VoiceCategory::Bass, 1.0);

        let mut instrumental = HashMap::new();
        instrumental.insert(VoiceCategory::Vocals, 0.0);
        instrumental.insert(VoiceCategory::Melody, 1.0);
        instrumental.insert(VoiceCategory::Drums, 1.0);
        instrumental.insert(VoiceCategory::Bass, 1.0);

        vec![
            StemMixTemplate {
                name: "No Vocals".to_string(),
                description: "Remove vocals for DJ mixing".to_string(),
                gains: no_vocals,
                default_mutes: HashMap::new(),
                is_builtin: true,
            },
            StemMixTemplate {
                name: "Karaoke".to_string(),
                description: "Full instrumental, no vocals".to_string(),
                gains: karaoke,
                default_mutes: HashMap::new(),
                is_builtin: true,
            },
            StemMixTemplate {
                name: "Drums + Bass".to_string(),
                description: "Only drums and bass".to_string(),
                gains: {
                    let mut g = HashMap::new();
                    g.insert(VoiceCategory::Drums, 1.0);
                    g.insert(VoiceCategory::Bass, 1.0);
                    g.insert(VoiceCategory::Melody, 0.0);
                    g.insert(VoiceCategory::Vocals, 0.0);
                    g
                },
                default_mutes: {
                    let mut m = HashMap::new();
                    m.insert(VoiceCategory::Melody, true);
                    m.insert(VoiceCategory::Vocals, true);
                    m
                },
                is_builtin: true,
            },
        ]
    }
}

impl Default for StemMixManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for StemStreamingSession {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id,
            destination: self.destination,
            active: self.active,
            packets_sent: AtomicU64::new(self.packets_sent.load(Ordering::Relaxed)),
            bytes_sent: AtomicU64::new(self.bytes_sent.load(Ordering::Relaxed)),
            source_stems: self.source_stems.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stem_bundle_from_separation() {
        let sep = StemSeparation {
            stems: vec![
                Stem::new(VoiceCategory::Drums, vec![0.1, 0.2, 0.3, 0.4], 44100),
                Stem::new(VoiceCategory::Vocals, vec![0.5, 0.6, 0.7, 0.8], 44100),
            ],
            source_path: "/test.wav".into(),
            method: crate::stems::SeparationMethod::BandSplit,
            quality: 1.0,
            processing_duration: std::time::Duration::ZERO,
            complete: true,
        };
        let bundle = StemBundle::from_separation(&sep, "test.wav");
        assert_eq!(bundle.stems.len(), 2);
        assert_eq!(bundle.stem_gains.len(), 2);
    }

    #[test]
    fn test_stem_bundle_apply_gains() {
        let sep = StemSeparation {
            stems: vec![Stem::new(VoiceCategory::Drums, vec![0.5], 44100)],
            source_path: "/test.wav".into(),
            method: crate::stems::SeparationMethod::BandSplit,
            quality: 1.0,
            processing_duration: std::time::Duration::ZERO,
            complete: true,
        };
        let mut bundle = StemBundle::from_separation(&sep, "test.wav");
        bundle.stem_gains.insert(VoiceCategory::Drums, 0.5);
        bundle.apply_gains();
        assert_eq!(bundle.stems[0].gain, 0.5);
    }

    #[test]
    fn test_stem_bundle_mutes() {
        let sep = StemSeparation {
            stems: vec![
                Stem::new(VoiceCategory::Drums, vec![0.5], 44100),
                Stem::new(VoiceCategory::Vocals, vec![0.5], 44100),
            ],
            source_path: "/test.wav".into(),
            method: crate::stems::SeparationMethod::BandSplit,
            quality: 1.0,
            processing_duration: std::time::Duration::ZERO,
            complete: true,
        };
        let mut bundle = StemBundle::from_separation(&sep, "test.wav");
        bundle.stem_muted.insert(VoiceCategory::Vocals, true);
        bundle.apply_mutes();

        let active = bundle.active_stems();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].category, VoiceCategory::Drums);
    }

    #[test]
    fn test_stereo_mix() {
        let sep = StemSeparation {
            stems: vec![Stem::new(
                VoiceCategory::Drums,
                vec![0.3, 0.4, 0.5, 0.6],
                44100,
            )],
            source_path: "/test.wav".into(),
            method: crate::stems::SeparationMethod::BandSplit,
            quality: 1.0,
            processing_duration: std::time::Duration::ZERO,
            complete: true,
        };
        let bundle = StemBundle::from_separation(&sep, "test.wav");
        let (left, right) = bundle.stereo_mix().unwrap();
        assert_eq!(left.len(), 2);
        assert_eq!(right.len(), 2);
    }

    #[test]
    fn test_stem_export_manager() {
        let config = StemExportConfig {
            output_mode: OutputMode::SeparateFiles,
            ..Default::default()
        };
        let mgr = StemExportManager::new(config);

        let sep = StemSeparation {
            stems: vec![Stem::new(VoiceCategory::Drums, vec![0.5; 88200], 44100)],
            source_path: "/test.wav".into(),
            method: crate::stems::SeparationMethod::BandSplit,
            quality: 1.0,
            processing_duration: std::time::Duration::ZERO,
            complete: true,
        };
        let bundle = StemBundle::from_separation(&sep, "test.wav");
        let result = mgr.export_stems(&bundle);
        assert!(result.files_written > 0);
    }

    #[test]
    fn test_stem_export_statistics() {
        let config = StemExportConfig::default();
        let mgr = StemExportManager::new(config);
        let (count, bytes) = mgr.statistics();
        assert_eq!(count, 0);
        assert_eq!(bytes, 0);
    }

    #[test]
    fn test_stem_streaming_session() {
        let addr: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let mut session = StemStreamingSession::new(addr, vec![VoiceCategory::Drums]);
        assert!(!session.active);
        session.start(1);
        assert!(session.active);
        assert_eq!(session.session_id, 1);

        session.record_packet(1024);
        let (pkts, bytes, active) = session.statistics();
        assert_eq!(pkts, 1);
        assert_eq!(bytes, 1024);
        assert!(active);

        session.stop();
        assert!(!session.active);
    }

    #[test]
    fn test_stem_streaming_manager() {
        let mgr = StemStreamingManager::new();
        let addr: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let id = mgr.start_session(addr, vec![VoiceCategory::Drums, VoiceCategory::Bass]);
        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.session_stats(id).is_some());

        mgr.stop_session(id);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_stem_mix_snapshot() {
        let mut stems = vec![Stem::new(VoiceCategory::Drums, vec![0.5], 44100)];
        stems[0].gain = 0.5;
        let snap = StemMixSnapshot::from_stems(&stems, "test.wav");
        assert_eq!(snap.source_track, "test.wav");

        // Modify and re-apply.
        let mut new_stems = vec![Stem::new(VoiceCategory::Drums, vec![0.8], 44100)];
        snap.apply_to_stems(&mut new_stems);
        assert_eq!(new_stems[0].gain, 0.5); // restored.
    }

    #[test]
    fn test_stem_mix_manager_snapshots() {
        let mgr = StemMixManager::new();
        let snap = StemMixSnapshot {
            name: "TestSnap".to_string(),
            gains: HashMap::new(),
            mutes: HashMap::new(),
            solos: HashMap::new(),
            source_track: "test".to_string(),
            timestamp: std::time::SystemTime::now(),
        };
        assert!(mgr.save_snapshot(snap).is_ok());
        assert_eq!(mgr.snapshots().len(), 1);
    }

    #[test]
    fn test_stem_mix_manager_snapshot_empty_name() {
        let mgr = StemMixManager::new();
        let snap = StemMixSnapshot {
            name: "".to_string(),
            gains: HashMap::new(),
            mutes: HashMap::new(),
            solos: HashMap::new(),
            source_track: "test".to_string(),
            timestamp: std::time::SystemTime::now(),
        };
        assert!(mgr.save_snapshot(snap).is_err());
    }

    #[test]
    fn test_stem_mix_templates() {
        let mgr = StemMixManager::new();
        let templates = mgr.templates();
        assert!(templates.len() >= 3);

        let names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"No Vocals"));
        assert!(names.contains(&"Karaoke"));
        assert!(names.contains(&"Drums + Bass"));
    }

    #[test]
    fn test_template_built_with_gains() {
        let mgr = StemMixManager::new();
        let mut stems = vec![
            Stem::new(VoiceCategory::Vocals, vec![0.8], 44100),
            Stem::new(VoiceCategory::Drums, vec![0.5], 44100),
        ];
        mgr.apply_template("No Vocals", &mut stems);
        assert_eq!(stems[0].gain, 0.0); // vocals muted
        assert_eq!(stems[1].gain, 1.0); // drums full
    }

    #[test]
    fn test_output_mode_variants() {
        assert_eq!(OutputMode::SeparateFiles, OutputMode::SeparateFiles);
        assert_ne!(OutputMode::SeparateFiles, OutputMode::SingleMultichannel);
    }

    #[test]
    fn test_export_format_variants() {
        assert_eq!(ExportFormat::WavPcm16, ExportFormat::WavPcm16);
        assert_ne!(ExportFormat::WavPcm16, ExportFormat::Flac);
    }

    #[test]
    fn test_stem_bundle_empty_mix() {
        let bundle = StemBundle {
            source_name: "empty".to_string(),
            stems: Vec::new(),
            stem_gains: HashMap::new(),
            stem_muted: HashMap::new(),
        };
        let (left, right) = bundle.stereo_mix().unwrap();
        assert!(left.is_empty());
        assert!(right.is_empty());
    }
}
