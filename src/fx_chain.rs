//! Three fixed serial slots. All buffers and coefficient controls are owned by the engine.
use crate::{delay_fx, mixer_dsp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    #[default]
    Delay,
    LowPass,
    HighPass,
}
impl Kind {
    pub const ALL: [Self; 3] = [Self::Delay, Self::LowPass, Self::HighPass];
    pub fn index(self) -> usize {
        match self {
            Self::Delay => 0,
            Self::LowPass => 1,
            Self::HighPass => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Slot {
    pub kind: Kind,
    pub delay: delay_fx::DelaySettings,
    pub timing: crate::fx_timing::Timing,
}
impl Slot {
    pub fn validate(self) -> Result<(), String> {
        self.delay.validate()?;
        self.timing.validate()
    }
}
pub(crate) struct Control {
    pub delay: Arc<delay_fx::DelayControl>,
    pub filter: Arc<mixer_dsp::EqControl>,
}
impl Control {
    pub fn new(rate: u32) -> Arc<Self> {
        Arc::new(Self {
            delay: delay_fx::DelayControl::new(),
            filter: mixer_dsp::EqControl::new(rate),
        })
    }
    pub fn publish(&self, slot: Slot) {
        let mut delay = slot.delay;
        delay.enabled &= slot.kind == Kind::Delay;
        self.delay.publish(delay);
        let amount = if slot.delay.enabled {
            f64::from(slot.delay.wet)
        } else {
            0.
        };
        self.filter.publish(mixer_dsp::EqSettings {
            filter: match slot.kind {
                Kind::Delay => 0.,
                Kind::LowPass => -amount,
                Kind::HighPass => amount,
            },
            ..Default::default()
        });
    }
}
/// Delay then filter per slot; the three slots are composed in fixed order1->2->3.
pub(crate) fn source<S: rodio::Source>(
    input: S,
    control: &Control,
    ring: Arc<delay_fx::DelayRing>,
    cue: [Arc<crate::cue_bus::DeckMixControl>; 2],
    deck: usize,
) -> mixer_dsp::EqSource<delay_fx::DelaySource<S>> {
    mixer_dsp::EqSource::new(
        delay_fx::DelaySource::new(input, Arc::clone(&control.delay), ring)
            .with_cue_controls(cue.clone(), deck),
        Arc::clone(&control.filter),
    )
    .with_cue_controls(cue, deck)
}
/// Three one-second rings share an aggregate64MiB per-deck limit, checked before allocation.
pub(crate) fn rings(rate: u32, channels: u16) -> Result<[Arc<delay_fx::DelayRing>; 3], String> {
    let bytes = (u64::from(rate) + 1) * u64::from(channels) * 4 * 3;
    if bytes > 64 * 1024 * 1024 {
        return Err("FX chain exceeds64 MiB per deck".into());
    }
    Ok([
        delay_fx::DelayRing::new(rate, channels)?,
        delay_fx::DelayRing::new(rate, channels)?,
        delay_fx::DelayRing::new(rate, channels)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;
    #[test]
    fn fx_chain_three_serial_delays_order_and_bypass() {
        let cue = std::array::from_fn(|_| crate::cue_bus::DeckMixControl::new());
        let controls: [Arc<Control>; 3] = std::array::from_fn(|_| Control::new(1000));
        let rings = rings(1000, 2).unwrap();
        for c in &controls {
            c.publish(Slot {
                delay: delay_fx::DelaySettings {
                    enabled: true,
                    time_ms: 20,
                    feedback: 0.,
                    wet: 1.,
                },
                ..Default::default()
            });
        }
        let mut samples = vec![0.; 200];
        samples[0] = 0.5;
        samples[1] = -0.5;
        let a = source(
            rodio::buffer::SamplesBuffer::new(2, 1000, samples),
            &controls[0],
            rings[0].clone(),
            cue.clone(),
            0,
        );
        let b = source(a, &controls[1], rings[1].clone(), cue.clone(), 0);
        let mut c = source(b, &controls[2], rings[2].clone(), cue, 0);
        for frame in 0..100 {
            let a = c.next().unwrap();
            let b = c.next().unwrap();
            assert_eq!(a, -b);
            assert_eq!(a, if frame == 60 { 0.5 } else { 0. });
        }
        assert!(super::rings(192000, 32).is_err());
    }
    #[test]
    fn fx_chain_filters_frequency_and_cue_isolation() {
        for (kind, hz) in [(Kind::LowPass, 1000.), (Kind::HighPass, 100.)] {
            let control = Control::new(48000);
            control.publish(Slot {
                kind,
                delay: delay_fx::DelaySettings {
                    enabled: true,
                    wet: 0.7,
                    ..Default::default()
                },
                ..Default::default()
            });
            let cue = std::array::from_fn(|_| crate::cue_bus::DeckMixControl::new());
            let mut filtered = source(
                rodio::source::SineWave::new(hz).amplify(0.1),
                &control,
                super::rings(48000, 1).unwrap()[0].clone(),
                cue,
                0,
            );
            filtered.by_ref().take(24000).for_each(drop);
            let rms = (filtered
                .take(24000)
                .map(|v| f64::from(v).powi(2))
                .sum::<f64>()
                / 24000.)
                .sqrt();
            assert!(rms < 0.007, "{kind:?}: {rms}");
        }
        let cue: [Arc<crate::cue_bus::DeckMixControl>; 2] =
            std::array::from_fn(|_| crate::cue_bus::DeckMixControl::new());
        cue[1].set_cue(true);
        let control = Control::new(1000);
        control.publish(Slot {
            kind: Kind::HighPass,
            delay: delay_fx::DelaySettings {
                enabled: true,
                wet: 1.,
                ..Default::default()
            },
            ..Default::default()
        });
        let mut output = source(
            rodio::buffer::SamplesBuffer::new(4, 1000, [0.1, 0.2, 0.3, -0.4].repeat(100)),
            &control,
            super::rings(1000, 4).unwrap()[0].clone(),
            cue.clone(),
            0,
        );
        for _ in 0..100 {
            output.next();
            output.next();
            assert_eq!(output.next(), Some(0.3));
            assert_eq!(output.next(), Some(-0.4));
        }
        cue[1].set_cue(false);
        // Cue history was cleared, so changing roles cannot reveal an old headphone signal.
        for _ in 0..25 {
            output.next();
            output.next();
            assert_eq!(output.next(), Some(0.));
            assert_eq!(output.next(), Some(0.));
        }
    }
    #[test]
    fn fx_chain_legacy_profiles_presets_and_bounded_payload() {
        let mut state = delay_fx::FxPreferences::default();
        state.decks[0].enabled = true;
        state.decks[0].time_ms = 777;
        let mut raw = serde_json::to_value(&state).unwrap();
        for key in ["extra", "kind", "preset_chain", "legacy_knobs"] {
            raw.as_object_mut().unwrap().remove(key);
        }
        let migrated: delay_fx::FxPreferences = serde_json::from_value(raw).unwrap();
        assert_eq!(migrated.slot(0, 0).delay.time_ms, 777);
        assert!(migrated.slot(0, 0).delay.enabled);
        assert!(!migrated.slot(0, 1).delay.enabled && !migrated.slot(0, 2).delay.enabled);
        assert_eq!(migrated.legacy_knobs, [true, true]);
        for d in 0..2 {
            for s in 0..3 {
                state.put_slot(
                    d,
                    s,
                    Slot {
                        kind: Kind::HighPass,
                        delay: delay_fx::DelaySettings {
                            enabled: true,
                            feedback: 0.12345678,
                            wet: 0.9876543,
                            ..Default::default()
                        },
                        timing: crate::fx_timing::Timing {
                            base: crate::fx_timing::TimeBase::Tap,
                            tapped_bpm: Some(123.12345678901234),
                            ..Default::default()
                        },
                    },
                );
            }
        }
        for slot in 0..8 {
            state.presets[slot] = Some(state.decks[0]);
            state.preset_timing[slot] = state.timing[0];
            state.preset_chain[slot] = Some(state.chain(0));
        }
        state.validate().unwrap();
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.len() <= 8192, "{}", json.len());
        assert_eq!(
            serde_json::from_str::<delay_fx::FxPreferences>(&json).unwrap(),
            state
        );
    }
}
