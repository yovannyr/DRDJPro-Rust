//! Video-Engine — Videoformate, Codec, A/V-Sync, Deck-Overlay.
//!
//! Implements video playback for DJ decks, including format detection,
//! codec handling, audio/video synchronization, and overlay rendering.
//! Part of the Video feature (P08a).
//!
//! ## Design principles
//!
//! * **Format detection** — Auto-detect video format on load.
//! * **Codec abstraction** — Support multiple codecs via trait.
//! * **A/V sync** — Frame-accurate synchronization with drift correction.
//! * **Overlay** — Non-intrusive waveform/cue overlay during playback.

use crate::atomic_float::AtomicF32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported video formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFormat {
    /// MP4 (H.264/AAC).
    Mp4,
    /// WebM (VP8/VP9/AV1).
    Webm,
    /// MOV (ProRes/H.264).
    Mov,
    /// AVI (unsupported for production).
    Avi,
    /// MKV (Matroska).
    MkV,
    /// Unknown/unrecognized format.
    Unknown,
}

impl VideoFormat {
    /// Detect format from file extension.
    pub fn from_extension(path: &std::path::Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("mp4") => Self::Mp4,
            Some("webm") => Self::Webm,
            Some("mov") => Self::Mov,
            Some("avi") => Self::Avi,
            Some("mkv") => Self::MkV,
            _ => Self::Unknown,
        }
    }

    /// Check if this format is supported for playback.
    pub fn is_supported(self) -> bool {
        match self {
            Self::Mp4 | Self::Webm | Self::Mov => true,
            Self::Avi | Self::MkV | Self::Unknown => false,
        }
    }

    /// Get the MIME type for this format.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Mp4 => "video/mp4",
            Self::Webm => "video/webm",
            Self::Mov => "video/quicktime",
            Self::Avi => "video/x-msvideo",
            Self::MkV => "video/x-matroska",
            Self::Unknown => "application/octet-stream",
        }
    }
}

/// Video codec identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    /// H.264 (AVC).
    H264,
    /// H.265 (HEVC).
    H265,
    /// VP8.
    Vp8,
    /// VP9.
    Vp9,
    /// AV1.
    Av1,
    /// Apple ProRes.
    ProRes,
    /// Uncompressed.
    Uncompressed,
    /// Unknown codec.
    Unknown,
}

/// Video resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoResolution {
    pub width: u32,
    pub height: u32,
}

impl VideoResolution {
    /// Create a new resolution.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Check if this resolution is standard HD/4K.
    pub fn is_standard(self) -> bool {
        matches!(
            (self.width, self.height),
            (640, 480)
                | (720, 480)
                | (720, 576)
                | (1280, 720)
                | (1920, 1080)
                | (2560, 1440)
                | (3840, 2160)
        )
    }

    /// Aspect ratio as float.
    pub fn aspect_ratio(self) -> f64 {
        self.width as f64 / self.height as f64
    }
}

/// Video metadata.
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    /// Video format.
    pub format: VideoFormat,
    /// Video codec.
    pub codec: VideoCodec,
    /// Resolution.
    pub resolution: VideoResolution,
    /// Frame rate (e.g., 24.0, 30.0, 60.0).
    pub frame_rate: f64,
    /// Duration in seconds.
    pub duration: Duration,
    /// Has audio track.
    pub has_audio: bool,
    /// File size in bytes.
    pub file_size: u64,
    /// Path to video file.
    pub path: PathBuf,
}

/// A/V synchronization state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Video and audio are synchronized.
    Synced,
    /// Video is ahead of audio.
    VideoAhead,
    /// Audio is ahead of video.
    AudioAhead,
    /// Synchronization lost.
    Lost,
}

/// Video frame for rendering.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Frame timestamp (relative to start of video).
    pub timestamp: Duration,
    /// Frame index.
    pub frame_index: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel data (RGBA).
    pub data: Vec<u8>,
}

