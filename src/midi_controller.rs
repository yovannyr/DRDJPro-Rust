//! MIDI controller support for DRDJ Pro.
//!
//! Handles MIDI device discovery, message parsing, preset management,
//! and custom mapping configuration. Part of the MIDI Controller feature
//! (P04a).
//!
//! ## Design principles
//!
//! * **Device discovery** — Enumerate available MIDI input/output devices.
//! * **Message parsing** — Parse standard MIDI messages (note on/off, CC, pitch bend, etc.).
//! * **Preset system** — Ship with default MIDI mappings, allow user overrides.
//! * **Custom mapping** — Allow users to create and save custom mappings.
//! * **Thread-safe** — Device discovery and message handling are Mutex-protected.

use std::fmt;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// MIDI message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessageType {
    /// Note On (0x9n).
    NoteOn,
    /// Note Off (0x8n).
    NoteOff,
    /// Continuous Controller (0xBn).
    ControlChange,
    /// Pitch Bend (0xEn).
    PitchBend,
    /// Program Change (0xCn).
    ProgramChange,
    /// Channel Pressure (0xDn).
    ChannelPressure,
    /// Polyphonic Pressure (0xAn).
    PolyPressure,
    /// System Exclusive (0xF0-0xF7).
    SystemExclusive,
    /// Timing Clock (0xF8).
    TimingClock,
    /// Start (0xFA).
    Start,
    /// Continue (0xFB).
    Continue,
    /// Stop (0xFC).
    Stop,
    /// Active Sensing (0xFE).
    ActiveSensing,
    /// Reset (0xFF).
    Reset,
}

impl fmt::Display for MidiMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiMessageType::NoteOn => write!(f, "note-on"),
            MidiMessageType::NoteOff => write!(f, "note-off"),
            MidiMessageType::ControlChange => write!(f, "cc"),
            MidiMessageType::PitchBend => write!(f, "pitch-bend"),
            MidiMessageType::ProgramChange => write!(f, "program-change"),
            MidiMessageType::ChannelPressure => write!(f, "channel-pressure"),
            MidiMessageType::PolyPressure => write!(f, "poly-pressure"),
            MidiMessageType::SystemExclusive => write!(f, "sysex"),
            MidiMessageType::TimingClock => write!(f, "timing-clock"),
            MidiMessageType::Start => write!(f, "start"),
            MidiMessageType::Continue => write!(f, "continue"),
            MidiMessageType::Stop => write!(f, "stop"),
            MidiMessageType::ActiveSensing => write!(f, "active-sensing"),
            MidiMessageType::Reset => write!(f, "reset"),
        }
    }
}

/// A parsed MIDI message.
#[derive(Debug, Clone)]
pub struct MidiMessage {
    /// Message type.
    pub message_type: MidiMessageType,
    /// MIDI channel (0-15, i.e., 1-16).
    pub channel: u8,
    /// Data byte 1 (note number, controller number, etc.).
    pub data1: u8,
    /// Data byte 2 (velocity, value, etc.).
    pub data2: u8,
    /// Raw bytes (for sysex and debugging).
    pub raw: Vec<u8>,
}

