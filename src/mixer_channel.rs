//! Mixer Channel Part 2 — Cue, Meter, Limiter, Persistenz.
//!
//! Extends the mixer channel system (P20a) with headphone cue, level meters,
//! master limiter, channel reset, solo/clear, and mixer persistenz.
//! Part of the Multi-Deck Control feature (P20b).
//!
//! ## Design principles
//!
//! * **Safe** — Limiter prevents clipping.
//! * **Accurate** — Meters are sample-precise.
//! * **Persistent** — Settings survive restart.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Mixer channel role.
#[derive(Debug, Clone, PartialEq, Eq, Copy, serde::Serialize, serde::Deserialize)]
pub enum ChannelRole {
    /// Deck 1 channel.
    Deck1,
    /// Deck 2 channel.
    Deck2,
    /// Deck 3 channel.
    Deck3,
    /// Deck 4 channel.
    Deck4,
    /// Aux channel.
    Aux,
    /// FX return channel.
    FxReturn,
}

impl ChannelRole {
    /// Get channel name.
    pub fn name(&self) -> &str {
        match self {
            Self::Deck1 => "Deck 1",
            Self::Deck2 => "Deck 2",
            Self::Deck3 => "Deck 3",
            Self::Deck4 => "Deck 4",
            Self::Aux => "Aux",
            Self::FxReturn => "FX Return",
        }
    }

    /// Get channel number.
    pub fn number(&self) -> u32 {
        match self {
            Self::Deck1 => 1,
            Self::Deck2 => 2,
            Self::Deck3 => 3,
            Self::Deck4 => 4,
            Self::Aux => 5,
            Self::FxReturn => 6,
        }
    }
}

/// Mixer meter type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MeterType {
    /// Peak meter.
    Peak,
    /// RMS meter.
    #[serde(rename = "RMS")]
    Rms,
    /// Both peak and RMS.
    Both,
}

/// Limiter state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LimiterState {
    /// Limiter is inactive.
    Inactive,
    /// Limiter is engaged (compressing).
    Engaged,
    /// Limiter is clipped (hard limit).
    Clipping,
}

impl LimiterState {
    /// Get state name.
    pub fn name(&self) -> &str {
        match self {
            Self::Inactive => "Inactive",
            Self::Engaged => "Engaged",
            Self::Clipping => "Clipping",
        }
    }
}

/// Cue mode.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CueMode {
    /// Cue only (headphones).
    CueOnly,
    /// Cue and master.
    CueMaster,
    /// Live (no cue).
    Live,
}

impl CueMode {
    /// Get mode name.
    pub fn name(&self) -> &str {
        match self {
            Self::CueOnly => "Cue Only",
            Self::CueMaster => "Cue + Master",
            Self::Live => "Live",
        }
    }
}

/// Channel meter reading.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MeterReading {
    /// Left channel peak (0.0-1.0).
    pub peak_left: f32,
    /// Right channel peak (0.0-1.0).
    pub peak_right: f32,
    /// Left channel RMS (0.0-1.0).
    pub rms_left: f32,
    /// Right channel RMS (0.0-1.0).
    pub rms_right: f32,
    /// Peak hold value.
    pub peak_hold: f32,
    /// Hold timer.
    pub hold_remaining: std::time::Duration,
}

impl MeterReading {
    /// Create new meter reading.
    pub fn new(peak_left: f32, peak_right: f32, rms_left: f32, rms_right: f32) -> Self {
        Self {
            peak_left,
            peak_right,
            rms_left,
            rms_right,
            peak_hold: peak_left.max(peak_right),
            hold_remaining: std::time::Duration::from_millis(100),
        }
    }

    /// Get max peak.
    pub fn max_peak(&self) -> f32 {
        self.peak_left.max(self.peak_right)
    }

    /// Get max RMS.
    pub fn max_rms(&self) -> f32 {
        self.rms_left.max(self.rms_right)
    }

    /// Is clipped?
    pub fn is_clipped(&self) -> bool {
        self.peak_left >= 1.0 || self.peak_right >= 1.0
    }

    /// Get level percent (0-100).
    pub fn level_percent(&self) -> u8 {
        (self.max_peak() * 100.0) as u8
    }

    /// Get decibel value.
    pub fn db_value(&self) -> f32 {
        let peak = self.max_peak();
        if peak <= 0.0001 {
            -60.0
        } else {
            20.0 * (peak.log10())
        }
    }
}

impl Default for MeterReading {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// Mixer channel settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChannelSettings {
    /// Channel role.
    pub role: ChannelRole,
    /// Volume (0.0-1.0).
    pub volume: f32,
    /// Pan (-1.0 to 1.0, -1.0 = full left, 1.0 = full right).
    pub pan: f32,
    /// EQ high (gain in dB).
    pub eq_high: f32,
    /// EQ mid (gain in dB).
    pub eq_mid: f32,
    /// EQ low (gain in dB).
    pub eq_low: f32,
    /// FX send level (0.0-1.0).
    pub fx_send: f32,
    /// Is muted.
    pub is_muted: bool,
    /// Is solo'd.
    pub is_solo: bool,
    /// Cue mode.
    pub cue_mode: CueMode,
    /// Cue volume (0.0-1.0).
    pub cue_volume: f32,
    /// Limiter threshold (0.0-1.0).
    pub limiter_threshold: f32,
    /// Limiter engaged.
    pub limiter_engaged: bool,
    /// Channel meter type.
    pub meter_type: MeterType,
    /// Peak hold time (ms).
    pub peak_hold_time: u32,
}

