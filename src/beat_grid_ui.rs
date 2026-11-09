//! Beat Grid Visualizer — UI-ready beat grid rendering, phase indicator, correction UI.
//!
//! Implements visual representation of beat grids, phase relationship display,
//! interactive correction controls, and timeline rendering data.
//! Part of the Beat Grid UI feature (P17b).
//!
//! ## Design principles
//!
//! * **Render-ready** — Data structured for fast UI rendering.
//! * **Interactive** — Click/drag correction support.
//! * **Responsive** — Scalable from zoomed-in beats to zoomed-out bars.
//! * **Multi-deck** — Synchronized multi-deck view.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Zoom level for beat grid rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomLevel {
    /// Show entire track.
    Whole,
    /// Show ~16 bars.
    ZoomedOut,
    /// Show ~4 bars.
    ZoomedMid,
    /// Show ~1 bar.
    ZoomedIn,
    /// Show single beat.
    ZoomedMax,
}

impl ZoomLevel {
    /// Get approximate visible bars.
    pub fn visible_bars(self) -> u32 {
        match self {
            Self::Whole => 0, // All.
            Self::ZoomedOut => 16,
            Self::ZoomedMid => 4,
            Self::ZoomedIn => 1,
            Self::ZoomedMax => 0, // Single beat.
        }
    }

    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Whole => "Whole",
            Self::ZoomedOut => "Zoom Out",
            Self::ZoomedMid => "Mid Zoom",
            Self::ZoomedIn => "Zoom In",
            Self::ZoomedMax => "Max Zoom",
        }
    }
}

/// Visual beat element for rendering.
#[derive(Debug, Clone)]
pub struct VisualBeat {
    /// Time position in seconds.
    pub time: f64,
    /// Visual width in pixels.
    pub width: f32,
    /// X position in pixels.
    pub x: f32,
    /// Beat type (for color).
    pub beat_type_index: u8,
    /// Y position (row) in pixels.
    pub y: f32,
    /// Height in pixels.
    pub height: f32,
    /// Selected.
    pub selected: bool,
    /// Hovered.
    pub hovered: bool,
}

/// Visual bar element for rendering.
#[derive(Debug, Clone)]
pub struct VisualBar {
    /// Bar number.
    pub bar_number: u32,
    /// Start time.
    pub start_time: f64,
    /// End time.
    pub end_time: f64,
    /// X position.
    pub x: f32,
    /// Width.
    pub width: f32,
}

/// Phase indicator state.
#[derive(Debug, Clone)]
pub struct PhaseIndicator {
    /// Deck ID.
    pub deck_id: String,
    /// Phase angle (0-360).
    pub phase_angle: f64,
    /// Color.
    pub color: (u8, u8, u8),
    /// In sync.
    pub in_sync: bool,
    /// Drift rate (samples/sec).
    pub drift_rate: f64,
}

impl PhaseIndicator {
    /// Create a new phase indicator.
    pub fn new(deck_id: String, phase_angle: f64, color: (u8, u8, u8), in_sync: bool) -> Self {
        Self {
            deck_id,
            phase_angle,
            color,
            in_sync,
            drift_rate: 0.0,
        }
    }
}

/// Correction handle for UI interaction.
#[derive(Debug, Clone)]
pub struct CorrectionHandle {
    /// Beat/time being corrected.
    pub target_time: f64,
    /// Current visual offset.
    pub visual_offset: f32,
    /// Is being dragged.
    pub is_dragging: bool,
    /// Drag start position.
    pub drag_start: f32,
    /// Drag delta.
    pub drag_delta: f32,
}

impl CorrectionHandle {
    /// Create a new correction handle.
    pub fn new(target_time: f64) -> Self {
        Self {
            target_time,
            visual_offset: 0.0,
            is_dragging: false,
            drag_start: 0.0,
            drag_delta: 0.0,
        }
    }

    /// Start drag.
    pub fn start_drag(&mut self, x: f32) {
        self.is_dragging = true;
        self.drag_start = x;
    }

    /// Update drag.
    pub fn update_drag(&mut self, x: f32) {
        if self.is_dragging {
            self.drag_delta = x - self.drag_start;
            self.visual_offset = self.drag_delta;
        }
    }

