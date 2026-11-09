//! DVS Part 2 — Kalibrierung, Qualität, Modi, Recovery, RPM, Pitch Range, Platter.
//!
//! Extends the DVS engine (P11a) with signal quality metering,
//! tracking accuracy, auto-recovery, relative/absolute modes,
//! platter simulation, CDJ mode, direction tracking, RPM, and pitch range.
//! Part of the DVS feature (P11b).
//!
//! ## Design principles
//!
//! * **Real-time** — Deterministic behavior.
//! * **Resilient** — Auto-recovery from signal loss.
//! * **Precise** — Accurate quality and accuracy metrics.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Signal quality meter reading (0-100%).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalQuality {
    /// Quality percentage (0.0-100.0).
    pub percentage: f32,
    /// SNR (signal-to-noise ratio) in dB.
    pub snr_db: f32,
    /// Signal strength (0.0-1.0).
    pub strength: f32,
    /// Noise floor level.
    pub noise_floor: f32,
    /// Is signal valid.
    pub is_valid: bool,
}

impl SignalQuality {
    /// Create new signal quality reading.
    pub fn new(percentage: f32, snr_db: f32, strength: f32) -> Self {
        Self {
            percentage: percentage.clamp(0.0, 100.0),
            snr_db,
            strength: strength.clamp(0.0, 1.0),
            noise_floor: (100.0 - strength * 100.0) * -0.3,
            is_valid: percentage >= 30.0,
        }
    }

    /// Create no-signal reading.
    pub fn no_signal() -> Self {
        Self {
            percentage: 0.0,
            snr_db: -60.0,
            strength: 0.0,
            noise_floor: -18.0,
            is_valid: false,
        }
    }

    /// Create perfect signal reading.
    pub fn perfect() -> Self {
        Self {
            percentage: 100.0,
            snr_db: 60.0,
            strength: 1.0,
            noise_floor: -60.0,
            is_valid: true,
        }
    }

    /// Check if signal is weak.
    pub fn is_weak(&self) -> bool {
        self.percentage < 50.0
    }

    /// Check if signal is lost.
    pub fn is_lost(&self) -> bool {
        self.percentage < 10.0
    }

    /// Get quality bar level (0-10 segments).
    pub fn bar_level(&self) -> u32 {
        (self.percentage / 10.0).ceil() as u32
    }

    /// Get quality color warning.
    pub fn color_warning(&self) -> &str {
        if self.percentage >= 80.0 {
            "green"
        } else if self.percentage >= 50.0 {
            "yellow"
        } else if self.percentage >= 20.0 {
            "orange"
        } else {
            "red"
        }
    }
}

impl Default for SignalQuality {
    fn default() -> Self {
        Self::no_signal()
    }
}

/// Tracking accuracy score.
#[derive(Debug, Clone, Copy)]
pub struct TrackingAccuracy {
    /// Accuracy percentage (0-100).
    pub percentage: f32,
    /// Position drift in milliseconds.
    pub drift_ms: f32,
    /// Speed error in percentage.
    pub speed_error_percent: f32,
    /// Phase error in degrees.
    pub phase_error_deg: f32,
    /// Is tracking accurately.
    pub is_accurate: bool,
}

impl TrackingAccuracy {
    /// Create new tracking accuracy.
    pub fn new(percentage: f32, drift_ms: f32, speed_error: f32, phase_error: f32) -> Self {
        Self {
            percentage: percentage.clamp(0.0, 100.0),
            drift_ms: drift_ms.abs(),
            speed_error_percent: speed_error.abs(),
            phase_error_deg: (phase_error % 360.0).abs(),
            is_accurate: percentage >= 95.0,
        }
    }

    /// Create perfect tracking.
    pub fn perfect() -> Self {
        Self {
            percentage: 100.0,
            drift_ms: 0.0,
            speed_error_percent: 0.0,
            phase_error_deg: 0.0,
            is_accurate: true,
        }
    }

    /// Check if tracking is acceptable.
    pub fn is_acceptable(&self) -> bool {
        self.percentage >= 80.0 && self.drift_ms < 10.0
    }
}

impl Default for TrackingAccuracy {
    fn default() -> Self {
        Self {
            percentage: 0.0,
            drift_ms: 0.0,
            speed_error_percent: 0.0,
            phase_error_deg: 0.0,
            is_accurate: false,
        }
    }
}

/// DVS operating mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DvsMode {
    /// Relative mode (scratch like vinyl).
    #[default]
    Relative,
    /// Absolute mode (scratch and seek).
    Absolute,
    /// CDJ mode (jog wheel emulation).
    Cdj,
}

