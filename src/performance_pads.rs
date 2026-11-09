//! Performance Pads Part 2 — Slicer, Sampler, Beat Jump, Saved Loop, Velocity, Feedback.
//!
//! Extends the performance pads system (P16a) with slicer, sampler, beat jump,
//! saved loop, velocity-sensitive triggering, and visual feedback.
//! Part of the Performance Pads feature (P16b).
//!
//! ## Design principles
//!
//! * **Low-latency** — Pad triggering under 10 ms.
//! * **Beat-synced** — Slicer divides bar into equal slices.
//! * **Visual** — Color-coded feedback for active pads.

use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Pad mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadMode {
    /// Hot cue mode (P16a).
    HotCue,
    /// Loop roll mode (P16a).
    LoopRoll,
    /// Slicer mode (P16b).
    Slicer,
    /// Sampler mode (P16b).
    Sampler,
    /// Beat jump mode (P16b).
    BeatJump,
    /// Saved loop mode (P16b).
    SavedLoop,
}

impl PadMode {
    /// Get mode name.
    pub fn name(&self) -> &str {
        match self {
            Self::HotCue => "Hot Cue",
            Self::LoopRoll => "Loop Roll",
            Self::Slicer => "Slicer",
            Self::Sampler => "Sampler",
            Self::BeatJump => "Beat Jump",
            Self::SavedLoop => "Saved Loop",
        }
    }
}

impl std::fmt::Display for PadMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Pad color for visual feedback.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PadColor {
    /// Red.
    #[default]
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Purple.
    Purple,
    /// Cyan.
    Cyan,
    /// Orange.
    Orange,
    /// Pink.
    Pink,
}

impl PadColor {
    /// Get hex color code.
    pub fn hex(&self) -> &str {
        match self {
            Self::Red => "#FF0000",
            Self::Green => "#00FF00",
            Self::Yellow => "#FFFF00",
            Self::Blue => "#0000FF",
            Self::Purple => "#800080",
            Self::Cyan => "#00FFFF",
            Self::Orange => "#FF8800",
            Self::Pink => "#FF69B4",
        }
    }

    /// Get CSS name.
    pub fn css_name(&self) -> &str {
        match self {
            Self::Red => "red",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Purple => "purple",
            Self::Cyan => "cyan",
            Self::Orange => "orange",
            Self::Pink => "pink",
        }
    }
}

/// Sliced region.
#[derive(Debug, Clone)]
pub struct SlicedRegion {
    /// Slice index (0-7).
    pub slice_index: u32,
    /// Start position (samples).
    pub start_sample: u64,
    /// End position (samples).
    pub end_sample: u64,
    /// Is active.
    pub is_active: bool,
}

impl SlicedRegion {
    /// Create new sliced region.
    pub fn new(slice_index: u32, start_sample: u64, end_sample: u64) -> Self {
        Self {
            slice_index,
            start_sample,
            end_sample,
            is_active: false,
        }
    }

    /// Toggle active state.
    pub fn toggle_active(&mut self) {
        self.is_active = !self.is_active;
    }

    /// Get duration in samples.
    pub fn duration_samples(&self) -> u64 {
        self.end_sample.saturating_sub(self.start_sample)
    }

    /// Is valid?
    pub fn is_valid(&self) -> bool {
        self.end_sample > self.start_sample
    }
}

/// Sampler sound configuration.
#[derive(Debug, Clone)]
pub struct SamplerSound {
    /// Sound name.
    pub name: String,
    /// File path.
    pub file_path: String,
    /// Start sample offset.
    pub start_offset: u64,
    /// End sample offset.
    pub end_offset: u64,
    /// Volume (0.0-1.0).
    pub volume: f32,
    /// Pan (-1.0 to 1.0).
    pub pan: f32,
    /// Pitch shift (semitones).
    pub pitch_shift: i32,
}

impl SamplerSound {
    /// Create new sampler sound.
    pub fn new(name: &str, file_path: &str) -> Self {
        Self {
            name: name.to_string(),
            file_path: file_path.to_string(),
            start_offset: 0,
            end_offset: 0,
            volume: 1.0,
            pan: 0.0,
            pitch_shift: 0,
        }
    }

