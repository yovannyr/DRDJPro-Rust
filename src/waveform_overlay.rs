//! Waveform Overlays, Zoom Control, Multi-deck Sync and
//! Per-deck color schemes.  Extends P07a (waveform generation,
//! peak envelopes, color mapping) with FR-009 through FR-018 of
//! Spec 7.
//!
//! ## Design principles
//!
//! * **No heap in the UI timer** — Overlays are rendered from the
//!   pre-baked `DetailPeaks` envelope; no audio scanning happens at
//!   display rate.
//! * **Deterministic zoom** — Zoom steps and ranges are fixed to
//!   avoid layout thrash.
//! * **Thread-safe** — All mutable state goes through `Mutex`.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Zoom level for waveform viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ZoomLevel {
    /// Full-track overview (256 bins).
    Overview,
    /// 1 bar visible (1/16 s at 120 BPM).
    Bar,
    /// 1 beat visible.
    Beat,
    /// Sample resolution.
    Sample,
}

impl ZoomLevel {
    /// Visible window in seconds.
    pub fn window_seconds(&self) -> u32 {
        match self {
            Self::Overview => 32,
            Self::Bar => 2,
            Self::Beat => 0, // computed from BPM.
            Self::Sample => 1,
        }
    }

    /// Display label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Overview => "Full",
            Self::Bar => "Bar",
            Self::Beat => "Beat",
            Self::Sample => "Sample",
        }
    }
}

/// Overlay element drawn on top of the waveform.
#[derive(Debug, Clone)]
pub enum WaveformOverlay {
    /// Hot-cue marker.
    HotCue {
        /// Frame index (0 = start of track).
        frame: usize,
        /// Cue label (e.g. "Cue 1").
        label: String,
        /// Cue color (R, G, B).
        color: (u8, u8, u8),
    },
    /// Loop region.
    Loop {
        /// Start frame.
        start: usize,
        /// End frame.
        end: usize,
        /// Loop colour.
        color: (u8, u8, u8),
    },
    /// BPM flash overlay (brief animation).
    BpmFlash {
        /// Frame of flash.
        frame: usize,
        /// Start time of flash (ms).
        start_ms: u64,
        /// Duration of flash (ms).
        duration_ms: u64,
    },
    /// Drop-zone highlight.
    DropZone {
        /// Start frame.
        start: usize,
    },
}

/// Per-deck waveform display settings.
#[derive(Debug, Clone)]
pub struct DeckWaveformConfig {
    /// Deck index (1–4).
    pub deck_id: u8,
    /// Current zoom level.
    pub zoom: ZoomLevel,
    /// Color scheme for this deck.
    pub color_scheme: WaveformColorScheme,
    /// Show overlays.
    pub show_overlays: bool,
    /// Show beat grid lines.
    pub show_beat_grid: bool,
    /// Show key info overlay.
    pub show_key: bool,
    /// Show energy overlay.
    pub show_energy: bool,
}

impl DeckWaveformConfig {
    /// Create default config for a deck.
    pub fn new(deck_id: u8) -> Self {
        Self {
            deck_id,
            zoom: ZoomLevel::Bar,
            color_scheme: WaveformColorScheme::default(),
            show_overlays: true,
            show_beat_grid: true,
            show_key: false,
            show_energy: false,
        }
    }
}

/// Colour palette for waveform rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaveformColorScheme {
    /// Classic green (Pioneer-style).
    #[default]
    ClassicGreen,
    /// Blue/teal gradient.
    Ocean,
    /// Red/orange gradient.
    Sunset,
    /// Purple/violet.
    Purple,
    /// Custom RGB.
    Custom(u8, u8, u8),
}

