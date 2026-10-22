//! Pure state transitions for the simulated deck transport.
//! No audio playback is implemented in this UI prototype.

pub const STEP_PERCENT: f32 = 0.12;
const END_PERCENT: f32 = 100.0;

pub fn toggle_playing(playing: bool) -> bool {
    !playing
}

pub fn cue_position() -> f32 {
    0.0
}

pub fn advance_position(position: f32, playing: bool) -> f32 {
    if !playing {
        return position;
    }
    (position + STEP_PERCENT).rem_euclid(END_PERCENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_play_pause() {
        assert!(toggle_playing(false));
        assert!(!toggle_playing(true));
    }

    #[test]
    fn cue_resets_to_start() {
        assert_eq!(cue_position(), 0.0);
    }

    #[test]
    fn paused_deck_does_not_advance() {
        assert_eq!(advance_position(42.0, false), 42.0);
    }

    #[test]
    fn playing_deck_advances() {
        assert!((advance_position(10.0, true) - 10.12).abs() < 0.0001);
    }

    #[test]
    fn playing_deck_wraps_at_end() {
        assert!((advance_position(99.95, true) - 0.07).abs() < 0.0001);
    }

    #[test]
    fn deck_positions_are_independent() {
        let left = advance_position(20.0, true);
        let right = advance_position(70.0, false);
        assert!((left - 20.12).abs() < 0.0001);
        assert_eq!(right, 70.0);
    }
}
