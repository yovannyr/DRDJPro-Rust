//! Harmony Wheel UI — Interactive key compatibility visualization and selection.
//!
//! Provides the visual harmony wheel interface with key selection,
//! compatibility display, correction controls, and chromagram visualization.
//! Part of the Key Detection feature (P06b).
//!
//! ## Design principles
//!
//! * **Interactive** — Click/drag key selection on wheel.
//! * **Informative** — Clear compatibility indicators.
//! * **Accurate** — Full 24×24 compatibility table.
//! * **Consistent** — Camelot notation with musical key display.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Musical key representation (Camelot + standard).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MusicalKey {
    /// Camelot notation (1A-12A, 1B-12B).
    pub camelot: String,
    /// Standard notation (e.g., "Am", "C", "G").
    pub standard: String,
    /// Camelot number (1-12).
    pub camelot_number: u8,
    /// Camelot side (A or B).
    pub camelot_side: char,
}

impl MusicalKey {
    /// Parse from Camelot notation.
    pub fn from_camelot(camelot: &str) -> Option<Self> {
        let parsed = crate::harmonie_wheel::CamelotKey::parse(camelot)?;
        Some(Self {
            camelot: camelot.to_string(),
            standard: parsed.to_standard(),
            camelot_number: parsed.number,
            camelot_side: if parsed.mode == crate::harmonie_wheel::KeyMode::Minor {
                'A'
            } else {
                'B'
            },
        })
    }

    /// Get compatible keys.
    pub fn get_compatible_keys(&self) -> Vec<String> {
        let mut compatible = Vec::new();

        // Same position (Perfect).
        compatible.push(self.camelot.clone());

        // Same side, +/-1 (Dramatic - 1 semitone).
        if self.camelot_number > 1 {
            let num = self.camelot_number - 1;
            if self.camelot_side == 'A' {
                compatible.push(format!("{}A", num));
            } else {
                compatible.push(format!("{}B", num));
            }
        }
        if self.camelot_number < 12 {
            let num = self.camelot_number + 1;
            if self.camelot_side == 'A' {
                compatible.push(format!("{}A", num));
            } else {
                compatible.push(format!("{}B", num));
            }
        }

        // Same number, switch side (Relative - 3 semitones).
        let other_side = if self.camelot_side == 'A' { 'B' } else { 'A' };
        compatible.push(format!("{}{}", self.camelot_number, other_side));

        // Same side, +/-2 (Less Dramatic - 2 semitones).
        if self.camelot_number > 2 {
            let num = self.camelot_number - 2;
            if self.camelot_side == 'A' {
                compatible.push(format!("{}A", num));
            } else {
                compatible.push(format!("{}B", num));
            }
        }
        if self.camelot_number < 11 {
            let num = self.camelot_number + 2;
            if self.camelot_side == 'A' {
                compatible.push(format!("{}A", num));
            } else {
                compatible.push(format!("{}B", num));
            }
        }

        compatible
    }

    /// Get compatibility score for another key.
    pub fn compatibility_score(&self, other: &MusicalKey) -> f64 {
        if self.camelot == other.camelot {
            return 1.0; // Perfect
        }

        // Same side, +/-1 (Dramatic).
        if self.camelot_side == other.camelot_side {
            let diff = (self.camelot_number as i32 - other.camelot_number as i32).abs();
            if diff == 1 {
                return 0.85;
            }
            if diff == 2 {
                return 0.7;
            }
        }

        // Same number, different side (Relative).
        if self.camelot_number == other.camelot_number {
            return 0.9;
        }

        // Generic compatibility (lower).
        // Check if they share any relationship.
        let self_compat = self.get_compatible_keys();
        if self_compat.contains(&other.camelot) {
            return 0.6;
        }

        0.3 // Low compatibility
    }

    /// Get standard key name from Camelot.
    pub fn to_standard(&self) -> &str {
        // Camelot to standard key mapping (simplified).
        // 1A = Bbm, 1B = F#, 2A = Fm, 2B = C#, etc.
        // Full mapping would be extensive; returning placeholder.
        &self.standard
    }
}