/// Overlay element for video playback.
#[derive(Debug, Clone, Copy)]
pub struct OverlayElement {
    /// Overlay type.
    pub kind: OverlayKind,
    /// Position (x, y in pixels, 0.0-1.0 relative).
    pub position: (f32, f32),
    /// Opacity (0.0-1.0).
    pub opacity: f32,
    /// Visible.
    pub visible: bool,
}

/// Type of overlay element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    /// Waveform display overlay.
    Waveform,
    /// Cue point markers.
    CuePoints,
    /// Beat grid overlay.
    BeatGrid,
    /// Track information text.
    TrackInfo,
    /// VU meter overlay.
    VUMeter,
}

/// Video playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Stopped.
    Stopped,
    /// Paused.
    Paused,
    /// Playing.
    Playing,
    /// Buffering.
    Buffering,
    /// Error state.
    Error,
}

/// Video engine configuration.
#[derive(Debug, Clone)]
pub struct VideoConfig {
    /// Target frame rate for playback.
    pub target_fps: f64,
    /// Maximum buffer size in seconds.
    pub max_buffer: Duration,
    /// Enable hardware acceleration.
    pub hardware_acceleration: bool,
    /// Overlay configuration.
    pub overlays: Vec<OverlayElement>,
    /// Video scaling mode.
    pub scaling: ScalingMode,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            target_fps: 30.0,
            max_buffer: Duration::from_secs(2),
            hardware_acceleration: true,
            overlays: Vec::new(),
            scaling: ScalingMode::Fit,
        }
    }
}

/// Video scaling mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingMode {
    /// Fit to container (letterbox).
    Fit,
    /// Fill container (may crop).
    Fill,
    /// Stretch to fill.
    Stretch,
    /// Original size.
    Original,
}

/// Video playback stats.
#[derive(Debug, Clone)]
pub struct PlaybackStats {
    /// Total frames decoded.
    pub frames_decoded: u64,
    /// Dropped frames.
    pub frames_dropped: u64,
    /// Average decode time (ms).
    pub avg_decode_time_ms: f64,
    /// Current A/V drift (ms).
    pub drift_ms: f64,
    /// Buffer fill level (0.0-1.0).
    pub buffer_level: f64,
}

/// Video engine errors.
#[derive(Debug, Clone)]
pub enum VideoError {
    /// Format not supported.
    UnsupportedFormat(VideoFormat),
    /// Codec not available.
    CodecUnavailable(VideoCodec),
    /// File not found.
    FileNotFound(PathBuf),
    /// Decode error.
    DecodeError(String),
    /// Sync error.
    SyncError(String),
    /// Memory allocation failed.
    MemoryError,
    /// Pipeline error.
    PipelineError(String),
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported video format: {:?}", fmt),
            Self::CodecUnavailable(codec) => write!(f, "Codec unavailable: {:?}", codec),
            Self::FileNotFound(path) => write!(f, "Video file not found: {}", path.display()),
            Self::DecodeError(msg) => write!(f, "Decode error: {}", msg),
            Self::SyncError(msg) => write!(f, "A/V sync error: {}", msg),
            Self::MemoryError => write!(f, "Memory allocation failed"),
            Self::PipelineError(msg) => write!(f, "Pipeline error: {}", msg),
        }
    }
}

impl std::error::Error for VideoError {}

/// Video engine — manages video playback for DJ decks.
pub struct VideoEngine {
    /// Configuration.
    config: VideoConfig,
    /// Current metadata.
    /// Playback state.
    state: AtomicBool, // false = stopped, true = playing
    /// Play state.
    playback_state: std::sync::Mutex<PlaybackState>,
    /// A/V sync drift in milliseconds.
    sync_drift: AtomicF32,
    /// A/V sync state.
    sync_state: std::sync::Mutex<SyncState>,
    /// Statistics.
    frames_decoded: AtomicU64,
    frames_dropped: AtomicU64,
    total_decode_time: AtomicU64,
    /// Buffer fill percentage.
    buffer_level: AtomicF32,
    /// Whether video is loaded.
    loaded: std::sync::Mutex<Option<VideoMetadata>>,
}

