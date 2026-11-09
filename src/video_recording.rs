//! Video Recording Part 2 — Chroma-Key, Transitions, Layouts, 1080p, Audio Sync, Capture.
//!
//! Extends the video engine (P08a) with chroma-key (green screen),
//! pre-programmed transitions, easing functions, video layouts,
//! aspect ratio support, 1080p rendering, frame capture, and audio extraction.
//! Part of the Video Mixing feature (P08b).
//!
//! ## Design principles
//!
//! * **Robust** — Graceful error handling and recovery.
//! * **Efficient** — Minimal frame processing for real-time performance.
//! * **Configurable** — All parameters adjustable during runtime.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Chroma key color.
#[derive(Debug, Clone)]
pub struct ChromaKeyColor {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
}

impl ChromaKeyColor {
    /// Standard green screen color.
    pub fn green_screen() -> Self {
        Self { r: 0, g: 255, b: 0 }
    }

    /// Standard blue screen color.
    pub fn blue_screen() -> Self {
        Self { r: 0, g: 0, b: 255 }
    }

    /// Create from components.
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Check if a pixel matches this color (within tolerance).
    pub fn matches(&self, pixel_r: u8, pixel_g: u8, pixel_b: u8, tolerance: f32) -> bool {
        let tol = tolerance as i32;
        let diff_r = (pixel_r as i32 - self.r as i32).abs();
        let diff_g = (pixel_g as i32 - self.g as i32).abs();
        let diff_b = (pixel_b as i32 - self.b as i32).abs();
        diff_r <= tol && diff_g <= tol && diff_b <= tol
    }

    /// Get RGB as tuple.
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }
}

impl Default for ChromaKeyColor {
    fn default() -> Self {
        Self::green_screen()
    }
}

/// Chroma key settings.
pub struct ChromaKeySettings {
    /// Background color to remove.
    pub color: ChromaKeyColor,
    /// Color tolerance (0-100%).
    pub tolerance_percent: f32,
    /// Edge smoothing radius (0-10 pixels).
    pub edge_smoothing: f32,
    /// Spill suppression strength (0-100%).
    pub spill_suppression: f32,
    /// Is chroma key enabled.
    pub enabled: bool,
}

impl ChromaKeySettings {
    /// Create new chroma key settings.
    pub fn new() -> Self {
        Self {
            color: ChromaKeyColor::green_screen(),
            tolerance_percent: 50.0,
            edge_smoothing: 2.0,
            spill_suppression: 30.0,
            enabled: false,
        }
    }

    /// Set tolerance.
    pub fn set_tolerance(&mut self, percent: f32) {
        self.tolerance_percent = percent.clamp(0.0, 100.0);
    }

    /// Set edge smoothing.
    pub fn set_edge_smoothing(&mut self, radius: f32) {
        self.edge_smoothing = radius.clamp(0.0, 10.0);
    }

    /// Set spill suppression.
    pub fn set_spill_suppression(&mut self, strength: f32) {
        self.spill_suppression = strength.clamp(0.0, 100.0);
    }

    /// Set color.
    pub fn set_color(&mut self, color: ChromaKeyColor) {
        self.color = color;
    }

    /// Enable chroma key.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable chroma key.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Toggle chroma key.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Check if enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for ChromaKeySettings {
    fn default() -> Self {
        Self::new()
    }
}

/// Transition type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TransitionType {
    /// Simple crossfade.
    #[default]
    Fade,
    /// Wipe from one side to the other.
    Wipe { direction: WipeDirection },
    /// Slide from one side.
    Slide { direction: SlideDirection },
    /// Zoom in/out transition.
    Zoom { direction: ZoomDirection },
    /// Dissolve with random patterns.
    Dissolve,
}

/// Wipe direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WipeDirection {
    /// Left to right.
    LeftToRight,
    /// Right to left.
    RightToLeft,
    /// Top to bottom.
    TopToBottom,
    /// Bottom to top.
    BottomToTop,
}

/// Slide direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlideDirection {
    /// Slide in from left.
    Left,
    /// Slide in from right.
    Right,
    /// Slide in from top.
    Top,
    /// Slide in from bottom.
    Bottom,
}

