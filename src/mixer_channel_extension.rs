//! Mixer channel extension — per-channel EQ presets,
//! automated fade/curve, channel grouping, and
//! channel state history/undo.
//! Extends P20a (mixer channel settings, metering, limiter).
//!
//! ## Design principles
//!
//! * **Undo-friendly** — Every setting change is logged.
//! * **EQ presets** — User-selectable EQ curves per channel.
//! * **Grouping** — Channels can be grouped for sync-adjustments.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::mixer_channel::{ChannelRole, ChannelSettings};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// EQ preset curve for a mixer channel.
#[derive(Debug, Clone, PartialEq)]
pub enum EqPreset {
    /// Flat, no EQ.
    Flat,
    /// Bass boost.
    BassBoost,
    /// Treble boost.
    TrebleBoost,
    /// Vocal cut.
    VocalCut,
    /// Lo-Fi (low + high cut).
    LoFi,
    /// Club (sub-bass + high shelf).
    Club,
    /// Custom user curve.
    Custom(f32, f32, f32), // (low, mid, high)
}

impl EqPreset {
    /// Get default EQ gains for the preset.
    pub fn gains(&self) -> (f32, f32, f32) {
        match self {
            Self::Flat => (0.0, 0.0, 0.0),
            Self::BassBoost => (4.0, 0.0, -1.0),
            Self::TrebleBoost => (-1.0, 0.0, 4.0),
            Self::VocalCut => (0.0, -3.0, 0.0),
            Self::LoFi => (3.0, -2.0, -4.0),
            Self::Club => (2.0, 0.0, 2.0),
            Self::Custom(low, mid, high) => (*low, *mid, *high),
        }
    }

    /// Display label.
    pub fn label(&self) -> &str {
        match self {
            Self::Flat => "Flat",
            Self::BassBoost => "Bass Boost",
            Self::TrebleBoost => "Treble Boost",
            Self::VocalCut => "Vocal Cut",
            Self::LoFi => "Lo-Fi",
            Self::Club => "Club",
            Self::Custom(_, _, _) => "Custom",
        }
    }
}

/// Fade curve type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeCurve {
    /// Linear fade.
    Linear,
    /// Equal-power (sinusoidal).
    EqualPower,
    /// Slow-in, fast-out (S-curve).
    SCurve,
}

impl FadeCurve {
    /// Compute fade factor for a linear position [0.0, 1.0].
    pub fn apply(&self, pos: f32) -> f32 {
        match self {
            Self::Linear => pos,
            Self::EqualPower => ((1.0 - pos) * std::f32::consts::PI / 2.0).sin().powi(2),
            Self::SCurve => {
                // Simple S-curve approximation.
                pos * pos * (3.0 - 2.0 * pos)
            }
        }
    }
}

/// Channel fade transition.
#[derive(Debug, Clone)]
pub struct FadeTransition {
    /// Start level (0.0-1.0).
    pub start_level: f32,
    /// Target level (0.0-1.0).
    pub target_level: f32,
    /// Duration.
    pub duration: Duration,
    /// Curve type.
    pub curve: FadeCurve,
    /// Start time.
    pub start_time: Instant,
    /// Active.
    pub active: bool,
}

impl FadeTransition {
    /// Create a new fade transition.
    pub fn new(start_level: f32, target_level: f32, duration: Duration, curve: FadeCurve) -> Self {
        Self {
            start_level: start_level.clamp(0.0, 1.0),
            target_level: target_level.clamp(0.0, 1.0),
            duration,
            curve,
            start_time: Instant::now(),
            active: true,
        }
    }

    /// Get current interpolated level.
    pub fn current_level(&self) -> f32 {
        if !self.active {
            return self.target_level;
        }

        let elapsed = self.start_time.elapsed();
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        let clamped_t = t.min(1.0);

        let fade = self.curve.apply(clamped_t);
        self.start_level + (self.target_level - self.start_level) * fade
    }

    /// Check if transition is complete.
    pub fn is_complete(&self) -> bool {
        !self.active || self.start_time.elapsed() >= self.duration
    }
}

/// Channel setting change log entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelSettingChange {
    /// Timestamp.
    pub timestamp: std::time::SystemTime,
    /// Setting that changed.
    pub setting: String,
    /// Old value (display string).
    pub old_value: String,
    /// New value (display string).
    pub new_value: String,
    /// Undo token.
    pub undo_token: u64,
}