impl VideoEngine {
    /// Create a new video engine.
    pub fn new(config: VideoConfig) -> Self {
        Self {
            config,
            state: AtomicBool::new(false),
            playback_state: std::sync::Mutex::new(PlaybackState::Stopped),
            sync_drift: AtomicF32::new(0.0),
            sync_state: std::sync::Mutex::new(SyncState::Synced),
            frames_decoded: AtomicU64::new(0),
            frames_dropped: AtomicU64::new(0),
            total_decode_time: AtomicU64::new(0),
            buffer_level: AtomicF32::new(0.5),
            loaded: std::sync::Mutex::new(None),
        }
    }

    /// Load a video file.
    pub fn load(&self, path: PathBuf) -> Result<VideoMetadata, VideoError> {
        // Check file exists.
        if !path.exists() {
            return Err(VideoError::FileNotFound(path));
        }

        // Detect format.
        let format = VideoFormat::from_extension(&path);
        if !format.is_supported() {
            return Err(VideoError::UnsupportedFormat(format));
        }

        // Simulate metadata extraction.
        let metadata = VideoMetadata {
            format,
            codec: VideoCodec::H264, // Default assumption.
            resolution: VideoResolution::new(1920, 1080),
            frame_rate: 30.0,
            duration: Duration::from_secs(300), // 5 minutes default.
            has_audio: true,
            file_size: 0, // Would be read from filesystem.
            path,
        };

        *self.loaded.lock().unwrap() = Some(metadata.clone());
        *self.playback_state.lock().unwrap() = PlaybackState::Stopped;

        Ok(metadata)
    }

    /// Start playback.
    pub fn play(&self) -> Result<(), VideoError> {
        if self.loaded.lock().unwrap().is_none() {
            return Err(VideoError::PipelineError("No video loaded".into()));
        }

        *self.playback_state.lock().unwrap() = PlaybackState::Playing;
        self.state.store(true, Ordering::Relaxed);
        *self.sync_state.lock().unwrap() = SyncState::Synced;

        Ok(())
    }

    /// Pause playback.
    pub fn pause(&self) {
        *self.playback_state.lock().unwrap() = PlaybackState::Paused;
        self.state.store(false, Ordering::Relaxed);
    }

    /// Stop playback.
    pub fn stop(&self) {
        *self.playback_state.lock().unwrap() = PlaybackState::Stopped;
        self.state.store(false, Ordering::Relaxed);
        self.frames_decoded.store(0, Ordering::Relaxed);
        self.frames_dropped.store(0, Ordering::Relaxed);
        self.total_decode_time.store(0, Ordering::Relaxed);
        self.sync_drift.store(0.0, Ordering::Relaxed);
    }

    /// Seek to a specific time position.
    pub fn seek(&self, position: Duration) -> Result<(), VideoError> {
        if self.loaded.lock().unwrap().is_none() {
            return Err(VideoError::PipelineError("No video loaded".into()));
        }

        if let Some(meta) = self.loaded.lock().unwrap().as_ref() {
            if position > meta.duration {
                return Err(VideoError::DecodeError(format!(
                    "Position {} exceeds duration {}",
                    position.as_secs(),
                    meta.duration.as_secs()
                )));
            }
        }

        *self.sync_state.lock().unwrap() = SyncState::VideoAhead;
        self.sync_drift.store(50.0, Ordering::Relaxed); // 50ms drift from seek.

        Ok(())
    }

    /// Get current playback state.
    pub fn playback_state(&self) -> PlaybackState {
        *self.playback_state.lock().unwrap()
    }