/// Compatibility stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityStage {
    /// Perfect match (same key).
    Perfect,
    /// Relative match (same number, different side).
    Relative,
    /// Dramatic match (1 semitone).
    Dramatic,
    /// Less dramatic (2 semitones).
    LessDramatic,
    /// Generic (lower compatibility).
    Generic,
    /// Poor compatibility.
    Poor,
}

impl CompatibilityStage {
    /// Get label.
    pub fn label(&self) -> &str {
        match self {
            Self::Perfect => "Perfect",
            Self::Relative => "Relative",
            Self::Dramatic => "Dramatic",
            Self::LessDramatic => "Less Dramatic",
            Self::Generic => "Generic",
            Self::Poor => "Poor",
        }
    }

    /// Get color (CSS-like hex).
    pub fn color(&self) -> &str {
        match self {
            Self::Perfect => "#00ff00",
            Self::Relative => "#00cc00",
            Self::Dramatic => "#ffcc00",
            Self::LessDramatic => "#ff9900",
            Self::Generic => "#ff6600",
            Self::Poor => "#ff0000",
        }
    }

    /// Get score threshold.
    pub fn score_threshold(&self) -> f64 {
        match self {
            Self::Perfect => 1.0,
            Self::Relative => 0.9,
            Self::Dramatic => 0.85,
            Self::LessDramatic => 0.7,
            Self::Generic => 0.6,
            Self::Poor => 0.0,
        }
    }
}

/// Harmony wheel state for rendering.
#[derive(Debug, Clone)]
pub struct HarmonyWheelState {
    /// Selected key.
    pub selected_key: Option<MusicalKey>,
    /// Highlighted key (hover).
    pub hovered_key: Option<MusicalKey>,
    /// Comparison key (for mixing).
    pub comparison_key: Option<MusicalKey>,
    /// Wheel radius.
    pub radius: f64,
    /// Wheel center (x, y).
    pub center: (f64, f64),
    /// Show A side.
    pub show_a_side: bool,
    /// Show B side.
    pub show_b_side: bool,
    /// Show compatibility hints.
    pub show_hints: bool,
    /// Zoom level.
    pub zoom: f64,
    /// Rotation offset (radians).
    pub rotation: f64,
}

impl HarmonyWheelState {
    pub fn set_zoom(&mut self, zoom: f64) {
        if zoom.is_finite() {
            self.zoom = zoom.clamp(0.25, 4.0);
        }
    }
    /// Create new wheel state.
    pub fn new() -> Self {
        Self {
            selected_key: None,
            hovered_key: None,
            comparison_key: None,
            radius: 200.0,
            center: (400.0, 400.0),
            show_a_side: true,
            show_b_side: true,
            show_hints: true,
            zoom: 1.0,
            rotation: 0.0,
        }
    }

    /// Set selected key.
    pub fn set_selected_key(&mut self, key: MusicalKey) {
        self.selected_key = Some(key);
    }

    /// Set comparison key.
    pub fn set_comparison_key(&mut self, key: MusicalKey) {
        self.comparison_key = Some(key);
    }

    /// Get selected key.
    pub fn selected_key(&self) -> Option<&MusicalKey> {
        self.selected_key.as_ref()
    }

    /// Get compatibility between selected and comparison key.
    pub fn compatibility(&self) -> Option<(f64, CompatibilityStage)> {
        match (&self.selected_key, &self.comparison_key) {
            (Some(s), Some(c)) => {
                let score = s.compatibility_score(c);
                let stage = if score >= 1.0 {
                    CompatibilityStage::Perfect
                } else if score >= 0.9 {
                    CompatibilityStage::Relative
                } else if score >= 0.85 {
                    CompatibilityStage::Dramatic
                } else if score >= 0.7 {
                    CompatibilityStage::LessDramatic
                } else if score >= 0.6 {
                    CompatibilityStage::Generic
                } else {
                    CompatibilityStage::Poor
                };
                Some((score, stage))
            }
            _ => None,
        }
    }