/// Zoom direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoomDirection {
    /// Zoom in.
    In,
    /// Zoom out.
    Out,
}

/// Easing function for transitions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EasingFunction {
    /// Linear easing.
    #[default]
    Linear,
    /// Ease in (slow start).
    EaseIn,
    /// Ease out (slow end).
    EaseOut,
    /// Ease in and out.
    EaseInOut,
}

impl EasingFunction {
    /// Apply easing to progress (0.0-1.0).
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => t * (2.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    t * (4.0 - 2.0 * t) - 1.0
                }
            }
        }
    }
}

/// Transition configuration.
pub struct TransitionConfig {
    /// Transition type.
    pub typ: TransitionType,
    /// Easing function.
    pub easing: EasingFunction,
    /// Duration in seconds.
    pub duration: Duration,
    /// Is active.
    pub active: bool,
}

impl TransitionConfig {
    /// Create new fade transition.
    pub fn fade(duration: Duration) -> Self {
        Self {
            typ: TransitionType::Fade,
            easing: EasingFunction::default(),
            duration,
            active: false,
        }
    }

    /// Create new wipe transition.
    pub fn wipe(direction: WipeDirection, duration: Duration) -> Self {
        Self {
            typ: TransitionType::Wipe { direction },
            easing: EasingFunction::default(),
            duration,
            active: false,
        }
    }

    /// Create new slide transition.
    pub fn slide(direction: SlideDirection, duration: Duration) -> Self {
        Self {
            typ: TransitionType::Slide { direction },
            easing: EasingFunction::default(),
            duration,
            active: false,
        }
    }

    /// Create new zoom transition.
    pub fn zoom(direction: ZoomDirection, duration: Duration) -> Self {
        Self {
            typ: TransitionType::Zoom { direction },
            easing: EasingFunction::default(),
            duration,
            active: false,
        }
    }

    /// Create new dissolve transition.
    pub fn dissolve(duration: Duration) -> Self {
        Self {
            typ: TransitionType::Dissolve,
            easing: EasingFunction::default(),
            duration,
            active: false,
        }
    }

    /// Set easing function.
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// Activate the transition.
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate the transition.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Check if active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get eased progress.
    pub fn eased_progress(&self, progress: f32) -> f32 {
        self.easing.apply(progress.clamp(0.0, 1.0))
    }
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self::fade(Duration::from_millis(1000))
    }
}

/// Video layout.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VideoLayout {
    /// Single deck (crossfader controlled).
    #[default]
    Single,
    /// Split screen (50/50).
    SplitScreen { orientation: SplitOrientation },
    /// Picture-in-Picture.
    PictureInPicture {
        position: PiPPosition,
        inner_size: (u32, u32),
    },
}

/// Split screen orientation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitOrientation {
    /// Side by side.
    Horizontal,
    /// Top and bottom.
    Vertical,
}

/// PiP position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiPPosition {
    /// Top left.
    TopLeft,
    /// Top right.
    TopRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom right.
    BottomRight,
    /// Center.
    Center,
}

/// Aspect ratio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AspectRatio {
    /// 16:9 widescreen.
    #[default]
    _16_9,
    /// 4:3 standard.
    _4_3,
    /// 1:1 square.
    _1_1,
    /// Auto (match source).
    Auto,
}

/// Resolution preset.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ResolutionPreset {
    /// 720p.
    _720p,
    /// 1080p Full HD.
    #[default]
    _1080p,
    /// 4K Ultra HD.
    _4K,
}

impl ResolutionPreset {
    /// Get resolution.
    pub fn resolution(&self) -> (u32, u32) {
        match self {
            Self::_720p => (1280, 720),
            Self::_1080p => (1920, 1080),
            Self::_4K => (3840, 2160),
        }
    }
}

/// Video recording configuration.
pub struct RecordingConfig {
    /// Resolution preset.
    pub resolution: ResolutionPreset,
    /// Target frame rate.
    pub frame_rate: f64,
    /// Video layout.
    pub layout: VideoLayout,
    /// Aspect ratio.
    pub aspect_ratio: AspectRatio,
    /// Chroma key settings.
    pub chroma_key: ChromaKeySettings,
    /// Active transition.
    pub transition: TransitionConfig,
    /// Audio sync mode.
    pub audio_sync: AudioSyncMode,
    /// Frame capture enabled.
    pub frame_capture: bool,
}