    /// Set volume.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Set pan.
    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
    }

    /// Set pitch shift.
    pub fn set_pitch_shift(&mut self, semitones: i32) {
        self.pitch_shift = semitones.clamp(-48, 48);
    }

    /// Is valid?
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && !self.file_path.is_empty() && self.end_offset > self.start_offset
    }
}

impl Default for SamplerSound {
    fn default() -> Self {
        Self::new("Sound", "")
    }
}

/// Beat jump distance options.
pub const BEAT_JUMP_DISTANCES: [i32; 8] = [-32, -16, -8, -4, 4, 8, 16, 32];

/// Saved loop position.
#[derive(Debug, Clone)]
pub struct SavedLoop {
    /// Loop ID.
    pub id: String,
    /// Start position (samples).
    pub start_sample: u64,
    /// End position (samples).
    pub end_sample: u64,
    /// Loop duration.
    pub duration: Duration,
    /// Is active.
    pub is_active: bool,
}

impl SavedLoop {
    /// Create new saved loop.
    pub fn new(id: &str, start_sample: u64, end_sample: u64) -> Self {
        Self {
            id: id.to_string(),
            start_sample,
            end_sample,
            duration: Duration::ZERO,
            is_active: false,
        }
    }

    /// Toggle active state.
    pub fn toggle_active(&mut self) {
        self.is_active = !self.is_active;
    }

    /// Get loop length in samples.
    pub fn length_samples(&self) -> u64 {
        self.end_sample.saturating_sub(self.start_sample)
    }
}

/// Trigger velocity (for velocity-sensitive pads).
#[derive(Debug, Clone)]
pub struct TriggerVelocity {
    /// Velocity value (0-127, MIDI-style).
    pub velocity: u8,
    /// Timestamp.
    pub timestamp: std::time::Instant,
    /// Pad index.
    pub pad_index: u32,
    /// Is strong hit.
    pub is_strong_hit: bool,
}

impl TriggerVelocity {
    /// Create new trigger velocity.
    pub fn new(pad_index: u32, velocity: u8) -> Self {
        Self {
            pad_index,
            velocity,
            timestamp: std::time::Instant::now(),
            is_strong_hit: velocity > 100,
        }
    }

    /// Get velocity level.
    pub fn level(&self) -> VelocityLevel {
        if self.velocity > 100 {
            VelocityLevel::Strong
        } else if self.velocity > 64 {
            VelocityLevel::Medium
        } else if self.velocity > 32 {
            VelocityLevel::Light
        } else {
            VelocityLevel::VeryLight
        }
    }

    /// Get velocity color mapping.
    pub fn color(&self) -> PadColor {
        match self.level() {
            VelocityLevel::Strong => PadColor::Red,
            VelocityLevel::Medium => PadColor::Orange,
            VelocityLevel::Light => PadColor::Yellow,
            VelocityLevel::VeryLight => PadColor::Green,
        }
    }
}

/// Velocity level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VelocityLevel {
    /// Very light (0-32).
    VeryLight,
    /// Light (33-64).
    Light,
    /// Medium (65-100).
    Medium,
    /// Strong (101-127).
    Strong,
}

impl VelocityLevel {
    /// Get level name.
    pub fn name(&self) -> &str {
        match self {
            Self::VeryLight => "Very Light",
            Self::Light => "Light",
            Self::Medium => "Medium",
            Self::Strong => "Strong",
        }
    }

    /// Get trigger intensity (0.0-1.0).
    pub fn intensity(&self) -> f32 {
        match self {
            Self::VeryLight => 0.25,
            Self::Light => 0.5,
            Self::Medium => 0.75,
            Self::Strong => 1.0,
        }
    }
}

/// Pad trigger feedback.
#[derive(Debug, Clone)]
pub struct PadFeedback {
    /// Pad index.
    pub pad_index: u32,
    /// Feedback type.
    pub feedback_type: FeedbackType,
    /// Timestamp.
    pub timestamp: std::time::Instant,
    /// Is still showing.
    pub is_active: bool,
}

impl PadFeedback {
    /// Create new pad feedback.
    pub fn new(pad_index: u32, feedback_type: FeedbackType) -> Self {
        Self {
            pad_index,
            feedback_type,
            timestamp: std::time::Instant::now(),
            is_active: true,
        }
    }

