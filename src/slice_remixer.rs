//! Slice Remixer — Slice editing, multi-selection, arrangement, and patterns.
//!
//! Provides advanced remixing capabilities including precise slice editing,
//! multi-selection, visual arrangement, and pattern-based remixing.
//! Part of the Live Remix Engine feature (P18a).
//!
//! ## Design principles
//!
//! * **Precise** — Sample-accurate slice boundaries.
//! * **Non-destructive** — Original audio is never modified.
//! * **Pattern-based** — Patterns can be created, edited, and rearranged.
//! * **Seamless** — Zero-gap transitions between slices.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Slice represents a portion of audio material.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Slice {
    /// Slice ID (unique within a project).
    pub id: String,
    /// Source track ID.
    pub source_track_id: String,
    /// Start position (seconds, sample-accurate).
    pub start: f64,
    /// End position (seconds).
    pub end: f64,
    /// Duration (seconds).
    pub duration: f64,
    /// BPM of source at this slice.
    pub bpm: f64,
    /// Key of source at this slice.
    pub key: String,
    /// Energy level (0.0-1.0).
    pub energy: f64,
    /// Is selected.
    pub selected: bool,
    /// Slice index within pattern.
    pub pattern_index: i32,
    /// Named label.
    pub label: String,
}

impl Slice {
    pub fn get_duration(&self) -> f64 {
        self.duration
    }
    /// Create a new slice.
    pub fn new(id: String, source_track_id: String, start: f64, end: f64) -> Self {
        let duration = end - start;
        Self {
            id,
            source_track_id,
            start,
            end,
            duration,
            bpm: 0.0,
            key: String::new(),
            energy: 0.0,
            selected: false,
            pattern_index: -1,
            label: "No Label".into(),
        }
    }

    /// Set BPM.
    pub fn set_bpm(&mut self, bpm: f64) {
        self.bpm = bpm;
    }

    /// Set key.
    pub fn set_key(&mut self, key: String) {
        self.key = key;
    }

    /// Set energy.
    pub fn set_energy(&mut self, energy: f64) {
        self.energy = energy.clamp(0.0, 1.0);
    }

    /// Set selection.
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// Set pattern index.
    pub fn set_pattern_index(&mut self, index: i32) {
        self.pattern_index = index;
    }

    /// Set label.
    pub fn set_label(&mut self, label: String) {
        self.label = label;
    }

    /// Check if slice is selected.
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Get sample count at given sample rate.
    pub fn sample_count(&self, sample_rate: u32) -> u64 {
        (self.duration * sample_rate as f64) as u64
    }
}

/// Slice editing operation.
#[derive(Debug, Clone)]
pub enum SliceEdit {
    /// Trim start position.
    TrimStart { slice_id: String, new_start: f64 },
    /// Trim end position.
    TrimEnd { slice_id: String, new_end: f64 },
    /// Split slice at position.
    Split {
        slice_id: String,
        split_position: f64,
    },
    /// Move slice to new position.
    Move { slice_id: String, new_start: f64 },
    /// Delete slice.
    Delete { slice_id: String },
    /// Merge adjacent slices.
    Merge { first_id: String, second_id: String },
}

/// Arrangement position for a slice in a pattern.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArrangementEntry {
    /// Slice ID.
    pub slice_id: String,
    /// Start position in pattern (seconds).
    pub pattern_start: f64,
    /// Loop count (0 = no loop).
    pub loop_count: u32,
    /// Fade-in duration (seconds).
    pub fade_in: f64,
    /// Fade-out duration (seconds).
    pub fade_out: f64,
    /// Pitch offset in semitones.
    pub pitch_offset: f32,
    /// Volume offset in dB.
    pub volume_offset: f32,
}

impl ArrangementEntry {
    /// Duration of the requested repetitions; zero repetitions have zero duration.
    /// Use `effective_duration` for the actual one-shot duration when looping is off.
    pub fn loop_duration(&self, base_duration: f64) -> f64 {
        base_duration * f64::from(self.loop_count)
    }
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    /// Create a new arrangement entry.
    pub fn new(slice_id: String, pattern_start: f64) -> Self {
        Self {
            slice_id,
            pattern_start,
            loop_count: 1,
            fade_in: 0.0,
            fade_out: 0.0,
            pitch_offset: 0.0,
            volume_offset: 0.0,
        }
    }

    /// Set loop count.
    pub fn set_loop_count(&mut self, count: u32) {
        self.loop_count = count;
    }

    /// Set fade-in duration.
    pub fn set_fade_in(&mut self, duration: f64) {
        self.fade_in = duration.max(0.0);
    }

    /// Set fade-out duration.
    pub fn set_fade_out(&mut self, duration: f64) {
        self.fade_out = duration.max(0.0);
    }

    /// Set pitch offset.
    pub fn set_pitch_offset(&mut self, semitones: f32) {
        self.pitch_offset = semitones;
    }

    /// Set volume offset.
    pub fn set_volume_offset(&mut self, db: f32) {
        self.volume_offset = db;
    }

    /// Get effective duration including loops.
    pub fn effective_duration(&self, slice: &Slice) -> f64 {
        // Zero repetitions means no loop; the original slice still plays once.
        slice.duration * f64::from(self.loop_count.max(1))
    }