impl MidiMessage {
    /// Create a note-on message.
    pub fn note_on(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            message_type: MidiMessageType::NoteOn,
            channel,
            data1: note,
            data2: velocity,
            raw: vec![0x90 | channel, note, velocity],
        }
    }

    /// Create a note-off message.
    pub fn note_off(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            message_type: MidiMessageType::NoteOff,
            channel,
            data1: note,
            data2: velocity,
            raw: vec![0x80 | channel, note, velocity],
        }
    }

    /// Create a control-change message.
    pub fn control_change(channel: u8, controller: u8, value: u8) -> Self {
        Self {
            message_type: MidiMessageType::ControlChange,
            channel,
            data1: controller,
            data2: value,
            raw: vec![0xB0 | channel, controller, value],
        }
    }

    /// Create a pitch-bend message.
    pub fn pitch_bend(channel: u8, value: u16) -> Self {
        let msb = (value >> 7) as u8;
        let lsb = (value & 0x7F) as u8;
        Self {
            message_type: MidiMessageType::PitchBend,
            channel,
            data1: msb,
            data2: lsb,
            raw: vec![0xE0 | channel, msb, lsb],
        }
    }

    /// Create a program-change message.
    pub fn program_change(channel: u8, program: u8) -> Self {
        Self {
            message_type: MidiMessageType::ProgramChange,
            channel,
            data1: program,
            data2: 0,
            raw: vec![0xC0 | channel, program],
        }
    }

    /// Check if the message is a note message.
    pub fn is_note(&self) -> bool {
        matches!(
            self.message_type,
            MidiMessageType::NoteOn | MidiMessageType::NoteOff
        )
    }

    /// Check if the message is a controller message.
    pub fn is_control_change(&self) -> bool {
        self.message_type == MidiMessageType::ControlChange
    }

    /// Get controller value as a normalized float (0.0 - 1.0).
    pub fn controller_value(&self) -> Option<f64> {
        if self.message_type == MidiMessageType::ControlChange {
            Some(self.data2 as f64 / 127.0)
        } else {
            None
        }
    }

    /// Get pitch-bend value as a normalized float (-1.0 - 1.0).
    pub fn pitch_bend_value(&self) -> Option<f64> {
        if self.message_type == MidiMessageType::PitchBend {
            let value = ((self.data1 as u16) << 7) | (self.data2 as u16);
            // Center is 8192.
            Some((value as f64 - 8192.0) / 8192.0)
        } else {
            None
        }
    }

    /// Get note number (for note-on/note-off).
    pub fn note(&self) -> Option<u8> {
        if self.is_note() {
            Some(self.data1)
        } else {
            None
        }
    }

    /// Get note velocity (for note-on/note-off).
    pub fn velocity(&self) -> Option<u8> {
        if self.is_note() {
            Some(self.data2)
        } else {
            None
        }
    }
}

/// A MIDI device (input or output).
#[derive(Debug, Clone)]
pub struct MidiDevice {
    /// Unique device identifier.
    pub id: String,
    /// Device name.
    pub name: String,
    /// Whether this is an input device.
    pub is_input: bool,
    /// Whether the device is currently connected.
    pub is_connected: bool,
}

/// MIDI action to perform on a mapped control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiAction {
    /// Play/pause playback.
    PlayPause,
    /// Stop playback.
    Stop,
    /// Seek to a position in the track.
    Seek(u32),
    /// Adjust pitch rate.
    SetPitchRate(f64),
    /// Load a track into a deck.
    LoadDeck(u8), // Deck ID (1-4)
    /// Toggle hot cue.
    ToggleHotCue(u8), // Slot (0-16)
    /// Trigger hot cue.
    TriggerHotCue(u8),
    /// Set loop start.
    SetLoopStart,
    /// Set loop end.
    SetLoopEnd,
    /// Toggle loop.
    ToggleLoop,
    /// Adjust EQ (low/mid/high).
    SetEq(MidiEqBand, f64),
    /// Adjust volume/gain.
    SetGain(f64),
    /// Crossfade to a deck.
    Crossfade(u8), // Target deck (1-4)
    /// Tap tempo.
    TapTempo,
    /// Cue point (go to cue).
    GoToCue(u8), // Cue slot
    /// Custom action (unmapped).
    Unknown,
}