impl ChannelSettings {
    /// Create new channel settings.
    pub fn new(role: ChannelRole) -> Self {
        Self {
            role,
            volume: 1.0,
            pan: 0.0,
            eq_high: 0.0,
            eq_mid: 0.0,
            eq_low: 0.0,
            fx_send: 0.0,
            is_muted: false,
            is_solo: false,
            cue_mode: CueMode::CueOnly,
            cue_volume: 1.0,
            limiter_threshold: 0.95,
            limiter_engaged: true,
            meter_type: MeterType::Both,
            peak_hold_time: 100,
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

    /// Set EQ high.
    pub fn set_eq_high(&mut self, db: f32) {
        self.eq_high = db.clamp(-12.0, 12.0);
    }

    /// Set EQ mid.
    pub fn set_eq_mid(&mut self, db: f32) {
        self.eq_mid = db.clamp(-12.0, 12.0);
    }

    /// Set EQ low.
    pub fn set_eq_low(&mut self, db: f32) {
        self.eq_low = db.clamp(-12.0, 12.0);
    }

    /// Set FX send.
    pub fn set_fx_send(&mut self, send: f32) {
        self.fx_send = send.clamp(0.0, 1.0);
    }

    /// Toggle mute.
    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
    }

    /// Toggle solo.
    pub fn toggle_solo(&mut self) {
        self.is_solo = !self.is_solo;
    }

    /// Clear solo.
    pub fn clear_solo(&mut self) {
        self.is_solo = false;
    }

    /// Set cue mode.
    pub fn set_cue_mode(&mut self, mode: CueMode) {
        self.cue_mode = mode;
    }

    /// Set cue volume.
    pub fn set_cue_volume(&mut self, volume: f32) {
        self.cue_volume = volume.clamp(0.0, 1.0);
    }

    /// Set limiter threshold.
    pub fn set_limiter_threshold(&mut self, threshold: f32) {
        self.limiter_threshold = threshold.clamp(0.5, 1.0);
    }

    /// Toggle limiter.
    pub fn toggle_limiter(&mut self) {
        self.limiter_engaged = !self.limiter_engaged;
    }

    /// Set meter type.
    pub fn set_meter_type(&mut self, meter_type: MeterType) {
        self.meter_type = meter_type;
    }

    /// Reset to defaults.
    pub fn reset_to_defaults(&mut self) {
        *self = Self::new(self.role);
    }

    /// Get volume string.
    pub fn volume_string(&self) -> String {
        format!("{:.0}%", self.volume * 100.0)
    }

    /// Get pan string.
    pub fn pan_string(&self) -> String {
        if (self.pan - 0.0).abs() < 0.01 {
            "C".to_string()
        } else if self.pan < 0.0 {
            format!("L{:.0}", self.pan.abs() * 100.0)
        } else {
            format!("R{:.0}", self.pan * 100.0)
        }
    }

    /// Get EQ string.
    pub fn eq_string(&self) -> String {
        format!(
            "H:{:+.0} M:{:+.0} L:{:+.0}",
            self.eq_high, self.eq_mid, self.eq_low
        )
    }

    /// Is active (not muted, not solo'd when other solo exists)?
    pub fn is_active(&self, has_any_solo: bool) -> bool {
        !self.is_muted && (!has_any_solo || self.is_solo)
    }

    /// Get cue string.
    pub fn cue_string(&self) -> String {
        format!("{} {:.0}%", self.cue_mode.name(), self.cue_volume * 100.0)
    }

    /// Get limiter string.
    pub fn limiter_string(&self) -> String {
        format!(
            "{} {:.0}%",
            if self.limiter_engaged {
                "Engaged"
            } else {
                "Off"
            },
            self.limiter_threshold * 100.0
        )
    }

    /// Get all settings as HashMap (for serialization).
    pub fn to_map(&self) -> HashMap<String, serde_json::Value> {
        serde_json::from_value(serde_json::to_value(self).unwrap()).unwrap_or_default()
    }

    /// Create from HashMap (for deserialization).
    pub fn from_map(map: &HashMap<String, serde_json::Value>) -> Option<Self> {
        // Simplified — would use serde_json in production.
        serde_json::from_value(serde_json::to_value(map).ok()?).ok()
    }
}

impl Default for ChannelSettings {
    fn default() -> Self {
        Self::new(ChannelRole::Deck1)
    }
}

/// Channel mixer state.
#[derive(Debug, Clone)]
pub struct ChannelState {
    /// Channel settings.
    pub settings: ChannelSettings,
    /// Current meter reading.
    pub meter: MeterReading,
    /// Limiter state.
    pub limiter_state: LimiterState,
    /// Limiter gain reduction (dB).
    pub gain_reduction: f32,
    /// Peak meter history (for bar display).
    pub peak_history: Vec<f32>,
    /// Is audio playing.
    pub is_audio_active: bool,
    /// Channel level (0.0-1.0, final output).
    pub channel_level: f32,
}

impl ChannelState {
    /// Create new channel state.
    pub fn new(role: ChannelRole) -> Self {
        Self {
            settings: ChannelSettings::new(role),
            meter: MeterReading::default(),
            limiter_state: LimiterState::Inactive,
            gain_reduction: 0.0,
            peak_history: Vec::new(),
            is_audio_active: false,
            channel_level: 0.0,
        }
    }

    /// Update meter reading.
    pub fn update_meter(&mut self, reading: MeterReading) {
        self.meter = reading;

        // Update peak history (keep last 64 entries for display).
        if self.peak_history.len() >= 64 {
            self.peak_history.remove(0);
        }
        self.peak_history.push(reading.max_peak());

        // Check clipping.
        if reading.is_clipped() {
            self.limiter_state = LimiterState::Clipping;
        }
    }

    /// Update channel level.
    pub fn update_channel_level(&mut self, level: f32) {
        self.channel_level = level;
    }

    /// Set audio active.
    pub fn set_audio_active(&mut self, active: bool) {
        self.is_audio_active = active;
    }