impl RecordingConfig {
    /// Create default recording config (1080p, 30 FPS).
    pub fn default_recording() -> Self {
        Self {
            resolution: ResolutionPreset::_1080p,
            frame_rate: 30.0,
            layout: VideoLayout::default(),
            aspect_ratio: AspectRatio::default(),
            chroma_key: ChromaKeySettings::default(),
            transition: TransitionConfig::default(),
            audio_sync: AudioSyncMode::default(),
            frame_capture: true,
        }
    }

    /// Get output resolution.
    pub fn output_resolution(&self) -> (u32, u32) {
        let (w, h) = self.resolution.resolution();
        match self.aspect_ratio {
            AspectRatio::_16_9 => (w, h),
            AspectRatio::_4_3 => {
                let new_w = (h as f64 * 4.0 / 3.0) as u32;
                (new_w, h)
            }
            AspectRatio::_1_1 => (h, h),
            AspectRatio::Auto => (w, h),
        }
    }
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self::default_recording()
    }
}

/// Audio sync mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AudioSyncMode {
    /// Sync audio from video files only.
    #[default]
    VideoFileOnly,
    /// Mix with DJ audio engine.
    MixedWithDJ,
    /// DJ audio only (ignore video audio).
    DJOnly,
}

/// Video recording engine — manages transitions, layouts, chroma key, and capture.
pub struct VideoRecordingEngine {
    /// Recording configuration.
    config: RecordingConfig,
    /// Current frame rate.
    current_fps: f64,
    /// Frame count.
    frame_count: u64,
    /// Captured frames count.
    captured_frames: u64,
    /// Last frame timestamp.
    last_frame_time: Option<std::time::Instant>,
    /// Audio extraction status.
    audio_extracted: bool,
    /// Preview buffer (simplified).
    preview_data: Vec<u8>,
}

impl VideoRecordingEngine {
    /// Create new video recording engine.
    pub fn new() -> Self {
        Self {
            config: RecordingConfig::default_recording(),
            current_fps: 30.0,
            frame_count: 0,
            captured_frames: 0,
            last_frame_time: None,
            audio_extracted: false,
            preview_data: Vec::new(),
        }
    }

    /// Set recording config.
    pub fn set_config(&mut self, config: RecordingConfig) {
        self.current_fps = config.frame_rate;
        self.config = config;
    }

    /// Get recording config.
    pub fn config(&self) -> &RecordingConfig {
        &self.config
    }

    /// Set chroma key settings.
    pub fn set_chroma_key(&mut self, settings: ChromaKeySettings) {
        self.config.chroma_key = settings;
    }

    /// Get chroma key settings.
    pub fn chroma_key_settings(&self) -> &ChromaKeySettings {
        &self.config.chroma_key
    }

    /// Enable chroma key.
    pub fn enable_chroma_key(&mut self) {
        self.config.chroma_key.enable();
    }

    /// Disable chroma key.
    pub fn disable_chroma_key(&mut self) {
        self.config.chroma_key.disable();
    }

    /// Toggle chroma key.
    pub fn toggle_chroma_key(&mut self) {
        self.config.chroma_key.toggle();
    }

    /// Check if chroma key is enabled.
    pub fn is_chroma_key_enabled(&self) -> bool {
        self.config.chroma_key.is_enabled()
    }

    /// Set transition.
    pub fn set_transition(&mut self, config: TransitionConfig) {
        self.config.transition = config;
    }

    /// Start transition.
    pub fn start_transition(&mut self) {
        self.config.transition.activate();
    }

    /// Stop transition.
    pub fn stop_transition(&mut self) {
        self.config.transition.deactivate();
    }

    /// Check if transition is active.
    pub fn is_transition_active(&self) -> bool {
        self.config.transition.is_active()
    }

    /// Get eased transition progress.
    pub fn eased_transition_progress(&self, raw_progress: f32) -> f32 {
        self.config.transition.eased_progress(raw_progress)
    }