impl fmt::Display for MidiAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiAction::PlayPause => write!(f, "play/pause"),
            MidiAction::Stop => write!(f, "stop"),
            MidiAction::Seek(_) => write!(f, "seek"),
            MidiAction::SetPitchRate(_) => write!(f, "pitch-rate"),
            MidiAction::LoadDeck(d) => write!(f, "load-deck-{}", d),
            MidiAction::ToggleHotCue(s) => write!(f, "toggle-hotcue-{}", s),
            MidiAction::TriggerHotCue(s) => write!(f, "trigger-hotcue-{}", s),
            MidiAction::SetLoopStart => write!(f, "loop-start"),
            MidiAction::SetLoopEnd => write!(f, "loop-end"),
            MidiAction::ToggleLoop => write!(f, "toggle-loop"),
            MidiAction::SetEq(band, _) => write!(f, "eq-{}", band),
            MidiAction::SetGain(_) => write!(f, "gain"),
            MidiAction::Crossfade(d) => write!(f, "crossfade-deck-{}", d),
            MidiAction::TapTempo => write!(f, "tap-tempo"),
            MidiAction::GoToCue(s) => write!(f, "go-to-cue-{}", s),
            MidiAction::Unknown => write!(f, "unknown"),
        }
    }
}

/// EQ band for MIDI mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiEqBand {
    Low,
    Mid,
    High,
}

impl fmt::Display for MidiEqBand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiEqBand::Low => write!(f, "low"),
            MidiEqBand::Mid => write!(f, "mid"),
            MidiEqBand::High => write!(f, "high"),
        }
    }
}

/// Mapping from a MIDI control to an action.
#[derive(Debug, Clone)]
pub struct MidiMapping {
    /// MIDI device ID.
    pub device_id: String,
    /// MIDI channel (0-15).
    pub channel: u8,
    /// MIDI controller number or note number.
    pub control: u8,
    /// Action to perform.
    pub action: MidiAction,
    /// Minimum value for this mapping (for range mappings).
    pub min_value: u8,
    /// Maximum value for this mapping.
    pub max_value: u8,
    /// Whether this mapping is active.
    pub enabled: bool,
}

/// Preset mapping for a specific MIDI controller model.
#[derive(Debug, Clone)]
pub struct MidiPreset {
    /// Preset name (e.g., "Pioneer DDJ-400").
    pub name: String,
    /// Manufacturer.
    pub manufacturer: String,
    /// Model.
    pub model: String,
    /// Mappings for this preset.
    pub mappings: Vec<MidiMapping>,
}

/// MIDI controller manager — handles devices, mappings, and presets.
pub struct MidiManager {
    devices: Arc<Mutex<Vec<MidiDevice>>>,
    mappings: Arc<Mutex<Vec<MidiMapping>>>,
    presets: Arc<Mutex<Vec<MidiPreset>>>,
    active_preset: Arc<Mutex<Option<String>>>,
}

impl MidiManager {
    /// Create a new MIDI manager.
    pub fn new() -> Self {
        let manager = Self {
            devices: Arc::new(Mutex::new(Vec::new())),
            mappings: Arc::new(Mutex::new(Vec::new())),
            presets: Arc::new(Mutex::new(Vec::new())),
            active_preset: Arc::new(Mutex::new(None)),
        };

        // Load default presets.
        manager.load_default_presets();

        manager
    }

    /// Enumerate available MIDI devices.
    ///
    /// In production, this would query the OS MIDI API.
    /// For now, returns a simulated list for testing.
    pub fn enumerate_devices(&self) -> Vec<MidiDevice> {
        let devices = vec![
            MidiDevice {
                id: "virtual-input-1".to_string(),
                name: "Virtual Input 1".to_string(),
                is_input: true,
                is_connected: false,
            },
            MidiDevice {
                id: "virtual-output-1".to_string(),
                name: "Virtual Output 1".to_string(),
                is_input: false,
                is_connected: false,
            },
        ];

        let mut devices_lock = self.devices.lock().unwrap();
        devices_lock.clone_from(&devices);

        devices
    }