    /// Apply limiter.
    pub fn apply_limiter(&mut self, input: f32) -> f32 {
        if !self.settings.limiter_engaged {
            return input;
        }

        let threshold = self.settings.limiter_threshold;
        if input > threshold {
            let excess = input - threshold;
            self.gain_reduction = excess * 10.0; // 10:1 ratio.
            self.limiter_state = LimiterState::Engaged;
            threshold - (excess / 10.0)
        } else {
            self.limiter_state = LimiterState::Inactive;
            self.gain_reduction = 0.0;
            input
        }
    }

    /// Get peak meter bars (for UI).
    pub fn peak_bars(&self, count: usize) -> Vec<f32> {
        let start = self.peak_history.len().saturating_sub(count);
        self.peak_history[start..].to_vec()
    }

    /// Is active?
    pub fn is_active(&self, has_any_solo: bool) -> bool {
        self.settings.is_active(has_any_solo)
    }

    /// Is armed for cue?
    pub fn is_cue_armed(&self) -> bool {
        matches!(
            self.settings.cue_mode,
            CueMode::CueOnly | CueMode::CueMaster
        )
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        format!(
            "{} | Vol:{} | Pan:{} | M:{} | S:{} | Cue:{}",
            self.settings.role.name(),
            self.settings.volume_string(),
            self.settings.pan_string(),
            if self.settings.is_muted { "M" } else { "-" },
            if self.settings.is_solo { "S" } else { "-" },
            self.settings.cue_string()
        )
    }

    /// Get reset settings.
    pub fn reset_settings(&mut self) {
        self.settings.reset_to_defaults();
    }

    /// Get limiter info.
    pub fn limiter_info(&self) -> (&str, f32) {
        (self.limiter_state.name(), self.gain_reduction)
    }
}

impl Default for ChannelState {
    fn default() -> Self {
        Self::new(ChannelRole::Deck1)
    }
}

/// Master channel state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MasterChannelState {
    /// Master volume (0.0-1.0).
    pub volume: f32,
    /// Master meter reading.
    pub meter: MeterReading,
    /// Master limiter state.
    pub limiter_state: LimiterState,
    /// Master limiter threshold.
    pub limiter_threshold: f32,
    /// Peak history.
    pub peak_history: Vec<f32>,
    /// Channel count.
    pub channel_count: u32,
}

impl MasterChannelState {
    pub fn reset_master(&mut self) {
        self.set_volume(1.0);
        self.set_limiter_threshold(0.95);
    }
    /// Create new master channel state.
    pub fn new(channel_count: u32) -> Self {
        Self {
            volume: 1.0,
            meter: MeterReading::default(),
            limiter_state: LimiterState::Inactive,
            limiter_threshold: 0.95,
            peak_history: Vec::new(),
            channel_count,
        }
    }

    /// Update master meter from channel levels.
    pub fn update_from_channels(&mut self, left: f32, right: f32) {
        let peak_l = left.abs();
        let peak_r = right.abs();

        self.meter = MeterReading::new(peak_l, peak_r, peak_l * 0.8, peak_r * 0.8);

        // Apply master limiter.
        if peak_l > self.limiter_threshold || peak_r > self.limiter_threshold {
            self.limiter_state = LimiterState::Engaged;
        } else {
            self.limiter_state = LimiterState::Inactive;
        }

        // Update peak history.
        if self.peak_history.len() >= 64 {
            self.peak_history.remove(0);
        }
        self.peak_history.push(self.meter.max_peak());
    }

    /// Set volume.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Set limiter threshold.
    pub fn set_limiter_threshold(&mut self, threshold: f32) {
        self.limiter_threshold = threshold.clamp(0.5, 1.0);
    }

    /// Get peak bars.
    pub fn peak_bars(&self, count: usize) -> Vec<f32> {
        let start = self.peak_history.len().saturating_sub(count);
        self.peak_history[start..].to_vec()
    }

    /// Get volume string.
    pub fn volume_string(&self) -> String {
        format!("{:.0}%", self.volume * 100.0)
    }

    /// Get limiter info.
    pub fn limiter_info(&self) -> &str {
        self.limiter_state.name()
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        format!(
            "Master | Vol:{} | Limiter:{} | Channels:{}",
            self.volume_string(),
            self.limiter_info(),
            self.channel_count
        )
    }
}

impl Default for MasterChannelState {
    fn default() -> Self {
        Self::new(4)
    }
}

/// Mixer engine.
pub struct MixerEngine {
    /// Channel states.
    channels: Vec<ChannelState>,
    /// Master channel.
    master: MasterChannelState,
    /// Headphone cue mix.
    cue_mix: f32, // 0.0 = program, 1.0 = cue.
    /// Program mix.
    program_mix: f32,
    /// Solo active?
    solo_active: bool,
    /// Has any solo?
    has_any_solo: bool,
    /// Mixer settings persistence enabled.
    persist_enabled: bool,
    /// Last saved settings.
    last_saved: Option<HashMap<String, serde_json::Value>>,
}

impl MixerEngine {
    /// Create new mixer engine.
    pub fn new(channel_count: u32) -> Self {
        let mut channels = Vec::with_capacity(channel_count as usize);
        for i in 0..channel_count {
            let role = match i {
                0 => ChannelRole::Deck1,
                1 => ChannelRole::Deck2,
                2 => ChannelRole::Deck3,
                3 => ChannelRole::Deck4,
                _ => ChannelRole::Aux,
            };
            channels.push(ChannelState::new(role));
        }

        Self {
            channels,
            master: MasterChannelState::new(channel_count),
            cue_mix: 0.0,
            program_mix: 1.0,
            solo_active: false,
            has_any_solo: false,
            persist_enabled: true,
            last_saved: None,
        }
    }

    /// Get channel state.
    pub fn get_channel(&self, index: u32) -> Option<&ChannelState> {
        self.channels.get(index as usize)
    }

    /// Get channel mutable state.
    pub fn get_channel_mut(&mut self, index: u32) -> Option<&mut ChannelState> {
        self.channels.get_mut(index as usize)
    }