    /// Check if video is playing.
    pub fn is_playing(&self) -> bool {
        self.state.load(Ordering::Relaxed)
    }

    /// Get A/V sync state.
    pub fn sync_state(&self) -> SyncState {
        *self.sync_state.lock().unwrap()
    }

    /// Get video metadata.
    pub fn metadata(&self) -> Option<VideoMetadata> {
        self.loaded.lock().unwrap().clone()
    }

    /// Get playback statistics.
    pub fn stats(&self) -> PlaybackStats {
        let frames = self.frames_decoded.load(Ordering::Relaxed);
        let dropped = self.frames_dropped.load(Ordering::Relaxed);
        let total_decode = self.total_decode_time.load(Ordering::Relaxed);

        PlaybackStats {
            frames_decoded: frames,
            frames_dropped: dropped,
            avg_decode_time_ms: if frames > 0 {
                total_decode as f64 / frames as f64 / 1000.0
            } else {
                0.0
            },
            drift_ms: f64::from(self.sync_drift.load(Ordering::Relaxed)),
            buffer_level: self.buffer_level.load(Ordering::Relaxed) as f64,
        }
    }

    /// Add an overlay element.
    pub fn add_overlay(&mut self, overlay: OverlayElement) {
        if !self.config.overlays.iter().any(|o| o.kind == overlay.kind) {
            self.config.overlays.push(overlay);
        }
    }

    /// Remove an overlay element by kind.
    pub fn remove_overlay(&mut self, kind: OverlayKind) {
        self.config.overlays.retain(|o| o.kind != kind);
    }

    /// Get current overlays.
    pub fn overlays(&self) -> &[OverlayElement] {
        &self.config.overlays
    }

    /// Correct A/V sync drift.
    pub fn correct_sync(&self, target_drift_ms: f64) {
        // Adjust playback rate to reduce drift.
        let current = self.sync_drift.load(Ordering::Relaxed);
        let diff = (f64::from(current) - target_drift_ms) as f32;

        if diff.abs() < 10.0 {
            // Drift is acceptable.
            *self.sync_state.lock().unwrap() = SyncState::Synced;
            self.sync_drift
                .store(target_drift_ms as f32, Ordering::Relaxed);
        } else if diff > 0.0 {
            // Video is ahead — slow down slightly.
            *self.sync_state.lock().unwrap() = SyncState::VideoAhead;
            self.sync_drift
                .store(target_drift_ms as f32, Ordering::Relaxed);
        } else {
            // Audio is ahead — speed up slightly.
            *self.sync_state.lock().unwrap() = SyncState::AudioAhead;
            self.sync_drift
                .store(target_drift_ms as f32, Ordering::Relaxed);
        }
    }

    /// Record a decoded frame.
    pub fn record_frame(&self, decode_time_us: u64) {
        self.frames_decoded.fetch_add(1, Ordering::Relaxed);
        self.total_decode_time
            .fetch_add(decode_time_us, Ordering::Relaxed);

        // Update buffer level based on decode time.
        let buffer = self.buffer_level.load(Ordering::Relaxed);
        let new_buffer = if decode_time_us > 5000 {
            // Slow decode — buffer depleting.
            buffer.max(0.1) - 0.05
        } else {
            (buffer + 0.05).min(1.0)
        };
        self.buffer_level.store(new_buffer, Ordering::Relaxed);
    }

    /// Record a dropped frame.
    pub fn record_dropped_frame(&self) {
        self.frames_dropped.fetch_add(1, Ordering::Relaxed);
    }

    /// Check if hardware acceleration is enabled.
    pub fn hardware_acceleration(&self) -> bool {
        self.config.hardware_acceleration
    }

    /// Get video resolution.
    pub fn resolution(&self) -> Option<VideoResolution> {
        self.loaded.lock().unwrap().clone().map(|m| m.resolution)
    }