    /// Connect a MIDI device.
    pub fn connect_device(&self, device_id: &str) -> Result<(), String> {
        let mut devices = self.devices.lock().map_err(|e| e.to_string())?;
        let device = devices
            .iter_mut()
            .find(|d| d.id == device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?;

        device.is_connected = true;
        Ok(())
    }

    /// Disconnect a MIDI device.
    pub fn disconnect_device(&self, device_id: &str) -> Result<(), String> {
        let mut devices = self.devices.lock().map_err(|e| e.to_string())?;
        let device = devices
            .iter_mut()
            .find(|d| d.id == device_id)
            .ok_or_else(|| format!("Device {} not found", device_id))?;

        device.is_connected = false;
        Ok(())
    }

    /// Get connected input devices.
    pub fn get_input_devices(&self) -> Result<Vec<MidiDevice>, String> {
        let devices = self.devices.lock().map_err(|e| e.to_string())?;
        Ok(devices
            .iter()
            .filter(|d| d.is_input && d.is_connected)
            .cloned()
            .collect())
    }

    /// Apply a preset mapping.
    pub fn apply_preset(&self, preset_name: &str) -> Result<usize, String> {
        let presets = self.presets.lock().map_err(|e| e.to_string())?;
        let preset = presets
            .iter()
            .find(|p| p.name == preset_name)
            .ok_or_else(|| format!("Preset {} not found", preset_name))?;

        let mut mappings = self.mappings.lock().map_err(|e| e.to_string())?;
        mappings.clear();
        mappings.extend(preset.mappings.clone());

        let mut active = self.active_preset.lock().map_err(|e| e.to_string())?;
        *active = Some(preset_name.to_string());

        Ok(preset.mappings.len())
    }

    /// Add a custom mapping.
    pub fn add_mapping(&self, mapping: MidiMapping) -> Result<(), String> {
        if !mapping.enabled {
            return Err("Mapping must be enabled".to_string());
        }

        let mut mappings = self.mappings.lock().map_err(|e| e.to_string())?;

        // Check for duplicate mappings (same device, channel, control).
        let duplicate = mappings.iter().find(|m| {
            m.device_id == mapping.device_id
                && m.channel == mapping.channel
                && m.control == mapping.control
        });

        if duplicate.is_some() {
            return Err(format!(
                "Duplicate mapping for device {} on channel {} control {}",
                mapping.device_id, mapping.channel, mapping.control
            ));
        }

        mappings.push(mapping);
        Ok(())
    }

    /// Remove a mapping.
    pub fn remove_mapping(&self, device_id: &str, channel: u8, control: u8) -> Result<(), String> {
        let mut mappings = self.mappings.lock().map_err(|e| e.to_string())?;
        let index = mappings
            .iter()
            .position(|m| m.device_id == device_id && m.channel == channel && m.control == control)
            .ok_or("Mapping not found")?;

        mappings.remove(index);
        Ok(())
    }

    /// Resolve a MIDI message to an action.
    ///
    /// Returns the first matching action for the given message.
    pub fn resolve_action(&self, message: &MidiMessage) -> Option<MidiAction> {
        let mappings = self.mappings.lock().unwrap();

        for mapping in mappings.iter().filter(|m| {
            m.enabled && m.device_id == message.raw.first().map(|_| "any").unwrap_or("any")
                || (m.channel == message.channel)
        }) {
            if mapping.is_control_change_match(message) || mapping.is_note_match(message) {
                return Some(mapping.action);
            }
        }

        None
    }

    /// Get the active preset name.
    pub fn active_preset(&self) -> Result<Option<String>, String> {
        self.active_preset
            .lock()
            .map(|p| p.clone())
            .map_err(|e| e.to_string())
    }

    /// Get all available presets.
    pub fn list_presets(&self) -> Result<Vec<String>, String> {
        let presets = self.presets.lock().map_err(|e| e.to_string())?;
        Ok(presets.iter().map(|p| p.name.clone()).collect())
    }

    /// Get current mapping count.
    pub fn mapping_count(&self) -> Result<usize, String> {
        self.mappings
            .lock()
            .map(|m| m.len())
            .map_err(|e| e.to_string())
    }

    /// Load default presets.
    fn load_default_presets(&self) {
        let mut presets = self.presets.lock().unwrap();

        // Default "Universal" preset.
        presets.push(MidiPreset {
            name: "Universal Default".to_string(),
            manufacturer: "Generic".to_string(),
            model: "Universal".to_string(),
            mappings: self.create_default_mappings(),
        });
    }

    /// Create default MIDI mappings.
    fn create_default_mappings(&self) -> Vec<MidiMapping> {
        vec![
            // Channel 1, CC 1 → Play/Pause.
            MidiMapping {
                device_id: "any".to_string(),
                channel: 0,
                control: 1,
                action: MidiAction::PlayPause,
                min_value: 0,
                max_value: 127,
                enabled: true,
            },
            // Channel 1, CC 2 → Stop.
            MidiMapping {
                device_id: "any".to_string(),
                channel: 0,
                control: 2,
                action: MidiAction::Stop,
                min_value: 0,
                max_value: 127,
                enabled: true,
            },
            // Channel 1, CC 3 → Tap Tempo.
            MidiMapping {
                device_id: "any".to_string(),
                channel: 0,
                control: 3,
                action: MidiAction::TapTempo,
                min_value: 64,
                max_value: 127,
                enabled: true,
            },
            // Channel 1, CC 10 → Deck 1 Load.
            MidiMapping {
                device_id: "any".to_string(),
                channel: 0,
                control: 10,
                action: MidiAction::LoadDeck(1),
                min_value: 0,
                max_value: 127,
                enabled: true,
            },
            // Channel 1, CC 11 → Deck 2 Load.
            MidiMapping {
                device_id: "any".to_string(),
                channel: 0,
                control: 11,
                action: MidiAction::LoadDeck(2),
                min_value: 0,
                max_value: 127,
                enabled: true,
            },
        ]
    }
}

impl Default for MidiManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper extensions
// ---------------------------------------------------------------------------

impl MidiMapping {
    /// Check if this mapping matches a control-change message.
    fn is_control_change_match(&self, message: &MidiMessage) -> bool {
        message.message_type == MidiMessageType::ControlChange
            && message.channel == self.channel
            && message.data1 == self.control
            && message.data2 >= self.min_value
            && message.data2 <= self.max_value
    }