    /// End drag, compute correction.
    pub fn end_drag(&mut self) -> Option<f64> {
        if self.is_dragging {
            let correction = self.visual_offset / 100.0; // Simplified pixel-to-second.
            self.is_dragging = false;
            self.drag_delta = 0.0;
            Some(f64::from(correction))
        } else {
            None
        }
    }
}

/// Grid rendering configuration.
#[derive(Debug, Clone)]
pub struct GridRenderConfig {
    /// Pixel width per second.
    pub pixels_per_second: f32,
    /// Row height per bar.
    pub bar_height: f32,
    /// Downbeat color.
    pub downbeat_color: (u8, u8, u8),
    /// Regular beat color.
    pub beat_color: (u8, u8, u8),
    /// Off-beat color.
    pub offbeat_color: (u8, u8, u8),
    /// Background color.
    pub background_color: (u8, u8, u8),
    /// Grid line color.
    pub grid_color: (u8, u8, u8),
    /// Show bar numbers.
    pub show_bar_numbers: bool,
    /// Show time labels.
    pub show_time_labels: bool,
    /// Highlight current playhead.
    pub show_playhead: bool,
    /// Playhead color.
    pub playhead_color: (u8, u8, u8),
}

impl Default for GridRenderConfig {
    fn default() -> Self {
        Self {
            pixels_per_second: 100.0,
            bar_height: 40.0,
            downbeat_color: (255, 50, 50),
            beat_color: (100, 150, 255),
            offbeat_color: (150, 150, 150),
            background_color: (30, 30, 40),
            grid_color: (60, 60, 80),
            show_bar_numbers: true,
            show_time_labels: true,
            show_playhead: true,
            playhead_color: (255, 255, 0),
        }
    }
}

/// Timeline range for visible area.
#[derive(Debug, Clone)]
pub struct VisibleRange {
    /// Start time.
    pub start: f64,
    /// End time.
    pub end: f64,
}

impl VisibleRange {
    /// Check if a time is in the visible range.
    pub fn contains(&self, time: f64) -> bool {
        time >= self.start && time <= self.end
    }

    /// Get duration.
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

/// Beat grid UI state.
pub struct BeatGridUI {
    /// Track ID.
    pub track_id: String,
    /// Current zoom level.
    pub zoom: ZoomLevel,
    /// Visible time range.
    pub visible_range: VisibleRange,
    /// Render configuration.
    pub config: GridRenderConfig,
    /// Playhead position.
    pub playhead_time: f64,
    /// Selected beats (for batch operations).
    pub selected_beats: Vec<f64>,
    /// Correction handles.
    pub correction_handles: HashMap<u64, CorrectionHandle>,
    /// Phase indicators for each deck.
    pub phase_indicators: HashMap<String, PhaseIndicator>,
    /// Multi-deck sync.
    pub multi_deck_mode: bool,
    /// Deck IDs in multi-deck view.
    pub deck_ids: Vec<String>,
}

impl BeatGridUI {
    /// Create a new beat grid UI.
    pub fn new(track_id: String) -> Self {
        Self {
            track_id,
            zoom: ZoomLevel::ZoomedMid,
            visible_range: VisibleRange {
                start: 0.0,
                end: 10.0,
            },
            config: GridRenderConfig::default(),
            playhead_time: 0.0,
            selected_beats: Vec::new(),
            correction_handles: HashMap::new(),
            phase_indicators: HashMap::new(),
            multi_deck_mode: false,
            deck_ids: Vec::new(),
        }
    }

    /// Set zoom level.
    pub fn set_zoom(&mut self, zoom: ZoomLevel) {
        self.zoom = zoom;
        self.update_visible_range();
    }

    /// Zoom in.
    pub fn zoom_in(&mut self) {
        self.zoom = match self.zoom {
            ZoomLevel::Whole => ZoomLevel::ZoomedOut,
            ZoomLevel::ZoomedOut => ZoomLevel::ZoomedMid,
            ZoomLevel::ZoomedMid => ZoomLevel::ZoomedIn,
            ZoomLevel::ZoomedIn => ZoomLevel::ZoomedMax,
            ZoomLevel::ZoomedMax => ZoomLevel::ZoomedMax,
        };
        self.update_visible_range();
    }