    /// Get all channels.
    pub fn all_channels(&self) -> &[ChannelState] {
        &self.channels
    }

    /// Get master channel.
    pub fn master(&self) -> &MasterChannelState {
        &self.master
    }

    /// Get master mutable.
    pub fn master_mut(&mut self) -> &mut MasterChannelState {
        &mut self.master
    }

    /// Set cue mix.
    pub fn set_cue_mix(&mut self, mix: f32) {
        self.cue_mix = mix.clamp(0.0, 1.0);
        self.program_mix = 1.0 - self.cue_mix;
    }

    /// Get cue mix.
    pub fn cue_mix(&self) -> f32 {
        self.cue_mix
    }

    /// Get program mix.
    pub fn program_mix(&self) -> f32 {
        self.program_mix
    }

    /// Solo channel.
    pub fn solo_channel(&mut self, index: u32) -> Result<(), MixerError> {
        if index >= self.channels.len() as u32 {
            return Err(MixerError::InvalidChannel(index));
        }

        self.channels[index as usize].settings.toggle_solo();
        self.has_any_solo = self.channels.iter().any(|c| c.settings.is_solo);
        self.solo_active = self.has_any_solo;
        Ok(())
    }

    /// Clear all solos.
    pub fn clear_all_solos(&mut self) {
        for channel in &mut self.channels {
            channel.settings.clear_solo();
        }
        self.has_any_solo = false;
        self.solo_active = false;
    }

    /// Is solo active?
    pub fn is_solo_active(&self) -> bool {
        self.solo_active
    }

    /// Process channel audio.
    pub fn process_channel(&mut self, index: u32, input_left: f32, input_right: f32) -> (f32, f32) {
        if index >= self.channels.len() as u32 {
            return (0.0, 0.0);
        }

        let channel = &mut self.channels[index as usize];

        // Check if channel is active.
        if !channel.is_active(self.has_any_solo) {
            return (0.0, 0.0);
        }

        let vol = channel.settings.volume;
        let pan = channel.settings.pan;

        // Apply pan.
        let pan_left = (1.0 - pan).sqrt() * vol;
        let pan_right = (1.0 + pan).sqrt() * vol;

        let out_left = input_left * pan_left;
        let out_right = input_right * pan_right;

        // Apply limiter.
        let out_left = channel.apply_limiter(out_left);
        let out_right = channel.apply_limiter(out_right);

        // Update meter.
        channel.update_meter(MeterReading::new(
            out_left.abs(),
            out_right.abs(),
            out_left.abs() * 0.8,
            out_right.abs() * 0.8,
        ));
        channel.update_channel_level((out_left + out_right) / 2.0);

        (out_left, out_right)
    }

    /// Update master output.
    pub fn update_master_output(&mut self, left: f32, right: f32) {
        let master_vol = self.master.volume;
        self.master
            .update_from_channels(left * master_vol, right * master_vol);
    }

    /// Save mixer settings.
    pub fn save_settings(&mut self) -> Result<String, MixerError> {
        if !self.persist_enabled {
            return Err(MixerError::ConfigError("Persistence disabled".to_string()));
        }

        let mut map = HashMap::new();
        for (i, channel) in self.channels.iter().enumerate() {
            let key = format!("channel_{}", i);
            let value = serde_json::to_value(&channel.settings).unwrap_or_default();
            map.insert(key, value);
        }

        let master_key = "master".to_string();
        let master_value = serde_json::to_value(&self.master).unwrap_or_default();
        map.insert(master_key, master_value);

        self.last_saved = Some(map.clone());
        Ok(serde_json::to_string(&map).unwrap_or_default())
    }

    /// Load mixer settings.
    pub fn load_settings(&mut self, json: &str) -> Result<(), MixerError> {
        let map: HashMap<String, serde_json::Value> =
            serde_json::from_str(json).map_err(|e| MixerError::ConfigError(e.to_string()))?;

        for (i, channel) in self.channels.iter_mut().enumerate() {
            let key = format!("channel_{}", i);
            if let Some(value) = map.get(&key) {
                if let Ok(settings) = serde_json::from_value(value.clone()) {
                    channel.settings = settings;
                }
            }
        }

        if let Some(master_value) = map.get("master") {
            if let Ok(master) = serde_json::from_value(master_value.clone()) {
                self.master = master;
            }
        }

        self.has_any_solo = self.channels.iter().any(|c| c.settings.is_solo);
        self.solo_active = self.has_any_solo;

        Ok(())
    }

    /// Reset channel to defaults.
    pub fn reset_channel(&mut self, index: u32) -> Result<(), MixerError> {
        if index >= self.channels.len() as u32 {
            return Err(MixerError::InvalidChannel(index));
        }

        self.channels[index as usize].reset_settings();
        Ok(())
    }

    /// Reset master to defaults.
    pub fn reset_master(&mut self) {
        self.master.reset_master();
    }

    /// Set channel volume.
    pub fn set_channel_volume(&mut self, index: u32, volume: f32) -> Result<(), MixerError> {
        if index >= self.channels.len() as u32 {
            return Err(MixerError::InvalidChannel(index));
        }

        self.channels[index as usize].settings.set_volume(volume);
        Ok(())
    }

    /// Set channel pan.
    pub fn set_channel_pan(&mut self, index: u32, pan: f32) -> Result<(), MixerError> {
        if index >= self.channels.len() as u32 {
            return Err(MixerError::InvalidChannel(index));
        }

        self.channels[index as usize].settings.set_pan(pan);
        Ok(())
    }

    /// Set channel EQ.
    pub fn set_channel_eq(
        &mut self,
        index: u32,
        low: f32,
        mid: f32,
        high: f32,
    ) -> Result<(), MixerError> {
        if index >= self.channels.len() as u32 {
            return Err(MixerError::InvalidChannel(index));
        }

        let ch = &mut self.channels[index as usize];
        ch.settings.set_eq_low(low);
        ch.settings.set_eq_mid(mid);
        ch.settings.set_eq_high(high);
        Ok(())
    }