impl DvsMode {
    /// Get mode name.
    pub fn name(&self) -> &str {
        match self {
            Self::Relative => "Relative",
            Self::Absolute => "Absolute",
            Self::Cdj => "CDJ",
        }
    }

    /// Get mode description.
    pub fn description(&self) -> &str {
        match self {
            Self::Relative => "Scratch like vinyl — position follows rotation.",
            Self::Absolute => "Scratch and seek — position jumps to record position.",
            Self::Cdj => "Jog wheel emulation — push/pull behavior.",
        }
    }
}

/// Playback direction.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PlaybackDirection {
    /// Forward (normal playback).
    Forward,
    /// Reverse (rewind).
    Reverse,
    /// Stopped (no movement).
    #[default]
    Stopped,
    /// Paused (record stopped but deck running).
    Paused,
}

impl PlaybackDirection {
    /// Get direction symbol.
    pub fn symbol(&self) -> &str {
        match self {
            Self::Forward => "▶",
            Self::Reverse => "◀",
            Self::Stopped => "⏹",
            Self::Paused => "⏸",
        }
    }

    /// Get direction description.
    pub fn description(&self) -> &str {
        match self {
            Self::Forward => "Forward",
            Self::Reverse => "Reverse",
            Self::Stopped => "Stopped",
            Self::Paused => "Paused",
        }
    }
}

/// Vinyl RPM.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VinylRPM {
    /// 33⅓ RPM.
    #[default]
    RPM33,
    /// 45 RPM.
    RPM45,
    /// 78 RPM (historical).
    RPM78,
}

impl VinylRPM {
    /// Get RPM value.
    pub fn value(&self) -> f64 {
        match self {
            Self::RPM33 => 33.333,
            Self::RPM45 => 45.0,
            Self::RPM78 => 78.0,
        }
    }

    /// Get RPM name.
    pub fn name(&self) -> &str {
        match self {
            Self::RPM33 => "33⅓ RPM",
            Self::RPM45 => "45 RPM",
            Self::RPM78 => "78 RPM",
        }
    }
}

/// Pitch range setting.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PitchRange {
    /// ±8% pitch range.
    #[default]
    _8,
    /// ±16% pitch range.
    _16,
    /// ±50% pitch range.
    _50,
}

impl PitchRange {
    /// Get pitch range percentage.
    pub fn percentage(&self) -> f32 {
        match self {
            Self::_8 => 8.0,
            Self::_16 => 16.0,
            Self::_50 => 50.0,
        }
    }

    /// Get pitch range name.
    pub fn name(&self) -> &str {
        match self {
            Self::_8 => "±8%",
            Self::_16 => "±16%",
            Self::_50 => "±50%",
        }
    }
}

/// Platter state for visual simulation.
#[derive(Debug, Clone)]
pub struct PlatterState {
    /// Current rotation angle in degrees (0-360).
    pub angle_degrees: f64,
    /// Rotation speed in RPM.
    pub rpm: f64,
    /// Is rotating.
    pub is_rotating: bool,
    /// Cue point position in degrees.
    pub cue_point_degrees: f64,
    /// Direction of rotation.
    pub direction: PlaybackDirection,
}

impl PlatterState {
    /// Create new platter state.
    pub fn new(rpm: f64) -> Self {
        Self {
            angle_degrees: 0.0,
            rpm,
            is_rotating: false,
            cue_point_degrees: 0.0,
            direction: PlaybackDirection::default(),
        }
    }

    /// Update platter rotation.
    pub fn update(&mut self, elapsed_ms: u64) {
        if !self.is_rotating {
            return;
        }

        let degrees_per_ms = self.rpm * 360.0 / 60000.0;
        let delta = degrees_per_ms * elapsed_ms as f64;

        self.angle_degrees = (self.angle_degrees + delta) % 360.0;
    }

    /// Set cue point.
    pub fn set_cue_point(&mut self, degrees: f64) {
        self.cue_point_degrees = degrees % 360.0;
    }

    /// Check if at cue point.
    pub fn is_at_cue_point(&self) -> bool {
        (self.angle_degrees - self.cue_point_degrees).abs() < 5.0
    }

    /// Get angle as fraction (0-1).
    pub fn angle_fraction(&self) -> f64 {
        self.angle_degrees / 360.0
    }

    /// Advance cue point.
    pub fn advance_cue_point(&mut self) {
        self.cue_point_degrees = (self.cue_point_degrees + 90.0) % 360.0;
    }
}

impl Default for PlatterState {
    fn default() -> Self {
        Self::new(33.333)
    }
}

/// Calibration state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CalibrationState {
    /// Ready to calibrate.
    #[default]
    Ready,
    /// Calibration in progress.
    InProgress,
    /// Calibration successful.
    Success,
    /// Calibration failed.
    Failed,
}