    /// Set video layout.
    pub fn set_layout(&mut self, layout: VideoLayout) {
        self.config.layout = layout;
    }

    /// Get video layout.
    pub fn layout(&self) -> &VideoLayout {
        &self.config.layout
    }

    /// Set aspect ratio.
    pub fn set_aspect_ratio(&mut self, ratio: AspectRatio) {
        self.config.aspect_ratio = ratio;
    }

    /// Get aspect ratio.
    pub fn aspect_ratio(&self) -> &AspectRatio {
        &self.config.aspect_ratio
    }

    /// Set resolution preset.
    pub fn set_resolution(&mut self, preset: ResolutionPreset) {
        self.config.resolution = preset;
    }

    /// Get output resolution.
    pub fn output_resolution(&self) -> (u32, u32) {
        self.config.output_resolution()
    }

    /// Get target frame rate.
    pub fn frame_rate(&self) -> f64 {
        self.current_fps
    }

    /// Check if minimum FPS is met.
    pub fn meets_minimum_fps(&self) -> bool {
        self.current_fps >= 30.0
    }

    /// Capture current frame.
    /// In production: would capture actual frame buffer.
    pub fn capture_frame(&mut self) -> Option<Vec<u8>> {
        if !self.config.frame_capture {
            return None;
        }

        self.captured_frames += 1;

        // Simulate frame capture (simplified RGB buffer).
        let (w, h) = self.output_resolution();
        let size = w as usize * h as usize * 3; // RGB.

        // Return a black frame (simplified).
        Some(vec![0u8; size.min(1000)])
    }

    /// Extract audio from video.
    /// In production: would use audio decoding library.
    pub fn extract_audio(&mut self) -> Result<Duration, VideoError> {
        // Simulate audio extraction.
        self.audio_extracted = true;
        Ok(Duration::from_secs(180)) // Simulated 3-minute video.
    }

    /// Check if audio is extracted.
    pub fn audio_extracted(&self) -> bool {
        self.audio_extracted
    }

    /// Set audio sync mode.
    pub fn set_audio_sync(&mut self, mode: AudioSyncMode) {
        self.config.audio_sync = mode;
    }

    /// Get audio sync mode.
    pub fn audio_sync_mode(&self) -> &AudioSyncMode {
        &self.config.audio_sync
    }

    /// Advance frame.
    pub fn advance_frame(&mut self) {
        self.frame_count += 1;
        self.last_frame_time = Some(std::time::Instant::now());
    }

    /// Get frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get captured frame count.
    pub fn captured_frames(&self) -> u64 {
        self.captured_frames
    }

    /// Get recording duration.
    pub fn recording_duration(&self) -> Duration {
        if self.frame_count == 0 || self.current_fps == 0.0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(self.frame_count as f64 / self.current_fps)
    }

    /// Get preview data.
    pub fn preview_data(&self) -> &[u8] {
        &self.preview_data
    }

    /// Update preview.
    pub fn update_preview(&mut self) {
        // In production: would generate actual preview.
    }

    /// Get recording status.
    pub fn status_string(&self) -> String {
        format!(
            "{}p, {} FPS, {:.1}s",
            self.config.resolution.resolution().0,
            self.current_fps,
            self.recording_duration().as_secs_f64()
        )
    }

    /// Get recording info.
    pub fn recording_info(&self) -> String {
        format!(
            "Recording: {} ({:.0} FPS) | {} | Frames: {}",
            self.config.resolution.resolution().0,
            self.current_fps,
            self.layout_name(),
            self.frame_count,
        )
    }

    /// Get layout name.
    pub fn layout_name(&self) -> String {
        match self.config.layout {
            VideoLayout::Single => "Single".to_string(),
            VideoLayout::SplitScreen { .. } => "Split Screen".to_string(),
            VideoLayout::PictureInPicture { .. } => "Picture-in-Picture".to_string(),
        }
    }

    /// Get transition type name.
    pub fn transition_name(&self) -> String {
        match self.config.transition.typ {
            TransitionType::Fade => "Fade".to_string(),
            TransitionType::Wipe { .. } => "Wipe".to_string(),
            TransitionType::Slide { .. } => "Slide".to_string(),
            TransitionType::Zoom { .. } => "Zoom".to_string(),
            TransitionType::Dissolve => "Dissolve".to_string(),
        }
    }