    /// Get key at angle (for mouse interaction).
    pub fn key_at_angle(&self, angle: f64) -> Option<MusicalKey> {
        // 12 positions around the wheel.
        let normalized_angle = angle.rem_euclid(2.0 * std::f64::consts::PI);
        let position = (normalized_angle / (2.0 * std::f64::consts::PI) * 12.0).floor() as u8 + 1;

        // Alternate A/B sides.
        let side = if ((normalized_angle / std::f64::consts::PI).floor() as u8).is_multiple_of(2) {
            'A'
        } else {
            'B'
        };

        MusicalKey::from_camelot(&format!("{}{}", position, side))
    }

    /// Get wheel dimensions.
    pub fn dimensions(&self) -> (f64, f64) {
        let scaled_radius = self.radius * self.zoom;
        (self.center.0 + scaled_radius, self.center.1 + scaled_radius)
    }

    /// Check if point is inside wheel.
    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        let dx = x - self.center.0;
        let dy = y - self.center.1;
        (dx * dx + dy * dy).sqrt() <= self.radius * self.zoom
    }

    /// Get wheel preview text.
    pub fn preview(&self) -> String {
        match (&self.selected_key, &self.comparison_key) {
            (Some(s), Some(c)) => {
                if let Some((score, stage)) = self.compatibility() {
                    format!(
                        "{} ↔ {} → {:.2} ({}) — {}",
                        s.camelot,
                        c.camelot,
                        score,
                        stage.label(),
                        stage.color()
                    )
                } else {
                    format!("{} ↔ {} — No compatibility", s.camelot, c.camelot)
                }
            }
            (Some(s), None) => format!("Selected: {}", s.camelot),
            _ => "Harmony Wheel — No key selected".to_string(),
        }
    }
}

impl Default for HarmonyWheelState {
    fn default() -> Self {
        Self::new()
    }
}

/// Key correction state.
#[derive(Debug, Clone)]
pub struct KeyCorrection {
    /// Original detected key.
    pub original_key: MusicalKey,
    /// Corrected key.
    pub corrected_key: MusicalKey,
    /// Confidence of original.
    pub original_confidence: f64,
    /// User override.
    pub user_override: bool,
}

impl KeyCorrection {
    /// Create a new key correction.
    pub fn new(original_key: MusicalKey, corrected_key: MusicalKey, confidence: f64) -> Self {
        Self {
            original_key,
            corrected_key,
            original_confidence: confidence,
            user_override: false,
        }
    }

    /// Mark as user override.
    pub fn set_user_override(&mut self) {
        self.user_override = true;
    }

    /// Check if user overrode.
    pub fn is_user_override(&self) -> bool {
        self.user_override
    }
}

/// Chromagram data for display.
#[derive(Debug, Clone)]
pub struct Chromagram {
    /// 12 pitch classes (C, C#, D, D#, E, F, F#, G, G#, A, A#, B).
    pub pitch_classes: [f64; 12],
    /// Time range (seconds).
    pub start_time: f64,
    pub end_time: f64,
    /// Average key prediction.
    pub predicted_key: String,
}

impl Chromagram {
    /// Create new chromagram.
    pub fn new(pitch_classes: [f64; 12], start_time: f64, end_time: f64) -> Self {
        // Find dominant pitch class.
        let max_idx = pitch_classes
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let key_names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let predicted_key = key_names[max_idx].to_string();

        Self {
            pitch_classes,
            start_time,
            end_time,
            predicted_key,
        }
    }

    /// Get dominant pitch class.
    pub fn dominant_class(&self) -> usize {
        self.pitch_classes
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Get normalized values (0-1).
    pub fn normalized(&self) -> [f64; 12] {
        let max = *self
            .pitch_classes
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&1.0);
        if max == 0.0 {
            return [0.0; 12];
        }
        self.pitch_classes.map(|v| v / max)
    }

    /// Get bar chart representation (ASCII).
    pub fn ascii_art(&self, bar_width: usize) -> String {
        let normalized = self.normalized();
        let key_names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let mut result = String::new();

        for (i, &val) in normalized.iter().enumerate() {
            let bar_len = (val * bar_width as f64) as usize;
            let bar = "#".repeat(bar_len);
            result.push_str(&format!("{:3}: {} {}\n", key_names[i], bar, val));
        }

        result
    }
}

