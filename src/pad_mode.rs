//! Control-plane routing for the MC4000's four pads (no audio-thread state).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    HotCue,
    Sampler,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Cue { held: bool, delete: bool },
    Sample { play: bool },
}

#[derive(Default)]
pub struct Routing {
    modes: [Mode; 2],
    down: [u8; 2],
    cues: [u8; 2],
    mode_down: [bool; 2],
}

impl Routing {
    pub fn mode(&self, deck: usize) -> Mode {
        self.modes[deck]
    }

    /// Toggle once per physical press, including NoteOn-zero releases.
    pub fn mode_edge(&mut self, deck: usize, held: bool) -> bool {
        let toggle = held && !self.mode_down[deck];
        self.mode_down[deck] = held;
        toggle
    }

    pub fn toggle(&mut self, deck: usize) {
        self.modes[deck] = match self.modes[deck] {
            Mode::HotCue => Mode::Sampler,
            Mode::Sampler => Mode::HotCue,
        };
        // Gedrueckte Pads bleiben gesperrt bis zum echten Release. Ein spaetes
        // NoteOff darf im neuen Modus keine andere Aktion ausloesen.
        self.cues[deck] = 0;
    }

    #[cfg(any(target_os = "windows", test))]
    pub fn cancel_cues(&mut self) {
        self.cues = [0; 2];
    }

    pub fn pad(&mut self, deck: usize, slot: usize, held: bool, delete: bool) -> Option<Action> {
        if deck >= 2 || slot >= 4 {
            return None;
        }
        let bit = 1 << (slot + if delete { 4 } else { 0 });
        if !held {
            self.down[deck] &= !bit;
            let cue = self.cues[deck] & bit != 0;
            self.cues[deck] &= !bit;
            return cue.then_some(Action::Cue {
                held: false,
                delete,
            });
        }
        if self.down[deck] & bit != 0 {
            return None;
        }
        self.down[deck] |= bit;
        match self.modes[deck] {
            Mode::HotCue => {
                self.cues[deck] |= bit;
                Some(Action::Cue { held: true, delete })
            }
            Mode::Sampler => Some(Action::Sample { play: !delete }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_edges_and_held_pads_require_real_release_across_both_modes() {
        let mut r = Routing::default();
        for deck in 0..2 {
            assert_eq!(r.mode(deck), Mode::HotCue);
            assert!(r.mode_edge(deck, true));
            assert!(!r.mode_edge(deck, true));
            assert!(!r.mode_edge(deck, false));
            for slot in 0..4 {
                assert_eq!(
                    r.pad(deck, slot, true, false),
                    Some(Action::Cue {
                        held: true,
                        delete: false
                    })
                );
            }
            r.toggle(deck);
            for slot in 0..4 {
                assert_eq!(r.pad(deck, slot, true, false), None);
                assert_eq!(r.pad(deck, slot, false, false), None);
                assert_eq!(
                    r.pad(deck, slot, true, false),
                    Some(Action::Sample { play: true })
                );
            }
            r.toggle(deck);
            for slot in 0..4 {
                assert_eq!(r.pad(deck, slot, true, false), None);
                assert_eq!(r.pad(deck, slot, false, false), None);
                assert_eq!(
                    r.pad(deck, slot, true, false),
                    Some(Action::Cue {
                        held: true,
                        delete: false
                    })
                );
                assert_eq!(
                    r.pad(deck, slot, false, false),
                    Some(Action::Cue {
                        held: false,
                        delete: false
                    })
                );
            }
        }
    }

    #[test]
    fn shifted_stop_is_one_shot_and_decks_are_independent() {
        let mut r = Routing::default();
        r.toggle(0);
        for slot in 0..4 {
            assert_eq!(
                r.pad(0, slot, true, true),
                Some(Action::Sample { play: false })
            );
            assert_eq!(r.pad(0, slot, false, true), None);
            assert_eq!(
                r.pad(1, slot, true, false),
                Some(Action::Cue {
                    held: true,
                    delete: false
                })
            );
        }
        r.cancel_cues();
        assert_eq!(r.pad(1, 0, true, false), None);
        assert_eq!(r.pad(1, 0, false, false), None);
        assert_eq!(r.pad(2, 0, true, false), None);
        assert_eq!(r.pad(0, 4, true, false), None);
    }
}