    /// Get available transitions.
    pub fn available_transitions() -> &'static [TransitionType] {
        &[
            TransitionType::Fade,
            TransitionType::Wipe {
                direction: WipeDirection::LeftToRight,
            },
            TransitionType::Wipe {
                direction: WipeDirection::RightToLeft,
            },
            TransitionType::Wipe {
                direction: WipeDirection::TopToBottom,
            },
            TransitionType::Wipe {
                direction: WipeDirection::BottomToTop,
            },
            TransitionType::Slide {
                direction: SlideDirection::Left,
            },
            TransitionType::Slide {
                direction: SlideDirection::Right,
            },
            TransitionType::Slide {
                direction: SlideDirection::Top,
            },
            TransitionType::Slide {
                direction: SlideDirection::Bottom,
            },
            TransitionType::Zoom {
                direction: ZoomDirection::In,
            },
            TransitionType::Zoom {
                direction: ZoomDirection::Out,
            },
            TransitionType::Dissolve,
        ]
    }

    /// Get available easings.
    pub fn available_easings() -> &'static [EasingFunction] {
        &[
            EasingFunction::Linear,
            EasingFunction::EaseIn,
            EasingFunction::EaseOut,
            EasingFunction::EaseInOut,
        ]
    }

    /// Get available layouts.
    pub fn available_layouts() -> &'static [VideoLayout] {
        &[
            VideoLayout::Single,
            VideoLayout::SplitScreen {
                orientation: SplitOrientation::Horizontal,
            },
            VideoLayout::SplitScreen {
                orientation: SplitOrientation::Vertical,
            },
            VideoLayout::PictureInPicture {
                position: PiPPosition::BottomRight,
                inner_size: (320, 240),
            },
        ]
    }

    /// Get available aspect ratios.
    pub fn available_aspect_ratios() -> &'static [AspectRatio] {
        &[
            AspectRatio::_16_9,
            AspectRatio::_4_3,
            AspectRatio::_1_1,
            AspectRatio::Auto,
        ]
    }

    /// Get available resolutions.
    pub fn available_resolutions() -> &'static [ResolutionPreset] {
        &[
            ResolutionPreset::_720p,
            ResolutionPreset::_1080p,
            ResolutionPreset::_4K,
        ]
    }
}

