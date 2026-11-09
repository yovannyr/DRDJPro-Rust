//! Harmonie-Wheel — key compatibility and harmonic mixing support.
//!
//! Implements the Camelot Wheel for determining key compatibility
//! between tracks and enabling harmonic mixing. Part of the Harmonie
//! feature (P06a).
//!
//! ## Design principles
//!
//! * **Camelot Wheel standard** — Compatible with Serato/Pioneer systems.
//! * **Harmonic mixing** — Identify compatible keys for smooth transitions.
//! * **Confidence scores** — Report analysis reliability.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Musical key in Camelot notation (e.g., "8A", "11B").
///
/// A-keys = Minor, B-keys = Major.
/// Number 1-12 corresponds to the circle of fifths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CamelotKey {
    /// Circle of fifths position (1-12).
    pub number: u8,
    /// Mode: A = Minor, B = Major.
    pub mode: KeyMode,
}

impl CamelotKey {
    /// Parse a Camelot key string.
    pub fn parse(key: &str) -> Option<Self> {
        let (number, suffix) = key.split_at_checked(key.len().checked_sub(1)?)?;
        if number.is_empty() || number.len() > 2 || number.starts_with('0') {
            return None;
        }
        let num = number.parse::<u8>().ok()?;
        if !(1..=12).contains(&num) {
            return None;
        }
        let mode = match suffix {
            "A" => KeyMode::Minor,
            "B" => KeyMode::Major,
            _ => return None,
        };

        Some(Self { number: num, mode })
    }

    /// Convert to standard notation (e.g., "C Major", "A Minor").
    pub fn to_standard(&self) -> String {
        let note = self.to_note();
        let mode_str = match self.mode {
            KeyMode::Major => "Major",
            KeyMode::Minor => "Minor",
        };
        format!("{} {}", note, mode_str)
    }

    /// Get the note name for this key.
    pub fn to_note(&self) -> &'static str {
        // Camelot: A is minor, B is major; 8A = A minor, 8B = C major.
        const MAJOR: [&str; 12] = [
            "B", "F#/Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E",
        ];
        const MINOR: [&str; 12] = [
            "G#/Ab", "D#/Eb", "A#/Bb", "F", "C", "G", "D", "A", "E", "B", "F#/Gb", "C#/Db",
        ];
        let notes = match self.mode {
            KeyMode::Major => &MAJOR,
            KeyMode::Minor => &MINOR,
        };
        self.number
            .checked_sub(1)
            .and_then(|i| notes.get(i as usize))
            .copied()
            .unwrap_or("?")
    }
}

impl fmt::Display for CamelotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            self.number,
            match self.mode {
                KeyMode::Minor => 'A',
                KeyMode::Major => 'B',
            }
        )
    }
}

/// Key mode: Major or Minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyMode {
    Major,
    Minor,
}

impl fmt::Display for KeyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyMode::Major => write!(f, "Major"),
            KeyMode::Minor => write!(f, "Minor"),
        }
    }
}

/// Key compatibility relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCompatibility {
    /// Same key — perfectly compatible.
    Same,
    /// One step on the Camelot wheel — very compatible.
    Adjacent,
    /// Relative major/minor — compatible.
    Relative,
    /// Enharmonic equivalent — compatible.
    Enharmonic,
    /// Five steps on the wheel — somewhat compatible.
    Distant,
    /// Not recommended for mixing.
    Incompatible,
}

impl fmt::Display for KeyCompatibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCompatibility::Same => write!(f, "same"),
            KeyCompatibility::Adjacent => write!(f, "adjacent"),
            KeyCompatibility::Relative => write!(f, "relative"),
            KeyCompatibility::Enharmonic => write!(f, "enharmonic"),
            KeyCompatibility::Distant => write!(f, "distant"),
            KeyCompatibility::Incompatible => write!(f, "incompatible"),
        }
    }
}

/// Harmonic analysis result for a track.
#[derive(Debug, Clone)]
pub struct KeyAnalysis {
    /// Detected Camelot key.
    pub key: CamelotKey,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Alternative key suggestions (with confidence).
    pub alternatives: Vec<(CamelotKey, f64)>,
    /// Whether the analysis is reliable.
    pub reliable: bool,
}