/// Harmony wheel UI engine.
pub struct HarmonyWheelUI {
    /// Current wheel state.
    state: HarmonyWheelState,
    /// Key corrections.
    corrections: HashMap<String, KeyCorrection>,
    /// Chromagram data.
    chromagram: Option<Chromagram>,
    /// Display mode (wheel, table, chromagram).
    display_mode: DisplayMode,
}

/// Display mode for the wheel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    /// Circular wheel view.
    Wheel,
    /// 24×24 compatibility table.
    Table,
    /// Chromagram view.
    Chromagram,
}

impl HarmonyWheelUI {
    /// Create new harmony wheel UI.
    pub fn new() -> Self {
        Self {
            state: HarmonyWheelState::new(),
            corrections: HashMap::new(),
            chromagram: None,
            display_mode: DisplayMode::Wheel,
        }
    }

    /// Set the wheel state.
    pub fn set_state(&mut self, state: HarmonyWheelState) {
        self.state = state;
    }

    /// Get current wheel state.
    pub fn state(&self) -> &HarmonyWheelState {
        &self.state
    }

    /// Get mutable wheel state.
    pub fn state_mut(&mut self) -> &mut HarmonyWheelState {
        &mut self.state
    }

    /// Set selected key.
    pub fn set_selected_key(&mut self, key: MusicalKey) {
        self.state.set_selected_key(key);
    }

    /// Set comparison key.
    pub fn set_comparison_key(&mut self, key: MusicalKey) {
        self.state.set_comparison_key(key);
    }

    /// Get compatibility.
    pub fn compatibility(&self) -> Option<(f64, CompatibilityStage)> {
        self.state.compatibility()
    }

    /// Add a key correction.
    pub fn add_correction(&mut self, track_id: &str, correction: KeyCorrection) {
        self.corrections.insert(track_id.to_string(), correction);
    }

    /// Get correction for track.
    pub fn get_correction(&self, track_id: &str) -> Option<&KeyCorrection> {
        self.corrections.get(track_id)
    }

    /// Remove correction for track.
    pub fn remove_correction(&mut self, track_id: &str) {
        self.corrections.remove(track_id);
    }

    /// Set chromagram data.
    pub fn set_chromagram(&mut self, chromagram: Chromagram) {
        self.chromagram = Some(chromagram);
    }

    /// Get chromagram.
    pub fn chromagram(&self) -> Option<&Chromagram> {
        self.chromagram.as_ref()
    }

    /// Set display mode.
    pub fn set_display_mode(&mut self, mode: DisplayMode) {
        self.display_mode = mode;
    }

    /// Get display mode.
    pub fn display_mode(&self) -> &DisplayMode {
        &self.display_mode
    }

    /// Get correction count.
    pub fn correction_count(&self) -> usize {
        self.corrections.len()
    }

    /// Get compatibility table as HashMap.
    pub fn get_compatibility_table(&self) -> HashMap<(String, String), (f64, CompatibilityStage)> {
        let mut table = HashMap::new();

        for num_a in 1..=12 {
            for side_a in ['A', 'B'] {
                let key_a_str = format!("{}{}", num_a, side_a);
                if let Some(key_a) = MusicalKey::from_camelot(&key_a_str) {
                    for num_b in 1..=12 {
                        for side_b in ['A', 'B'] {
                            let key_b_str = format!("{}{}", num_b, side_b);
                            if let Some(key_b) = MusicalKey::from_camelot(&key_b_str) {
                                let score = key_a.compatibility_score(&key_b);
                                let stage = if score >= 1.0 {
                                    CompatibilityStage::Perfect
                                } else if score >= 0.9 {
                                    CompatibilityStage::Relative
                                } else if score >= 0.85 {
                                    CompatibilityStage::Dramatic
                                } else if score >= 0.7 {
                                    CompatibilityStage::LessDramatic
                                } else if score >= 0.6 {
                                    CompatibilityStage::Generic
                                } else {
                                    CompatibilityStage::Poor
                                };
                                table.insert(
                                    (key_a_str.clone(), key_b_str.clone()),
                                    (score, stage.clone()),
                                );
                            }
                        }
                    }
                }
            }
        }

        table
    }