/// Undo/redo stack for a mixer channel.
pub struct ChannelUndoStack {
    /// History entries.
    history: Vec<ChannelSettingChange>,
    /// Current index in history (for undo/redo).
    index: usize,
    /// Maximum history length.
    max_length: usize,
    /// Next undo token.
    next_token: AtomicU64,
}

impl ChannelUndoStack {
    pub fn new(max_length: usize) -> Self {
        Self {
            history: Vec::new(),
            index: 0,
            max_length,
            next_token: AtomicU64::new(0),
        }
    }

    /// Record a setting change.
    pub fn record(&mut self, setting: &str, old_value: &str, new_value: &str) {
        let token = self.next_token.fetch_add(1, Ordering::Relaxed);
        let entry = ChannelSettingChange {
            timestamp: std::time::SystemTime::now(),
            setting: setting.to_string(),
            old_value: old_value.to_string(),
            new_value: new_value.to_string(),
            undo_token: token,
        };

        // Discard redo entries.
        if self.index < self.history.len() {
            self.history.truncate(self.index);
        }

        self.history.push(entry);
        self.index = self.history.len();

        // Trim old entries.
        if self.history.len() > self.max_length {
            let excess = self.history.len() - self.max_length;
            self.history.drain(..excess);
            self.index = self.history.len();
        }
    }

    /// Undo the last change.
    pub fn undo(&mut self) -> Option<&ChannelSettingChange> {
        if self.index == 0 {
            return None;
        }
        self.index -= 1;
        Some(&self.history[self.index])
    }

    /// Redo the last undone change.
    pub fn redo(&mut self) -> Option<&ChannelSettingChange> {
        if self.index >= self.history.len() {
            return None;
        }
        let entry = &self.history[self.index];
        self.index += 1;
        Some(entry)
    }

    /// Get current undo index.
    pub fn current_index(&self) -> usize {
        self.index
    }

    /// Get history length.
    pub fn length(&self) -> usize {
        self.history.len()
    }

    /// Check if undo is possible.
    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    /// Check if redo is possible.
    pub fn can_redo(&self) -> bool {
        self.index < self.history.len()
    }

    /// Get all history entries.
    pub fn all_entries(&self) -> &[ChannelSettingChange] {
        &self.history
    }
}

/// Channel grouping.
#[derive(Debug, Clone)]
pub struct ChannelGroup {
    /// Group name (e.g. "Decks 1&2").
    pub name: String,
    /// Channel roles in the group.
    pub channel_roles: Vec<ChannelRole>,
    /// Master gain for the group.
    pub master_gain: f32,
    /// Link EQ across channels.
    pub link_eq: bool,
    /// Link fader across channels.
    pub link_fader: bool,
}

impl ChannelGroup {
    /// Create a new channel group.
    pub fn new(name: &str, channels: Vec<ChannelRole>) -> Self {
        Self {
            name: name.to_string(),
            channel_roles: channels,
            master_gain: 1.0,
            link_eq: false,
            link_fader: false,
        }
    }

    /// Apply group EQ to channel settings.
    pub fn apply_eq(
        &self,
        eq_low: f32,
        eq_mid: f32,
        eq_high: f32,
        settings: &mut [ChannelSettings],
    ) {
        for role in &self.channel_roles {
            for setting in settings.iter_mut() {
                if setting.role == *role {
                    setting.eq_low = eq_low;
                    setting.eq_mid = eq_mid;
                    setting.eq_high = eq_high;
                }
            }
        }
    }

    /// Apply group fader to channel settings.
    pub fn apply_fader(&self, gain: f32, settings: &mut [ChannelSettings]) {
        let clamped = gain * self.master_gain;
        for role in &self.channel_roles {
            for setting in settings.iter_mut() {
                if setting.role == *role {
                    setting.set_volume(clamped);
                }
            }
        }
    }
}