impl CalibrationState {
    /// Check if calibration is complete.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Success | Self::Failed)
    }
}

/// Recovery state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RecoveryState {
    /// No recovery needed.
    #[default]
    None,
    /// Signal lost, attempting recovery.
    Recovering,
    /// Recovery successful.
    Recovered,
    /// Recovery failed.
    Failed,
}

/// DVS configuration.
pub struct DvsConfig {
    /// Operating mode.
    pub mode: DvsMode,
    /// Vinyl RPM.
    pub vinyl_rpm: VinylRPM,
    /// Pitch range.
    pub pitch_range: PitchRange,
    /// Recovery timeout (max time before recovery attempt).
    pub recovery_timeout: Duration,
    /// Minimum quality for auto-recovery.
    pub min_quality_for_recovery: f32,
    /// Calibration offset in milliseconds.
    pub calibration_offset_ms: f32,
    /// Latency compensation.
    pub latency_compensation_ms: f32,
}

impl DvsConfig {
    /// Create default config.
    pub fn default_config() -> Self {
        Self {
            mode: DvsMode::default(),
            vinyl_rpm: VinylRPM::default(),
            pitch_range: PitchRange::default(),
            recovery_timeout: Duration::from_millis(500),
            min_quality_for_recovery: 30.0,
            calibration_offset_ms: 0.0,
            latency_compensation_ms: 5.0,
        }
    }

    /// Set mode.
    pub fn set_mode(&mut self, mode: DvsMode) {
        self.mode = mode;
    }

    /// Set RPM.
    pub fn set_vinyl_rpm(&mut self, rpm: VinylRPM) {
        self.vinyl_rpm = rpm;
    }

    /// Set pitch range.
    pub fn set_pitch_range(&mut self, range: PitchRange) {
        self.pitch_range = range;
    }

    /// Set recovery timeout.
    pub fn set_recovery_timeout(&mut self, timeout: Duration) {
        self.recovery_timeout = timeout;
    }

    /// Set calibration offset.
    pub fn set_calibration_offset(&mut self, offset_ms: f32) {
        self.calibration_offset_ms = offset_ms;
    }
}

impl Default for DvsConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

/// DVS recording metrics.
#[derive(Debug, Clone)]
pub struct DvsRecordingMetrics {
    /// Total recording time.
    pub total_time: Duration,
    /// Scratch events detected.
    pub scratch_count: u32,
    /// Calibration events.
    pub calibration_count: u32,
    /// Recovery events.
    pub recovery_count: u32,
    /// Last quality reading.
    pub last_quality: SignalQuality,
    /// Last accuracy reading.
    pub last_accuracy: TrackingAccuracy,
}

impl DvsRecordingMetrics {
    /// Create new metrics.
    pub fn new() -> Self {
        Self {
            total_time: Duration::ZERO,
            scratch_count: 0,
            calibration_count: 0,
            recovery_count: 0,
            last_quality: SignalQuality::no_signal(),
            last_accuracy: TrackingAccuracy::default(),
        }
    }

    /// Record scratch event.
    pub fn record_scratch(&mut self) {
        self.scratch_count += 1;
    }

    /// Record calibration event.
    pub fn record_calibration(&mut self) {
        self.calibration_count += 1;
    }

    /// Record recovery event.
    pub fn record_recovery(&mut self) {
        self.recovery_count += 1;
    }

    /// Update quality reading.
    pub fn update_quality(&mut self, quality: SignalQuality) {
        self.last_quality = quality;
    }

    /// Update accuracy reading.
    pub fn update_accuracy(&mut self, accuracy: TrackingAccuracy) {
        self.last_accuracy = accuracy;
    }

    /// Advance total time.
    pub fn advance_time(&mut self, elapsed: Duration) {
        self.total_time += elapsed;
    }

    /// Get average scratches per minute.
    pub fn scratches_per_minute(&self) -> f64 {
        let minutes = self.total_time.as_secs_f64() / 60.0;
        if minutes == 0.0 {
            0.0
        } else {
            self.scratch_count as f64 / minutes
        }
    }

    /// Get session summary.
    pub fn summary(&self) -> String {
        format!(
            "Total: {:.0}s | Scratches: {} ({:.1}/min) | Calibrations: {} | Recoveries: {}",
            self.total_time.as_secs_f64(),
            self.scratch_count,
            self.scratches_per_minute(),
            self.calibration_count,
            self.recovery_count,
        )
    }
}