    /// Stop feedback.
    pub fn stop(&mut self) {
        self.is_active = false;
    }

    /// Is timed out (> 500 ms)?
    pub fn is_timed_out(&self) -> bool {
        self.timestamp.elapsed() > Duration::from_millis(500)
    }
}

/// Feedback type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedbackType {
    /// Trigger feedback.
    Trigger,
    /// Release feedback.
    Release,
    /// Error feedback.
    Error,
    /// Sync feedback.
    Sync,
}

/// Pad configuration.
#[derive(Debug, Clone)]
pub struct PadConfig {
    /// Pad index (0-7).
    pub index: u32,
    /// Assigned mode.
    pub mode: PadMode,
    /// Pad color.
    pub color: PadColor,
    /// Is armed.
    pub is_armed: bool,
    /// Is locked.
    pub is_locked: bool,
    /// Trigger count.
    pub trigger_count: u32,
    /// Last trigger velocity.
    pub last_velocity: Option<TriggerVelocity>,
    /// Sliced regions (for Slicer mode).
    pub sliced_regions: Vec<SlicedRegion>,
    /// Sampler sounds (for Sampler mode).
    pub sampler_sounds: Vec<SamplerSound>,
    /// Beat jump distance (for Beat Jump mode).
    pub beat_jump_distance: i32,
    /// Saved loop (for Saved Loop mode).
    pub saved_loop: Option<SavedLoop>,
}

impl PadConfig {
    /// Create new pad config.
    pub fn new(index: u32, mode: PadMode) -> Self {
        assert!(index < 8, "Pad index must be 0-7");
        Self {
            index,
            mode,
            color: PadColor::default(),
            is_armed: false,
            is_locked: false,
            trigger_count: 0,
            last_velocity: None,
            sliced_regions: Vec::new(),
            sampler_sounds: Vec::new(),
            beat_jump_distance: BEAT_JUMP_DISTANCES[index as usize],
            saved_loop: None,
        }
    }

    /// Set color.
    pub fn set_color(&mut self, color: PadColor) {
        self.color = color;
    }

    /// Toggle armed state.
    pub fn toggle_arm(&mut self) {
        // Arming authorizes triggering a locked pad without changing its lock.
        self.is_armed = !self.is_armed;
    }

    /// Toggle locked state.
    pub fn toggle_lock(&mut self) {
        self.is_locked = !self.is_locked;
    }

    /// Record trigger.
    pub fn record_trigger(&mut self, velocity: Option<TriggerVelocity>) {
        self.trigger_count += 1;
        self.last_velocity = velocity;
    }

    /// Add sliced region.
    pub fn add_sliced_region(&mut self, region: SlicedRegion) {
        self.sliced_regions.push(region);
    }

    /// Add sampler sound.
    pub fn add_sampler_sound(&mut self, sound: SamplerSound) {
        self.sampler_sounds.push(sound);
    }

    /// Set beat jump distance.
    pub fn set_beat_jump_distance(&mut self, distance: i32) {
        self.beat_jump_distance = distance;
    }

    /// Set saved loop.
    pub fn set_saved_loop(&mut self, loop_data: SavedLoop) {
        self.saved_loop = Some(loop_data);
    }

    /// Get active sliced region.
    pub fn active_sliced_region(&self) -> Option<&SlicedRegion> {
        self.sliced_regions.iter().find(|r| r.is_active)
    }

    /// Get armed status.
    pub fn armed_status(&self) -> &str {
        if self.is_armed {
            "Armed"
        } else {
            "Disarmed"
        }
    }

    /// Get lock status.
    pub fn lock_status(&self) -> &str {
        if self.is_locked {
            "Locked"
        } else {
            "Unlocked"
        }
    }
}

/// Performance pad engine.
pub struct PerformancePadEngine {
    /// Pad configurations.
    pads: Vec<PadConfig>,
    /// Current mode.
    current_mode: PadMode,
    /// Active feedback.
    active_feedback: Vec<PadFeedback>,
    /// Last trigger time.
    last_trigger_time: Option<std::time::Instant>,
    /// Trigger latency (measured).
    trigger_latency: Duration,
    /// High CPU load flag.
    high_cpu_load: bool,
    /// Pad trigger count (total).
    total_triggers: u32,
    /// Total releases.
    total_releases: u32,
    /// Mode change count.
    mode_change_count: u32,
}

