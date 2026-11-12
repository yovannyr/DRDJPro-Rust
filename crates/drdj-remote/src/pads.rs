/// Normalized monotonic response curves; zero remains a release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VelocityCurve {
    #[default]
    Linear,
    Exponential,
    Logarithmic,
}
impl VelocityCurve {
    pub fn apply(self, value: u8) -> u8 {
        let x = f64::from(value.min(127)) / 127.0;
        let y = match self {
            Self::Linear => x,
            Self::Exponential => x * x,
            Self::Logarithmic => x.sqrt(),
        };
        if value == 0 {
            0
        } else {
            (y * 127.0).round().clamp(1.0, 127.0) as u8
        }
    }
}
/// Explicit standard MIDI mapping, not a manufacturer/device capability claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadProfile {
    pub channel: u8,
    pub first_note: u8,
    pub curve: VelocityCurve,
}
impl PadProfile {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.channel > 15 || self.first_note > 112 {
            Err("16 pads must fit one MIDI channel/note range")
        } else {
            Ok(self)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadEvent {
    Down { pad: u8, velocity: u8 },
    Pressure { pad: u8, value: u8 },
    Up { pad: u8 },
}
/// Fixed-state decoder. Duplicate strikes cannot retrigger held controls.
/// Session loss releases all pads and prevents pressure from resurrecting a hold.
pub struct PadState {
    profile: PadProfile,
    held: [bool; 16],
}
impl PadState {
    /// Channel pressure affects every held pad; other messages yield at most one event.
    pub fn decode_message(&mut self, bytes: &[u8]) -> [Option<PadEvent>; 16] {
        if bytes.len() == 2 && bytes[0] == (0xd0 | self.profile.channel) && bytes[1] <= 127 {
            return std::array::from_fn(|pad| {
                self.held[pad].then_some(PadEvent::Pressure {
                    pad: pad as u8,
                    value: bytes[1],
                })
            });
        }
        let mut events = [None; 16];
        events[0] = self.decode(bytes);
        events
    }
    pub fn new(profile: PadProfile) -> Result<Self, &'static str> {
        Ok(Self {
            profile: profile.validate()?,
            held: [false; 16],
        })
    }
    pub fn decode(&mut self, bytes: &[u8]) -> Option<PadEvent> {
        if bytes.len() != 3
            || bytes[0] & 15 != self.profile.channel
            || bytes[1] > 127
            || bytes[2] > 127
        {
            return None;
        }
        let pad = bytes[1].checked_sub(self.profile.first_note)?;
        if pad >= 16 {
            return None;
        }
        let held = &mut self.held[usize::from(pad)];
        match (bytes[0] & 0xf0, bytes[2]) {
            (0x90, 1..=127) if !*held => {
                *held = true;
                Some(PadEvent::Down {
                    pad,
                    velocity: self.profile.curve.apply(bytes[2]),
                })
            }
            (0x80, _) | (0x90, 0) if *held => {
                *held = false;
                Some(PadEvent::Up { pad })
            }
            (0xa0, value) if *held => Some(PadEvent::Pressure { pad, value }),
            _ => None,
        }
    }
    /// Fixed release list usable after disconnect, overflow, source/profile change.
    pub fn release_all(&mut self) -> [Option<PadEvent>; 16] {
        std::array::from_fn(|pad| {
            let held = std::mem::take(&mut self.held[pad]);
            held.then_some(PadEvent::Up { pad: pad as u8 })
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_keep_endpoints_and_monotonicity() {
        for curve in [
            VelocityCurve::Linear,
            VelocityCurve::Exponential,
            VelocityCurve::Logarithmic,
        ] {
            assert_eq!(curve.apply(0), 0);
            assert_eq!(curve.apply(127), 127);
            for x in 1..=127 {
                assert!(curve.apply(x) >= curve.apply(x - 1));
                assert!(curve.apply(x) > 0);
            }
        }
        assert!(VelocityCurve::Exponential.apply(64) < 64);
        assert!(VelocityCurve::Logarithmic.apply(64) > 64);
    }
    #[test]
    fn all_pads_velocity_pressure_release_and_reconnect() {
        let mut state = PadState::new(PadProfile {
            channel: 9,
            first_note: 36,
            curve: VelocityCurve::Linear,
        })
        .unwrap();
        for pad in 0..16 {
            assert_eq!(
                state.decode(&[0x99, 36 + pad, 64]),
                Some(PadEvent::Down { pad, velocity: 64 })
            );
            assert_eq!(state.decode(&[0x99, 36 + pad, 127]), None);
            assert_eq!(
                state.decode(&[0xa9, 36 + pad, 91]),
                Some(PadEvent::Pressure { pad, value: 91 })
            );
        }
        assert_eq!(state.release_all().iter().flatten().count(), 16);
        for pad in 0..16 {
            assert_eq!(state.decode(&[0xa9, 36 + pad, 91]), None);
        }
        assert_eq!(
            state.decode(&[0x99, 36, 1]),
            Some(PadEvent::Down {
                pad: 0,
                velocity: 1
            })
        );
        assert_eq!(state.decode(&[0x99, 36, 0]), Some(PadEvent::Up { pad: 0 }));
        assert_eq!(
            state.decode(&[0x99, 36, 1]),
            Some(PadEvent::Down {
                pad: 0,
                velocity: 1
            })
        );
        assert_eq!(
            state.decode(&[0x89, 36, 127]),
            Some(PadEvent::Up { pad: 0 })
        );
    }
    #[test]
    fn malformed_and_unmapped_input_does_not_change_holds() {
        assert!(PadState::new(PadProfile {
            channel: 16,
            first_note: 36,
            curve: VelocityCurve::Linear
        })
        .is_err());
        assert!(PadState::new(PadProfile {
            channel: 0,
            first_note: 113,
            curve: VelocityCurve::Linear
        })
        .is_err());
        let mut s = PadState::new(PadProfile {
            channel: 0,
            first_note: 36,
            curve: VelocityCurve::Linear,
        })
        .unwrap();
        for bytes in [
            &[0x91, 36, 1][..],
            &[0x90, 35, 1],
            &[0x90, 52, 1],
            &[0x90, 36, 128],
            &[0x90, 36],
            &[0xf0, 36, 1],
        ] {
            assert_eq!(s.decode(bytes), None);
        }
        assert_eq!(s.release_all().iter().flatten().count(), 0);
    }
    #[test]
    fn channel_pressure_only_updates_held_pads() {
        let mut s = PadState::new(PadProfile {
            channel: 2,
            first_note: 0,
            curve: VelocityCurve::Linear,
        })
        .unwrap();
        s.decode(&[0x92, 0, 127]);
        s.decode(&[0x92, 15, 64]);
        let events = s.decode_message(&[0xd2, 71]);
        assert_eq!(events.iter().flatten().count(), 2);
        assert_eq!(events[0], Some(PadEvent::Pressure { pad: 0, value: 71 }));
        assert_eq!(events[15], Some(PadEvent::Pressure { pad: 15, value: 71 }));
        assert!(s.decode_message(&[0xd3, 71]).iter().all(Option::is_none));
        s.release_all();
        assert!(s.decode_message(&[0xd2, 71]).iter().all(Option::is_none));
    }
}