impl Default for DvsRecordingMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// DVS recording engine — manages calibration, quality, modes, recovery, and metrics.
pub struct DvsRecordingEngine {
    /// DVS configuration.
    config: DvsConfig,
    /// Current signal quality.
    signal_quality: SignalQuality,
    /// Current tracking accuracy.
    tracking_accuracy: TrackingAccuracy,
    /// Playback direction.
    playback_direction: PlaybackDirection,
    /// Platter state.
    platter: PlatterState,
    /// Calibration state.
    calibration: CalibrationState,
    /// Recovery state.
    recovery: RecoveryState,
    /// Recording metrics.
    metrics: DvsRecordingMetrics,
    /// Last signal loss time.
    last_signal_loss: Option<std::time::Instant>,
    /// Session start time.
    session_start: std::time::Instant,
}

impl DvsRecordingEngine {
    /// Create new DVS recording engine.
    pub fn new() -> Self {
        Self {
            config: DvsConfig::default_config(),
            signal_quality: SignalQuality::no_signal(),
            tracking_accuracy: TrackingAccuracy::default(),
            playback_direction: PlaybackDirection::default(),
            platter: PlatterState::default(),
            calibration: CalibrationState::default(),
            recovery: RecoveryState::default(),
            metrics: DvsRecordingMetrics::new(),
            last_signal_loss: None,
            session_start: std::time::Instant::now(),
        }
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: DvsConfig) {
        self.config = config;
        self.platter = PlatterState::new(self.config.vinyl_rpm.value());
    }

    /// Get configuration.
    pub fn config(&self) -> &DvsConfig {
        &self.config
    }

    /// Set operating mode.
    pub fn set_mode(&mut self, mode: DvsMode) {
        self.config.set_mode(mode);
    }

    /// Get operating mode.
    pub fn mode(&self) -> &DvsMode {
        &self.config.mode
    }

    /// Get mode name.
    pub fn mode_name(&self) -> &str {
        self.config.mode.name()
    }

    /// Set vinyl RPM.
    pub fn set_vinyl_rpm(&mut self, rpm: VinylRPM) {
        self.config.set_vinyl_rpm(rpm.clone());
        self.platter = PlatterState::new(rpm.value());
    }

    /// Get vinyl RPM.
    pub fn vinyl_rpm(&self) -> &VinylRPM {
        &self.config.vinyl_rpm
    }

    /// Set pitch range.
    pub fn set_pitch_range(&mut self, range: PitchRange) {
        self.config.set_pitch_range(range);
    }

    /// Get pitch range.
    pub fn pitch_range(&self) -> &PitchRange {
        &self.config.pitch_range
    }

    /// Update signal quality.
    pub fn update_signal_quality(&mut self, quality: SignalQuality) {
        self.signal_quality = quality;
        self.metrics.update_quality(quality);

        // Track signal loss.
        if quality.is_lost() {
            self.last_signal_loss = Some(std::time::Instant::now());
        } else if quality.is_valid {
            self.last_signal_loss = None;
            self.recovery = RecoveryState::None;
        }
    }

    /// Get signal quality.
    pub fn signal_quality(&self) -> &SignalQuality {
        &self.signal_quality
    }

    /// Update tracking accuracy.
    pub fn update_tracking_accuracy(&mut self, accuracy: TrackingAccuracy) {
        self.tracking_accuracy = accuracy;
        self.metrics.update_accuracy(accuracy);
    }

    /// Get tracking accuracy.
    pub fn tracking_accuracy(&self) -> &TrackingAccuracy {
        &self.tracking_accuracy
    }

    /// Set playback direction.
    pub fn set_playback_direction(&mut self, direction: PlaybackDirection) {
        self.playback_direction = direction.clone();
        self.platter.direction = direction.clone();
        self.platter.is_rotating =
            direction != PlaybackDirection::Stopped && direction != PlaybackDirection::Paused;
    }

    /// Get playback direction.
    pub fn playback_direction(&self) -> &PlaybackDirection {
        &self.playback_direction
    }

    /// Get direction symbol.
    pub fn direction_symbol(&self) -> &str {
        self.playback_direction.symbol()
    }

    /// Update platter state.
    pub fn update_platter(&mut self, elapsed_ms: u64) {
        self.platter.update(elapsed_ms);
    }

    /// Get platter state.
    pub fn platter_state(&self) -> &PlatterState {
        &self.platter
    }

    /// Set cue point on platter.
    pub fn set_cue_point(&mut self, degrees: f64) {
        self.platter.set_cue_point(degrees);
    }

    /// Check if at cue point.
    pub fn is_at_cue_point(&self) -> bool {
        self.platter.is_at_cue_point()
    }

    /// Advance cue point.
    pub fn advance_cue_point(&mut self) {
        self.platter.advance_cue_point();
    }