/// Mixer channel extension.
pub struct MixerChannelExtension {
    /// EQ presets per deck.
    eq_presets: std::sync::Mutex<HashMap<u8, EqPreset>>,
    /// Channel undo stacks.
    undo_stacks: std::sync::Mutex<HashMap<u8, ChannelUndoStack>>,
    /// Channel groups.
    groups: std::sync::Mutex<Vec<ChannelGroup>>,
    /// Active fade transitions.
    fades: std::sync::Mutex<HashMap<u8, FadeTransition>>,
    /// Setting change count.
    change_count: AtomicU64,
}

impl MixerChannelExtension {
    pub fn new() -> Self {
        Self {
            eq_presets: std::sync::Mutex::new(HashMap::new()),
            undo_stacks: std::sync::Mutex::new(HashMap::new()),
            groups: std::sync::Mutex::new(Vec::new()),
            fades: std::sync::Mutex::new(HashMap::new()),
            change_count: AtomicU64::new(0),
        }
    }

    /// Set EQ preset for a channel.
    pub fn set_eq_preset(&self, channel_id: u8, preset: EqPreset, settings: &mut ChannelSettings) {
        let (low, mid, high) = preset.gains();
        let old = settings.eq_string();
        settings.eq_low = low;
        settings.eq_mid = mid;
        settings.eq_high = high;
        self.record_change(channel_id, "EQ Preset", &old, preset.label());
        self.eq_presets.lock().unwrap().insert(channel_id, preset);
    }

    /// Get EQ preset for a channel.
    pub fn get_eq_preset(&self, channel_id: u8) -> Option<EqPreset> {
        self.eq_presets.lock().unwrap().get(&channel_id).cloned()
    }

    /// Start a fade transition.
    pub fn start_fade(
        &self,
        channel_id: u8,
        current_level: f32,
        target: f32,
        duration: Duration,
        curve: FadeCurve,
    ) {
        let mut fades = self.fades.lock().unwrap();
        fades.insert(
            channel_id,
            FadeTransition::new(current_level, target.clamp(0.0, 1.0), duration, curve),
        );
    }

    /// Get current faded level for a channel.
    pub fn current_faded_level(&self, channel_id: u8) -> Option<f32> {
        self.fades
            .lock()
            .unwrap()
            .get(&channel_id)
            .map(|f| f.current_level())
    }

    /// Check if any fade is active.
    pub fn any_fade_active(&self) -> bool {
        self.fades
            .lock()
            .unwrap()
            .values()
            .any(|f| !f.is_complete())
    }

    /// Update fades (call on UI timer).
    pub fn update_fades(&self) -> Vec<u8> {
        let mut done = Vec::new();
        let mut fades = self.fades.lock().unwrap();
        fades.retain(|id, fade| {
            if fade.is_complete() {
                done.push(*id);
                false
            } else {
                true
            }
        });
        done
    }