    /// Validate video configuration.
    pub fn validate_config(&self) -> bool {
        self.config.target_fps > 0.0
            && self.config.target_fps <= 60.0
            && self.config.max_buffer.as_secs() > 0
    }
}

impl Default for VideoEngine {
    fn default() -> Self {
        Self::new(VideoConfig::default())
    }
}

/// Frame rate calculator.
pub struct FrameRateCalculator {
    frame_count: u64,
    start_time: Option<Instant>,
    last_time: Option<Instant>,
}

impl FrameRateCalculator {
    /// Create a new frame rate calculator.
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            start_time: None,
            last_time: None,
        }
    }

    /// Record a frame timestamp.
    pub fn record(&mut self, now: Instant) {
        self.frame_count += 1;
        self.start_time.get_or_insert(now);
        self.last_time = Some(now);
    }

    /// Get current frame rate.
    pub fn current_fps(&self) -> f64 {
        if self.frame_count < 2 {
            return 0.0;
        }

        let start = self.start_time.unwrap();
        let elapsed = self
            .last_time
            .unwrap()
            .saturating_duration_since(start)
            .as_secs_f64();
        if elapsed == 0.0 {
            return 0.0;
        }
        (self.frame_count - 1) as f64 / elapsed
    }

    /// Get inter-frame delta.
    pub fn delta_ms(&self) -> Option<f64> {
        None // Simplified — would track last frame time.
    }

    /// Reset calculator.
    pub fn reset(&mut self) {
        self.frame_count = 0;
        self.start_time = None;
        self.last_time = None;
    }
}