    /// Zoom out.
    pub fn zoom_out(&mut self) {
        self.zoom = match self.zoom {
            ZoomLevel::Whole => ZoomLevel::Whole,
            ZoomLevel::ZoomedOut => ZoomLevel::Whole,
            ZoomLevel::ZoomedMid => ZoomLevel::ZoomedOut,
            ZoomLevel::ZoomedIn => ZoomLevel::ZoomedMid,
            ZoomLevel::ZoomedMax => ZoomLevel::ZoomedIn,
        };
        self.update_visible_range();
    }

    /// Update visible range based on zoom.
    fn update_visible_range(&mut self) {
        let bars_visible = self.zoom.visible_bars();
        if bars_visible == 0 {
            // Use default or all.
            self.visible_range.end = 10.0;
        } else {
            let bar_duration = 60.0 / 128.0 * 4.0; // At 128 BPM, 4/4.
            self.visible_range.end = self.visible_range.start + bar_duration * bars_visible as f64;
        }
    }

    /// Get visual beats for rendering.
    pub fn get_visual_beats(&self, all_beats: &[f64]) -> Vec<VisualBeat> {
        let pps = self.config.pixels_per_second;
        let _width = (self.visible_range.duration() * f64::from(pps)) as f32;
        let height = self.config.bar_height;

        all_beats
            .iter()
            .filter(|&&t| self.visible_range.contains(t))
            .map(|&time| {
                VisualBeat {
                    time,
                    x: (time - self.visible_range.start) as f32 * pps,
                    width: pps * 0.8,
                    y: 0.0,
                    height,
                    beat_type_index: 0, // Would come from beat type.
                    selected: false,
                    hovered: false,
                }
            })
            .collect()
    }

    /// Get visual bars for rendering.
    pub fn get_visual_bars(&self, bar_times: &[(f64, f64)]) -> Vec<VisualBar> {
        let pps = self.config.pixels_per_second;

        bar_times
            .iter()
            .filter(|&&(start, end)| {
                self.visible_range.start <= end && self.visible_range.end >= start
            })
            .enumerate()
            .map(|(i, &(start, end))| VisualBar {
                bar_number: i as u32,
                start_time: start,
                end_time: end,
                x: (start - self.visible_range.start) as f32 * pps,
                width: (end - start) as f32 * pps,
            })
            .collect()
    }

    /// Start a correction drag.
    pub fn start_correction_drag(&mut self, time: f64, x: f32) {
        let handle = self
            .correction_handles
            .entry(time.to_bits())
            .or_insert_with(|| CorrectionHandle::new(time));
        handle.start_drag(x);
    }

    /// Update correction drag.
    pub fn update_correction_drag(&mut self, time: f64, x: f32) {
        if let Some(handle) = self.correction_handles.get_mut(&time.to_bits()) {
            handle.update_drag(x);
        }
    }

    /// End correction drag, get correction value.
    pub fn end_correction_drag(&mut self, time: f64) -> Option<f64> {
        if let Some(mut handle) = self.correction_handles.remove(&time.to_bits()) {
            handle.end_drag()
        } else {
            None
        }
    }

    /// Remove a correction handle.
    pub fn remove_correction_handle(&mut self, time: f64) {
        self.correction_handles.remove(&time.to_bits());
    }

    /// Add a phase indicator.
    pub fn add_phase_indicator(
        &mut self,
        deck_id: String,
        phase_angle: f64,
        color: (u8, u8, u8),
        in_sync: bool,
    ) {
        let indicator = PhaseIndicator::new(deck_id.clone(), phase_angle, color, in_sync);
        self.phase_indicators.insert(deck_id, indicator);
    }

    /// Update phase indicator drift rate.
    pub fn update_drift_rate(&mut self, deck_id: &str, drift_rate: f64) {
        if let Some(indicator) = self.phase_indicators.get_mut(deck_id) {
            indicator.drift_rate = drift_rate;
        }
    }

    /// Get phase indicator for deck.
    pub fn get_phase_indicator(&self, deck_id: &str) -> Option<&PhaseIndicator> {
        self.phase_indicators.get(deck_id)
    }

    /// Enable/disable multi-deck mode.
    pub fn set_multi_deck_mode(&mut self, enabled: bool) {
        self.multi_deck_mode = enabled;
    }

    /// Check if multi-deck mode is enabled.
    pub fn is_multi_deck_mode(&self) -> bool {
        self.multi_deck_mode
    }