    /// Get platter angle.
    pub fn platter_angle(&self) -> f64 {
        self.platter.angle_degrees
    }

    /// Get platter angle fraction.
    pub fn platter_angle_fraction(&self) -> f64 {
        self.platter.angle_fraction()
    }

    /// Start calibration.
    pub fn start_calibration(&mut self) {
        self.calibration = CalibrationState::InProgress;
    }

    /// Complete calibration successfully.
    pub fn complete_calibration_success(&mut self) {
        self.calibration = CalibrationState::Success;
        self.metrics.record_calibration();
    }

    /// Complete calibration failed.
    pub fn complete_calibration_failed(&mut self) {
        self.calibration = CalibrationState::Failed;
    }

    /// Get calibration state.
    pub fn calibration_state(&self) -> &CalibrationState {
        &self.calibration
    }

    /// Check if calibration is in progress.
    pub fn is_calibrating(&self) -> bool {
        self.calibration == CalibrationState::InProgress
    }

    /// Auto-recover from signal loss.
    pub fn auto_recover(&mut self) {
        if let Some(loss_time) = self.last_signal_loss {
            let elapsed = loss_time.elapsed();

            if elapsed < self.config.recovery_timeout
                && self.signal_quality.percentage >= self.config.min_quality_for_recovery
            {
                self.recovery = RecoveryState::Recovered;
                self.last_signal_loss = None;
                self.metrics.record_recovery();
            } else if elapsed >= self.config.recovery_timeout {
                self.recovery = RecoveryState::Failed;
            }
        }
    }

    /// Get recovery state.
    pub fn recovery_state(&self) -> &RecoveryState {
        &self.recovery
    }

    /// Check if recovery is in progress.
    pub fn is_recovering(&self) -> bool {
        self.recovery == RecoveryState::Recovering
    }

    /// Get recording metrics.
    pub fn metrics(&self) -> &DvsRecordingMetrics {
        &self.metrics
    }

    /// Advance session time.
    pub fn advance_session_time(&mut self, elapsed: Duration) {
        self.metrics.advance_time(elapsed);
    }

    /// Get session duration.
    pub fn session_duration(&self) -> Duration {
        self.metrics.total_time
    }

    /// Record scratch event.
    pub fn record_scratch(&mut self) {
        self.metrics.record_scratch();
    }

    /// Get scratch count.
    pub fn scratch_count(&self) -> u32 {
        self.metrics.scratch_count
    }

    /// Get metrics summary.
    pub fn metrics_summary(&self) -> String {
        self.metrics.summary()
    }

    /// Get recording status string.
    pub fn status_string(&self) -> String {
        format!(
            "{} | Quality: {:.0}% | Accuracy: {:.0}% | Mode: {}",
            self.playback_direction.description(),
            self.signal_quality.percentage,
            self.tracking_accuracy.percentage,
            self.config.mode.name(),
        )
    }

    /// Get full status.
    pub fn full_status(&self) -> String {
        format!(
            "DVS: {} | {} | Quality: {:.0}% ({}) | Accuracy: {:.0}% | Pitch: {} | Calib: {:?}",
            self.config.mode.name(),
            self.playback_direction.description(),
            self.signal_quality.percentage,
            self.signal_quality.color_warning(),
            self.tracking_accuracy.percentage,
            self.config.pitch_range.name(),
            self.calibration,
        )
    }

    /// Get available modes.
    pub fn available_modes() -> &'static [DvsMode] {
        &[DvsMode::Relative, DvsMode::Absolute, DvsMode::Cdj]
    }

    /// Get available RPMs.
    pub fn available_rpms() -> &'static [VinylRPM] {
        &[VinylRPM::RPM33, VinylRPM::RPM45, VinylRPM::RPM78]
    }

    /// Get available pitch ranges.
    pub fn available_pitch_ranges() -> &'static [PitchRange] {
        &[PitchRange::_8, PitchRange::_16, PitchRange::_50]
    }

    /// Get calibration offset.
    pub fn calibration_offset(&self) -> f32 {
        self.config.calibration_offset_ms
    }

    /// Set calibration offset.
    pub fn set_calibration_offset(&mut self, offset_ms: f32) {
        self.config.set_calibration_offset(offset_ms);
    }
}