    /// Find compatible tracks for a key.
    pub fn find_compatible_tracks<'a>(
        &self,
        target_key: &MusicalKey,
        tracks: &'a HashMap<String, TrackInfo>,
    ) -> Vec<&'a TrackInfo> {
        let compatible = target_key.get_compatible_keys();
        tracks
            .values()
            .filter(|t| {
                let key = MusicalKey::from_camelot(&t.key);
                key.map(|k| k.camelot == target_key.camelot || compatible.contains(&k.camelot))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get wheel preview.
    pub fn preview(&self) -> String {
        self.state.preview()
    }

    /// Get wheel dimensions.
    pub fn dimensions(&self) -> (f64, f64) {
        self.state.dimensions()
    }

    /// Get corrected key for track.
    pub fn get_corrected_key(&self, track_id: &str) -> Option<String> {
        self.corrections
            .get(track_id)
            .map(|c| c.corrected_key.camelot.clone())
    }
}

/// Track info for compatibility search.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    /// Track ID.
    pub id: String,
    /// Track title.
    pub title: String,
    /// Track key (Camelot).
    pub key: String,
    /// Track BPM.
    pub bpm: f64,
}

impl TrackInfo {
    /// Create new track info.
    pub fn new(id: String, title: String, key: String, bpm: f64) -> Self {
        Self {
            id,
            title,
            key,
            bpm,
        }
    }
}

impl Default for HarmonyWheelUI {
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
    fn musical_key_from_camelot() {
        let key = MusicalKey::from_camelot("8A").unwrap();
        assert_eq!(key.camelot, "8A");
        assert_eq!(key.camelot_number, 8);
        assert_eq!(key.camelot_side, 'A');
    }

    #[test]
    fn musical_key_invalid() {
        assert!(MusicalKey::from_camelot("13A").is_none());
        assert!(MusicalKey::from_camelot("0A").is_none());
        assert!(MusicalKey::from_camelot("ABC").is_none());
    }

    #[test]
    fn musical_key_compatible_keys() {
        let key = MusicalKey::from_camelot("8A").unwrap();
        let compatible = key.get_compatible_keys();

        assert!(compatible.contains(&"8A".to_string())); // Perfect
        assert!(compatible.contains(&"7A".to_string())); // Dramatic -1
        assert!(compatible.contains(&"9A".to_string())); // Dramatic +1
        assert!(compatible.contains(&"8B".to_string())); // Relative
    }

    #[test]
    fn musical_key_compatibility_score() {
        let key_a = MusicalKey::from_camelot("8A").unwrap();
        let key_b = MusicalKey::from_camelot("8A").unwrap();
        assert_eq!(key_a.compatibility_score(&key_b), 1.0);

        let key_c = MusicalKey::from_camelot("9A").unwrap();
        assert_eq!(key_a.compatibility_score(&key_c), 0.85); // Dramatic

        let key_d = MusicalKey::from_camelot("8B").unwrap();
        assert_eq!(key_a.compatibility_score(&key_d), 0.9); // Relative
    }

    #[test]
    fn compatibility_stage_labels() {
        assert_eq!(CompatibilityStage::Perfect.label(), "Perfect");
        assert_eq!(CompatibilityStage::Relative.label(), "Relative");
        assert_eq!(CompatibilityStage::Dramatic.label(), "Dramatic");
    }

    #[test]
    fn compatibility_stage_colors() {
        assert_eq!(CompatibilityStage::Perfect.color(), "#00ff00");
        assert_eq!(CompatibilityStage::Poor.color(), "#ff0000");
    }

    #[test]
    fn harmony_wheel_state_creation() {
        let state = HarmonyWheelState::new();
        assert!(state.selected_key.is_none());
        assert!(state.comparison_key.is_none());
    }