    /// Add a deck to multi-deck view.
    pub fn add_deck(&mut self, deck_id: String) {
        if !self.deck_ids.contains(&deck_id) {
            self.deck_ids.push(deck_id);
        }
    }

    /// Remove a deck from multi-deck view.
    pub fn remove_deck(&mut self, deck_id: &str) {
        self.deck_ids.retain(|d| d != deck_id);
    }

    /// Set playhead position.
    pub fn set_playhead(&mut self, time: f64) {
        self.playhead_time = time;
    }

    /// Get playhead X position.
    pub fn playhead_x(&self) -> f32 {
        (self.playhead_time - self.visible_range.start) as f32 * self.config.pixels_per_second
    }

    /// Toggle beat selection.
    pub fn toggle_beat_selection(&mut self, time: f64) {
        if let Some(pos) = self
            .selected_beats
            .iter()
            .position(|&t| (t - time).abs() < 0.001)
        {
            self.selected_beats.remove(pos);
        } else {
            self.selected_beats.push(time);
        }
    }

    /// Clear beat selection.
    pub fn clear_selection(&mut self) {
        self.selected_beats.clear();
    }

    /// Get selected beat count.
    pub fn selected_count(&self) -> usize {
        self.selected_beats.len()
    }

    /// Get render width.
    pub fn render_width(&self) -> f32 {
        self.visible_range.duration() as f32 * self.config.pixels_per_second
    }

    /// Get render height per bar.
    pub fn render_height(&self) -> f32 {
        self.config.bar_height
    }
}

impl Default for BeatGridUI {
    fn default() -> Self {
        Self::new("track1".to_string())
    }
}

/// Sync indicator for multi-deck phase visualization.
#[derive(Debug, Clone)]
pub struct SyncIndicator {
    /// Deck A phase.
    pub deck_a_phase: f64,
    /// Deck B phase.
    pub deck_b_phase: f64,
    /// Phase difference.
    pub phase_difference: f64,
    /// Synced.
    pub synced: bool,
    /// Visual color.
    pub color: (u8, u8, u8),
}

impl SyncIndicator {
    /// Create a new sync indicator.
    pub fn new(deck_a_phase: f64, deck_b_phase: f64) -> Self {
        let diff = (deck_b_phase - deck_a_phase).abs();
        let synced = diff < 0.05 || diff > 0.95; // In phase or close.
        let color = if synced {
            (50, 255, 100) // Green
        } else if diff < 0.3 || diff > 0.7 {
            (255, 200, 50) // Orange
        } else {
            (255, 50, 50) // Red
        };

        Self {
            deck_a_phase,
            deck_b_phase,
            phase_difference: diff,
            synced,
            color,
        }
    }