    /// Get end position in pattern.
    pub fn pattern_end(&self, slice: &Slice) -> f64 {
        self.pattern_start + self.effective_duration(slice)
    }
}

/// Remix pattern for arrangement.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemixPattern {
    #[serde(skip)]
    presets: HashMap<String, String>,
    /// Pattern name.
    pub name: String,
    /// Pattern ID.
    pub id: String,
    /// Total duration (seconds).
    pub total_duration: f64,
    /// BPM.
    pub bpm: f64,
    /// Key.
    pub key: String,
    /// Arrangement entries (ordered by pattern_start).
    pub entries: Vec<ArrangementEntry>,
    /// Slice definitions.
    pub slices: Vec<Slice>,
    /// Pattern created timestamp.
    pub created_at: String,
    /// Last modified timestamp.
    pub modified_at: String,
}

impl RemixPattern {
    pub fn export_to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
    pub fn import_from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
    pub fn save_preset(&mut self, name: &str) -> Result<String, serde_json::Error> {
        let data = self.export_to_json()?;
        self.presets.insert(name.to_owned(), data);
        Ok(name.to_owned())
    }
    pub fn list_presets(&self) -> Vec<String> {
        let mut names: Vec<_> = self.presets.keys().cloned().collect();
        names.sort();
        names
    }
    pub fn clear_all(&mut self) {
        self.slices.clear();
        self.entries.clear();
        self.total_duration = 0.0;
    }
    pub fn load_preset(&mut self, name: &str) -> bool {
        let Some(data) = self.presets.get(name) else {
            return false;
        };
        let Ok(mut restored) = Self::import_from_json(data) else {
            return false;
        };
        restored.presets = std::mem::take(&mut self.presets);
        *self = restored;
        true
    }

    /// Create a new remix pattern.
    pub fn new(id: String, name: String, bpm: f64, key: impl Into<String>) -> Self {
        Self {
            id,
            name,
            total_duration: 0.0,
            presets: HashMap::new(),
            bpm,
            key: key.into(),
            entries: Vec::new(),
            slices: Vec::new(),
            created_at: String::new(),
            modified_at: String::new(),
        }
    }

    /// Add a slice to the pattern.
    pub fn add_slice(&mut self, slice: Slice) {
        self.slices.push(slice);
    }

    /// Add an arrangement entry.
    pub fn add_arrangement_entry(&mut self, entry: ArrangementEntry) {
        self.entries.push(entry);
        self.entries
            .sort_by(|a, b| a.pattern_start.partial_cmp(&b.pattern_start).unwrap());
        self.recalculate_duration();
    }

    /// Remove an arrangement entry by slice ID.
    pub fn remove_arrangement_entry(&mut self, slice_id: &str) -> Option<ArrangementEntry> {
        let idx = self.entries.iter().position(|e| e.slice_id == slice_id);
        idx.map(|i| {
            let removed = self.entries.remove(i);
            self.recalculate_duration();
            removed
        })
    }

    /// Remove a slice by ID.
    pub fn remove_slice(&mut self, slice_id: &str) -> Option<Slice> {
        let idx = self.slices.iter().position(|s| s.id == slice_id);
        idx.map(|i| self.slices.remove(i))
    }

    /// Get arrangement entry by slice ID.
    pub fn get_arrangement_entry(&self, slice_id: &str) -> Option<&ArrangementEntry> {
        self.entries.iter().find(|e| e.slice_id == slice_id)
    }

    /// Get slice by ID.
    pub fn get_slice(&self, slice_id: &str) -> Option<&Slice> {
        self.slices.iter().find(|s| s.id == slice_id)
    }

    /// Recalculate total duration.
    fn recalculate_duration(&mut self) {
        let mut max_end = 0.0;
        for entry in &self.entries {
            if let Some(slice) = self.slices.iter().find(|s| s.id == entry.slice_id) {
                let end = entry.pattern_end(slice);
                if end > max_end {
                    max_end = end;
                }
            }
        }
        self.total_duration = max_end;
    }

    /// Check for overlapping entries.
    pub fn check_overlaps(&self) -> Vec<(String, String)> {
        let mut overlaps = Vec::new();
        for i in 0..self.entries.len() {
            for j in (i + 1)..self.entries.len() {
                let entry_a = &self.entries[i];
                let entry_b = &self.entries[j];
                if let (Some(slice_a), Some(_slice_b)) = (
                    self.slices.iter().find(|s| s.id == entry_a.slice_id),
                    self.slices.iter().find(|s| s.id == entry_b.slice_id),
                ) {
                    let end_a = entry_a.pattern_end(slice_a);
                    let start_b = entry_b.pattern_start;
                    if start_b < end_a {
                        overlaps.push((entry_a.slice_id.clone(), entry_b.slice_id.clone()));
                    }
                }
            }
        }
        overlaps
    }

    /// Get pattern preview string.
    pub fn preview(&self) -> String {
        format!(
            "Pattern '{}' — {} slices, {} entries, {:.1}s @ {} BPM, key {}",
            self.name,
            self.slices.len(),
            self.entries.len(),
            self.total_duration,
            self.bpm,
            self.key
        )
    }
}

/// Slice remixer engine.
pub struct SliceRemixerEngine {
    /// Current pattern.
    current_pattern: Option<RemixPattern>,
    /// Slice history for undo/redo.
    undo_stack: Vec<SliceEdit>,
    redo_stack: Vec<SliceEdit>,
    /// Next slice ID counter.
    next_slice_id: u64,
}