    /// Check if this mapping matches a note message.
    fn is_note_match(&self, message: &MidiMessage) -> bool {
        message.is_note() && message.channel == self.channel && message.data1 == self.control
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_note_on_message() {
        let msg = MidiMessage::note_on(0, 60, 100);
        assert_eq!(msg.message_type, MidiMessageType::NoteOn);
        assert_eq!(msg.channel, 0);
        assert_eq!(msg.data1, 60);
        assert_eq!(msg.data2, 100);
        assert!(msg.is_note());
        assert_eq!(msg.note(), Some(60));
        assert_eq!(msg.velocity(), Some(100));
    }

    #[test]
    fn create_control_change_message() {
        let msg = MidiMessage::control_change(0, 74, 64);
        assert_eq!(msg.message_type, MidiMessageType::ControlChange);
        assert!(msg.is_control_change());
        assert_eq!(msg.controller_value(), Some(64.0 / 127.0));
    }

    #[test]
    fn create_pitch_bend_message() {
        // Center: 8192.
        let msg = MidiMessage::pitch_bend(0, 8192);
        assert_eq!(msg.message_type, MidiMessageType::PitchBend);
        assert_eq!(msg.pitch_bend_value(), Some(0.0));

        // Max up: 16383.
        let msg = MidiMessage::pitch_bend(0, 16383);
        let value = msg.pitch_bend_value().unwrap();
        assert!(value > 0.9);

        // Max down: 0.
        let msg = MidiMessage::pitch_bend(0, 0);
        let value = msg.pitch_bend_value().unwrap();
        assert!(value < -0.9);
    }

    #[test]
    fn midi_message_type_display() {
        assert_eq!(format!("{}", MidiMessageType::NoteOn), "note-on");
        assert_eq!(format!("{}", MidiMessageType::ControlChange), "cc");
        assert_eq!(format!("{}", MidiMessageType::PitchBend), "pitch-bend");
    }

    #[test]
    fn midi_manager_enumerates_devices() {
        let manager = MidiManager::new();
        let devices = manager.enumerate_devices();
        assert!(!devices.is_empty());
        assert!(devices.iter().any(|d| d.is_input));
        assert!(devices.iter().any(|d| !d.is_input));
    }

    #[test]
    fn midi_manager_connect_disconnect() {
        let manager = MidiManager::new();
        manager.enumerate_devices();

        // Connect virtual input.
        manager.connect_device("virtual-input-1").unwrap();
        let inputs = manager.get_input_devices().unwrap();
        assert_eq!(inputs.len(), 1);

        // Disconnect.
        manager.disconnect_device("virtual-input-1").unwrap();
        let inputs = manager.get_input_devices().unwrap();
        assert!(inputs.is_empty());
    }

    #[test]
    fn midi_manager_preset_application() {
        let manager = MidiManager::new();
        let presets = manager.list_presets().unwrap();
        assert!(presets.contains(&"Universal Default".to_string()));

        let count = manager.apply_preset("Universal Default").unwrap();
        assert!(count > 0);
        assert_eq!(manager.mapping_count().unwrap(), count);
        assert_eq!(
            manager.active_preset().unwrap(),
            Some("Universal Default".to_string())
        );
    }

    #[test]
    fn midi_manager_custom_mapping() {
        let manager = MidiManager::new();
        manager.apply_preset("Universal Default").unwrap();

        // Add a custom mapping.
        manager
            .add_mapping(MidiMapping {
                device_id: "my-device".to_string(),
                channel: 0,
                control: 99,
                action: MidiAction::TapTempo,
                min_value: 0,
                max_value: 127,
                enabled: true,
            })
            .unwrap();

        assert_eq!(manager.mapping_count().unwrap(), 6); // 5 default + 1 custom

        // Try to add duplicate.
        assert!(manager
            .add_mapping(MidiMapping {
                device_id: "my-device".to_string(),
                channel: 0,
                control: 99,
                action: MidiAction::TapTempo,
                min_value: 0,
                max_value: 127,
                enabled: true,
            })
            .is_err());

        // Remove the custom mapping.
        manager.remove_mapping("my-device", 0, 99).unwrap();
        assert_eq!(manager.mapping_count().unwrap(), 5);
    }

    #[test]
    fn midi_manager_preset_not_found() {
        let manager = MidiManager::new();
        assert!(manager.apply_preset("Nonexistent").is_err());
    }

    #[test]
    fn midi_manager_disabled_mapping_rejected() {
        let manager = MidiManager::new();
        assert!(manager
            .add_mapping(MidiMapping {
                device_id: "test".to_string(),
                channel: 0,
                control: 1,
                action: MidiAction::Unknown,
                min_value: 0,
                max_value: 127,
                enabled: false,
            })
            .is_err());
    }

    #[test]
    fn midi_action_display() {
        assert_eq!(format!("{}", MidiAction::PlayPause), "play/pause");
        assert_eq!(format!("{}", MidiAction::Stop), "stop");
        assert_eq!(format!("{}", MidiAction::TapTempo), "tap-tempo");
        assert_eq!(format!("{}", MidiAction::LoadDeck(2)), "load-deck-2");
    }

    #[test]
    fn midi_message_is_note_and_is_cc() {
        let note = MidiMessage::note_on(0, 60, 100);
        assert!(note.is_note());
        assert!(!note.is_control_change());

        let cc = MidiMessage::control_change(0, 74, 64);
        assert!(!cc.is_note());
        assert!(cc.is_control_change());
    }

    #[test]
    fn eq_band_display() {
        assert_eq!(format!("{}", MidiEqBand::Low), "low");
        assert_eq!(format!("{}", MidiEqBand::Mid), "mid");
        assert_eq!(format!("{}", MidiEqBand::High), "high");
    }
}