    /// Record a setting change.
    fn record_change(&self, channel_id: u8, setting: &str, old_value: &str, new_value: &str) {
        if let Some(stack) = self.undo_stacks.lock().unwrap().get_mut(&channel_id) {
            stack.record(setting, old_value, new_value);
        }
        self.change_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a change manually (for external callers).
    pub fn record_change_manual(
        &self,
        channel_id: u8,
        setting: &str,
        old_value: &str,
        new_value: &str,
    ) {
        self.record_change(channel_id, setting, old_value, new_value);
    }

    /// Get undo stack for a channel.
    pub fn undo_stack(&self, channel_id: u8) -> Option<ChannelUndoStack> {
        self.undo_stacks.lock().unwrap().get(&channel_id).cloned()
    }

    /// Create undo stack for a channel.
    pub fn create_undo_stack(&self, channel_id: u8, max_length: usize) {
        self.undo_stacks
            .lock()
            .unwrap()
            .insert(channel_id, ChannelUndoStack::new(max_length));
    }

    /// Undo last change on a channel.
    pub fn undo(&self, channel_id: u8) -> Option<ChannelSettingChange> {
        self.undo_stacks
            .lock()
            .unwrap()
            .get_mut(&channel_id)?
            .undo()
            .cloned()
    }

    /// Add a channel group.
    pub fn add_group(&self, group: ChannelGroup) {
        self.groups.lock().unwrap().push(group);
    }

    /// Remove a channel group by name.
    pub fn remove_group(&self, name: &str) -> bool {
        let mut groups = self.groups.lock().unwrap();
        let idx = groups.iter().position(|g| g.name == name);
        if let Some(idx) = idx {
            groups.remove(idx);
            true
        } else {
            false
        }
    }

    /// Get all groups.
    pub fn groups(&self) -> Vec<ChannelGroup> {
        self.groups.lock().unwrap().clone()
    }

    /// Apply a group's EQ to its channels.
    pub fn apply_group_eq(
        &self,
        group_name: &str,
        eq_low: f32,
        eq_mid: f32,
        eq_high: f32,
        settings: &mut [ChannelSettings],
    ) -> bool {
        if let Some(group) = self
            .groups
            .lock()
            .unwrap()
            .iter()
            .find(|g| g.name == group_name)
        {
            group.apply_eq(eq_low, eq_mid, eq_high, settings);
            true
        } else {
            false
        }
    }

    /// Apply a group's fader to its channels.
    pub fn apply_group_fader(
        &self,
        group_name: &str,
        gain: f32,
        settings: &mut [ChannelSettings],
    ) -> bool {
        if let Some(group) = self
            .groups
            .lock()
            .unwrap()
            .iter()
            .find(|g| g.name == group_name)
        {
            group.apply_fader(gain, settings);
            true
        } else {
            false
        }
    }

    /// Get setting change count.
    pub fn change_count(&self) -> u64 {
        self.change_count.load(Ordering::Relaxed)
    }

    /// Default undo stack length.
    const DEFAULT_UNDO_LENGTH: usize = 50;

    /// Ensure undo stacks exist for all given channel IDs.
    pub fn ensure_undo_stacks(&self, channel_ids: &[u8]) {
        for &id in channel_ids {
            if !self.undo_stacks.lock().unwrap().contains_key(&id) {
                self.create_undo_stack(id, Self::DEFAULT_UNDO_LENGTH);
            }
        }
    }
}

impl Default for MixerChannelExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ChannelUndoStack {
    fn clone(&self) -> Self {
        Self {
            history: self.history.clone(),
            index: self.index,
            max_length: self.max_length,
            next_token: AtomicU64::new(self.next_token.load(Ordering::Relaxed)),
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
    fn test_eq_preset_gains() {
        let (low, mid, high) = EqPreset::Flat.gains();
        assert_eq!(low, 0.0);
        assert_eq!(mid, 0.0);
        assert_eq!(high, 0.0);

        let (low, mid, high) = EqPreset::BassBoost.gains();
        assert!(low > 0.0);
        assert_eq!(mid, 0.0);
        assert!(high < 0.0);
    }

    #[test]
    fn test_eq_preset_labels() {
        assert_eq!(EqPreset::Flat.label(), "Flat");
        assert_eq!(EqPreset::BassBoost.label(), "Bass Boost");
        assert_eq!(EqPreset::LoFi.label(), "Lo-Fi");
    }

    #[test]
    fn test_fade_curve_linear() {
        let curve = FadeCurve::Linear;
        assert_eq!(curve.apply(0.0), 0.0);
        assert_eq!(curve.apply(0.5), 0.5);
        assert_eq!(curve.apply(1.0), 1.0);
    }

    #[test]
    fn test_fade_curve_equal_power() {
        let curve = FadeCurve::EqualPower;
        assert_eq!(curve.apply(0.0), 1.0);
        assert_eq!(curve.apply(1.0), 0.0);
        // At 0.5, should be ~0.5 (sin(pi/4)^2 = 0.5).
        assert!((curve.apply(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_fade_curve_scurve() {
        let curve = FadeCurve::SCurve;
        assert_eq!(curve.apply(0.0), 0.0);
        assert_eq!(curve.apply(0.5), 0.5);
        assert_eq!(curve.apply(1.0), 1.0);
    }

    #[test]
    fn test_fade_transition_interpolation() {
        let fade = FadeTransition::new(0.0, 1.0, Duration::from_millis(1000), FadeCurve::Linear);
        // Immediately: level should be ~0.0.
        assert!((fade.current_level() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_channel_undo_stack() {
        let mut stack = ChannelUndoStack::new(50);
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());

        stack.record("Volume", "100%", "50%");
        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        let entry = stack.undo().unwrap();
        assert_eq!(entry.setting, "Volume");
        assert_eq!(entry.old_value, "100%");
        assert_eq!(entry.new_value, "50%");

        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        let entry = stack.redo().unwrap();
        assert_eq!(entry.setting, "Volume");
    }

    #[test]
    fn test_channel_undo_stack_redo_discard() {
        let mut stack = ChannelUndoStack::new(50);
        stack.record("Volume", "100%", "80%");
        stack.record("Volume", "80%", "50%");

        stack.undo(); // back to 80%.
        stack.record("Volume", "80%", "30%"); // discards redo.

        assert!(!stack.can_redo());
    }

    #[test]
    fn test_channel_undo_max_length() {
        let mut stack = ChannelUndoStack::new(3);
        for i in 0..5 {
            stack.record(
                "Vol",
                &format!("{}%", i * 20),
                &format!("{}%", (i + 1) * 20),
            );
        }
        assert!(stack.length() <= 3);
    }

    #[test]
    fn test_channel_group_create() {
        let group = ChannelGroup::new("Decks 1&2", vec![ChannelRole::Deck1, ChannelRole::Deck2]);
        assert_eq!(group.name, "Decks 1&2");
        assert_eq!(group.master_gain, 1.0);
        assert_eq!(group.channel_roles.len(), 2);
    }

    #[test]
    fn test_channel_group_apply_eq() {
        let group = ChannelGroup::new("Decks 1&2", vec![ChannelRole::Deck1, ChannelRole::Deck2]);
        let mut settings = vec![
            ChannelSettings::new(ChannelRole::Deck1),
            ChannelSettings::new(ChannelRole::Deck2),
        ];
        group.apply_eq(2.0, -1.0, 1.0, &mut settings);
        assert_eq!(settings[0].eq_low, 2.0);
        assert_eq!(settings[1].eq_mid, -1.0);
    }

    #[test]
    fn test_mixer_channel_extension() {
        let ext = MixerChannelExtension::new();
        ext.create_undo_stack(1, 50);
        ext.set_eq_preset(
            1,
            EqPreset::BassBoost,
            &mut ChannelSettings::new(ChannelRole::Deck1),
        );
        assert_eq!(ext.get_eq_preset(1), Some(EqPreset::BassBoost));
        assert!(ext.change_count() > 0);
    }

    #[test]
    fn test_mixer_channel_extension_fade() {
        let ext = MixerChannelExtension::new();
        ext.start_fade(1, 1.0, 0.0, Duration::from_millis(500), FadeCurve::Linear);
        assert!(ext.any_fade_active());
        let level = ext.current_faded_level(1).unwrap();
        assert!((0.0..=1.0).contains(&level));
    }

    #[test]
    fn test_mixer_channel_extension_groups() {
        let ext = MixerChannelExtension::new();
        ext.add_group(ChannelGroup::new("Test", vec![ChannelRole::Deck1]));
        assert_eq!(ext.groups().len(), 1);

        assert!(ext.apply_group_eq(
            "Test",
            2.0,
            0.0,
            0.0,
            &mut [ChannelSettings::new(ChannelRole::Deck1)]
        ));
        assert!(!ext.apply_group_eq("Nonexistent", 2.0, 0.0, 0.0, &mut []));
        assert!(ext.remove_group("Test"));
        assert_eq!(ext.groups().len(), 0);
    }

    #[test]
    fn test_ensure_undo_stacks() {
        let ext = MixerChannelExtension::new();
        ext.ensure_undo_stacks(&[1, 2, 3]);
        assert!(ext.undo_stack(1).is_some());
        assert!(ext.undo_stack(2).is_some());
        assert!(ext.undo_stack(3).is_some());
        assert!(ext.undo_stack(4).is_none());
    }

    #[test]
    fn test_fade_transition_clamping() {
        let fade = FadeTransition::new(0.5, 2.0, Duration::from_secs(1), FadeCurve::Linear);
        // Target > 1.0 should be clamped.
        assert!(fade.target_level <= 1.0);
    }

    #[test]
    fn test_channel_setting_change_serialization() {
        let entry = ChannelSettingChange {
            timestamp: std::time::SystemTime::now(),
            setting: "Volume".to_string(),
            old_value: "100%".to_string(),
            new_value: "50%".to_string(),
            undo_token: 42,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: ChannelSettingChange = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.setting, "Volume");
        assert_eq!(parsed.undo_token, 42);
    }
}