    #[test]
    fn harmony_wheel_set_keys() {
        let mut state = HarmonyWheelState::new();
        let key_a = MusicalKey::from_camelot("8A").unwrap();
        let key_b = MusicalKey::from_camelot("9A").unwrap();

        state.set_selected_key(key_a);
        state.set_comparison_key(key_b);

        assert!(state.selected_key.is_some());
        assert!(state.compatibility().is_some());
    }

    #[test]
    fn harmony_wheel_compatibility() {
        let mut state = HarmonyWheelState::new();
        let key_a = MusicalKey::from_camelot("8A").unwrap();
        let key_b = MusicalKey::from_camelot("8A").unwrap();

        state.set_selected_key(key_a);
        state.set_comparison_key(key_b);

        let (score, stage) = state.compatibility().unwrap();
        assert_eq!(score, 1.0);
        assert_eq!(stage, CompatibilityStage::Perfect);
    }

    #[test]
    fn harmony_wheel_key_at_angle() {
        let state = HarmonyWheelState::new();
        // Angle 0 should map to position 1.
        let key = state.key_at_angle(0.0);
        assert!(key.is_some());
    }

    #[test]
    fn harmony_wheel_dimensions() {
        let mut state = HarmonyWheelState::new();
        state.set_zoom(2.0);
        let (w, _h) = state.dimensions();
        assert!(w > 400.0); // center.x + radius * zoom
    }

    #[test]
    fn harmony_wheel_contains_point() {
        let state = HarmonyWheelState::new();
        assert!(state.contains_point(400.0, 400.0)); // Center
        assert!(!state.contains_point(1000.0, 1000.0)); // Outside
    }

    #[test]
    fn harmony_wheel_preview() {
        let mut state = HarmonyWheelState::new();
        let key_a = MusicalKey::from_camelot("8A").unwrap();
        let key_b = MusicalKey::from_camelot("9A").unwrap();

        state.set_selected_key(key_a);
        state.set_comparison_key(key_b);

        let preview = state.preview();
        assert!(preview.contains("8A"));
        assert!(preview.contains("9A"));
    }

    #[test]
    fn key_correction_creation() {
        let original = MusicalKey::from_camelot("8A").unwrap();
        let corrected = MusicalKey::from_camelot("9A").unwrap();
        let correction = KeyCorrection::new(original.clone(), corrected.clone(), 0.7);

        assert_eq!(correction.original_key.camelot, "8A");
        assert_eq!(correction.corrected_key.camelot, "9A");
        assert!(!correction.is_user_override());

        correction.clone_into(&mut correction.clone());
        let mut c2 = correction.clone();
        c2.set_user_override();
        assert!(c2.is_user_override());
    }

    #[test]
    fn chromagram_creation() {
        let mut pc = [0.0; 12];
        pc[0] = 1.0; // C dominant
        let chroma = Chromagram::new(pc, 0.0, 240.0);
        assert_eq!(chroma.predicted_key, "C");
        assert_eq!(chroma.dominant_class(), 0);
    }