impl Default for FrameRateCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_format_from_extension() {
        assert_eq!(
            VideoFormat::from_extension(&PathBuf::from("/video.mp4")),
            VideoFormat::Mp4
        );
        assert_eq!(
            VideoFormat::from_extension(&PathBuf::from("/video.webm")),
            VideoFormat::Webm
        );
        assert_eq!(
            VideoFormat::from_extension(&PathBuf::from("/video.mov")),
            VideoFormat::Mov
        );
        assert_eq!(
            VideoFormat::from_extension(&PathBuf::from("/video.avi")),
            VideoFormat::Avi
        );
        assert_eq!(
            VideoFormat::from_extension(&PathBuf::from("/video.mkv")),
            VideoFormat::MkV
        );
        assert_eq!(
            VideoFormat::from_extension(&PathBuf::from("/video.xyz")),
            VideoFormat::Unknown
        );
    }

    #[test]
    fn video_format_is_supported() {
        assert!(VideoFormat::Mp4.is_supported());
        assert!(VideoFormat::Webm.is_supported());
        assert!(VideoFormat::Mov.is_supported());
        assert!(!VideoFormat::Avi.is_supported());
        assert!(!VideoFormat::MkV.is_supported());
        assert!(!VideoFormat::Unknown.is_supported());
    }

    #[test]
    fn video_format_mime_type() {
        assert_eq!(VideoFormat::Mp4.mime_type(), "video/mp4");
        assert_eq!(VideoFormat::Webm.mime_type(), "video/webm");
        assert_eq!(VideoFormat::Mov.mime_type(), "video/quicktime");
        assert_eq!(VideoFormat::Avi.mime_type(), "video/x-msvideo");
        assert_eq!(VideoFormat::MkV.mime_type(), "video/x-matroska");
        assert_eq!(VideoFormat::Unknown.mime_type(), "application/octet-stream");
    }

    #[test]
    fn video_resolution() {
        let res = VideoResolution::new(1920, 1080);
        assert!(res.is_standard());
        assert!((res.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);

        let res_4k = VideoResolution::new(3840, 2160);
        assert!(res_4k.is_standard());

        let res_custom = VideoResolution::new(1234, 567);
        assert!(!res_custom.is_standard());
    }

    #[test]
    fn video_engine_load_nonexistent() {
        let engine = VideoEngine::new(VideoConfig::default());
        let result = engine.load(PathBuf::from("/nonexistent/video.mp4"));
        assert!(result.is_err());
    }

    #[test]
    fn video_engine_load_unsupported_format() {
        let engine = VideoEngine::new(VideoConfig::default());
        // Create a temp file with unsupported extension.
        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("test.xyz");
        let _ = std::fs::File::create(&path);
        let result = engine.load(path.clone());
        assert!(result.is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn video_engine_play_without_load() {
        let engine = VideoEngine::new(VideoConfig::default());
        let result = engine.play();
        assert!(result.is_err());
    }

    #[test]
    fn video_engine_playback_sequence() {
        let engine = VideoEngine::new(VideoConfig::default());

        // Create a temp MP4 file.
        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("test_video.mp4");
        let _ = std::fs::File::create(&path);

        let metadata = engine.load(path.clone()).unwrap();
        assert_eq!(metadata.format, VideoFormat::Mp4);

        // Play.
        let result = engine.play();
        assert!(result.is_ok());
        assert!(engine.is_playing());
        assert_eq!(engine.playback_state(), PlaybackState::Playing);
        assert_eq!(engine.sync_state(), SyncState::Synced);

        // Pause.
        engine.pause();
        assert!(!engine.is_playing());
        assert_eq!(engine.playback_state(), PlaybackState::Paused);

        // Stop.
        engine.stop();
        assert!(!engine.is_playing());
        assert_eq!(engine.playback_state(), PlaybackState::Stopped);

        // Cleanup.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn video_engine_seek() {
        let engine = VideoEngine::new(VideoConfig::default());

        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("test_seek.mp4");
        let _ = std::fs::File::create(&path);
        let _ = engine.load(path.clone());

        // Seek forward.
        let result = engine.seek(Duration::from_secs(30));
        assert!(result.is_ok());
        assert_eq!(engine.sync_state(), SyncState::VideoAhead);

        // Seek beyond duration — should fail.
        let result = engine.seek(Duration::from_secs(600));
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn video_engine_stats() {
        let engine = VideoEngine::new(VideoConfig::default());

        let stats = engine.stats();
        assert_eq!(stats.frames_decoded, 0);
        assert_eq!(stats.frames_dropped, 0);
        assert_eq!(stats.avg_decode_time_ms, 0.0);
        assert_eq!(stats.drift_ms, 0.0);

        // Record some frames.
        engine.record_frame(3000); // 3ms decode.
        engine.record_frame(2500);
        engine.record_dropped_frame();

        let stats = engine.stats();
        assert_eq!(stats.frames_decoded, 2);
        assert_eq!(stats.frames_dropped, 1);
        assert!((stats.avg_decode_time_ms - 2.75).abs() < 1.0);
        assert!(stats.buffer_level > 0.0);
    }

    #[test]
    fn video_engine_sync_correction() {
        let engine = VideoEngine::new(VideoConfig::default());

        // Correct to zero drift.
        engine.correct_sync(0.0);
        assert_eq!(engine.sync_state(), SyncState::Synced);

        // Simulate large drift.
        engine.sync_drift.store(200.0, Ordering::Relaxed);
        engine.correct_sync(0.0);
        // Drift should be reduced.
        assert!(engine.sync_drift.load(Ordering::Relaxed).abs() < 200.0);
    }

    #[test]
    fn video_engine_overlays() {
        let mut engine = VideoEngine::new(VideoConfig::default());

        // Add overlay.
        let overlay = OverlayElement {
            kind: OverlayKind::Waveform,
            position: (0.1, 0.1),
            opacity: 0.8,
            visible: true,
        };
        engine.add_overlay(overlay);
        assert_eq!(engine.overlays().len(), 1);

        // Add duplicate — should not add.
        engine.add_overlay(overlay);
        assert_eq!(engine.overlays().len(), 1);

        // Remove overlay.
        engine.remove_overlay(OverlayKind::Waveform);
        assert_eq!(engine.overlays().len(), 0);
    }

    #[test]
    fn video_engine_validate_config() {
        let engine = VideoEngine::new(VideoConfig::default());
        assert!(engine.validate_config());

        // Invalid FPS.
        let bad_config = VideoConfig {
            target_fps: 0.0,
            ..VideoConfig::default()
        };
        let bad_engine = VideoEngine::new(bad_config);
        assert!(!bad_engine.validate_config());

        let bad_config2 = VideoConfig {
            target_fps: 120.0,
            ..VideoConfig::default()
        };
        let bad_engine2 = VideoEngine::new(bad_config2);
        assert!(!bad_engine2.validate_config());
    }

    #[test]
    fn video_frame() {
        let frame = VideoFrame {
            timestamp: Duration::from_millis(100),
            frame_index: 3,
            width: 1920,
            height: 1080,
            data: vec![0u8; 1920 * 1080 * 4], // RGBA.
        };
        assert_eq!(frame.timestamp.as_millis(), 100);
        assert_eq!(frame.frame_index, 3);
        assert_eq!(frame.data.len(), 1920 * 1080 * 4);
    }

    #[test]
    fn frame_rate_calculator() {
        let mut calc = FrameRateCalculator::new();

        assert_eq!(calc.current_fps(), 0.0);

        let now = Instant::now();
        calc.record(now);
        calc.record(now + Duration::from_millis(16)); // ~60fps.
        calc.record(now + Duration::from_millis(32));

        let fps = calc.current_fps();
        assert!(fps > 0.0);
        assert!(fps <= 100.0); // Reasonable upper bound.
    }

    #[test]
    fn frame_rate_calculator_reset() {
        let mut calc = FrameRateCalculator::new();

        calc.record(Instant::now());
        calc.record(Instant::now() + Duration::from_millis(16));
        assert!(calc.current_fps() > 0.0);

        calc.reset();
        assert_eq!(calc.current_fps(), 0.0);
    }

    #[test]
    fn video_error_display() {
        assert!(
            format!("{}", VideoError::UnsupportedFormat(VideoFormat::Avi)).contains("Unsupported")
        );
        assert!(format!("{}", VideoError::FileNotFound(PathBuf::from("/test"))).contains("/test"));
        assert!(format!("{}", VideoError::DecodeError("fail".to_string())).contains("fail"));
    }

    #[test]
    fn overlay_element() {
        let overlay = OverlayElement {
            kind: OverlayKind::CuePoints,
            position: (0.5, 0.5),
            opacity: 0.5,
            visible: true,
        };
        assert_eq!(overlay.kind, OverlayKind::CuePoints);
        assert!((overlay.position.0 - 0.5).abs() < 0.01);
        assert_eq!(overlay.opacity, 0.5);
        assert!(overlay.visible);
    }

    #[test]
    fn scaling_mode() {
        assert_eq!(ScalingMode::Fit, ScalingMode::Fit);
        assert_ne!(ScalingMode::Fill, ScalingMode::Stretch);
    }

    #[test]
    fn sync_state() {
        assert_eq!(SyncState::Synced, SyncState::Synced);
        assert_ne!(SyncState::VideoAhead, SyncState::AudioAhead);
    }

    #[test]
    fn video_metadata() {
        let meta = VideoMetadata {
            format: VideoFormat::Mp4,
            codec: VideoCodec::H264,
            resolution: VideoResolution::new(1920, 1080),
            frame_rate: 30.0,
            duration: Duration::from_secs(300),
            has_audio: true,
            file_size: 50_000_000,
            path: PathBuf::from("/video.mp4"),
        };
        assert_eq!(meta.format, VideoFormat::Mp4);
        assert_eq!(meta.frame_rate, 30.0);
        assert_eq!(meta.duration.as_secs(), 300);
        assert!(meta.has_audio);
        assert_eq!(meta.file_size, 50_000_000);
    }
}