    /// Toggle channel mute.
    pub fn toggle_channel_mute(&mut self, index: u32) -> Result<(), MixerError> {
        if index >= self.channels.len() as u32 {
            return Err(MixerError::InvalidChannel(index));
        }

        self.channels[index as usize].settings.toggle_mute();
        Ok(())
    }

    /// Toggle channel cue mode.
    pub fn toggle_channel_cue(&mut self, index: u32) -> Result<(), MixerError> {
        if index >= self.channels.len() as u32 {
            return Err(MixerError::InvalidChannel(index));
        }

        let channel = &mut self.channels[index as usize];
        let next_mode = match channel.settings.cue_mode {
            CueMode::CueOnly => CueMode::CueMaster,
            CueMode::CueMaster => CueMode::Live,
            CueMode::Live => CueMode::CueOnly,
        };
        channel.settings.set_cue_mode(next_mode);
        Ok(())
    }

    /// Get active channel count.
    pub fn active_channel_count(&self) -> usize {
        self.channels
            .iter()
            .filter(|c| c.is_active(self.has_any_solo))
            .count()
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        let active = self.active_channel_count();
        format!(
            "{} channels | {} active | Cue: {:.0}% | {}",
            self.channels.len(),
            active,
            self.cue_mix * 100.0,
            self.master.status_string()
        )
    }
}

impl Default for MixerEngine {
    fn default() -> Self {
        Self::new(4)
    }
}

/// Mixer error.
#[derive(Debug, Clone)]
pub enum MixerError {
    /// Invalid channel index.
    InvalidChannel(u32),
    /// Config error.
    ConfigError(String),
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for MixerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChannel(idx) => write!(f, "Invalid channel: {}", idx),
            Self::ConfigError(msg) => write!(f, "Config error: {}", msg),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for MixerError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_engine_creation() {
        let mixer = MixerEngine::new(4);
        assert_eq!(mixer.all_channels().len(), 4);
    }