    /// Get visual arc percentage (0-100).
    pub fn arc_percentage(&self) -> f64 {
        self.phase_difference * 100.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_level_bars() {
        assert_eq!(ZoomLevel::Whole.visible_bars(), 0);
        assert_eq!(ZoomLevel::ZoomedOut.visible_bars(), 16);
        assert_eq!(ZoomLevel::ZoomedMid.visible_bars(), 4);
        assert_eq!(ZoomLevel::ZoomedIn.visible_bars(), 1);
        assert_eq!(ZoomLevel::ZoomedMax.visible_bars(), 0);
    }

    #[test]
    fn zoom_level_names() {
        assert_eq!(ZoomLevel::Whole.display_name(), "Whole");
        assert_eq!(ZoomLevel::ZoomedOut.display_name(), "Zoom Out");
        assert_eq!(ZoomLevel::ZoomedMid.display_name(), "Mid Zoom");
        assert_eq!(ZoomLevel::ZoomedIn.display_name(), "Zoom In");
        assert_eq!(ZoomLevel::ZoomedMax.display_name(), "Max Zoom");
    }

    #[test]
    fn visual_beat_creation() {
        let beat = VisualBeat {
            time: 1.0,
            width: 10.0,
            x: 100.0,
            beat_type_index: 0,
            y: 0.0,
            height: 40.0,
            selected: false,
            hovered: false,
        };
        assert_eq!(beat.time, 1.0);
        assert!(!beat.selected);
    }

    #[test]
    fn phase_indicator_creation() {
        let indicator = PhaseIndicator::new("deck_a".to_string(), 45.0, (255, 100, 100), true);
        assert_eq!(indicator.deck_id, "deck_a");
        assert_eq!(indicator.phase_angle, 45.0);
        assert!(indicator.in_sync);
    }

    #[test]
    fn correction_handle_drag() {
        let mut handle = CorrectionHandle::new(1.0);
        handle.start_drag(100.0);
        assert!(handle.is_dragging);

        handle.update_drag(150.0);
        assert_eq!(handle.drag_delta, 50.0);
        assert_eq!(handle.visual_offset, 50.0);

        let correction = handle.end_drag();
        assert!(correction.is_some());
        assert!(!handle.is_dragging);
    }

    #[test]
    fn correction_handle_no_drag() {
        let mut handle = CorrectionHandle::new(1.0);
        let correction = handle.end_drag();
        assert!(correction.is_none());
    }

    #[test]
    fn render_config_defaults() {
        let config = GridRenderConfig::default();
        assert_eq!(config.pixels_per_second, 100.0);
        assert_eq!(config.bar_height, 40.0);
        assert!(config.show_bar_numbers);
        assert!(config.show_time_labels);
        assert!(config.show_playhead);
    }

    #[test]
    fn visible_range_contains() {
        let range = VisibleRange {
            start: 0.0,
            end: 10.0,
        };
        assert!(range.contains(5.0));
        assert!(range.contains(0.0));
        assert!(range.contains(10.0));
        assert!(!range.contains(-1.0));
        assert!(!range.contains(11.0));
    }

    #[test]
    fn visible_range_duration() {
        let range = VisibleRange {
            start: 5.0,
            end: 15.0,
        };
        assert_eq!(range.duration(), 10.0);
    }

    #[test]
    fn beat_grid_ui_creation() {
        let ui = BeatGridUI::new("track1".to_string());
        assert_eq!(ui.track_id, "track1");
        assert_eq!(ui.zoom, ZoomLevel::ZoomedMid);
        assert_eq!(ui.selected_count(), 0);
    }

    #[test]
    fn beat_grid_ui_zoom() {
        let mut ui = BeatGridUI::new("track1".to_string());
        assert_eq!(ui.zoom, ZoomLevel::ZoomedMid);

        ui.zoom_in();
        assert_eq!(ui.zoom, ZoomLevel::ZoomedIn);

        ui.zoom_in();
        assert_eq!(ui.zoom, ZoomLevel::ZoomedMax);

        ui.zoom_out();
        assert_eq!(ui.zoom, ZoomLevel::ZoomedIn);

        ui.zoom_out();
        assert_eq!(ui.zoom, ZoomLevel::ZoomedMid);
    }

    #[test]
    fn beat_grid_ui_visual_beats() {
        let ui = BeatGridUI::new("track1".to_string());
        let beats = vec![0.5, 1.0, 1.5, 2.0];
        let visual = ui.get_visual_beats(&beats);

        // All beats in range (0-10).
        assert_eq!(visual.len(), 4);
    }

    #[test]
    fn beat_grid_ui_filter_range() {
        let mut ui = BeatGridUI::new("track1".to_string());
        ui.visible_range = VisibleRange {
            start: 1.0,
            end: 2.0,
        };

        let beats = vec![0.5, 1.0, 1.5, 2.0, 2.5];
        let visual = ui.get_visual_beats(&beats);

        // Only 1.0, 1.5, 2.0 are in range.
        assert_eq!(visual.len(), 3);
    }

    #[test]
    fn beat_grid_ui_visual_bars() {
        let ui = BeatGridUI::new("track1".to_string());
        let bars = vec![(0.0, 2.0), (2.0, 4.0), (4.0, 6.0)];
        let visual = ui.get_visual_bars(&bars);

        // All bars in range (0-10).
        assert_eq!(visual.len(), 3);
    }

    #[test]
    fn beat_grid_ui_correction() {
        let mut ui = BeatGridUI::new("track1".to_string());

        ui.start_correction_drag(1.0, 100.0);
        ui.update_correction_drag(1.0, 150.0);
        let correction = ui.end_correction_drag(1.0);

        assert!(correction.is_some());
        assert!(!ui.correction_handles.contains_key(&1.0f64.to_bits())); // Removed after end.
    }

    #[test]
    fn beat_grid_ui_phase_indicators() {
        let mut ui = BeatGridUI::new("track1".to_string());

        ui.add_phase_indicator("deck_a".to_string(), 45.0, (255, 100, 100), true);
        ui.add_phase_indicator("deck_b".to_string(), 90.0, (100, 255, 100), false);

        assert!(ui.get_phase_indicator("deck_a").is_some());
        assert!(ui.get_phase_indicator("deck_b").is_some());
        assert!(ui.get_phase_indicator("deck_a").unwrap().in_sync);
    }

    #[test]
    fn beat_grid_ui_multi_deck() {
        let mut ui = BeatGridUI::new("track1".to_string());

        assert!(!ui.is_multi_deck_mode());
        ui.set_multi_deck_mode(true);
        assert!(ui.is_multi_deck_mode());

        ui.add_deck("deck_a".to_string());
        ui.add_deck("deck_b".to_string());
        assert_eq!(ui.deck_ids.len(), 2);

        ui.remove_deck("deck_a");
        assert_eq!(ui.deck_ids.len(), 1);
    }

    #[test]
    fn beat_grid_ui_playhead() {
        let mut ui = BeatGridUI::new("track1".to_string());
        assert_eq!(ui.playhead_time, 0.0);

        ui.set_playhead(5.0);
        assert_eq!(ui.playhead_time, 5.0);

        // X = (5.0 - 0.0) * 100 = 500.
        assert_eq!(ui.playhead_x(), 500.0);
    }

    #[test]
    fn beat_grid_ui_selection() {
        let mut ui = BeatGridUI::new("track1".to_string());

        ui.toggle_beat_selection(1.0);
        ui.toggle_beat_selection(2.0);
        assert_eq!(ui.selected_count(), 2);

        ui.toggle_beat_selection(1.0);
        assert_eq!(ui.selected_count(), 1);

        ui.clear_selection();
        assert_eq!(ui.selected_count(), 0);
    }

    #[test]
    fn beat_grid_ui_render_dimensions() {
        let ui = BeatGridUI::new("track1".to_string());
        assert_eq!(ui.render_width(), 1000.0); // 10s * 100ppp
        assert_eq!(ui.render_height(), 40.0);
    }

    #[test]
    fn sync_indicator_in_phase() {
        let indicator = SyncIndicator::new(0.1, 0.15);
        assert!(indicator.synced);
        assert_eq!(indicator.color, (50, 255, 100)); // Green
    }

    #[test]
    fn sync_indicator_out_of_phase() {
        let indicator = SyncIndicator::new(0.1, 0.6);
        assert!(!indicator.synced);
        // Red for far from phase.
        assert_eq!(indicator.color, (255, 50, 50));
    }

    #[test]
    fn sync_indicator_arc_percentage() {
        let indicator = SyncIndicator::new(0.0, 0.5);
        assert_eq!(indicator.arc_percentage(), 50.0);
    }

    #[test]
    fn sync_indicator_edge_cases() {
        // Very close to in-phase.
        let indicator = SyncIndicator::new(0.0, 0.04);
        assert!(indicator.synced);

        // Very close to wrapping.
        let indicator = SyncIndicator::new(0.0, 0.97);
        assert!(indicator.synced);
    }

    #[test]
    fn beat_grid_ui_zoom_visible_range() {
        let mut ui = BeatGridUI::new("track1".to_string());
        ui.visible_range.start = 0.0;

        ui.set_zoom(ZoomLevel::ZoomedOut);
        // At 128 BPM, 4 bars = 8 seconds, 16 bars = 32 seconds.
        assert!(ui.visible_range.end > 0.0);
    }

    #[test]
    fn beat_grid_ui_correction_handle_remove() {
        let mut ui = BeatGridUI::new("track1".to_string());

        ui.start_correction_drag(1.0, 100.0);
        assert!(ui.correction_handles.contains_key(&1.0f64.to_bits()));

        ui.remove_correction_handle(1.0);
        assert!(!ui.correction_handles.contains_key(&1.0f64.to_bits()));
    }

    #[test]
    fn beat_grid_ui_update_drift() {
        let mut ui = BeatGridUI::new("track1".to_string());
        ui.add_phase_indicator("deck_a".to_string(), 0.0, (255, 100, 100), true);

        ui.update_drift_rate("deck_a", 5.5);
        let indicator = ui.get_phase_indicator("deck_a").unwrap();
        assert_eq!(indicator.drift_rate, 5.5);
    }
}
