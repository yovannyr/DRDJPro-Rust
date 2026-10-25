//! Durable loop metadata; independent of playback and source sample rate.
use std::time::Duration;
pub const SLOT_COUNT: usize = 8;
pub const MAX_POSITION: Duration = Duration::from_secs(604800);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedLoop {
    start: Duration,
    end: Duration,
    name: String,
}
impl SavedLoop {
    pub fn new(start: Duration, end: Duration, name: String) -> Result<Self, String> {
        if start >= end || end > MAX_POSITION {
            return Err("Saved loop needs In < Out within seven days".into());
        }
        if name.chars().count() > 128 || name.chars().any(char::is_control) {
            return Err("Loop name must contain at most 128 characters and no controls".into());
        }
        Ok(Self { start, end, name })
    }
    pub fn start(&self) -> Duration {
        self.start
    }
    pub fn end(&self) -> Duration {
        self.end
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedLoops {
    slots: [Option<SavedLoop>; SLOT_COUNT],
}
impl SavedLoops {
    pub fn set(&mut self, slot: usize, value: Option<SavedLoop>) -> Result<(), String> {
        *self
            .slots
            .get_mut(slot)
            .ok_or("Loop slot must be 0 through 7")? = value;
        Ok(())
    }
    pub fn slots(&self) -> &[Option<SavedLoop>; SLOT_COUNT] {
        &self.slots
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_names_sparse_identity_and_failed_changes_are_validated() {
        let value = SavedLoop::new(
            Duration::from_nanos(1),
            Duration::from_nanos(2),
            "入り口 🎵".into(),
        )
        .unwrap();
        let mut loops = SavedLoops::default();
        loops.set(0, Some(value.clone())).unwrap();
        loops.set(7, Some(value.clone())).unwrap();
        let before = loops.clone();
        assert!(loops.set(8, None).is_err());
        assert_eq!(loops, before);
        loops.set(0, None).unwrap();
        assert!(loops.slots()[0].is_none());
        assert_eq!(loops.slots()[7], Some(value));
        assert!(SavedLoop::new(Duration::ZERO, Duration::ZERO, String::new()).is_err());
        assert!(SavedLoop::new(
            Duration::from_secs(2),
            Duration::from_secs(1),
            String::new()
        )
        .is_err());
        assert!(SavedLoop::new(Duration::ZERO, MAX_POSITION, "🎵".repeat(128)).is_ok());
        assert!(SavedLoop::new(
            Duration::ZERO,
            MAX_POSITION + Duration::from_nanos(1),
            String::new()
        )
        .is_err());
        for name in ["a".repeat(129), "a\nb".into(), "a\0b".into()] {
            assert!(SavedLoop::new(Duration::ZERO, Duration::from_secs(1), name).is_err());
        }
    }
}