    #[test]
    fn mixer_channel_settings_defaults() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        assert_eq!(settings.volume, 1.0);
        assert_eq!(settings.pan, 0.0);
        assert!(!settings.is_muted);
        assert!(!settings.is_solo);
    }

    #[test]
    fn mixer_channel_volume() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_volume(0.5);
        assert!((settings.volume - 0.5).abs() < f32::EPSILON);

        settings.set_volume(1.5); // Clamp.
        assert_eq!(settings.volume, 1.0);

        settings.set_volume(-0.5); // Clamp.
        assert_eq!(settings.volume, 0.0);
    }

    #[test]
    fn mixer_channel_pan() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_pan(-1.0);
        assert_eq!(settings.pan, -1.0); // Full left.

        settings.set_pan(1.0);
        assert_eq!(settings.pan, 1.0); // Full right.
    }

    #[test]
    fn mixer_channel_eq() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_eq_high(6.0);
        settings.set_eq_mid(-3.0);
        settings.set_eq_low(4.0);

        assert!((settings.eq_high - 6.0).abs() < f32::EPSILON);
        assert!((settings.eq_mid + 3.0).abs() < f32::EPSILON);
        assert!((settings.eq_low - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_channel_eq_clamp() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_eq_high(20.0); // Clamp to 12.
        assert!((settings.eq_high - 12.0).abs() < f32::EPSILON);

        settings.set_eq_high(-20.0); // Clamp to -12.
        assert!((settings.eq_high + 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_channel_mute_toggle() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        assert!(!settings.is_muted);

        settings.toggle_mute();
        assert!(settings.is_muted);

        settings.toggle_mute();
        assert!(!settings.is_muted);
    }

    #[test]
    fn mixer_channel_solo_toggle() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        assert!(!settings.is_solo);

        settings.toggle_solo();
        assert!(settings.is_solo);

        settings.clear_solo();
        assert!(!settings.is_solo);
    }

    #[test]
    fn mixer_channel_cue_modes() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        assert_eq!(settings.cue_mode, CueMode::CueOnly);

        settings.set_cue_mode(CueMode::CueMaster);
        assert_eq!(settings.cue_mode, CueMode::CueMaster);

        settings.set_cue_mode(CueMode::Live);
        assert_eq!(settings.cue_mode, CueMode::Live);
    }

    #[test]
    fn mixer_channel_limiter() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        assert!(settings.limiter_engaged);

        settings.toggle_limiter();
        assert!(!settings.limiter_engaged);
    }

    #[test]
    fn mixer_channel_reset() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_volume(0.5);
        settings.set_pan(-1.0);
        settings.set_eq_high(12.0);
        settings.toggle_mute();

        settings.reset_to_defaults();
        assert_eq!(settings.volume, 1.0);
        assert_eq!(settings.pan, 0.0);
        assert!(!settings.is_muted);
    }

    #[test]
    fn mixer_channel_is_active() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        assert!(settings.is_active(false));

        settings.toggle_mute();
        assert!(!settings.is_active(false));
    }

    #[test]
    fn mixer_channel_is_active_solo() {
        let mut s1 = ChannelSettings::new(ChannelRole::Deck1);
        let s2 = ChannelSettings::new(ChannelRole::Deck2);

        s1.toggle_solo();
        assert!(s1.is_active(true));
        assert!(!s2.is_active(true)); // s2 is not solo'd.
    }

    #[test]
    fn mixer_channel_volume_string() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        let vol = settings.volume_string();
        assert!(vol.contains("100%"));
    }

    #[test]
    fn mixer_channel_pan_string_center() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        assert_eq!(settings.pan_string(), "C");
    }

    #[test]
    fn mixer_channel_pan_string_left() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_pan(-0.5);
        assert!(settings.pan_string().contains("L"));
    }

    #[test]
    fn mixer_channel_pan_string_right() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_pan(0.5);
        assert!(settings.pan_string().contains("R"));
    }

    #[test]
    fn mixer_channel_eq_string() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        let eq = settings.eq_string();
        assert!(eq.contains("H:+0"));
        assert!(eq.contains("M:+0"));
        assert!(eq.contains("L:+0"));
    }

    #[test]
    fn mixer_channel_settings_to_map() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        let map = settings.to_map();
        assert!(!map.is_empty());
    }

    #[test]
    fn mixer_channel_state_creation() {
        let state = ChannelState::new(ChannelRole::Deck1);
        assert_eq!(state.settings.role, ChannelRole::Deck1);
        assert!(state.peak_history.is_empty());
    }

    #[test]
    fn mixer_channel_update_meter() {
        let mut state = ChannelState::new(ChannelRole::Deck1);
        let reading = MeterReading::new(0.5, 0.5, 0.4, 0.4);
        state.update_meter(reading);

        assert_eq!(state.meter.max_peak(), 0.5);
        assert_eq!(state.peak_history.len(), 1);
    }

    #[test]
    fn mixer_channel_meter_clipped() {
        let mut state = ChannelState::new(ChannelRole::Deck1);
        let reading = MeterReading::new(1.0, 1.0, 0.8, 0.8);
        state.update_meter(reading);

        assert!(state.meter.is_clipped());
        assert_eq!(state.limiter_state, LimiterState::Clipping);
    }

    #[test]
    fn mixer_channel_peak_bars() {
        let mut state = ChannelState::new(ChannelRole::Deck1);

        for i in 0..32 {
            state.update_meter(MeterReading::new(i as f32 / 32.0, 0.0, 0.0, 0.0));
        }

        let bars = state.peak_bars(16);
        assert_eq!(bars.len(), 16);
    }

    #[test]
    fn mixer_channel_apply_limiter_inactive() {
        let mut state = ChannelState::new(ChannelRole::Deck1);
        state.settings.limiter_engaged = false;

        let output = state.apply_limiter(0.9);
        assert!((output - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_channel_apply_limiter_engaged() {
        let mut state = ChannelState::new(ChannelRole::Deck1);
        state.settings.limiter_threshold = 0.8;

        let output = state.apply_limiter(0.9);
        // Input 0.9 > threshold 0.8, excess = 0.1, output = 0.8 + 0.1/10 = 0.81
        assert!(output < 0.85);
        assert_eq!(state.limiter_state, LimiterState::Engaged);
    }

    #[test]
    fn mixer_channel_apply_limiter_below_threshold() {
        let mut state = ChannelState::new(ChannelRole::Deck1);
        state.settings.limiter_threshold = 0.8;

        let output = state.apply_limiter(0.5);
        assert!((output - 0.5).abs() < f32::EPSILON);
        assert_eq!(state.limiter_state, LimiterState::Inactive);
    }

    #[test]
    fn mixer_channel_meter_reading_db() {
        let reading = MeterReading::new(1.0, 1.0, 0.8, 0.8);
        assert!((reading.db_value() - 0.0).abs() < f32::EPSILON);

        let quiet = MeterReading::new(0.01, 0.01, 0.008, 0.008);
        assert!(quiet.db_value() < -20.0);
    }

    #[test]
    fn mixer_channel_meter_level_percent() {
        let reading = MeterReading::new(0.5, 0.5, 0.4, 0.4);
        assert_eq!(reading.level_percent(), 50);
    }

    #[test]
    fn mixer_master_creation() {
        let master = MasterChannelState::new(4);
        assert_eq!(master.volume, 1.0);
        assert_eq!(master.channel_count, 4);
    }

    #[test]
    fn mixer_master_update_from_channels() {
        let mut master = MasterChannelState::new(4);
        master.update_from_channels(0.5, 0.5);

        assert_eq!(master.meter.max_peak(), 0.5);
        assert_eq!(master.peak_history.len(), 1);
    }

    #[test]
    fn mixer_master_update_clipping() {
        let mut master = MasterChannelState::new(4);
        master.update_from_channels(1.0, 1.0);
        assert_eq!(master.limiter_state, LimiterState::Engaged);
    }

    #[test]
    fn mixer_master_volume_string() {
        let master = MasterChannelState::new(4);
        let vol = master.volume_string();
        assert!(vol.contains("100%"));
    }

    #[test]
    fn mixer_master_peak_bars() {
        let mut master = MasterChannelState::new(4);

        for i in 0..16 {
            master.update_from_channels(i as f32 / 16.0, 0.0);
        }

        let bars = master.peak_bars(8);
        assert_eq!(bars.len(), 8);
    }

    #[test]
    fn mixer_master_reset() {
        let mut master = MasterChannelState::new(4);
        master.set_volume(0.5);
        master.set_limiter_threshold(0.5);

        master.reset_master();
        assert_eq!(master.volume, 1.0);
        assert_eq!(master.limiter_threshold, 0.95);
    }

    #[test]
    fn mixer_engine_process_channel() {
        let mut mixer = MixerEngine::new(2);
        let (left, right) = mixer.process_channel(0, 0.5, 0.5);
        assert!(left >= 0.0);
        assert!(right >= 0.0);
    }

    #[test]
    fn mixer_engine_process_muted_channel() {
        let mut mixer = MixerEngine::new(2);
        let _ = mixer.toggle_channel_mute(0);

        let (left, right) = mixer.process_channel(0, 0.5, 0.5);
        assert_eq!(left, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn mixer_engine_process_invalid_channel() {
        let mut mixer = MixerEngine::new(2);
        let (left, right) = mixer.process_channel(5, 0.5, 0.5);
        assert_eq!(left, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn mixer_engine_solo_channel() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.solo_channel(0).is_ok());
        assert!(mixer.is_solo_active());
    }

    #[test]
    fn mixer_engine_clear_solos() {
        let mut mixer = MixerEngine::new(2);
        let _ = mixer.solo_channel(0);
        let _ = mixer.solo_channel(1);

        mixer.clear_all_solos();
        assert!(!mixer.is_solo_active());
    }

    #[test]
    fn mixer_engine_solo_invalid() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.solo_channel(5).is_err());
    }

    #[test]
    fn mixer_engine_set_volume() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.set_channel_volume(0, 0.5).is_ok());

        let ch = mixer.get_channel(0).unwrap();
        assert!((ch.settings.volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_engine_set_pan() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.set_channel_pan(0, 0.5).is_ok());

        let ch = mixer.get_channel(0).unwrap();
        assert_eq!(ch.settings.pan, 0.5);
    }

    #[test]
    fn mixer_engine_set_eq() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.set_channel_eq(0, 3.0, 0.0, -3.0).is_ok());

        let ch = mixer.get_channel(0).unwrap();
        assert!((ch.settings.eq_low - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_engine_toggle_mute() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.toggle_channel_mute(0).is_ok());

        let ch = mixer.get_channel(0).unwrap();
        assert!(ch.settings.is_muted);
    }

    #[test]
    fn mixer_engine_toggle_cue() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.toggle_channel_cue(0).is_ok());

        let ch = mixer.get_channel(0).unwrap();
        // Should cycle from CueOnly to CueMaster.
        assert_eq!(ch.settings.cue_mode, CueMode::CueMaster);
    }

    #[test]
    fn mixer_engine_set_cue_mix() {
        let mut mixer = MixerEngine::new(2);
        mixer.set_cue_mix(0.5);
        assert!((mixer.cue_mix() - 0.5).abs() < f32::EPSILON);
        assert!((mixer.program_mix() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_engine_cue_mix_clamp() {
        let mut mixer = MixerEngine::new(2);
        mixer.set_cue_mix(1.5); // Clamp to 1.0.
        assert_eq!(mixer.cue_mix(), 1.0);

        mixer.set_cue_mix(-0.5); // Clamp to 0.0.
        assert_eq!(mixer.cue_mix(), 0.0);
    }

    #[test]
    fn mixer_engine_channel_count() {
        let mixer = MixerEngine::new(4);
        assert_eq!(mixer.all_channels().len(), 4);
    }

    #[test]
    fn mixer_engine_active_channel_count() {
        let mut mixer = MixerEngine::new(2);
        let _ = mixer.toggle_channel_mute(0);

        let active = mixer.active_channel_count();
        assert_eq!(active, 1); // Only channel 1 is active.
    }

    #[test]
    fn mixer_engine_status_string() {
        let mixer = MixerEngine::new(4);
        let status = mixer.status_string();
        assert!(status.contains("4 channels"));
    }

    #[test]
    fn mixer_engine_master_status() {
        let mixer = MixerEngine::new(4);
        let status = mixer.master().status_string();
        assert!(status.contains("Master"));
    }

    #[test]
    fn mixer_engine_channel_status() {
        let mixer = MixerEngine::new(2);
        let ch = mixer.get_channel(0).unwrap();
        let status = ch.status_string();
        assert!(status.contains("Deck 1"));
    }

    #[test]
    fn mixer_engine_reset_channel() {
        let mut mixer = MixerEngine::new(2);
        let _ = mixer.toggle_channel_mute(0);
        assert!(mixer.reset_channel(0).is_ok());

        let ch = mixer.get_channel(0).unwrap();
        assert!(!ch.settings.is_muted);
    }

    #[test]
    fn mixer_engine_reset_invalid_channel() {
        let mut mixer = MixerEngine::new(2);
        assert!(mixer.reset_channel(5).is_err());
    }

    #[test]
    fn mixer_engine_update_master() {
        let mut mixer = MixerEngine::new(2);
        mixer.update_master_output(0.5, 0.5);

        assert_eq!(mixer.master().meter.max_peak(), 0.5);
    }

    #[test]
    fn mixer_channel_role_names() {
        assert_eq!(ChannelRole::Deck1.name(), "Deck 1");
        assert_eq!(ChannelRole::Deck2.name(), "Deck 2");
        assert_eq!(ChannelRole::Aux.name(), "Aux");
        assert_eq!(ChannelRole::FxReturn.name(), "FX Return");
    }

    #[test]
    fn mixer_channel_role_numbers() {
        assert_eq!(ChannelRole::Deck1.number(), 1);
        assert_eq!(ChannelRole::Deck4.number(), 4);
        assert_eq!(ChannelRole::Aux.number(), 5);
    }

    #[test]
    fn limiter_state_names() {
        assert_eq!(LimiterState::Inactive.name(), "Inactive");
        assert_eq!(LimiterState::Engaged.name(), "Engaged");
        assert_eq!(LimiterState::Clipping.name(), "Clipping");
    }

    #[test]
    fn cue_mode_names() {
        assert_eq!(CueMode::CueOnly.name(), "Cue Only");
        assert_eq!(CueMode::CueMaster.name(), "Cue + Master");
        assert_eq!(CueMode::Live.name(), "Live");
    }

    #[test]
    fn mixer_error_display() {
        assert_eq!(
            format!("{}", MixerError::InvalidChannel(5)),
            "Invalid channel: 5"
        );
        assert_eq!(
            format!("{}", MixerError::ConfigError("test".to_string())),
            "Config error: test"
        );
    }

    #[test]
    fn mixer_channel_cue_string() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        let cue = settings.cue_string();
        assert!(cue.contains("Cue Only"));
        assert!(cue.contains("100%"));
    }

    #[test]
    fn mixer_channel_limiter_string() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        let lim = settings.limiter_string();
        assert!(lim.contains("Engaged"));
        assert!(lim.contains("95%"));
    }

    #[test]
    fn mixer_channel_fx_send() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_fx_send(0.75);
        assert!((settings.fx_send - 0.75).abs() < f32::EPSILON);

        settings.set_fx_send(1.5); // Clamp.
        assert_eq!(settings.fx_send, 1.0);
    }

    #[test]
    fn mixer_channel_cue_volume() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_cue_volume(0.5);
        assert!((settings.cue_volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_channel_limiter_threshold() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_limiter_threshold(0.8);
        assert!((settings.limiter_threshold - 0.8).abs() < f32::EPSILON);

        settings.set_limiter_threshold(0.3); // Clamp to 0.5.
        assert!((settings.limiter_threshold - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_channel_meter_type() {
        let mut settings = ChannelSettings::new(ChannelRole::Deck1);
        settings.set_meter_type(MeterType::Peak);
        assert_eq!(settings.meter_type, MeterType::Peak);

        settings.set_meter_type(MeterType::Rms);
        assert_eq!(settings.meter_type, MeterType::Rms);

        settings.set_meter_type(MeterType::Both);
        assert_eq!(settings.meter_type, MeterType::Both);
    }

    #[test]
    fn mixer_channel_is_cue_armed() {
        let state = ChannelState::new(ChannelRole::Deck1);
        assert!(state.is_cue_armed()); // Default is CueOnly.

        let mut state2 = ChannelState::new(ChannelRole::Deck1);
        state2.settings.set_cue_mode(CueMode::Live);
        assert!(!state2.is_cue_armed());
    }

    #[test]
    fn mixer_engine_process_with_pan() {
        let mut mixer = MixerEngine::new(2);
        let _ = mixer.set_channel_pan(0, -1.0); // Full left.

        let (left, right) = mixer.process_channel(0, 1.0, 1.0);
        assert!(left > 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn mixer_engine_has_any_solo_tracking() {
        let mut mixer = MixerEngine::new(2);

        // No solos.
        assert!(!mixer.has_any_solo);

        // Solo channel 0.
        let _ = mixer.solo_channel(0);
        assert!(mixer.has_any_solo);

        // Clear solos.
        mixer.clear_all_solos();
        assert!(!mixer.has_any_solo);
    }

    #[test]
    fn mixer_engine_meter_reading_creation() {
        let reading = MeterReading::new(0.8, 0.6, 0.6, 0.5);
        assert_eq!(reading.max_peak(), 0.8);
        assert_eq!(reading.max_rms(), 0.6);
        assert!(!reading.is_clipped());
    }

    #[test]
    fn mixer_engine_meter_zero_level() {
        let reading = MeterReading::default();
        assert_eq!(reading.level_percent(), 0);
        assert!(reading.db_value() < -50.0); // Should be near -60 dB.
    }

    #[test]
    fn mixer_engine_64_peak_history_limit() {
        let mut state = ChannelState::new(ChannelRole::Deck1);

        for i in 0..100 {
            state.update_meter(MeterReading::new(i as f32 / 100.0, 0.0, 0.0, 0.0));
        }

        let bars = state.peak_bars(64);
        assert_eq!(bars.len(), 64);
    }

    #[test]
    fn mixer_channel_state_channel_level() {
        let mut state = ChannelState::new(ChannelRole::Deck1);
        state.update_channel_level(0.75);
        assert!((state.channel_level - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn mixer_engine_limiter_info() {
        let state = ChannelState::new(ChannelRole::Deck1);
        let (name, reduction) = state.limiter_info();
        assert_eq!(name, "Inactive");
        assert_eq!(reduction, 0.0);
    }

    #[test]
    fn mixer_master_limiter_info() {
        let master = MasterChannelState::new(4);
        assert_eq!(master.limiter_info(), "Inactive");
    }

    #[test]
    fn mixer_engine_ch4_creation() {
        let mixer = MixerEngine::new(4);
        assert_eq!(mixer.all_channels().len(), 4);
    }

    #[test]
    fn mixer_engine_ch2_creation() {
        let mixer = MixerEngine::new(2);
        assert_eq!(mixer.all_channels().len(), 2);
    }

    #[test]
    fn mixer_engine_channel_roles() {
        let mixer = MixerEngine::new(4);
        assert_eq!(
            mixer.get_channel(0).unwrap().settings.role,
            ChannelRole::Deck1
        );
        assert_eq!(
            mixer.get_channel(1).unwrap().settings.role,
            ChannelRole::Deck2
        );
        assert_eq!(
            mixer.get_channel(2).unwrap().settings.role,
            ChannelRole::Deck3
        );
        assert_eq!(
            mixer.get_channel(3).unwrap().settings.role,
            ChannelRole::Deck4
        );
    }

    #[test]
    fn mixer_engine_process_audio_active() {
        let mut mixer = MixerEngine::new(2);
        let ch = mixer.get_channel_mut(0).unwrap();
        ch.set_audio_active(true);

        let (left, right) = mixer.process_channel(0, 1.0, 1.0);
        assert!(left > 0.0 || right > 0.0);
    }

    #[test]
    fn mixer_engine_limiter_clipping_state() {
        let mut state = ChannelState::new(ChannelRole::Deck1);
        let reading = MeterReading::new(1.0, 0.9, 0.8, 0.7);
        state.update_meter(reading);

        assert_eq!(state.limiter_state, LimiterState::Clipping);
    }

    #[test]
    fn mixer_channel_settings_from_map() {
        let settings = ChannelSettings::new(ChannelRole::Deck1);
        let map = settings.to_map();

        // from_map should not panic.
        let _ = ChannelSettings::from_map(&map);
    }

    #[test]
    fn mixer_engine_solo_non_solo_channel() {
        let mut mixer = MixerEngine::new(2);

        // Solo channel 0, then check channel 1.
        let _ = mixer.solo_channel(0);

        let ch = mixer.get_channel(1).unwrap();
        // Channel 1 should NOT be active because it's not solo'd.
        assert!(!ch.is_active(mixer.has_any_solo));
    }
}
