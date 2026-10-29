//! Versioned session-independent preferences, separate from track/catalog identities.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckQuantizePreferences {
    pub cues: bool,
    pub triggers: bool,
    pub auto_loop_snap: bool,
    pub step: u8,
}
impl Default for DeckQuantizePreferences {
    fn default() -> Self {
        Self {
            cues: false,
            triggers: false,
            auto_loop_snap: false,
            step: 3,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizePreferences {
    pub version: u8,
    pub enabled: bool,
    pub decks: [DeckQuantizePreferences; 2],
}
impl Default for QuantizePreferences {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            decks: [DeckQuantizePreferences::default(); 2],
        }
    }
}
impl QuantizePreferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported Quantize preference version".into());
        }
        for deck in &self.decks {
            crate::beatgrid::QuantizeStep::try_from(i32::from(deck.step))?;
        }
        Ok(())
    }
}