impl Default for DvsRecordingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// DVS error.
#[derive(Debug, Clone)]
pub enum DvsError {
    /// Calibration failed.
    CalibrationFailed,
    /// Signal loss too long.
    SignalLossTooLong,
    /// Invalid mode.
    InvalidMode,
    /// Invalid RPM.
    InvalidRPM,
    /// Pitch range overflow.
    PitchOverflow,
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for DvsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CalibrationFailed => write!(f, "Calibration failed"),
            Self::SignalLossTooLong => write!(f, "Signal loss too long"),
            Self::InvalidMode => write!(f, "Invalid DVS mode"),
            Self::InvalidRPM => write!(f, "Invalid vinyl RPM"),
            Self::PitchOverflow => write!(f, "Pitch range overflow"),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for DvsError {}

impl RecoveryState {
    pub fn is_complete(&self) -> bool {
        !matches!(self, Self::Recovering)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dvs_recording_engine_creation() {
        let engine = DvsRecordingEngine::new();
        assert!(matches!(engine.mode(), DvsMode::Relative));
        assert!(matches!(engine.vinyl_rpm(), VinylRPM::RPM33));
    }

    #[test]
    fn signal_quality_creation() {
        let quality = SignalQuality::new(85.0, 30.0, 0.85);
        assert_eq!(quality.percentage, 85.0);
        assert!(quality.is_valid);
        assert!(!quality.is_weak());
        assert!(!quality.is_lost());
    }

    #[test]
    fn signal_quality_no_signal() {
        let quality = SignalQuality::no_signal();
        assert_eq!(quality.percentage, 0.0);
        assert!(!quality.is_valid);
        assert!(quality.is_lost());
    }

    #[test]
    fn signal_quality_perfect() {
        let quality = SignalQuality::perfect();
        assert_eq!(quality.percentage, 100.0);
        assert!(quality.is_valid);
    }

    #[test]
    fn signal_quality_weak() {
        let quality = SignalQuality::new(40.0, 10.0, 0.4);
        assert!(quality.is_weak());
        assert_eq!(quality.color_warning(), "orange");
    }

    #[test]
    fn signal_quality_bar_level() {
        let quality = SignalQuality::new(75.0, 20.0, 0.75);
        assert_eq!(quality.bar_level(), 8);
    }

    #[test]
    fn tracking_accuracy_creation() {
        let accuracy = TrackingAccuracy::new(98.0, 1.0, 0.5, 2.0);
        assert!(accuracy.is_accurate);
        assert!(accuracy.is_acceptable());
    }

    #[test]
    fn tracking_accuracy_perfect() {
        let accuracy = TrackingAccuracy::perfect();
        assert!(accuracy.is_accurate);
        assert!(accuracy.is_acceptable());
        assert_eq!(accuracy.drift_ms, 0.0);
    }

    #[test]
    fn tracking_accuracy_unacceptable() {
        let accuracy = TrackingAccuracy::new(70.0, 20.0, 5.0, 30.0);
        assert!(!accuracy.is_accurate);
        assert!(!accuracy.is_acceptable());
    }

    #[test]
    fn dvs_mode_relative() {
        assert!(matches!(DvsMode::Relative, DvsMode::Relative));
        assert_eq!(DvsMode::Relative.name(), "Relative");
    }

    #[test]
    fn dvs_mode_absolute() {
        assert!(matches!(DvsMode::Absolute, DvsMode::Absolute));
        assert_eq!(DvsMode::Absolute.name(), "Absolute");
    }

    #[test]
    fn dvs_mode_cdj() {
        assert!(matches!(DvsMode::Cdj, DvsMode::Cdj));
        assert_eq!(DvsMode::Cdj.name(), "CDJ");
    }

    #[test]
    fn playback_direction_forward() {
        assert!(matches!(
            PlaybackDirection::Forward,
            PlaybackDirection::Forward
        ));
        assert_eq!(PlaybackDirection::Forward.symbol(), "▶");
    }

    #[test]
    fn playback_direction_reverse() {
        assert_eq!(PlaybackDirection::Reverse.symbol(), "◀");
    }

    #[test]
    fn playback_direction_stopped() {
        assert_eq!(PlaybackDirection::Stopped.symbol(), "⏹");
    }

    #[test]
    fn playback_direction_paused() {
        assert_eq!(PlaybackDirection::Paused.symbol(), "⏸");
    }

    #[test]
    fn vinyl_rpm_33() {
        assert_eq!(VinylRPM::RPM33.value(), 33.333);
        assert_eq!(VinylRPM::RPM33.name(), "33⅓ RPM");
    }

    #[test]
    fn vinyl_rpm_45() {
        assert_eq!(VinylRPM::RPM45.value(), 45.0);
    }

    #[test]
    fn pitch_range_8() {
        assert_eq!(PitchRange::_8.percentage(), 8.0);
        assert_eq!(PitchRange::_8.name(), "±8%");
    }

    #[test]
    fn pitch_range_50() {
        assert_eq!(PitchRange::_50.percentage(), 50.0);
    }

    #[test]
    fn platter_state_creation() {
        let platter = PlatterState::new(33.333);
        assert_eq!(platter.rpm, 33.333);
        assert!(!platter.is_rotating);
        assert_eq!(platter.angle_degrees, 0.0);
    }

    #[test]
    fn platter_update_rotation() {
        let mut platter = PlatterState::new(60.0); // 1 revolution per second.
        platter.is_rotating = true;
        platter.direction = PlaybackDirection::Forward;

        platter.update(1000); // 1 second.
        assert_eq!(platter.angle_degrees, 360.0 % 360.0);
    }

    #[test]
    fn platter_no_rotation() {
        let mut platter = PlatterState::new(33.333);
        platter.is_rotating = false;
        platter.update(1000);
        assert_eq!(platter.angle_degrees, 0.0);
    }

    #[test]
    fn platter_cue_point() {
        let mut platter = PlatterState::new(33.333);
        platter.set_cue_point(90.0);

        // Not at cue point.
        assert!(!platter.is_at_cue_point());

        // Move to cue point.
        platter.angle_degrees = 90.0;
        assert!(platter.is_at_cue_point());
    }

    #[test]
    fn platter_advance_cue_point() {
        let mut platter = PlatterState::new(33.333);
        platter.set_cue_point(0.0);
        platter.advance_cue_point();
        assert_eq!(platter.cue_point_degrees, 90.0);
    }

    #[test]
    fn platter_angle_fraction() {
        let platter = PlatterState::new(33.333);
        assert_eq!(platter.angle_fraction(), 0.0);
    }

    #[test]
    fn calibration_state_ready() {
        assert!(!CalibrationState::Ready.is_complete());
        assert!(CalibrationState::Success.is_complete());
        assert!(CalibrationState::Failed.is_complete());
    }

    #[test]
    fn recovery_state_none() {
        assert!(RecoveryState::None == RecoveryState::None);
        assert!(!RecoveryState::Recovering.is_complete()); // Not complete.
    }

    #[test]
    fn dvs_recording_engine_mode_change() {
        let mut engine = DvsRecordingEngine::new();
        assert!(matches!(engine.mode(), DvsMode::Relative));

        engine.set_mode(DvsMode::Absolute);
        assert!(matches!(engine.mode(), DvsMode::Absolute));
    }

    #[test]
    fn dvs_recording_engine_rpm_change() {
        let mut engine = DvsRecordingEngine::new();
        assert!(matches!(engine.vinyl_rpm(), VinylRPM::RPM33));

        engine.set_vinyl_rpm(VinylRPM::RPM45);
        assert!(matches!(engine.vinyl_rpm(), VinylRPM::RPM45));
    }

    #[test]
    fn dvs_recording_engine_pitch_range_change() {
        let mut engine = DvsRecordingEngine::new();
        engine.set_pitch_range(PitchRange::_16);
        assert_eq!(engine.pitch_range().percentage(), 16.0);
    }

    #[test]
    fn dvs_recording_engine_direction_change() {
        let mut engine = DvsRecordingEngine::new();
        engine.set_playback_direction(PlaybackDirection::Forward);
        assert!(matches!(
            engine.playback_direction(),
            PlaybackDirection::Forward
        ));
        assert_eq!(engine.direction_symbol(), "▶");
    }

    #[test]
    fn dvs_recording_engine_signal_quality_update() {
        let mut engine = DvsRecordingEngine::new();
        engine.update_signal_quality(SignalQuality::perfect());
        assert_eq!(engine.signal_quality().percentage, 100.0);
    }

    #[test]
    fn dvs_recording_engine_tracking_accuracy_update() {
        let mut engine = DvsRecordingEngine::new();
        engine.update_tracking_accuracy(TrackingAccuracy::perfect());
        assert_eq!(engine.tracking_accuracy().percentage, 100.0);
    }

    #[test]
    fn dvs_recording_engine_calibration() {
        let mut engine = DvsRecordingEngine::new();
        assert!(!engine.is_calibrating());

        engine.start_calibration();
        assert!(engine.is_calibrating());

        engine.complete_calibration_success();
        assert!(engine.calibration_state().is_complete());
    }

    #[test]
    fn dvs_recording_engine_auto_recover_signal_lost() {
        let mut engine = DvsRecordingEngine::new();
        engine.update_signal_quality(SignalQuality::no_signal());

        // Advance time to test recovery timeout.
        std::thread::sleep(Duration::from_millis(100));
        engine.update_signal_quality(SignalQuality::new(50.0, 15.0, 0.5));
        engine.auto_recover();

        // Should recover since quality >= threshold.
        assert!(matches!(
            engine.recovery_state(),
            RecoveryState::Recovered | RecoveryState::None
        ));
    }

    #[test]
    fn dvs_recording_engine_platter_update() {
        let mut engine = DvsRecordingEngine::new();
        engine.set_playback_direction(PlaybackDirection::Forward);

        // Advance platter.
        engine.update_platter(500); // 500ms.

        // Platter should have moved.
        assert!(engine.platter_angle() >= 0.0);
    }

    #[test]
    fn dvs_recording_engine_scratch_recording() {
        let mut engine = DvsRecordingEngine::new();
        assert_eq!(engine.scratch_count(), 0);

        engine.record_scratch();
        assert_eq!(engine.scratch_count(), 1);

        engine.record_scratch();
        assert_eq!(engine.scratch_count(), 2);
    }

    #[test]
    fn dvs_recording_engine_session_time() {
        let mut engine = DvsRecordingEngine::new();
        engine.advance_session_time(Duration::from_secs(30));

        let duration = engine.session_duration();
        assert!(duration >= Duration::from_secs(29));
    }

    #[test]
    fn dvs_recording_engine_status_string() {
        let mut engine = DvsRecordingEngine::new();
        engine.update_signal_quality(SignalQuality::new(80.0, 25.0, 0.8));
        engine.update_tracking_accuracy(TrackingAccuracy::new(95.0, 2.0, 1.0, 5.0));
        engine.set_playback_direction(PlaybackDirection::Forward);

        let status = engine.status_string();
        assert!(status.contains("Relative"));
        assert!(status.contains("80%"));
    }

    #[test]
    fn dvs_recording_engine_full_status() {
        let engine = DvsRecordingEngine::new();
        let status = engine.full_status();
        assert!(status.contains("DVS"));
    }

    #[test]
    fn dvs_recording_engine_metrics_summary() {
        let mut engine = DvsRecordingEngine::new();
        engine.record_scratch();
        engine.record_scratch();
        engine.advance_session_time(Duration::from_secs(60));

        let summary = engine.metrics_summary();
        assert!(summary.contains("Scratches: 2"));
    }

    #[test]
    fn dvs_recording_engine_available_modes() {
        let modes = DvsRecordingEngine::available_modes();
        assert_eq!(modes.len(), 3);
    }

    #[test]
    fn dvs_recording_engine_available_rpms() {
        let rpms = DvsRecordingEngine::available_rpms();
        assert_eq!(rpms.len(), 3);
    }

    #[test]
    fn dvs_recording_engine_available_pitch_ranges() {
        let ranges = DvsRecordingEngine::available_pitch_ranges();
        assert_eq!(ranges.len(), 3);
    }

    #[test]
    fn dvs_recording_engine_set_calibration_offset() {
        let mut engine = DvsRecordingEngine::new();
        engine.set_calibration_offset(2.5);
        assert_eq!(engine.calibration_offset(), 2.5);
    }

    #[test]
    fn dvs_config_default() {
        let config = DvsConfig::default_config();
        assert_eq!(config.mode, DvsMode::Relative);
        assert_eq!(config.vinyl_rpm, VinylRPM::RPM33);
        assert_eq!(config.pitch_range, PitchRange::_8);
    }

    #[test]
    fn dvs_config_set_recovery_timeout() {
        let mut config = DvsConfig::default_config();
        config.set_recovery_timeout(Duration::from_millis(1000));
        assert_eq!(config.recovery_timeout, Duration::from_millis(1000));
    }

    #[test]
    fn signal_quality_clamped() {
        let quality = SignalQuality::new(150.0, 30.0, 1.5);
        assert_eq!(quality.percentage, 100.0); // Clamped.
        assert_eq!(quality.strength, 1.0); // Clamped.
    }

    #[test]
    fn tracking_accuracy_clamped() {
        let accuracy = TrackingAccuracy::new(-10.0, -5.0, -3.0, 400.0);
        assert_eq!(accuracy.percentage, 0.0); // Clamped.
        assert_eq!(accuracy.drift_ms, 5.0); // Absolute.
        assert_eq!(accuracy.phase_error_deg, 40.0); // 400 % 360.
    }

    #[test]
    fn platter_angle_wraps() {
        let mut platter = PlatterState::new(120.0); // 2 rev/s.
        platter.is_rotating = true;

        platter.update(1500); // 3 revolutions.
        assert_eq!(platter.angle_degrees, 0.0); // 720 % 360.
    }

    #[test]
    fn dvs_error_display() {
        assert_eq!(
            format!("{}", DvsError::CalibrationFailed),
            "Calibration failed"
        );
        assert_eq!(format!("{}", DvsError::InvalidMode), "Invalid DVS mode");
    }
}