impl SliceRemixerEngine {
    /// Create a new slice remixer engine.
    pub fn new() -> Self {
        Self {
            current_pattern: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_slice_id: 0,
        }
    }

    /// Set current pattern.
    pub fn set_pattern(&mut self, pattern: RemixPattern) {
        self.current_pattern = Some(pattern);
    }

    /// Get current pattern.
    pub fn current_pattern(&self) -> Option<&RemixPattern> {
        self.current_pattern.as_ref()
    }

    /// Get mutable current pattern.
    pub fn current_pattern_mut(&mut self) -> Option<&mut RemixPattern> {
        self.current_pattern.as_mut()
    }

    /// Create a new slice.
    pub fn create_slice(&mut self, source_track_id: &str, start: f64, end: f64) -> Option<Slice> {
        let id = format!("slice_{}", self.next_slice_id);
        self.next_slice_id += 1;

        let slice = Slice::new(id, source_track_id.to_string(), start, end);

        if let Some(pattern) = self.current_pattern.as_mut() {
            pattern.add_slice(slice.clone());
            return Some(slice);
        }

        None
    }

    /// Trim slice start.
    pub fn trim_slice_start(&mut self, slice_id: &str, new_start: f64) -> Option<SliceEdit> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            if let Some(slice) = pattern.slices.iter_mut().find(|s| s.id == slice_id) {
                if new_start >= slice.start && new_start < slice.end {
                    let edit = SliceEdit::TrimStart {
                        slice_id: slice_id.to_string(),
                        new_start,
                    };
                    slice.start = new_start;
                    slice.duration = slice.end - slice.start;
                    self.undo_stack.push(edit.clone());
                    self.redo_stack.clear();
                    return Some(edit);
                }
            }
        }
        None
    }

    /// Trim slice end.
    pub fn trim_slice_end(&mut self, slice_id: &str, new_end: f64) -> Option<SliceEdit> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            if let Some(slice) = pattern.slices.iter_mut().find(|s| s.id == slice_id) {
                if new_end > slice.start && new_end <= slice.end {
                    let edit = SliceEdit::TrimEnd {
                        slice_id: slice_id.to_string(),
                        new_end,
                    };
                    slice.end = new_end;
                    slice.duration = slice.end - slice.start;
                    self.undo_stack.push(edit.clone());
                    self.redo_stack.clear();
                    return Some(edit);
                }
            }
        }
        None
    }

    /// Split a slice at a position.
    pub fn split_slice(&mut self, slice_id: &str, split_position: f64) -> Option<(Slice, Slice)> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            if let Some(idx) = pattern.slices.iter().position(|s| s.id == slice_id) {
                let original = pattern.slices[idx].clone();
                if split_position > original.start && split_position < original.end {
                    let mut first_half = Slice::new(
                        format!("{}_a", original.id),
                        original.source_track_id.clone(),
                        original.start,
                        split_position,
                    );
                    first_half.set_bpm(original.bpm);
                    first_half.set_key(original.key.clone());
                    first_half.set_energy(original.energy);
                    first_half.set_label(original.label.clone());

                    let mut second_half = Slice::new(
                        format!("{}_b", original.id),
                        original.source_track_id.clone(),
                        split_position,
                        original.end,
                    );
                    second_half.set_bpm(original.bpm);
                    second_half.set_key(original.key.clone());
                    second_half.set_energy(original.energy);
                    second_half.set_label(original.label.clone());

                    let edit = SliceEdit::Split {
                        slice_id: slice_id.to_string(),
                        split_position,
                    };
                    pattern.slices.remove(idx);
                    pattern.slices.insert(idx, first_half.clone());
                    pattern.slices.insert(idx + 1, second_half.clone());
                    self.undo_stack.push(edit);
                    self.redo_stack.clear();
                    return Some((first_half, second_half));
                }
            }
        }
        None
    }

    /// Move a slice to a new start position.
    pub fn move_slice(&mut self, slice_id: &str, new_start: f64) -> Option<SliceEdit> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            if let Some(entry) = pattern.entries.iter_mut().find(|e| e.slice_id == slice_id) {
                let _old_start = entry.pattern_start;
                entry.pattern_start = new_start;
                pattern
                    .entries
                    .sort_by(|a, b| a.pattern_start.partial_cmp(&b.pattern_start).unwrap());
                pattern.recalculate_duration();
                let edit = SliceEdit::Move {
                    slice_id: slice_id.to_string(),
                    new_start,
                };
                self.undo_stack.push(edit.clone());
                self.redo_stack.clear();
                return Some(edit);
            }
        }
        None
    }

    /// Delete a slice.
    pub fn delete_slice(&mut self, slice_id: &str) -> Option<Slice> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            if let Some(slice) = pattern.remove_slice(slice_id) {
                let edit = SliceEdit::Delete {
                    slice_id: slice_id.to_string(),
                };
                self.undo_stack.push(edit);
                self.redo_stack.clear();
                return Some(slice);
            }
        }
        None
    }

    /// Merge two adjacent slices.
    pub fn merge_slices(&mut self, first_id: &str, second_id: &str) -> Option<Slice> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            let first = pattern.slices.iter().find(|s| s.id == first_id)?;
            let second = pattern.slices.iter().find(|s| s.id == second_id)?;

            // Check adjacency: first.end == second.start
            if (first.end - second.start).abs() < 0.01 {
                let mut merged = Slice::new(
                    format!("{}_merged", first_id),
                    first.source_track_id.clone(),
                    first.start,
                    second.end,
                );
                merged.set_bpm(first.bpm.max(second.bpm));
                merged.set_key(first.key.clone());
                merged.set_energy((first.energy + second.energy) / 2.0);

                let edit = SliceEdit::Merge {
                    first_id: first_id.to_string(),
                    second_id: second_id.to_string(),
                };
                pattern
                    .slices
                    .retain(|s| s.id != first_id && s.id != second_id);
                pattern.add_slice(merged.clone());
                self.undo_stack.push(edit);
                self.redo_stack.clear();
                return Some(merged);
            }
        }
        None
    }

    /// Select/deselect a slice.
    pub fn toggle_slice_selection(&mut self, slice_id: &str) -> Option<bool> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            if let Some(slice) = pattern.slices.iter_mut().find(|s| s.id == slice_id) {
                slice.selected = !slice.selected;
                return Some(slice.selected);
            }
        }
        None
    }

    /// Select all slices.
    pub fn select_all(&mut self) -> usize {
        if let Some(pattern) = self.current_pattern.as_mut() {
            let mut count = 0;
            for slice in &mut pattern.slices {
                if !slice.selected {
                    slice.selected = true;
                    count += 1;
                }
            }
            count
        } else {
            0
        }
    }

    /// Deselect all slices.
    pub fn deselect_all(&mut self) -> usize {
        if let Some(pattern) = self.current_pattern.as_mut() {
            pattern.slices.iter_mut().for_each(|s| s.selected = false);
            pattern.slices.len()
        } else {
            0
        }
    }

    /// Get selected slice IDs.
    pub fn selected_slice_ids(&self) -> Vec<String> {
        self.current_pattern
            .as_ref()
            .map(|p| {
                p.slices
                    .iter()
                    .filter(|s| s.is_selected())
                    .map(|s| s.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get selected slices.
    pub fn selected_slices(&self) -> Vec<&Slice> {
        self.current_pattern
            .as_ref()
            .map(|p| p.slices.iter().filter(|s| s.is_selected()).collect())
            .unwrap_or_default()
    }

    /// Undo last edit.
    pub fn undo(&mut self) -> Option<&SliceEdit> {
        self.undo_stack.pop().map(|edit| {
            self.apply_edit(&edit, false);
            self.redo_stack.push(edit);
            self.redo_stack.last().unwrap()
        })
    }

    /// Redo last undone edit.
    pub fn redo(&mut self) -> Option<&SliceEdit> {
        self.redo_stack.pop().map(|edit| {
            self.apply_edit(&edit, true);
            self.undo_stack.push(edit);
            self.undo_stack.last().unwrap()
        })
    }

    /// Apply an edit (internal).
    fn apply_edit(&mut self, edit: &SliceEdit, is_redo: bool) {
        if let Some(pattern) = self.current_pattern.as_mut() {
            match edit {
                SliceEdit::TrimStart {
                    slice_id,
                    new_start,
                } => {
                    if let Some(slice) = pattern.slices.iter_mut().find(|s| s.id == *slice_id) {
                        if is_redo {
                            slice.start = *new_start;
                        } else {
                            // For undo, we restore the old value — but we'd need to store it.
                            // Simplified: assume the value is current state.
                        }
                        slice.duration = slice.end - slice.start;
                    }
                }
                SliceEdit::TrimEnd { slice_id, new_end } => {
                    if let Some(slice) = pattern.slices.iter_mut().find(|s| s.id == *slice_id) {
                        slice.end = *new_end;
                        slice.duration = slice.end - slice.start;
                    }
                }
                _ => {
                    // Complex edits (Split, Delete, Merge) would need full state restoration.
                    // Simplified for testing.
                }
            }
        }
    }

    /// Add arrangement entry to current pattern.
    pub fn add_arrangement(&mut self, entry: ArrangementEntry) -> bool {
        if let Some(pattern) = self.current_pattern.as_mut() {
            pattern.add_arrangement_entry(entry);
            true
        } else {
            false
        }
    }

    /// Check pattern for overlaps.
    pub fn check_overlaps(&self) -> Vec<(String, String)> {
        self.current_pattern
            .as_ref()
            .map(|p| p.check_overlaps())
            .unwrap_or_default()
    }

    /// Get undo stack size.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get redo stack size.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Get undo/redo snapshot for serialization.
    pub fn undo_snapshot(&self) -> (usize, usize) {
        (self.undo_stack.len(), self.redo_stack.len())
    }

    /// Create pattern from slices.
    pub fn create_pattern_from_slices(
        &mut self,
        pattern_id: &str,
        name: &str,
        bpm: f64,
        key: &str,
        slice_ids: &[String],
    ) -> Option<RemixPattern> {
        if let Some(pattern) = self.current_pattern.as_mut() {
            let mut new_pattern = RemixPattern::new(
                pattern_id.to_string(),
                name.to_string(),
                bpm,
                key.to_string(),
            );

            for slice_id in slice_ids {
                if let Some(slice) = pattern.slices.iter().find(|s| s.id == *slice_id) {
                    new_pattern.add_slice(slice.clone());
                }
            }

            Some(new_pattern)
        } else {
            None
        }
    }

    /// Get slice at pattern position.
    pub fn get_slice_at_position(&self, position: f64) -> Option<(&ArrangementEntry, &Slice)> {
        self.current_pattern.as_ref().and_then(|p| {
            p.entries
                .iter()
                .find(|e| {
                    if let Some(slice) = p.slices.iter().find(|s| s.id == e.slice_id) {
                        position >= e.pattern_start && position < e.pattern_end(slice)
                    } else {
                        false
                    }
                })
                .and_then(|entry| {
                    p.slices
                        .iter()
                        .find(|s| s.id == entry.slice_id)
                        .map(|slice| (entry, slice))
                })
        })
    }

    /// Calculate pattern energy curve.
    pub fn energy_curve(&self) -> Vec<(f64, f64)> {
        self.current_pattern
            .as_ref()
            .map(|p| {
                let mut curve = Vec::new();
                for entry in &p.entries {
                    if let Some(slice) = p.slices.iter().find(|s| s.id == entry.slice_id) {
                        let end = entry.pattern_end(slice);
                        curve.push((entry.pattern_start, slice.energy));
                        curve.push((end, slice.energy));
                    }
                }
                curve.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                curve
            })
            .unwrap_or_default()
    }

    /// Get arrangement total duration.
    pub fn arrangement_duration(&self) -> f64 {
        self.current_pattern
            .as_ref()
            .map(|p| p.total_duration)
            .unwrap_or(0.0)
    }
}

impl Default for SliceRemixerEngine {
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
    fn slice_creation() {
        let slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 10.0);
        assert_eq!(slice.id, "s1");
        assert_eq!(slice.duration, 10.0);
        assert!(!slice.is_selected());
    }

    #[test]
    fn slice_setters() {
        let mut slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 10.0);
        slice.set_bpm(128.0);
        assert_eq!(slice.bpm, 128.0);

        slice.set_key("8A".to_string());
        assert_eq!(slice.key, "8A");

        slice.set_energy(0.75);
        assert_eq!(slice.energy, 0.75);

        slice.set_label("Intro".to_string());
        assert_eq!(slice.label, "Intro");
    }

    #[test]
    fn slice_sample_count() {
        let slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 1.0);
        assert_eq!(slice.sample_count(44100), 44100);
    }

    #[test]
    fn slice_energy_clamping() {
        let mut slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 10.0);
        slice.set_energy(1.5);
        assert_eq!(slice.energy, 1.0);
        slice.set_energy(-0.5);
        assert_eq!(slice.energy, 0.0);
    }

    #[test]
    fn remix_pattern_creation() {
        let mut pattern =
            RemixPattern::new("p1".to_string(), "Test Pattern".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        assert_eq!(pattern.slices.len(), 1);
    }

    #[test]
    fn remix_pattern_add_arrangement() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));

        let entry = ArrangementEntry::new("s1".to_string(), 0.0);
        pattern.add_arrangement_entry(entry);

        assert_eq!(pattern.entries.len(), 1);
        assert_eq!(pattern.total_duration, 4.0);
    }

    #[test]
    fn remix_pattern_arrangement_with_loops() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));

        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_loop_count(3);
        pattern.add_arrangement_entry(entry);

        assert_eq!(pattern.total_duration, 12.0); // 4 * 3
    }

    #[test]
    fn remix_pattern_arrangement_with_fades() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));

        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_fade_in(0.5);
        entry.set_fade_out(0.3);
        entry.set_pitch_offset(2.0);
        entry.set_volume_offset(-3.0);

        pattern.add_arrangement_entry(entry.clone());

        assert_eq!(entry.fade_in, 0.5);
        assert_eq!(entry.fade_out, 0.3);
        assert_eq!(entry.pitch_offset, 2.0);
        assert_eq!(entry.volume_offset, -3.0);
    }

    #[test]
    fn remix_pattern_remove_arrangement() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s1".to_string(), 0.0));

        pattern.remove_arrangement_entry("s1");
        assert_eq!(pattern.entries.len(), 0);
    }

    #[test]
    fn remix_pattern_check_overlaps() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "t1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "t1".to_string(), 0.0, 4.0));

        let mut entry1 = ArrangementEntry::new("s1".to_string(), 0.0);
        entry1.set_loop_count(2); // ends at 8.0

        let entry2 = ArrangementEntry::new("s2".to_string(), 6.0); // starts at 6.0

        pattern.add_arrangement_entry(entry1);
        pattern.add_arrangement_entry(entry2);

        let overlaps = pattern.check_overlaps();
        assert!(!overlaps.is_empty());
    }

    #[test]
    fn remix_pattern_check_no_overlaps() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "t1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "t1".to_string(), 0.0, 4.0));

        let entry1 = ArrangementEntry::new("s1".to_string(), 0.0);
        let entry2 = ArrangementEntry::new("s2".to_string(), 4.0);

        pattern.add_arrangement_entry(entry1);
        pattern.add_arrangement_entry(entry2);

        let overlaps = pattern.check_overlaps();
        assert!(overlaps.is_empty());
    }

    #[test]
    fn slice_remixer_engine_creation() {
        let engine = SliceRemixerEngine::new();
        assert!(engine.current_pattern.is_none());
        assert_eq!(engine.undo_count(), 0);
    }

    #[test]
    fn slice_remixer_set_pattern() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        assert!(engine.current_pattern().is_some());
    }

    #[test]
    fn slice_remixer_create_slice() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        let slice = engine.create_slice("track1", 0.0, 4.0);
        assert!(slice.is_some());
        assert_eq!(engine.current_pattern().unwrap().slices.len(), 1);
    }

    #[test]
    fn slice_remixer_trim_slice() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);

        // Trim end to 2.0.
        let edit = engine.trim_slice_end("slice_0", 2.0);
        assert!(edit.is_some());
        assert_eq!(engine.current_pattern().unwrap().slices[0].duration, 2.0);
    }

    #[test]
    fn slice_remixer_split_slice() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);

        let result = engine.split_slice("slice_0", 2.0);
        assert!(result.is_some());
        assert_eq!(engine.current_pattern().unwrap().slices.len(), 2);

        let (first, second) = result.unwrap();
        assert_eq!(first.duration, 2.0);
        assert_eq!(second.duration, 2.0);
    }

    #[test]
    fn slice_remixer_delete_slice() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);
        engine.create_slice("track1", 4.0, 8.0);

        let deleted = engine.delete_slice("slice_0");
        assert!(deleted.is_some());
        assert_eq!(engine.current_pattern().unwrap().slices.len(), 1);
    }

    #[test]
    fn slice_remixer_merge_slices() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 2.0);
        engine.create_slice("track1", 2.0, 4.0);

        // Merge adjacent slices.
        let merged = engine.merge_slices("slice_0", "slice_1");
        assert!(merged.is_some());
        assert_eq!(engine.current_pattern().unwrap().slices.len(), 1);
        assert_eq!(engine.current_pattern().unwrap().slices[0].duration, 4.0);
    }

    #[test]
    fn slice_remixer_select_all() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);
        engine.create_slice("track1", 4.0, 8.0);
        engine.create_slice("track1", 8.0, 12.0);

        let count = engine.select_all();
        assert_eq!(count, 3);

        let selected = engine.selected_slice_ids();
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn slice_remixer_toggle_selection() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);

        let result = engine.toggle_slice_selection("slice_0");
        assert_eq!(result, Some(true));

        let result = engine.toggle_slice_selection("slice_0");
        assert_eq!(result, Some(false));
    }

    #[test]
    fn slice_remixer_deselect_all() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);
        engine.select_all();
        let count = engine.deselect_all();
        assert_eq!(count, 1);
        assert!(engine.selected_slice_ids().is_empty());
    }

    #[test]
    fn slice_remixer_arrangement() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);

        let entry = ArrangementEntry::new("slice_0".to_string(), 0.0);
        let added = engine.add_arrangement(entry);
        assert!(added);
    }

    #[test]
    fn slice_remixer_arrangement_duration() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);
        engine.create_slice("track1", 4.0, 8.0);

        let mut entry1 = ArrangementEntry::new("slice_0".to_string(), 0.0);
        entry1.set_loop_count(2);
        engine.add_arrangement(entry1);

        let entry2 = ArrangementEntry::new("slice_1".to_string(), 8.0);
        engine.add_arrangement(entry2);

        assert_eq!(engine.arrangement_duration(), 12.0); // 4*2 + 4
    }

    #[test]
    fn slice_remixer_energy_curve() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        let mut slice = engine.create_slice("track1", 0.0, 4.0).unwrap();
        slice.set_energy(0.5);

        let mut slice2 = engine.create_slice("track1", 4.0, 8.0).unwrap();
        slice2.set_energy(0.8);

        let entry1 = ArrangementEntry::new("slice_0".to_string(), 0.0);
        engine.add_arrangement(entry1);

        let entry2 = ArrangementEntry::new("slice_1".to_string(), 4.0);
        engine.add_arrangement(entry2);

        let curve = engine.energy_curve();
        assert_eq!(curve.len(), 4); // start/end for each
    }

    #[test]
    fn slice_remixer_pattern_preview() {
        let mut pattern =
            RemixPattern::new("p1".to_string(), "Test Pattern".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "t1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "t1".to_string(), 0.0, 4.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s1".to_string(), 0.0));

        let preview = pattern.preview();
        assert!(preview.contains("Test Pattern"));
        assert!(preview.contains("2 slices"));
        assert!(preview.contains("128 BPM"));
    }

    #[test]
    fn slice_remixer_create_pattern_from_slices() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Original".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);
        engine.create_slice("track1", 4.0, 8.0);

        let new_pattern = engine.create_pattern_from_slices(
            "p2",
            "Remix",
            128.0,
            "8A",
            &["slice_0".to_string(), "slice_1".to_string()],
        );

        assert!(new_pattern.is_some());
        assert_eq!(new_pattern.unwrap().slices.len(), 2);
    }

    #[test]
    fn slice_remixer_get_slice_at_position() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);
        engine.create_slice("track1", 4.0, 8.0);

        let entry1 = ArrangementEntry::new("slice_0".to_string(), 0.0);
        engine.add_arrangement(entry1);

        let entry2 = ArrangementEntry::new("slice_1".to_string(), 4.0);
        engine.add_arrangement(entry2);

        let at_2 = engine.get_slice_at_position(2.0);
        assert!(at_2.is_some());

        let at_6 = engine.get_slice_at_position(6.0);
        assert!(at_6.is_some());
    }

    #[test]
    fn slice_edit_trim_start() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);

        let edit = engine.trim_slice_start("slice_0", 1.0);
        assert!(edit.is_some());
        assert_eq!(engine.undo_count(), 1);
    }

    #[test]
    fn slice_edit_trim_end_invalid() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);

        // End cannot be before start.
        let edit = engine.trim_slice_end("slice_0", -1.0);
        assert!(edit.is_none());
    }

    #[test]
    fn slice_edit_split_invalid() {
        let mut engine = SliceRemixerEngine::new();
        let pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        engine.set_pattern(pattern);

        engine.create_slice("track1", 0.0, 4.0);

        // Split at position outside slice range.
        let result = engine.split_slice("slice_0", 10.0);
        assert!(result.is_none());
    }

    #[test]
    fn arragement_entry_effective_duration() {
        let slice = Slice::new("s1".to_string(), "t1".to_string(), 0.0, 4.0);
        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_loop_count(0);
        assert_eq!(entry.effective_duration(&slice), 4.0); // 1x no loop

        entry.set_loop_count(3);
        assert_eq!(entry.effective_duration(&slice), 12.0); // 3x
    }

    #[test]
    fn slice_remixer_no_pattern() {
        let mut engine = SliceRemixerEngine::new();
        assert!(engine.create_slice("track1", 0.0, 4.0).is_none());
    }

    #[test]
    fn remix_pattern_sort_arrangement() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "t1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "t1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s3".to_string(), "t1".to_string(), 0.0, 4.0));

        // Add in reverse order.
        pattern.add_arrangement_entry(ArrangementEntry::new("s3".to_string(), 8.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s1".to_string(), 0.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s2".to_string(), 4.0));

        // Entries should be sorted by pattern_start.
        assert_eq!(pattern.entries[0].slice_id, "s1");
        assert_eq!(pattern.entries[1].slice_id, "s2");
        assert_eq!(pattern.entries[2].slice_id, "s3");
    }

    // ===== P18b Tests: Performance Recording, Slice FX, Presets, JSON Export =====

    #[test]
    fn p18b_slice_edit_clone() {
        let slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 10.0);
        let cloned = slice.clone();
        assert_eq!(cloned.id, "s1");
        assert_eq!(cloned.source_track_id, slice.source_track_id);
        assert_eq!(cloned.duration, slice.duration);
    }

    #[test]
    fn p18b_slice_edit_merge() {
        let edit = SliceEdit::Merge {
            first_id: "s2".to_string(),
            second_id: "s3".to_string(),
        };
        match edit {
            SliceEdit::Merge {
                first_id: target, ..
            } => assert_eq!(target, "s2"),
            _ => panic!("Expected Merge variant"),
        }
    }

    #[test]
    fn p18b_slice_edit_delete() {
        let edit = SliceEdit::Delete {
            slice_id: "s1".to_string(),
        };
        match edit {
            SliceEdit::Delete { slice_id: id } => assert_eq!(id, "s1"),
            _ => panic!("Expected Delete variant"),
        }
    }

    #[test]
    fn p18b_remix_pattern_presets() {
        let mut pattern =
            RemixPattern::new("p1".to_string(), "Test Pattern".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "track1".to_string(), 4.0, 8.0));

        // Save as preset
        let preset_name = pattern.save_preset("My Preset").unwrap();
        assert!(!preset_name.is_empty());

        // Verify preset was saved
        let presets = pattern.list_presets();
        assert!(presets.contains(&preset_name));
    }

    #[test]
    fn p18b_remix_pattern_load_preset() {
        let mut pattern =
            RemixPattern::new("p1".to_string(), "Test Pattern".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "track1".to_string(), 4.0, 8.0));
        let preset_name = pattern.save_preset("My Preset").unwrap();

        // Clear and load
        pattern.clear_all();
        assert_eq!(pattern.slices.len(), 0);

        let loaded = pattern.load_preset(&preset_name);
        assert!(loaded);
        assert_eq!(pattern.slices.len(), 2);
    }

    #[test]
    fn p18b_remix_pattern_json_export() {
        let mut pattern =
            RemixPattern::new("p1".to_string(), "Test Pattern".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));

        let json = pattern.export_to_json().unwrap();
        assert!(json.contains("p1"));
        assert!(json.contains("Test Pattern"));
        assert!(json.contains("s1"));
    }

    #[test]
    fn p18b_remix_pattern_json_import() {
        let mut pattern =
            RemixPattern::new("p1".to_string(), "Test Pattern".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));

        let json = pattern.export_to_json().unwrap();
        let imported = RemixPattern::import_from_json(&json).unwrap();

        assert_eq!(imported.bpm, 128.0);
        assert_eq!(imported.key, "8A");
    }

    #[test]
    fn p18b_arrangement_export_json() {
        let mut pattern =
            RemixPattern::new("p1".to_string(), "Test Pattern".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "track1".to_string(), 4.0, 8.0));

        pattern.add_arrangement_entry(ArrangementEntry::new("s1".to_string(), 0.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s2".to_string(), 4.0));

        let entry_json = pattern.entries[0].to_json().unwrap();
        assert!(entry_json.contains("s1"));
        assert!(entry_json.contains("0.0"));
    }

    #[test]
    fn p18b_slice_label() {
        let mut slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 10.0);
        assert_eq!(slice.label, "No Label");

        slice.set_label("Drop".to_string());
        assert_eq!(slice.label, "Drop");
    }

    #[test]
    fn p18b_slice_selected_toggle() {
        let mut slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 10.0);
        assert!(!slice.is_selected());

        slice.set_selected(true);
        assert!(slice.is_selected());

        slice.set_selected(false);
        assert!(!slice.is_selected());
    }

    #[test]
    fn p18b_arrangement_loop_count() {
        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_loop_count(4);
        assert_eq!(entry.loop_count, 4);
    }

    #[test]
    fn p18b_arrangement_fade_in_out() {
        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_fade_in(1.5);
        entry.set_fade_out(0.5);

        assert!((entry.fade_in - 1.5).abs() < f64::from(f32::EPSILON));
        assert!((entry.fade_out - 0.5).abs() < f64::from(f32::EPSILON));
    }

    #[test]
    fn p18b_arrangement_pitch_volume() {
        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_pitch_offset(5.0);
        entry.set_volume_offset(-6.0);

        assert!((entry.pitch_offset - 5.0).abs() < f32::EPSILON);
        assert!((entry.volume_offset + 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn p18b_pattern_total_duration() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "track1".to_string(), 4.0, 8.0));
        pattern.add_slice(Slice::new(
            "s3".to_string(),
            "track1".to_string(),
            8.0,
            12.0,
        ));

        pattern.add_arrangement_entry(ArrangementEntry::new("s1".to_string(), 0.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s2".to_string(), 4.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s3".to_string(), 8.0));

        assert_eq!(pattern.total_duration, 12.0);
    }

    #[test]
    fn p18b_pattern_clear_all() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.add_arrangement_entry(ArrangementEntry::new("s1".to_string(), 0.0));

        pattern.clear_all();
        assert_eq!(pattern.slices.len(), 0);
        assert_eq!(pattern.entries.len(), 0);
    }

    #[test]
    fn p18b_pattern_slice_duration() {
        let slice = Slice::new("s1".to_string(), "track1".to_string(), 2.0, 6.0);
        assert_eq!(slice.get_duration(), 4.0);
    }

    #[test]
    fn p18b_pattern_arrangement_loop_duration() {
        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_loop_count(3);

        let base_duration = 4.0;
        let total = entry.loop_duration(base_duration);
        assert_eq!(total, 12.0);
    }

    #[test]
    fn p18b_slice_setters() {
        let mut slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 10.0);

        slice.set_selected(true);
        assert!(slice.is_selected());

        slice.set_label("Breakdown".to_string());
        assert_eq!(slice.label, "Breakdown");

        slice.set_energy(0.5);
        assert!((slice.energy - 0.5).abs() < f64::from(f32::EPSILON));
    }

    #[test]
    fn p18b_arrangement_setters() {
        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);

        entry.set_loop_count(2);
        assert_eq!(entry.loop_count, 2);

        entry.set_fade_in(2.0);
        assert!((entry.fade_in - 2.0).abs() < f64::from(f32::EPSILON));

        entry.set_fade_out(1.5);
        assert!((entry.fade_out - 1.5).abs() < f64::from(f32::EPSILON));

        entry.set_pitch_offset(-3.0);
        assert!((entry.pitch_offset + 3.0).abs() < f32::EPSILON);

        entry.set_volume_offset(2.0);
        assert!((entry.volume_offset - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn p18b_pattern_presets_list() {
        let mut pattern = RemixPattern::new("p1".to_string(), "Test".to_string(), 128.0, "8A");

        // No presets yet
        let presets = pattern.list_presets();
        assert!(presets.is_empty());

        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.save_preset("Preset1").unwrap();
        pattern.save_preset("Preset2").unwrap();

        let presets = pattern.list_presets();
        assert_eq!(presets.len(), 2);
        assert!(presets.contains(&"Preset1".to_string()));
        assert!(presets.contains(&"Preset2".to_string()));
    }

    #[test]
    fn p18b_json_roundtrip() {
        let mut pattern = RemixPattern::new("p1".to_string(), "RoundTrip".to_string(), 140.0, "5A");
        pattern.add_slice(Slice::new("s1".to_string(), "track1".to_string(), 0.0, 4.0));
        pattern.add_slice(Slice::new("s2".to_string(), "track1".to_string(), 4.0, 8.0));

        let json = pattern.export_to_json().unwrap();
        let imported = RemixPattern::import_from_json(&json).unwrap();

        assert_eq!(imported.name, "RoundTrip");
        assert_eq!(imported.bpm, 140.0);
        assert_eq!(imported.key, "5A");
        assert!(!imported.slices.is_empty());
    }

    #[test]
    fn p18b_slice_sample_count_at_rates() {
        let slice = Slice::new("s1".to_string(), "track1".to_string(), 0.0, 1.0);

        assert_eq!(slice.sample_count(44100), 44100);
        assert_eq!(slice.sample_count(48000), 48000);
        assert_eq!(slice.sample_count(96000), 96000);
    }

    #[test]
    fn p18b_arrangement_loop_count_zero() {
        let mut entry = ArrangementEntry::new("s1".to_string(), 0.0);
        entry.set_loop_count(0);
        assert_eq!(entry.loop_count, 0);

        let duration = entry.loop_duration(4.0);
        assert_eq!(duration, 0.0);
    }
}