impl Default for VideoRecordingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Video recording error.
#[derive(Debug, Clone)]
pub enum VideoError {
    /// Chroma key color out of range.
    ChromaKeyInvalid,
    /// Invalid transition duration.
    InvalidTransitionDuration,
    /// Invalid frame rate.
    InvalidFrameRate,
    /// Audio extraction failed.
    AudioExtractionFailed,
    /// Frame capture failed.
    FrameCaptureFailed,
    /// Resolution not supported.
    UnsupportedResolution,
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChromaKeyInvalid => write!(f, "Invalid chroma key color"),
            Self::InvalidTransitionDuration => write!(f, "Invalid transition duration"),
            Self::InvalidFrameRate => write!(f, "Invalid frame rate"),
            Self::AudioExtractionFailed => write!(f, "Audio extraction failed"),
            Self::FrameCaptureFailed => write!(f, "Frame capture failed"),
            Self::UnsupportedResolution => write!(f, "Unsupported resolution"),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for VideoError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chroma_key_color_creation() {
        let color = ChromaKeyColor::green_screen();
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn chroma_key_color_matches() {
        let color = ChromaKeyColor::green_screen();
        assert!(color.matches(0, 255, 0, 10.0));
        assert!(!color.matches(255, 0, 0, 10.0));
    }

    #[test]
    fn chroma_key_settings_tolerance() {
        let mut settings = ChromaKeySettings::new();
        settings.set_tolerance(75.0);
        assert_eq!(settings.tolerance_percent, 75.0);

        settings.set_tolerance(150.0); // Clamped to 100.
        assert_eq!(settings.tolerance_percent, 100.0);
    }

    #[test]
    fn chroma_key_toggle() {
        let mut settings = ChromaKeySettings::new();
        assert!(!settings.is_enabled());

        settings.enable();
        assert!(settings.is_enabled());

        settings.toggle();
        assert!(!settings.is_enabled());

        settings.disable();
        assert!(!settings.is_enabled());
    }

    #[test]
    fn transition_config_fade() {
        let config = TransitionConfig::fade(Duration::from_millis(2000));
        assert!(matches!(config.typ, TransitionType::Fade));
        assert_eq!(config.duration, Duration::from_millis(2000));
        assert!(!config.is_active());
    }

    #[test]
    fn transition_config_wipe() {
        let config =
            TransitionConfig::wipe(WipeDirection::LeftToRight, Duration::from_millis(1000));
        assert!(
            matches!(config.typ, TransitionType::Wipe { direction } if direction == WipeDirection::LeftToRight)
        );
    }

    #[test]
    fn transition_config_slide() {
        let config = TransitionConfig::slide(SlideDirection::Top, Duration::from_millis(1500));
        assert!(
            matches!(config.typ, TransitionType::Slide { direction } if direction == SlideDirection::Top)
        );
    }

    #[test]
    fn transition_config_zoom() {
        let config = TransitionConfig::zoom(ZoomDirection::In, Duration::from_millis(800));
        assert!(
            matches!(config.typ, TransitionType::Zoom { direction } if direction == ZoomDirection::In)
        );
    }

    #[test]
    fn transition_config_dissolve() {
        let config = TransitionConfig::dissolve(Duration::from_millis(1200));
        assert!(matches!(config.typ, TransitionType::Dissolve));
    }

    #[test]
    fn transition_config_easing() {
        let config = TransitionConfig::fade(Duration::from_millis(1000))
            .with_easing(EasingFunction::EaseOut);
        assert_eq!(config.easing, EasingFunction::EaseOut);
    }

    #[test]
    fn transition_activate_deactivate() {
        let mut config = TransitionConfig::fade(Duration::from_millis(1000));
        assert!(!config.is_active());

        config.activate();
        assert!(config.is_active());

        config.deactivate();
        assert!(!config.is_active());
    }

    #[test]
    fn easing_linear() {
        assert_eq!(EasingFunction::Linear.apply(0.5), 0.5);
        assert_eq!(EasingFunction::Linear.apply(0.0), 0.0);
        assert_eq!(EasingFunction::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn easing_ease_in() {
        let result = EasingFunction::EaseIn.apply(0.5);
        assert_eq!(result, 0.25);
    }

    #[test]
    fn easing_ease_out() {
        let result = EasingFunction::EaseOut.apply(0.5);
        assert_eq!(result, 0.75);
    }

    #[test]
    fn easing_ease_in_out() {
        assert_eq!(EasingFunction::EaseInOut.apply(0.25), 0.125);
        assert_eq!(EasingFunction::EaseInOut.apply(0.75), 0.875);
    }

    #[test]
    fn transition_eased_progress() {
        let config = TransitionConfig::fade(Duration::from_millis(1000))
            .with_easing(EasingFunction::EaseInOut);
        assert_eq!(config.eased_progress(0.5), 0.5);
    }

    #[test]
    fn video_layout_single() {
        let layout = VideoLayout::Single;
        assert!(matches!(layout, VideoLayout::Single));
    }

    #[test]
    fn video_layout_split_screen() {
        let layout = VideoLayout::SplitScreen {
            orientation: SplitOrientation::Horizontal,
        };
        assert!(
            matches!(layout, VideoLayout::SplitScreen { orientation } if orientation == SplitOrientation::Horizontal)
        );
    }

    #[test]
    fn video_layout_picture_in_picture() {
        let layout = VideoLayout::PictureInPicture {
            position: PiPPosition::BottomRight,
            inner_size: (320, 240),
        };
        assert!(
            matches!(layout, VideoLayout::PictureInPicture { position, inner_size } if position == PiPPosition::BottomRight && inner_size == (320, 240))
        );
    }

    #[test]
    fn recording_config_default_1080p() {
        let config = RecordingConfig::default_recording();
        assert_eq!(config.resolution, ResolutionPreset::_1080p);
        assert_eq!(config.frame_rate, 30.0);
    }

    #[test]
    fn recording_config_output_resolution_16_9() {
        let mut config = RecordingConfig::default_recording();
        config.aspect_ratio = AspectRatio::_16_9;
        assert_eq!(config.output_resolution(), (1920, 1080));
    }

    #[test]
    fn recording_config_output_resolution_4_3() {
        let mut config = RecordingConfig::default_recording();
        config.aspect_ratio = AspectRatio::_4_3;
        assert_eq!(config.output_resolution(), (1440, 1080));
    }

    #[test]
    fn recording_config_output_resolution_1_1() {
        let mut config = RecordingConfig::default_recording();
        config.aspect_ratio = AspectRatio::_1_1;
        assert_eq!(config.output_resolution(), (1080, 1080));
    }

    #[test]
    fn video_recording_engine_creation() {
        let engine = VideoRecordingEngine::new();
        assert_eq!(engine.frame_rate(), 30.0);
        assert_eq!(engine.frame_count(), 0);
        assert!(!engine.is_chroma_key_enabled());
        assert!(!engine.is_transition_active());
    }

    #[test]
    fn video_recording_engine_chroma_key_toggle() {
        let mut engine = VideoRecordingEngine::new();
        assert!(!engine.is_chroma_key_enabled());

        engine.enable_chroma_key();
        assert!(engine.is_chroma_key_enabled());

        engine.toggle_chroma_key();
        assert!(!engine.is_chroma_key_enabled());

        engine.disable_chroma_key();
        assert!(!engine.is_chroma_key_enabled());
    }

    #[test]
    fn video_recording_engine_transition() {
        let mut engine = VideoRecordingEngine::new();
        assert!(!engine.is_transition_active());

        engine.start_transition();
        assert!(engine.is_transition_active());

        engine.stop_transition();
        assert!(!engine.is_transition_active());
    }

    #[test]
    fn video_recording_engine_frame_advance() {
        let mut engine = VideoRecordingEngine::new();
        assert_eq!(engine.frame_count(), 0);

        engine.advance_frame();
        assert_eq!(engine.frame_count(), 1);

        engine.advance_frame();
        assert_eq!(engine.frame_count(), 2);
    }

    #[test]
    fn video_recording_engine_capture() {
        let mut engine = VideoRecordingEngine::new();
        let frame = engine.capture_frame();
        assert!(frame.is_some());
        assert_eq!(engine.captured_frames(), 1);
    }

    #[test]
    fn video_recording_engine_no_capture() {
        let mut config = RecordingConfig::default_recording();
        config.frame_capture = false;

        let mut engine = VideoRecordingEngine::new();
        engine.set_config(config);

        let frame = engine.capture_frame();
        assert!(frame.is_none());
    }

    #[test]
    fn video_recording_engine_audio_extraction() {
        let mut engine = VideoRecordingEngine::new();
        let duration = engine.extract_audio().unwrap();
        assert!(duration > Duration::ZERO);
        assert!(engine.audio_extracted());
    }

    #[test]
    fn video_recording_engine_meets_minimum_fps() {
        let engine = VideoRecordingEngine::new();
        assert!(engine.meets_minimum_fps());
    }

    #[test]
    fn video_recording_engine_layout_change() {
        let mut engine = VideoRecordingEngine::new();
        assert!(matches!(engine.layout(), VideoLayout::Single));

        engine.set_layout(VideoLayout::SplitScreen {
            orientation: SplitOrientation::Horizontal,
        });
        assert!(matches!(engine.layout(), VideoLayout::SplitScreen { .. }));
    }

    #[test]
    fn video_recording_engine_resolution_change() {
        let mut engine = VideoRecordingEngine::new();
        assert_eq!(engine.output_resolution(), (1920, 1080));

        engine.set_resolution(ResolutionPreset::_720p);
        assert_eq!(engine.output_resolution(), (1280, 720));

        engine.set_resolution(ResolutionPreset::_4K);
        assert_eq!(engine.output_resolution(), (3840, 2160));
    }

    #[test]
    fn video_recording_engine_recording_duration() {
        let mut engine = VideoRecordingEngine::new();
        engine.current_fps = 30.0;

        for _ in 0..30 {
            engine.advance_frame();
        }

        let duration = engine.recording_duration();
        assert_eq!(duration, Duration::from_secs(1));
    }

    #[test]
    fn video_recording_engine_status_string() {
        let engine = VideoRecordingEngine::new();
        let status = engine.status_string();
        assert!(status.contains("1920"));
        assert!(status.contains("30"));
    }

    #[test]
    fn video_recording_engine_layout_name() {
        let engine = VideoRecordingEngine::new();
        assert_eq!(engine.layout_name(), "Single");
    }

    #[test]
    fn video_recording_engine_transition_name() {
        let engine = VideoRecordingEngine::new();
        assert_eq!(engine.transition_name(), "Fade");
    }

    #[test]
    fn video_recording_engine_available_transitions() {
        let transitions = VideoRecordingEngine::available_transitions();
        assert!(!transitions.is_empty());
        assert!(transitions
            .iter()
            .any(|t| matches!(t, TransitionType::Fade)));
        assert!(transitions
            .iter()
            .any(|t| matches!(t, TransitionType::Wipe { .. })));
        assert!(transitions
            .iter()
            .any(|t| matches!(t, TransitionType::Slide { .. })));
        assert!(transitions
            .iter()
            .any(|t| matches!(t, TransitionType::Zoom { .. })));
        assert!(transitions
            .iter()
            .any(|t| matches!(t, TransitionType::Dissolve)));
    }

    #[test]
    fn video_recording_engine_available_easings() {
        let easings = VideoRecordingEngine::available_easings();
        assert_eq!(easings.len(), 4);
    }

    #[test]
    fn video_recording_engine_available_layouts() {
        let layouts = VideoRecordingEngine::available_layouts();
        assert!(!layouts.is_empty());
    }

    #[test]
    fn video_recording_engine_available_aspect_ratios() {
        let ratios = VideoRecordingEngine::available_aspect_ratios();
        assert!(!ratios.is_empty());
    }

    #[test]
    fn video_recording_engine_available_resolutions() {
        let resolutions = VideoRecordingEngine::available_resolutions();
        assert_eq!(resolutions.len(), 3);
    }

    #[test]
    fn video_recording_engine_audio_sync_modes() {
        let mut engine = VideoRecordingEngine::new();
        assert!(matches!(
            engine.audio_sync_mode(),
            AudioSyncMode::VideoFileOnly
        ));

        engine.set_audio_sync(AudioSyncMode::MixedWithDJ);
        assert!(matches!(
            engine.audio_sync_mode(),
            AudioSyncMode::MixedWithDJ
        ));

        engine.set_audio_sync(AudioSyncMode::DJOnly);
        assert!(matches!(engine.audio_sync_mode(), AudioSyncMode::DJOnly));
    }

    #[test]
    fn recording_config_update() {
        let mut engine = VideoRecordingEngine::new();
        let mut config = RecordingConfig::default_recording();
        config.frame_rate = 60.0;
        config.resolution = ResolutionPreset::_720p;

        engine.set_config(config);
        assert_eq!(engine.frame_rate(), 60.0);
        assert_eq!(engine.output_resolution(), (1280, 720));
    }

    #[test]
    fn chroma_key_color_custom() {
        let color = ChromaKeyColor::new(128, 64, 192);
        assert_eq!(color.r, 128);
        assert_eq!(color.g, 64);
        assert_eq!(color.b, 192);
        assert_eq!(color.to_rgb(), (128, 64, 192));
    }

    #[test]
    fn transition_type_default() {
        let config = TransitionConfig {
            typ: TransitionType::default(),
            ..TransitionConfig::default()
        };
        assert!(matches!(config.typ, TransitionType::Fade));
    }

    #[test]
    fn easing_applies_clamped_progress() {
        let easing = EasingFunction::EaseIn;
        // Should clamp to 0.0.
        assert_eq!(easing.apply(-1.0), 0.0);
        // Should clamp to 1.0.
        assert_eq!(easing.apply(2.0), 1.0);
    }

    #[test]
    fn video_error_display() {
        let err = VideoError::AudioExtractionFailed;
        assert_eq!(format!("{}", err), "Audio extraction failed");
    }
}