impl KeyAnalysis {
    /// Create a new key analysis.
    pub fn new(key: CamelotKey, confidence: f64, alternatives: Vec<(CamelotKey, f64)>) -> Self {
        Self {
            key,
            confidence,
            alternatives,
            reliable: confidence > 0.5,
        }
    }

    /// Get compatible keys for harmonic mixing.
    pub fn compatible_keys(&self) -> Vec<CamelotKey> {
        let mut compatible = vec![self.key];

        // Same key.
        compatible.push(self.key);

        // Adjacent keys on Camelot wheel (±1, same mode).
        for offset in [-1i32, 1] {
            let adj_num = ((self.key.number as i32 + offset - 1).rem_euclid(12) + 1) as u8;
            compatible.push(CamelotKey {
                number: adj_num,
                mode: self.key.mode,
            });
        }

        // Relative key (A↔B at same number).
        let relative_mode = match self.key.mode {
            KeyMode::Major => KeyMode::Minor,
            KeyMode::Minor => KeyMode::Major,
        };
        compatible.push(CamelotKey {
            number: self.key.number,
            mode: relative_mode,
        });

        compatible
    }
}

/// Key compatibility chart between two keys.
#[derive(Debug, Clone)]
pub struct KeyCompatibilityResult {
    /// First key.
    pub key_a: CamelotKey,
    /// Second key.
    pub key_b: CamelotKey,
    /// Compatibility level.
    pub compatibility: KeyCompatibility,
    /// Compatibility score (0.0 - 1.0).
    pub score: f64,
    /// Reason for the compatibility level.
    pub reason: String,
}

/// Harmonie wheel manager.
pub struct HarmonieWheel {
    /// Key compatibility cache (key_a → key_b → score).
    compatibility_cache:
        std::sync::Mutex<HashMap<(CamelotKey, CamelotKey), (KeyCompatibility, f64)>>,
}