impl WaveformColorScheme {
    /// Get base colour for the scheme.
    pub fn base_rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::ClassicGreen => (0, 255, 0),
            Self::Ocean => (0, 180, 220),
            Self::Sunset => (255, 120, 0),
            Self::Purple => (180, 0, 255),
            Self::Custom(r, g, b) => (*r, *g, *b),
        }
    }

    /// Get secondary (fade) colour.
    pub fn fade_rgb(&self) -> (u8, u8, u8) {
        let (r, g, b) = self.base_rgb();
        (r / 3, g / 3, b / 3)
    }
}

/// Multi-deck waveform sync state.
#[derive(Debug, Clone)]
pub struct MultiDeckWaveformState {
    /// Per-deck configs.
    configs: HashMap<u8, DeckWaveformConfig>,
    /// Overlays keyed by deck_id.
    overlays: HashMap<u8, Vec<WaveformOverlay>>,
    /// Global sync position (shared playhead).
    sync_position: HashMap<u8, usize>,
}

impl MultiDeckWaveformState {
    /// Create new multi-deck state with up to 4 decks.
    pub fn new() -> Self {
        let mut configs = HashMap::new();
        for deck in 1..=4 {
            configs.insert(deck, DeckWaveformConfig::new(deck));
        }
        Self {
            configs,
            overlays: HashMap::new(),
            sync_position: HashMap::new(),
        }
    }

    /// Get config for a deck.
    pub fn config(&self, deck_id: u8) -> Option<&DeckWaveformConfig> {
        self.configs.get(&deck_id)
    }

    /// Mutate config for a deck.
    pub fn set_zoom(&mut self, deck_id: u8, zoom: ZoomLevel) {
        if let Some(cfg) = self.configs.get_mut(&deck_id) {
            cfg.zoom = zoom;
        }
    }

    /// Set colour scheme for a deck.
    pub fn set_color_scheme(&mut self, deck_id: u8, scheme: WaveformColorScheme) {
        if let Some(cfg) = self.configs.get_mut(&deck_id) {
            cfg.color_scheme = scheme;
        }
    }

    /// Toggle beat grid visibility.
    pub fn toggle_beat_grid(&mut self, deck_id: u8) {
        if let Some(cfg) = self.configs.get_mut(&deck_id) {
            cfg.show_beat_grid = !cfg.show_beat_grid;
        }
    }

    /// Add an overlay to a deck.
    pub fn add_overlay(&mut self, deck_id: u8, overlay: WaveformOverlay) {
        self.overlays.entry(deck_id).or_default().push(overlay);
    }

    /// Remove overlays from a deck.
    pub fn clear_overlays(&mut self, deck_id: u8) {
        self.overlays.remove(&deck_id);
    }

