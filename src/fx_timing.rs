//! Musical delay timing on the control plane; no callback state or dynamic tap history.
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TimeBase {
    #[default]
    Milliseconds,
    Deck,
    Tap,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Division {
    Sixteenth,
    Eighth,
    Quarter,
    Half,
    #[default]
    One,
    Two,
    Four,
}
impl Division {
    pub const ALL: [Self; 7] = [
        Self::Sixteenth,
        Self::Eighth,
        Self::Quarter,
        Self::Half,
        Self::One,
        Self::Two,
        Self::Four,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(4)
    }
    pub fn beats(self) -> f64 {
        [0.0625, 0.125, 0.25, 0.5, 1., 2., 4.][self.index()]
    }
    pub fn label(self) -> &'static str {
        ["1/16", "1/8", "1/4", "1/2", "1", "2", "4"][self.index()]
    }
    pub fn stepped(self, step: i32) -> Self {
        Self::ALL[(self.index() as i32 + step).clamp(0, 6) as usize]
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Timing {
    pub base: TimeBase,
    pub division: Division,
    pub tapped_bpm: Option<f64>,
}
impl Timing {
    pub fn validate(self) -> Result<(), String> {
        if self
            .tapped_bpm
            .is_some_and(|v| !v.is_finite() || !(40. ..=240.).contains(&v))
            || (self.base == TimeBase::Tap && self.tapped_bpm.is_none())
        {
            return Err("Tap tempo requires 40-240 BPM".into());
        }
        Ok(())
    }
    pub fn milliseconds(self, manual: u16, deck_bpm: Option<f64>) -> Result<u16, String> {
        self.validate()?;
        if self.base == TimeBase::Milliseconds {
            return Ok(manual);
        }
        let bpm = match self.base {
            TimeBase::Tap => self.tapped_bpm,
            _ => deck_bpm,
        }
        .filter(|v| v.is_finite() && *v > 0.)
        .ok_or("Save a deck grid or tap a tempo first")?;
        let ms = 60_000. * self.division.beats() / bpm;
        if !(20. ..=1000.).contains(&ms) {
            return Err(format!(
                "{} beats at {bpm:.2} BPM exceeds delay range 20-1000 ms",
                self.division.label()
            ));
        }
        Ok(ms.round() as u16)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TapAction {
    Tempo(f64),
    Reset,
}
#[derive(Default)]
pub(crate) struct TapState {
    held: Option<Instant>,
    reset_sent: bool,
    last: Option<Instant>,
    intervals: [f64; 4],
    len: usize,
    cursor: usize,
    outliers: u8,
}
impl TapState {
    pub fn press(&mut self, now: Instant) {
        if self.held.is_none() {
            self.held = Some(now);
            self.reset_sent = false;
        }
    }
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
    pub fn poll(&mut self, now: Instant) -> Option<TapAction> {
        if self
            .held
            .and_then(|at| now.checked_duration_since(at))
            .is_some_and(|d| d >= Duration::from_secs(1))
            && !self.reset_sent
        {
            self.reset_sent = true;
            self.last = None;
            self.len = 0;
            self.cursor = 0;
            self.outliers = 0;
            return Some(TapAction::Reset);
        }
        None
    }
    pub fn release(&mut self, now: Instant) -> Option<TapAction> {
        let reset = self.poll(now);
        let at = self.held.take()?;
        if self.reset_sent {
            return reset;
        }
        if now < at {
            self.cancel();
            return None;
        }
        let Some(last) = self.last else {
            self.last = Some(at);
            return None;
        };
        let Some(elapsed) = at.checked_duration_since(last) else {
            self.cancel();
            return None;
        };
        let seconds = elapsed.as_secs_f64();
        if seconds < 0.25 {
            return None;
        } // bounce: retain the previous real tap
        self.last = Some(at);
        if seconds > 1.5 {
            self.len = 0;
            self.cursor = 0;
            self.outliers = 0;
            return None;
        }
        if self.len >= 2 && (seconds - self.median()).abs() > self.median() * 0.25 {
            self.outliers += 1;
            if self.outliers < 2 {
                return None;
            }
            self.len = 0;
            self.cursor = 0;
        }
        self.outliers = 0;
        self.intervals[self.cursor] = seconds;
        self.cursor = (self.cursor + 1) % 4;
        self.len = (self.len + 1).min(4);
        (self.len >= 2).then(|| TapAction::Tempo((60. / self.median()).clamp(40., 240.)))
    }
    fn median(&self) -> f64 {
        let mut sorted = self.intervals;
        sorted[..self.len].sort_unstable_by(f64::total_cmp);
        if self.len.is_multiple_of(2) {
            (sorted[self.len / 2 - 1] + sorted[self.len / 2]) * 0.5
        } else {
            sorted[self.len / 2]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fx_musical_preferences_legacy_and_preset_roundtrip() {
        let mut state = crate::delay_fx::FxPreferences::default();
        let mut legacy = serde_json::to_value(&state).unwrap();
        legacy.as_object_mut().unwrap().remove("timing");
        legacy.as_object_mut().unwrap().remove("preset_timing");
        assert_eq!(
            serde_json::from_value::<crate::delay_fx::FxPreferences>(legacy).unwrap(),
            state
        );
        state.timing[1] = Timing {
            base: TimeBase::Tap,
            division: Division::Half,
            tapped_bpm: Some(123.),
        };
        state.preset_timing[7] = state.timing[1];
        state.presets[7] = Some(state.decks[1]);
        state.validate().unwrap();
        assert_eq!(
            serde_json::from_str::<crate::delay_fx::FxPreferences>(
                &serde_json::to_string(&state).unwrap()
            )
            .unwrap(),
            state
        );
        state.timing[0] = Timing {
            base: TimeBase::Tap,
            ..Default::default()
        };
        assert!(state.validate().is_err());
        let malformed = r#"{"base":"deck","division":"sixteen","tapped_bpm":null}"#;
        assert!(serde_json::from_str::<Timing>(malformed).is_err());
    }
    #[test]
    fn fx_musical_units_limits_legacy_and_validation() {
        let mut t = Timing {
            base: TimeBase::Deck,
            ..Default::default()
        };
        assert_eq!(t.milliseconds(250, Some(120.)).unwrap(), 500);
        t.division = Division::Half;
        assert_eq!(t.milliseconds(250, Some(150.)).unwrap(), 200);
        assert!(t.milliseconds(250, None).is_err());
        assert!(t.milliseconds(250, Some(f64::NAN)).is_err());
        t.division = Division::Four;
        assert!(t.milliseconds(250, Some(120.)).is_err());
        assert_eq!(t.milliseconds(250, Some(240.)).unwrap(), 1000);
        t.division = Division::Sixteenth;
        assert!(t.milliseconds(250, Some(240.)).is_err());
        t.base = TimeBase::Tap;
        assert!(t.validate().is_err());
        t.tapped_bpm = Some(120.);
        assert_eq!(t.milliseconds(250, None).unwrap(), 31);
        t.tapped_bpm = Some(f64::INFINITY);
        assert!(t.validate().is_err());
        assert_eq!(Timing::default().milliseconds(333, None).unwrap(), 333);
        assert_eq!(Division::One.stepped(-99), Division::Sixteenth);
        assert_eq!(Division::One.stepped(99), Division::Four);
    }
    #[test]
    fn fx_tap_bounce_outliers_timeout_hold_and_cancellation() {
        let start = Instant::now();
        let mut t = TapState::default();
        let mut tap = |ms| {
            let at = start + Duration::from_millis(ms);
            t.press(at);
            t.press(at);
            t.release(at + Duration::from_millis(50))
        };
        assert_eq!(tap(0), None);
        assert_eq!(tap(500), None);
        assert_eq!(tap(550), None);
        assert_eq!(tap(1000), Some(TapAction::Tempo(120.)));
        assert_eq!(tap(1750), None);
        assert_eq!(tap(2250), Some(TapAction::Tempo(120.)));
        assert_eq!(tap(5000), None);
        assert_eq!(tap(5500), None);
        assert_eq!(tap(6000), Some(TapAction::Tempo(120.)));
        t.press(start + Duration::from_secs(7));
        assert_eq!(t.poll(start + Duration::from_millis(7999)), None);
        assert_eq!(
            t.poll(start + Duration::from_secs(8)),
            Some(TapAction::Reset)
        );
        assert_eq!(t.poll(start + Duration::from_secs(9)), None);
        assert_eq!(t.release(start + Duration::from_secs(9)), None);
        t.press(start + Duration::from_secs(10));
        t.cancel();
        assert_eq!(t.release(start + Duration::from_secs(12)), None);
        t.press(start + Duration::from_secs(15));
        assert_eq!(t.release(start + Duration::from_secs(14)), None);
    }
}