impl HarmonieWheel {
    /// Create a new harmonie wheel.
    pub fn new() -> Self {
        Self {
            compatibility_cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Get compatibility between two keys.
    pub fn get_compatibility(
        &self,
        key_a: CamelotKey,
        key_b: CamelotKey,
    ) -> KeyCompatibilityResult {
        // Check cache.
        if let Some(cached) = self
            .compatibility_cache
            .lock()
            .unwrap()
            .get(&(key_a, key_b))
        {
            return KeyCompatibilityResult {
                key_a,
                key_b,
                compatibility: cached.0,
                score: cached.1,
                reason: get_compatibility_reason(cached.0, key_a, key_b),
            };
        }

        let (compatibility, score) = calculate_key_compatibility(key_a, key_b);
        let reason = get_compatibility_reason(compatibility, key_a, key_b);

        let mut cache = self.compatibility_cache.lock().unwrap();
        cache.insert((key_a, key_b), (compatibility, score));

        KeyCompatibilityResult {
            key_a,
            key_b,
            compatibility,
            score,
            reason,
        }
    }

    /// Get all keys compatible with a given key for harmonic mixing.
    pub fn get_compatible_keys(&self, key: CamelotKey) -> Vec<(CamelotKey, KeyCompatibility, f64)> {
        let mut compatible = Vec::new();

        // Same key.
        compatible.push((key, KeyCompatibility::Same, 1.0));

        // Adjacent keys (±1 on wheel, same mode).
        for offset in [-1i32, 1] {
            let num = ((key.number as i32 + offset - 1).rem_euclid(12) + 1) as u8;
            let adj = CamelotKey {
                number: num,
                mode: key.mode,
            };
            compatible.push((adj, KeyCompatibility::Adjacent, 0.9));
        }

        // Relative key.
        let relative_mode = match key.mode {
            KeyMode::Major => KeyMode::Minor,
            KeyMode::Minor => KeyMode::Major,
        };
        let relative = CamelotKey {
            number: key.number,
            mode: relative_mode,
        };
        compatible.push((relative, KeyCompatibility::Relative, 0.85));

        // Fifth step away (powerful transition).
        let fifth_num = ((key.number as i32 + 5 - 1).rem_euclid(12) + 1) as u8;
        let fifth_major = CamelotKey {
            number: fifth_num,
            mode: KeyMode::Major,
        };
        let fifth_minor = CamelotKey {
            number: fifth_num,
            mode: KeyMode::Minor,
        };
        compatible.push((fifth_major, KeyCompatibility::Distant, 0.7));
        compatible.push((fifth_minor, KeyCompatibility::Distant, 0.7));

        compatible.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        compatible
    }

    /// Suggest the best mixing key for a given track key.
    pub fn suggest_mixing_key(
        &self,
        current_key: CamelotKey,
        exclude: &[CamelotKey],
    ) -> Option<(CamelotKey, f64)> {
        let compatible = self.get_compatible_keys(current_key);

        for (key, compat, score) in &compatible {
            if !exclude.contains(key) && *compat != KeyCompatibility::Same {
                return Some((*key, *score));
            }
        }

        None
    }

    /// Validate a Camelot key string.
    pub fn validate_key(key: &str) -> bool {
        CamelotKey::parse(key).is_some()
    }

    /// Convert a major-key tonic to its Camelot number (B ring).
    pub fn note_to_key_number(note: &str) -> Option<u8> {
        match note {
            "B" | "Cb" => Some(1),
            "F#" | "Gb" => Some(2),
            "C#" | "Db" => Some(3),
            "G#" | "Ab" => Some(4),
            "D#" | "Eb" => Some(5),
            "A#" | "Bb" => Some(6),
            "F" | "E#" => Some(7),
            "C" | "B#" => Some(8),
            "G" => Some(9),
            "D" => Some(10),
            "A" => Some(11),
            "E" | "Fb" => Some(12),
            _ => None,
        }
    }

    /// Get the major ring in ascending Camelot order.
    pub fn circle_of_fifths(&self) -> Vec<(u8, &'static str)> {
        (1..=12)
            .map(|number| {
                (
                    number,
                    CamelotKey {
                        number,
                        mode: KeyMode::Major,
                    }
                    .to_note(),
                )
            })
            .collect()
    }
}

impl Default for HarmonieWheel {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate key compatibility.
fn calculate_key_compatibility(key_a: CamelotKey, key_b: CamelotKey) -> (KeyCompatibility, f64) {
    // Same key.
    if key_a == key_b {
        return (KeyCompatibility::Same, 1.0);
    }

    // Same number, different mode (relative major/minor).
    if key_a.number == key_b.number && key_a.mode != key_b.mode {
        return (KeyCompatibility::Relative, 0.85);
    }

    // Adjacent on the Camelot wheel (same mode, ±1 number).
    if key_a.mode == key_b.mode {
        let diff = (key_a.number as i32 - key_b.number as i32).abs();
        if diff == 1 || diff == 11 {
            // 11 wraps around the circle.
            return (KeyCompatibility::Adjacent, 0.9);
        }
    }

    // Fifth step away (powerful key change).
    let num_diff = (key_a.number as i32 - key_b.number as i32)
        .rem_euclid(12)
        .abs();
    if num_diff == 5 || num_diff == 7 {
        // Perfect fifth/sixth.
        return (KeyCompatibility::Distant, 0.7);
    }

    // Otherwise incompatible for smooth mixing.
    (KeyCompatibility::Incompatible, 0.2)
}

/// Get human-readable reason for compatibility.
fn get_compatibility_reason(
    compatibility: KeyCompatibility,
    key_a: CamelotKey,
    key_b: CamelotKey,
) -> String {
    match compatibility {
        KeyCompatibility::Same => format!("Same key: {}", key_a),
        KeyCompatibility::Adjacent => {
            format!(
                "Adjacent on wheel: {} → {} (step {})",
                key_a,
                key_b,
                if key_b.number as i32 - key_a.number as i32 == 1
                    || key_a.number as i32 - key_b.number as i32 == 11
                {
                    "+"
                } else {
                    "-"
                }
            )
        }
        KeyCompatibility::Relative => {
            format!("Relative {} ({} → {})", key_a.mode, key_a, key_b)
        }
        KeyCompatibility::Distant => {
            format!("Powerful transition: {} → {} (fifth step)", key_a, key_b)
        }
        KeyCompatibility::Incompatible => {
            format!(
                "Not recommended: {} does not mix well with {}",
                key_a, key_b
            )
        }
        KeyCompatibility::Enharmonic => "Enharmonic equivalent".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_camelot_key() {
        assert_eq!(
            CamelotKey::parse("8A"),
            Some(CamelotKey {
                number: 8,
                mode: KeyMode::Minor
            })
        );
        assert_eq!(
            CamelotKey::parse("11B"),
            Some(CamelotKey {
                number: 11,
                mode: KeyMode::Major
            })
        );
        assert_eq!(
            CamelotKey::parse("1A"),
            Some(CamelotKey {
                number: 1,
                mode: KeyMode::Minor
            })
        );
        assert_eq!(
            CamelotKey::parse("12B"),
            Some(CamelotKey {
                number: 12,
                mode: KeyMode::Major
            })
        );
        assert!(CamelotKey::parse("13A").is_none());
        assert!(CamelotKey::parse("0A").is_none());
        assert!(CamelotKey::parse("8C").is_none());
        assert!(CamelotKey::parse("").is_none());
    }

    #[test]
    fn camelot_key_to_standard() {
        assert_eq!(CamelotKey::parse("8B").unwrap().to_standard(), "C Major");
        assert_eq!(CamelotKey::parse("5A").unwrap().to_standard(), "C Minor");
        assert_eq!(CamelotKey::parse("12B").unwrap().to_standard(), "E Major");
        assert_eq!(CamelotKey::parse("9A").unwrap().to_standard(), "E Minor");
    }

    #[test]
    fn camelot_key_display() {
        assert_eq!(format!("{}", CamelotKey::parse("8A").unwrap()), "8A");
        assert_eq!(format!("{}", CamelotKey::parse("11B").unwrap()), "11B");
    }

    #[test]
    fn key_compatibility_same() {
        let wheel = HarmonieWheel::new();
        let result = wheel.get_compatibility(
            CamelotKey {
                number: 8,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 8,
                mode: KeyMode::Major,
            },
        );
        assert_eq!(result.compatibility, KeyCompatibility::Same);
        assert_eq!(result.score, 1.0);
        assert!(result.reason.contains("Same key"));
    }

    #[test]
    fn key_compatibility_adjacent() {
        let wheel = HarmonieWheel::new();
        let result = wheel.get_compatibility(
            CamelotKey {
                number: 8,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 9,
                mode: KeyMode::Major,
            },
        );
        assert_eq!(result.compatibility, KeyCompatibility::Adjacent);
        assert_eq!(result.score, 0.9);
    }

    #[test]
    fn key_compatibility_relative() {
        let wheel = HarmonieWheel::new();
        let result = wheel.get_compatibility(
            CamelotKey {
                number: 8,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 8,
                mode: KeyMode::Minor,
            },
        );
        assert_eq!(result.compatibility, KeyCompatibility::Relative);
        assert_eq!(result.score, 0.85);
    }

    #[test]
    fn key_compatibility_distant() {
        let wheel = HarmonieWheel::new();
        let result = wheel.get_compatibility(
            CamelotKey {
                number: 1,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 6,
                mode: KeyMode::Major,
            },
        );
        // 5 steps apart.
        assert_eq!(result.compatibility, KeyCompatibility::Distant);
        assert_eq!(result.score, 0.7);
    }

    #[test]
    fn key_compatibility_incompatible() {
        let wheel = HarmonieWheel::new();
        let result = wheel.get_compatibility(
            CamelotKey {
                number: 1,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 4,
                mode: KeyMode::Major,
            },
        );
        assert_eq!(result.compatibility, KeyCompatibility::Incompatible);
        assert_eq!(result.score, 0.2);
    }

    #[test]
    fn compatible_keys_list() {
        let wheel = HarmonieWheel::new();
        let compatible = wheel.get_compatible_keys(CamelotKey {
            number: 8,
            mode: KeyMode::Major,
        });

        assert!(!compatible.is_empty());
        // First should be same key.
        assert_eq!(
            compatible[0].0,
            CamelotKey {
                number: 8,
                mode: KeyMode::Major
            }
        );
        assert_eq!(compatible[0].1, KeyCompatibility::Same);

        // Should include adjacent and relative.
        let has_adjacent = compatible.iter().any(|(k, compat, _)| {
            *compat == KeyCompatibility::Adjacent && k.mode == KeyMode::Major
        });
        let has_relative = compatible
            .iter()
            .any(|(_k, compat, _)| *compat == KeyCompatibility::Relative);
        assert!(has_adjacent);
        assert!(has_relative);
    }

    #[test]
    fn suggest_mixing_key() {
        let wheel = HarmonieWheel::new();
        let current = CamelotKey {
            number: 8,
            mode: KeyMode::Major,
        };

        let suggestion = wheel.suggest_mixing_key(current, &[]);
        assert!(suggestion.is_some());
        let (key, score) = suggestion.unwrap();
        assert!(score >= 0.7);
        assert!(key != current); // Should suggest something different.
    }

    #[test]
    fn suggest_mixing_key_excludes() {
        let wheel = HarmonieWheel::new();
        let current = CamelotKey {
            number: 8,
            mode: KeyMode::Major,
        };

        // Exclude all compatible keys except one.
        let compatible = wheel.get_compatible_keys(current);
        let exclude: Vec<CamelotKey> = compatible.iter().map(|(k, _, _)| *k).collect();

        let suggestion = wheel.suggest_mixing_key(current, &exclude);
        assert!(suggestion.is_none());
    }

    #[test]
    fn validate_key() {
        assert!(HarmonieWheel::validate_key("1A"));
        assert!(HarmonieWheel::validate_key("12B"));
        assert!(!HarmonieWheel::validate_key("13A"));
        assert!(!HarmonieWheel::validate_key("0A"));
        assert!(!HarmonieWheel::validate_key("8C"));
        assert!(!HarmonieWheel::validate_key(""));
        assert!(!HarmonieWheel::validate_key("ABC"));
    }

    #[test]
    fn note_to_key_number() {
        assert_eq!(HarmonieWheel::note_to_key_number("C"), Some(8));
        assert_eq!(HarmonieWheel::note_to_key_number("G"), Some(9));
        assert_eq!(HarmonieWheel::note_to_key_number("F"), Some(7));
        assert!(HarmonieWheel::note_to_key_number("X").is_none());
    }

    #[test]
    fn circle_of_fifths() {
        let wheel = HarmonieWheel::new();
        let cof = wheel.circle_of_fifths();
        assert_eq!(cof.len(), 12);
        assert_eq!(cof[0], (1, "B"));
        assert_eq!(cof[1], (2, "F#/Gb"));
        assert_eq!(cof[11], (12, "E"));
    }

    #[test]
    fn compatibility_cache() {
        let wheel = HarmonieWheel::new();

        // First call.
        let r1 = wheel.get_compatibility(
            CamelotKey {
                number: 8,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 9,
                mode: KeyMode::Major,
            },
        );

        // Reverse call (should use cache or recalculate).
        let r2 = wheel.get_compatibility(
            CamelotKey {
                number: 9,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 8,
                mode: KeyMode::Major,
            },
        );

        // Compatibility should be symmetric.
        assert_eq!(r1.compatibility, r2.compatibility);
        assert_eq!(r1.score, r2.score);
    }

    #[test]
    fn key_analysis() {
        let key = CamelotKey::parse("8A").unwrap();
        let analysis = KeyAnalysis::new(key, 0.92, vec![(CamelotKey::parse("9A").unwrap(), 0.65)]);

        assert_eq!(analysis.key, key);
        assert_eq!(analysis.confidence, 0.92);
        assert!(analysis.reliable);
        assert_eq!(analysis.alternatives.len(), 1);
    }

    #[test]
    fn key_analysis_unreliable() {
        let key = CamelotKey::parse("5B").unwrap();
        let analysis = KeyAnalysis::new(key, 0.3, vec![]);

        assert!(!analysis.reliable);
        assert!(analysis.alternatives.is_empty());
    }

    #[test]
    fn wrapping_key_numbers() {
        let wheel = HarmonieWheel::new();

        // 12 → 1 should be adjacent.
        let result = wheel.get_compatibility(
            CamelotKey {
                number: 12,
                mode: KeyMode::Major,
            },
            CamelotKey {
                number: 1,
                mode: KeyMode::Major,
            },
        );
        assert_eq!(result.compatibility, KeyCompatibility::Adjacent);
    }
}