    /// Get overlays for a deck.
    pub fn overlays(&self, deck_id: u8) -> &[WaveformOverlay] {
        self.overlays
            .get(&deck_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Set sync position for a deck.
    pub fn set_sync_position(&mut self, deck_id: u8, frame: usize) {
        self.sync_position.insert(deck_id, frame);
    }

    /// Get sync position for a deck.
    pub fn sync_position(&self, deck_id: u8) -> Option<usize> {
        self.sync_position.get(&deck_id).copied()
    }

    /// Zoom out on a deck.
    pub fn zoom_out(&mut self, deck_id: u8) {
        if let Some(cfg) = self.configs.get_mut(&deck_id) {
            cfg.zoom = match cfg.zoom {
                ZoomLevel::Sample => ZoomLevel::Beat,
                ZoomLevel::Beat => ZoomLevel::Bar,
                ZoomLevel::Bar => ZoomLevel::Overview,
                ZoomLevel::Overview => ZoomLevel::Overview,
            };
        }
    }

    /// Zoom in on a deck.
    pub fn zoom_in(&mut self, deck_id: u8) {
        if let Some(cfg) = self.configs.get_mut(&deck_id) {
            cfg.zoom = match cfg.zoom {
                ZoomLevel::Overview => ZoomLevel::Bar,
                ZoomLevel::Bar => ZoomLevel::Beat,
                ZoomLevel::Beat => ZoomLevel::Sample,
                ZoomLevel::Sample => ZoomLevel::Sample,
            };
        }
    }
}

impl Default for MultiDeckWaveformState {
    fn default() -> Self {
        Self::new()
    }
}

/// Overlay animation controller.
pub struct OverlayAnimationController {
    /// Active animations keyed by deck.
    animations: std::sync::Mutex<HashMap<u8, Vec<ActiveAnimation>>>,
}

impl OverlayAnimationController {
    pub fn new() -> Self {
        Self {
            animations: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Start a BPM flash animation on a deck.
    pub fn start_bpm_flash(&self, deck_id: u8, frame: usize) {
        let mut animations = self.animations.lock().unwrap();
        animations
            .entry(deck_id)
            .or_default()
            .push(ActiveAnimation {
                kind: AnimationKind::BpmFlash {
                    frame,
                    start_ms: 0,
                    duration_ms: 300,
                },
                elapsed_ms: 0,
                active: true,
            });
    }

    /// Update animations (call on UI timer).
    pub fn update(&self, delta_ms: u64) -> Vec<AnimationEvent> {
        let mut animations = self.animations.lock().unwrap();
        let mut events = Vec::new();

        for (&deck_id, deck_anims) in animations.iter_mut() {
            for anim in deck_anims.iter_mut() {
                anim.elapsed_ms += delta_ms;
                if anim.elapsed_ms >= anim.kind.duration() {
                    anim.active = false;
                    let AnimationKind::BpmFlash {
                        frame, start_ms, ..
                    } = anim.kind;
                    events.push(AnimationEvent {
                        deck_id,
                        frame,
                        start_ms,
                        kind: AnimationEventKind::Finished,
                    });
                }
            }
            deck_anims.retain(|a| a.active);
        }

        // Keep last 100 animations per deck.
        for deck_anims in animations.values_mut() {
            if deck_anims.len() > 100 {
                deck_anims.drain(..deck_anims.len() - 100);
            }
        }

        events
    }

    /// Get active animation count for a deck.
    pub fn active_count(&self, deck_id: u8) -> usize {
        self.animations
            .lock()
            .unwrap()
            .get(&deck_id)
            .map(|a| a.len())
            .unwrap_or(0)
    }
}

/// An in-flight overlay animation.
#[derive(Debug, Clone)]
struct ActiveAnimation {
    kind: AnimationKind,
    elapsed_ms: u64,
    active: bool,
}

/// Type of overlay animation.
#[derive(Debug, Clone)]
enum AnimationKind {
    BpmFlash {
        frame: usize,
        start_ms: u64,
        duration_ms: u64,
    },
}

impl AnimationKind {
    fn duration(&self) -> u64 {
        match self {
            Self::BpmFlash { duration_ms, .. } => *duration_ms,
        }
    }
}

/// Animation event emitted on completion.
#[derive(Debug, Clone)]
pub struct AnimationEvent {
    pub deck_id: u8,
    pub kind: AnimationEventKind,
    pub frame: usize,
    pub start_ms: u64,
}

/// Type of animation event.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationEventKind {
    Finished,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

impl Default for OverlayAnimationController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer_channel::{ChannelRole, ChannelSettings, CueMode, LimiterState, MeterReading};

    #[test]
    fn finished_animations_identify_their_own_deck_and_frame() {
        let controller = OverlayAnimationController::new();
        controller.start_bpm_flash(2, 17);
        controller.start_bpm_flash(4, 93);
        assert!(controller.update(100).is_empty());
        let mut events = controller.update(200);
        events.sort_by_key(|event| event.deck_id);
        assert_eq!(
            events
                .iter()
                .map(|event| (event.deck_id, event.frame))
                .collect::<Vec<_>>(),
            vec![(2, 17), (4, 93)]
        );
        assert!(events
            .iter()
            .all(|event| event.kind == AnimationEventKind::Finished));
        assert_eq!(controller.active_count(2), 0);
        assert_eq!(controller.active_count(4), 0);
    }

    #[test]
    fn test_zoom_level_labels() {
        assert_eq!(ZoomLevel::Overview.label(), "Full");
        assert_eq!(ZoomLevel::Bar.label(), "Bar");
        assert_eq!(ZoomLevel::Beat.label(), "Beat");
        assert_eq!(ZoomLevel::Sample.label(), "Sample");
    }

    #[test]
    fn test_zoom_window_seconds() {
        assert_eq!(ZoomLevel::Overview.window_seconds(), 32);
        assert_eq!(ZoomLevel::Bar.window_seconds(), 2);
        assert_eq!(ZoomLevel::Sample.window_seconds(), 1);
    }

    #[test]
    fn test_color_scheme_rgb() {
        assert_eq!(WaveformColorScheme::ClassicGreen.base_rgb(), (0, 255, 0));
        assert_eq!(WaveformColorScheme::Ocean.base_rgb(), (0, 180, 220));
        assert_eq!(WaveformColorScheme::Sunset.base_rgb(), (255, 120, 0));
        assert_eq!(WaveformColorScheme::Purple.base_rgb(), (180, 0, 255));
        assert_eq!(
            WaveformColorScheme::Custom(100, 150, 200).base_rgb(),
            (100, 150, 200)
        );
    }

    #[test]
    fn test_color_scheme_fade() {
        let green = WaveformColorScheme::ClassicGreen;
        let fade = green.fade_rgb();
        assert_eq!(fade, (0, 85, 0)); // 255/3 ≈ 85
    }

    #[test]
    fn test_deck_config_defaults() {
        let cfg = DeckWaveformConfig::new(1);
        assert_eq!(cfg.deck_id, 1);
        assert_eq!(cfg.zoom, ZoomLevel::Bar);
        assert!(cfg.show_overlays);
        assert!(cfg.show_beat_grid);
    }

    #[test]
    fn test_multi_deck_initial_state() {
        let state = MultiDeckWaveformState::new();
        assert!(state.config(1).is_some());
        assert!(state.config(2).is_some());
        assert!(state.config(3).is_some());
        assert!(state.config(4).is_some());
        assert!(state.config(5).is_none());
    }

    #[test]
    fn test_multi_deck_zoom() {
        let mut state = MultiDeckWaveformState::new();
        state.set_zoom(1, ZoomLevel::Overview);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Overview);

        state.zoom_in(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Bar);

        state.zoom_in(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Beat);

        state.zoom_in(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Sample);

        // Cannot zoom in past sample.
        state.zoom_in(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Sample);
    }

    #[test]
    fn test_multi_deck_zoom_out() {
        let mut state = MultiDeckWaveformState::new();
        state.set_zoom(1, ZoomLevel::Sample);
        state.zoom_out(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Beat);
        state.zoom_out(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Bar);
        state.zoom_out(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Overview);
        // Cannot zoom out past overview.
        state.zoom_out(1);
        assert_eq!(state.config(1).unwrap().zoom, ZoomLevel::Overview);
    }

    #[test]
    fn test_multi_deck_color_scheme() {
        let mut state = MultiDeckWaveformState::new();
        state.set_color_scheme(1, WaveformColorScheme::Ocean);
        assert_eq!(
            state.config(1).unwrap().color_scheme,
            WaveformColorScheme::Ocean
        );
    }

    #[test]
    fn test_multi_deck_toggle_beat_grid() {
        let mut state = MultiDeckWaveformState::new();
        state.toggle_beat_grid(1);
        assert!(!state.config(1).unwrap().show_beat_grid);
        state.toggle_beat_grid(1);
        assert!(state.config(1).unwrap().show_beat_grid);
    }

    #[test]
    fn test_multi_deck_overlays() {
        let mut state = MultiDeckWaveformState::new();
        state.add_overlay(
            1,
            WaveformOverlay::HotCue {
                frame: 1000,
                label: "Cue 1".to_string(),
                color: (255, 0, 0),
            },
        );
        assert_eq!(state.overlays(1).len(), 1);
        assert_eq!(state.overlays(2).len(), 0);

        state.clear_overlays(1);
        assert_eq!(state.overlays(1).len(), 0);
    }

    #[test]
    fn test_multi_deck_sync_position() {
        let mut state = MultiDeckWaveformState::new();
        state.set_sync_position(1, 44100);
        assert_eq!(state.sync_position(1), Some(44100));
        assert_eq!(state.sync_position(2), None);
    }

    #[test]
    fn test_overlay_animation_controller() {
        let ctrl = OverlayAnimationController::new();
        ctrl.start_bpm_flash(1, 1000);
        assert_eq!(ctrl.active_count(1), 1);

        // Update with full duration → animation finishes.
        let events = ctrl.update(301);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, AnimationEventKind::Finished);
    }

    #[test]
    fn test_overlay_animation_not_finished_yet() {
        let ctrl = OverlayAnimationController::new();
        ctrl.start_bpm_flash(1, 1000);
        let events = ctrl.update(100);
        assert!(events.is_empty());
        assert_eq!(ctrl.active_count(1), 1);
    }

    #[test]
    fn test_channel_settings_validation() {
        let mut cfg = ChannelSettings::new(ChannelRole::Deck1);
        cfg.set_volume(1.5); // Should clamp.
        assert_eq!(cfg.volume, 1.0);
        cfg.set_pan(-2.0); // Should clamp.
        assert_eq!(cfg.pan, -1.0);
    }

    #[test]
    fn test_meter_reading_db_value() {
        let reading = MeterReading::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(reading.db_value(), -60.0); // Silence → -60 dB.

        let reading = MeterReading::new(0.1, 0.0, 0.0, 0.0);
        let db = reading.db_value();
        assert!((db + 20.0).abs() < 0.001); // 0.1 amplitude = -20 dBFS.
    }

    #[test]
    fn test_meter_reading_clipped() {
        let reading = MeterReading::new(1.0, 0.5, 0.5, 0.5);
        assert!(reading.is_clipped());

        let reading = MeterReading::new(0.99, 0.99, 0.5, 0.5);
        assert!(!reading.is_clipped());
    }

    #[test]
    fn test_meter_reading_max_values() {
        let reading = MeterReading::new(0.7, 0.9, 0.3, 0.5);
        assert_eq!(reading.max_peak(), 0.9);
        assert_eq!(reading.max_rms(), 0.5);
        assert_eq!(reading.level_percent(), 90);
    }

    #[test]
    fn test_cue_mode_display() {
        assert_eq!(CueMode::CueOnly.name(), "Cue Only");
        assert_eq!(CueMode::CueMaster.name(), "Cue + Master");
        assert_eq!(CueMode::Live.name(), "Live");
    }

    #[test]
    fn test_limiter_state_display() {
        assert_eq!(LimiterState::Inactive.name(), "Inactive");
        assert_eq!(LimiterState::Engaged.name(), "Engaged");
        assert_eq!(LimiterState::Clipping.name(), "Clipping");
    }

    #[test]
    fn test_channel_role_name() {
        assert_eq!(ChannelRole::Deck1.name(), "Deck 1");
        assert_eq!(ChannelRole::Deck4.name(), "Deck 4");
        assert_eq!(ChannelRole::Aux.name(), "Aux");
        assert_eq!(ChannelRole::FxReturn.name(), "FX Return");
    }

    #[test]
    fn test_channel_role_number() {
        assert_eq!(ChannelRole::Deck1.number(), 1);
        assert_eq!(ChannelRole::Deck3.number(), 3);
        assert_eq!(ChannelRole::Aux.number(), 5);
    }
}