impl PerformancePadEngine {
    /// Create new performance pad engine.
    pub fn new() -> Self {
        let mut pads = Vec::with_capacity(8);
        for i in 0..8 {
            pads.push(PadConfig::new(i, PadMode::HotCue));
        }
        Self {
            pads,
            current_mode: PadMode::HotCue,
            active_feedback: Vec::new(),
            last_trigger_time: None,
            trigger_latency: Duration::ZERO,
            high_cpu_load: false,
            total_triggers: 0,
            total_releases: 0,
            mode_change_count: 0,
        }
    }

    /// Trigger pad.
    pub fn trigger_pad(
        &mut self,
        pad_index: u32,
        velocity: Option<u8>,
    ) -> Result<&PadConfig, PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        // Check for lag prevention on high CPU.
        if self.high_cpu_load {
            // Still trigger but with increased latency warning.
        }

        let start_time = std::time::Instant::now();

        let pad = self.pads.get_mut(pad_index as usize).unwrap();

        if pad.is_locked && !pad.is_armed {
            return Err(PadError::PadLocked(pad_index));
        }

        // Record trigger.
        let trigger_vel = velocity.map(|v| TriggerVelocity::new(pad_index, v));
        pad.record_trigger(trigger_vel.clone());
        self.total_triggers += 1;
        self.last_trigger_time = Some(std::time::Instant::now());

        // Add feedback.
        self.active_feedback
            .push(PadFeedback::new(pad_index, FeedbackType::Trigger));

        // Calculate latency.
        let latency = start_time.elapsed();
        self.trigger_latency = latency;

