//! Track-owned cue metadata. No audio, filesystem or GUI dependencies.
use std::time::Duration;

pub const SLOT_COUNT: usize = 8;
pub const MAX_POSITION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotCue {
    position: Duration,
    name: String,
    rgb: u32,
}
impl HotCue {
    pub fn new(position: Duration, name: String, rgb: u32) -> Result<Self, String> {
        if position > MAX_POSITION {
            return Err("Cue position exceeds seven days".into());
        }
        if name.chars().count() > 128 || name.chars().any(char::is_control) {
            return Err("Cue name must contain at most 128 characters and no controls".into());
        }
        if rgb > 0xffffff {
            return Err("Cue color must be a 24-bit RGB value".into());
        }
        Ok(Self {
            position,
            name,
            rgb,
        })
    }
    pub fn position(&self) -> Duration {
        self.position
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn rgb(&self) -> u32 {
        self.rgb
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HotCues {
    slots: [Option<HotCue>; SLOT_COUNT],
}
impl HotCues {
    /// Zero-based fixed slots; clearing never shifts another cue's identity.
    pub fn set(&mut self, slot: usize, cue: Option<HotCue>) -> Result<(), String> {
        *self
            .slots
            .get_mut(slot)
            .ok_or("Cue slot must be 0 through 7")? = cue;
        Ok(())
    }
    pub fn slots(&self) -> &[Option<HotCue>; SLOT_COUNT] {
        &self.slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slots_bounds_unicode_colors_and_clear_preserve_identity() {
        let mut cues = HotCues::default();
        let cue = HotCue::new(Duration::new(1, 123456789), "入り口 🎵".into(), 0x12abef).unwrap();
        cues.set(0, Some(cue.clone())).unwrap();
        cues.set(7, Some(cue.clone())).unwrap();
        let before = cues.clone();
        assert!(cues.set(8, None).is_err());
        assert_eq!(cues, before);
        cues.set(0, None).unwrap();
        assert!(cues.slots()[0].is_none());
        assert_eq!(cues.slots()[7], Some(cue));
        assert!(HotCue::new(MAX_POSITION, "🎵".repeat(128), 0xffffff).is_ok());
        assert!(HotCue::new(MAX_POSITION + Duration::from_nanos(1), String::new(), 0).is_err());
        assert!(HotCue::new(Duration::ZERO, "x".repeat(129), 0).is_err());
        assert!(HotCue::new(Duration::ZERO, "a\0b".into(), 0).is_err());
        assert!(HotCue::new(Duration::ZERO, "a\nb".into(), 0).is_err());
        assert!(HotCue::new(Duration::ZERO, String::new(), 0x1000000).is_err());
    }
}