    #[test]
    fn chromagram_normalized() {
        let pc = [1.0, 0.5, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let chroma = Chromagram::new(pc, 0.0, 60.0);
        let norm = chroma.normalized();
        assert_eq!(norm[0], 1.0);
        assert_eq!(norm[1], 0.5);
    }

    #[test]
    fn chromagram_ascii_art() {
        let mut pc = [0.0; 12];
        pc[0] = 1.0;
        pc[4] = 0.5;
        let chroma = Chromagram::new(pc, 0.0, 60.0);
        let art = chroma.ascii_art(20);
        assert!(art.contains("C  "));
        assert!(art.contains("E  "));
    }

    #[test]
    fn harmony_wheel_ui_creation() {
        let ui = HarmonyWheelUI::new();
        assert_eq!(ui.correction_count(), 0);
        assert!(ui.chromagram().is_none());
    }

    #[test]
    fn harmony_wheel_add_correction() {
        let mut ui = HarmonyWheelUI::new();
        let original = MusicalKey::from_camelot("8A").unwrap();
        let corrected = MusicalKey::from_camelot("9A").unwrap();
        let correction = KeyCorrection::new(original, corrected, 0.7);

        ui.add_correction("track1", correction);
        assert_eq!(ui.correction_count(), 1);
        assert_eq!(ui.get_corrected_key("track1"), Some("9A".to_string()));
    }

    #[test]
    fn harmony_wheel_compatibility_table() {
        let ui = HarmonyWheelUI::new();
        let table = ui.get_compatibility_table();

        // Should have 24 * 24 = 576 entries.
        assert_eq!(table.len(), 576);

        // Check perfect match.
        assert!(table.contains_key(&("8A".to_string(), "8A".to_string())));

        // Check relative match.
        assert!(table.contains_key(&("8A".to_string(), "8B".to_string())));
    }

    #[test]
    fn harmony_wheel_find_compatible() {
        let ui = HarmonyWheelUI::new();
        let mut tracks = HashMap::new();

        tracks.insert(
            "t1".to_string(),
            TrackInfo::new(
                "t1".to_string(),
                "Track 1".to_string(),
                "8A".to_string(),
                128.0,
            ),
        );
        tracks.insert(
            "t2".to_string(),
            TrackInfo::new(
                "t2".to_string(),
                "Track 2".to_string(),
                "9A".to_string(),
                126.0,
            ),
        );
        tracks.insert(
            "t3".to_string(),
            TrackInfo::new(
                "t3".to_string(),
                "Track 3".to_string(),
                "3C".to_string(),
                140.0,
            ),
        );

        let target = MusicalKey::from_camelot("8A").unwrap();
        let compatible = ui.find_compatible_tracks(&target, &tracks);

        // 8A and 9A should be compatible.
        assert_eq!(compatible.len(), 2);
    }

    #[test]
    fn harmony_wheel_display_modes() {
        let mut ui = HarmonyWheelUI::new();
        assert_eq!(ui.display_mode(), &DisplayMode::Wheel);

        ui.set_display_mode(DisplayMode::Table);
        assert_eq!(ui.display_mode(), &DisplayMode::Table);

        ui.set_display_mode(DisplayMode::Chromagram);
        assert_eq!(ui.display_mode(), &DisplayMode::Chromagram);
    }

    #[test]
    fn harmony_wheel_chromagram_data() {
        let mut ui = HarmonyWheelUI::new();
        let mut pc = [0.0; 12];
        pc[7] = 1.0; // G dominant
        let chroma = Chromagram::new(pc, 0.0, 240.0);

        ui.set_chromagram(chroma);
        assert!(ui.chromagram().is_some());
        assert_eq!(ui.chromagram().unwrap().predicted_key, "G");
    }

    #[test]
    fn harmony_wheel_remove_correction() {
        let mut ui = HarmonyWheelUI::new();
        let original = MusicalKey::from_camelot("8A").unwrap();
        let corrected = MusicalKey::from_camelot("9A").unwrap();
        let correction = KeyCorrection::new(original, corrected, 0.7);

        ui.add_correction("track1", correction);
        assert_eq!(ui.correction_count(), 1);

        ui.remove_correction("track1");
        assert_eq!(ui.correction_count(), 0);
    }

    #[test]
    fn key_compatibility_all_stages() {
        let key = MusicalKey::from_camelot("8A").unwrap();

        // Perfect.
        assert_eq!(
            key.compatibility_score(&MusicalKey::from_camelot("8A").unwrap()),
            1.0
        );

        // Relative.
        assert_eq!(
            key.compatibility_score(&MusicalKey::from_camelot("8B").unwrap()),
            0.9
        );

        // Dramatic.
        assert_eq!(
            key.compatibility_score(&MusicalKey::from_camelot("9A").unwrap()),
            0.85
        );

        // Less Dramatic.
        assert_eq!(
            key.compatibility_score(&MusicalKey::from_camelot("10A").unwrap()),
            0.7
        );
    }

    #[test]
    fn musical_key_edge_positions() {
        let one_a = MusicalKey::from_camelot("1A").unwrap();
        let compat = one_a.get_compatible_keys();
        // 1A should have 2B (relative), 2A (+1), and 12A might wrap.
        assert!(compat.contains(&"1B".to_string())); // Relative
        assert!(compat.contains(&"2A".to_string())); // +1
    }
}