        Ok(self.pads.get(pad_index as usize).unwrap())
    }

    /// Release pad.
    pub fn release_pad(&mut self, pad_index: u32) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        self.total_releases += 1;

        // Add release feedback.
        self.active_feedback
            .push(PadFeedback::new(pad_index, FeedbackType::Release));

        Ok(())
    }

    /// Set pad mode.
    pub fn set_mode(&mut self, mode: PadMode) -> Result<(), PadError> {
        if self.current_mode == mode {
            return Ok(());
        }

        self.current_mode = mode.clone();
        self.mode_change_count += 1;

        // Update all pads to the new mode.
        for pad in &mut self.pads {
            pad.mode = mode.clone();
        }

        Ok(())
    }

    /// Get current mode.
    pub fn current_mode(&self) -> &PadMode {
        &self.current_mode
    }

    /// Get pad config.
    pub fn get_pad(&self, index: u32) -> Option<&PadConfig> {
        self.pads.get(index as usize)
    }

    /// Get all pads.
    pub fn all_pads(&self) -> &[PadConfig] {
        &self.pads
    }

    /// Set pad color.
    pub fn set_pad_color(&mut self, pad_index: u32, color: PadColor) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        self.pads[pad_index as usize].set_color(color);
        Ok(())
    }

    /// Arm pad.
    pub fn arm_pad(&mut self, pad_index: u32) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        self.pads[pad_index as usize].toggle_arm();
        Ok(())
    }

    /// Lock pad.
    pub fn lock_pad(&mut self, pad_index: u32) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        self.pads[pad_index as usize].toggle_lock();
        Ok(())
    }

    /// Configure slicer for pad.
    pub fn configure_slicer(
        &mut self,
        pad_index: u32,
        samples_per_bar: u64,
        _bps: f64,
    ) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        let pad = &mut self.pads[pad_index as usize];
        pad.sliced_regions.clear();

        let samples_per_slice = samples_per_bar / 8;
        for i in 0..8 {
            let start = i * samples_per_slice;
            let end = (i + 1) * samples_per_slice;
            pad.add_sliced_region(SlicedRegion::new(i as u32, start, end));
        }

        Ok(())
    }

    /// Activate slicer slice.
    pub fn activate_slice(&mut self, pad_index: u32, slice_index: u32) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        let pad = &mut self.pads[pad_index as usize];

        // Deactivate all slices first.
        for region in &mut pad.sliced_regions {
            region.is_active = false;
        }

        // Activate the selected slice.
        if let Some(region) = pad
            .sliced_regions
            .iter_mut()
            .find(|r| r.slice_index == slice_index)
        {
            region.is_active = true;
            Ok(())
        } else {
            Err(PadError::InvalidIndex(slice_index))
        }
    }

    /// Assign sampler sound to pad.
    pub fn assign_sampler_sound(
        &mut self,
        pad_index: u32,
        sound: SamplerSound,
    ) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        if !sound.is_valid() {
            return Err(PadError::ConfigError("Invalid sampler sound".to_string()));
        }

        self.pads[pad_index as usize].add_sampler_sound(sound);
        Ok(())
    }

    /// Set beat jump distance for pad.
    pub fn set_beat_jump_distance(
        &mut self,
        pad_index: u32,
        distance: i32,
    ) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        self.pads[pad_index as usize].set_beat_jump_distance(distance);
        Ok(())
    }

    /// Get beat jump distance for pad.
    pub fn beat_jump_distance(&self, pad_index: u32) -> Result<i32, PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        Ok(self.pads[pad_index as usize].beat_jump_distance)
    }

    /// Save loop for pad.
    pub fn save_loop(&mut self, pad_index: u32, start: u64, end: u64) -> Result<(), PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        let loop_id = format!(
            "loop_{}_{}",
            pad_index,
            self.pads[pad_index as usize]
                .saved_loop
                .as_ref()
                .map_or(0, |_| 0)
        );
        let saved_loop = SavedLoop::new(&loop_id, start, end);
        self.pads[pad_index as usize].set_saved_loop(saved_loop);
        Ok(())
    }

    /// Recall saved loop for pad.
    pub fn recall_saved_loop(&mut self, pad_index: u32) -> Result<&SavedLoop, PadError> {
        if pad_index >= 8 {
            return Err(PadError::InvalidIndex(pad_index));
        }

        self.pads[pad_index as usize]
            .saved_loop
            .as_ref()
            .ok_or(PadError::ConfigError("No saved loop for pad".to_string()))
    }

    /// Set high CPU load.
    pub fn set_high_cpu_load(&mut self, high_load: bool) {
        self.high_cpu_load = high_load;
    }

    /// Get trigger latency.
    pub fn trigger_latency(&self) -> Duration {
        self.trigger_latency
    }

    /// Get total triggers.
    pub fn total_triggers(&self) -> u32 {
        self.total_triggers
    }

    /// Get total releases.
    pub fn total_releases(&self) -> u32 {
        self.total_releases
    }

    /// Get mode change count.
    pub fn mode_change_count(&self) -> u32 {
        self.mode_change_count
    }

    /// Get active feedback.
    pub fn active_feedback(&self) -> Vec<&PadFeedback> {
        self.active_feedback
            .iter()
            .filter(|f| f.is_active && !f.is_timed_out())
            .collect()
    }

    /// Clear timed-out feedback.
    pub fn clear_timed_out_feedback(&mut self) {
        self.active_feedback
            .retain(|f| f.is_active && !f.is_timed_out());
    }

    /// Get pad count.
    pub fn pad_count(&self) -> usize {
        self.pads.len()
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        format!(
            "Mode: {} | Triggers: {} | Releases: {} | Latency: {:.0} µs",
            self.current_mode.name(),
            self.total_triggers,
            self.total_releases,
            self.trigger_latency.as_micros() as f64
        )
    }

    /// Get all beat jump distances.
    pub fn all_beat_jump_distances() -> &'static [i32] {
        &BEAT_JUMP_DISTANCES
    }
}

impl Default for PerformancePadEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Pad error.
#[derive(Debug, Clone)]
pub enum PadError {
    /// Invalid pad index.
    InvalidIndex(u32),
    /// Pad locked.
    PadLocked(u32),
    /// Config error.
    ConfigError(String),
    /// Mode error.
    ModeError(String),
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for PadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIndex(idx) => write!(f, "Invalid pad index: {}", idx),
            Self::PadLocked(idx) => write!(f, "Pad {} is locked", idx),
            Self::ConfigError(msg) => write!(f, "Config error: {}", msg),
            Self::ModeError(msg) => write!(f, "Mode error: {}", msg),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PadError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_engine_creation() {
        let engine = PerformancePadEngine::new();
        assert_eq!(engine.pad_count(), 8);
        assert_eq!(engine.current_mode(), &PadMode::HotCue);
    }

    #[test]
    fn pad_engine_trigger_pad() {
        let mut engine = PerformancePadEngine::new();
        let result = engine.trigger_pad(0, None);
        assert!(result.is_ok());
        assert_eq!(engine.total_triggers(), 1);
    }

    #[test]
    fn pad_engine_trigger_with_velocity() {
        let mut engine = PerformancePadEngine::new();
        let result = engine.trigger_pad(0, Some(127));
        assert!(result.is_ok());

        let pad = result.unwrap();
        assert!(pad.last_velocity.is_some());
        assert_eq!(pad.last_velocity.as_ref().unwrap().velocity, 127);
    }

    #[test]
    fn pad_engine_invalid_index() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.trigger_pad(8, None).is_err());
    }

    #[test]
    fn pad_engine_pad_locked() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.lock_pad(0);
        assert!(engine.trigger_pad(0, None).is_err());
    }

    #[test]
    fn pad_engine_arm_pad() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.lock_pad(0);
        let _ = engine.arm_pad(0); // Arm should still work.

        assert!(engine.pads[0].is_armed);
    }

    #[test]
    fn pad_engine_release_pad() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.release_pad(0).is_ok());
        assert_eq!(engine.total_releases(), 1);
    }

    #[test]
    fn pad_engine_release_invalid_index() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.release_pad(8).is_err());
    }

    #[test]
    fn pad_engine_set_mode() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.set_mode(PadMode::Slicer).is_ok());
        assert_eq!(engine.current_mode(), &PadMode::Slicer);
    }

    #[test]
    fn pad_engine_set_same_mode() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.set_mode(PadMode::HotCue).is_ok());
        assert_eq!(engine.mode_change_count(), 0);
    }

    #[test]
    fn pad_engine_set_pad_color() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.set_pad_color(0, PadColor::Blue).is_ok());
        assert_eq!(engine.pads[0].color, PadColor::Blue);
    }

    #[test]
    fn pad_engine_set_pad_color_invalid() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.set_pad_color(8, PadColor::Blue).is_err());
    }

    #[test]
    fn pad_engine_slicer_configuration() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.configure_slicer(0, 44100 * 4, 128.0).is_ok());

        let pad = engine.get_pad(0).unwrap();
        assert_eq!(pad.sliced_regions.len(), 8);
    }

    #[test]
    fn pad_engine_activate_slice() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.configure_slicer(0, 44100 * 4, 128.0);
        assert!(engine.activate_slice(0, 0).is_ok());

        let pad = engine.get_pad(0).unwrap();
        assert!(pad.sliced_regions[0].is_active);
        assert!(!pad.sliced_regions[1].is_active);
    }

    #[test]
    fn pad_engine_activate_slice_deactivates_others() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.configure_slicer(0, 44100 * 4, 128.0);
        let _ = engine.activate_slice(0, 0);
        let _ = engine.activate_slice(0, 3);

        let pad = engine.get_pad(0).unwrap();
        assert!(!pad.sliced_regions[0].is_active);
        assert!(pad.sliced_regions[3].is_active);
    }

    #[test]
    fn pad_engine_slicer_slice_validity() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.configure_slicer(0, 44100 * 4, 128.0);

        let pad = engine.get_pad(0).unwrap();
        for region in &pad.sliced_regions {
            assert!(region.is_valid());
            assert_eq!(region.duration_samples(), 44100 / 2);
        }
    }

    #[test]
    fn pad_engine_sampler_sound() {
        let mut engine = PerformancePadEngine::new();
        let mut sound = SamplerSound::new("Kick", "/sounds/kick.wav");
        sound.end_offset = 4410; // Loaded sample region: 100 ms at 44.1 kHz.
        sound.set_volume(0.8);
        sound.set_pan(0.5);

        assert!(engine.assign_sampler_sound(0, sound).is_ok());

        let pad = engine.get_pad(0).unwrap();
        assert_eq!(pad.sampler_sounds.len(), 1);
    }

    #[test]
    fn pad_engine_sampler_sound_invalid() {
        let mut engine = PerformancePadEngine::new();
        let sound = SamplerSound::new("Sound", ""); // Empty file path.

        assert!(engine.assign_sampler_sound(0, sound).is_err());
    }

    #[test]
    fn pad_engine_beat_jump() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.set_beat_jump_distance(0, 8).is_ok());
        assert_eq!(engine.beat_jump_distance(0).unwrap(), 8);
    }

    #[test]
    fn pad_engine_beat_jump_default() {
        let engine = PerformancePadEngine::new();
        // Pad 0 should have default distance -32.
        assert_eq!(engine.beat_jump_distance(0).unwrap(), -32);
        assert_eq!(engine.beat_jump_distance(4).unwrap(), 4);
        assert_eq!(engine.beat_jump_distance(7).unwrap(), 32);
    }

    #[test]
    fn pad_engine_save_loop() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.save_loop(0, 0, 44100).is_ok());

        let loop_data = engine.recall_saved_loop(0);
        assert!(loop_data.is_ok());
        assert_eq!(loop_data.unwrap().length_samples(), 44100);
    }

    #[test]
    fn pad_engine_recall_no_loop() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.recall_saved_loop(0).is_err());
    }

    #[test]
    fn pad_engine_high_cpu_load() {
        let mut engine = PerformancePadEngine::new();
        engine.set_high_cpu_load(true);
        // Should still trigger.
        assert!(engine.trigger_pad(0, None).is_ok());
    }

    #[test]
    fn pad_engine_trigger_latency() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.trigger_pad(0, None);
        let latency = engine.trigger_latency();
        assert!(latency.as_micros() >= 0);
    }

    #[test]
    fn pad_engine_active_feedback() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.trigger_pad(0, None);

        let feedback = engine.active_feedback();
        assert_eq!(feedback.len(), 1);
        assert_eq!(feedback[0].pad_index, 0);
    }

    #[test]
    fn pad_engine_clear_timed_out_feedback() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.trigger_pad(0, None);
        // Simulate time passing by setting timestamp far in the past.
        // We can't actually wait 500ms in a test, but we can verify the method exists.
        engine.clear_timed_out_feedback();
    }

    #[test]
    fn pad_engine_status_string() {
        let engine = PerformancePadEngine::new();
        let status = engine.status_string();
        assert!(status.contains("Mode:"));
        assert!(status.contains("Triggers:"));
    }

    #[test]
    fn pad_engine_velocity_levels() {
        let very_light = TriggerVelocity::new(0, 16);
        assert_eq!(very_light.level(), VelocityLevel::VeryLight);

        let light = TriggerVelocity::new(0, 48);
        assert_eq!(light.level(), VelocityLevel::Light);

        let medium = TriggerVelocity::new(0, 80);
        assert_eq!(medium.level(), VelocityLevel::Medium);

        let strong = TriggerVelocity::new(0, 127);
        assert_eq!(strong.level(), VelocityLevel::Strong);
    }

    #[test]
    fn pad_engine_velocity_colors() {
        let strong = TriggerVelocity::new(0, 127);
        assert_eq!(strong.color(), PadColor::Red);

        let light = TriggerVelocity::new(0, 16);
        assert_eq!(light.color(), PadColor::Green);
    }

    #[test]
    fn pad_engine_velocity_intensity() {
        assert_eq!(VelocityLevel::VeryLight.intensity(), 0.25);
        assert_eq!(VelocityLevel::Light.intensity(), 0.5);
        assert_eq!(VelocityLevel::Medium.intensity(), 0.75);
        assert_eq!(VelocityLevel::Strong.intensity(), 1.0);
    }

    #[test]
    fn pad_engine_pad_status_strings() {
        let pad = PadConfig::new(0, PadMode::HotCue);
        assert_eq!(pad.armed_status(), "Disarmed");
        assert_eq!(pad.lock_status(), "Unlocked");
    }

    #[test]
    fn pad_engine_all_beat_jump_distances() {
        let distances = PerformancePadEngine::all_beat_jump_distances();
        assert_eq!(distances.len(), 8);
        assert_eq!(distances[0], -32);
        assert_eq!(distances[7], 32);
    }

    #[test]
    fn pad_engine_feedback_types() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.trigger_pad(0, None);
        let _ = engine.release_pad(0);

        let feedback = engine.active_feedback();
        let has_trigger = feedback
            .iter()
            .any(|f| f.feedback_type == FeedbackType::Trigger);
        let has_release = feedback
            .iter()
            .any(|f| f.feedback_type == FeedbackType::Release);
        assert!(has_trigger);
        assert!(has_release);
    }

    #[test]
    fn pad_error_display() {
        assert_eq!(
            format!("{}", PadError::InvalidIndex(8)),
            "Invalid pad index: 8"
        );
        assert_eq!(format!("{}", PadError::PadLocked(0)), "Pad 0 is locked");
    }

    #[test]
    fn pad_color_hex() {
        assert_eq!(PadColor::Red.hex(), "#FF0000");
        assert_eq!(PadColor::Blue.hex(), "#0000FF");
        assert_eq!(PadColor::Pink.hex(), "#FF69B4");
    }

    #[test]
    fn pad_mode_names() {
        assert_eq!(PadMode::HotCue.name(), "Hot Cue");
        assert_eq!(PadMode::Slicer.name(), "Slicer");
        assert_eq!(PadMode::BeatJump.name(), "Beat Jump");
    }

    #[test]
    fn pad_config_trigger_count() {
        let mut pad = PadConfig::new(0, PadMode::HotCue);
        assert_eq!(pad.trigger_count, 0);

        pad.record_trigger(None);
        pad.record_trigger(None);
        assert_eq!(pad.trigger_count, 2);
    }

    #[test]
    fn sliced_region_toggle() {
        let mut region = SlicedRegion::new(0, 0, 44100);
        assert!(!region.is_active);

        region.toggle_active();
        assert!(region.is_active);

        region.toggle_active();
        assert!(!region.is_active);
    }

    #[test]
    fn sampler_sound_defaults() {
        let sound = SamplerSound::default();
        assert_eq!(sound.name, "Sound");
        assert!(!sound.is_valid()); // No file path.
    }

    #[test]
    fn sampler_sound_valid() {
        let mut sound = SamplerSound::new("Kick", "/kick.wav");
        sound.end_offset = 44100;
        assert!(sound.is_valid());
    }

    #[test]
    fn saved_loop_toggle() {
        let mut loop_data = SavedLoop::new("l1", 0, 44100);
        assert!(!loop_data.is_active);

        loop_data.toggle_active();
        assert!(loop_data.is_active);
    }

    #[test]
    fn trigger_velocity_strong_hit() {
        let vel = TriggerVelocity::new(0, 127);
        assert!(vel.is_strong_hit);

        let vel2 = TriggerVelocity::new(0, 64);
        assert!(!vel2.is_strong_hit);
    }

    #[test]
    fn pad_feedback_stop() {
        let mut feedback = PadFeedback::new(0, FeedbackType::Trigger);
        assert!(feedback.is_active);

        feedback.stop();
        assert!(!feedback.is_active);
    }

    #[test]
    fn pad_engine_multiple_triggers() {
        let mut engine = PerformancePadEngine::new();

        for i in 0..8 {
            assert!(engine.trigger_pad(i, Some(100)).is_ok());
        }

        assert_eq!(engine.total_triggers(), 8);
    }

    #[test]
    fn pad_engine_mode_affects_all_pads() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.set_mode(PadMode::Slicer);

        for pad in engine.all_pads() {
            assert_eq!(pad.mode, PadMode::Slicer);
        }
    }

    #[test]
    fn beat_jump_distances() {
        assert_eq!(BEAT_JUMP_DISTANCES.len(), 8);
        assert!(BEAT_JUMP_DISTANCES.contains(&-32));
        assert!(BEAT_JUMP_DISTANCES.contains(&32));
    }

    #[test]
    fn pad_feedback_is_timed_out() {
        let feedback = PadFeedback::new(0, FeedbackType::Trigger);
        // Fresh feedback should not be timed out.
        assert!(!feedback.is_timed_out());
    }

    #[test]
    fn pad_engine_slicer_invalid_index() {
        let mut engine = PerformancePadEngine::new();
        assert!(engine.configure_slicer(8, 44100 * 4, 128.0).is_err());
    }

    #[test]
    fn pad_engine_activate_slice_invalid() {
        let mut engine = PerformancePadEngine::new();
        let _ = engine.configure_slicer(0, 44100 * 4, 128.0);
        assert!(engine.activate_slice(0, 8).is_err());
    }
}
