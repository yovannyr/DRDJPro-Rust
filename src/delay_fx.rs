//! Per-deck post-fader delay. Coherent controls; ring memory is retained by the engine.
use rodio::Source;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DelaySettings {
    pub enabled: bool,
    pub time_ms: u16,
    pub feedback: f32,
    pub wet: f32,
}
impl Default for DelaySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            time_ms: 250,
            feedback: 0.3,
            wet: 0.25,
        }
    }
}
impl DelaySettings {
    pub fn validate(self) -> Result<(), String> {
        if !(20..=1000).contains(&self.time_ms)
            || !self.feedback.is_finite()
            || !(0. ..=0.85).contains(&self.feedback)
            || !self.wet.is_finite()
            || !(0. ..=1.).contains(&self.wet)
        {
            return Err("Delay requires 20–1000 ms, feedback 0–85% and Dry/Wet 0–100%".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FxPreferences {
    pub version: u8,
    pub decks: [DelaySettings; 2],
    pub presets: [Option<DelaySettings>; 8],
}
impl Default for FxPreferences {
    fn default() -> Self {
        Self {
            version: 1,
            decks: [DelaySettings::default(); 2],
            presets: [None; 8],
        }
    }
}
impl FxPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported FX preferences version".into());
        }
        for value in self.decks.iter().chain(self.presets.iter().flatten()) {
            value.validate()?;
        }
        Ok(())
    }
}

pub(crate) struct DelayControl {
    sequence: AtomicU64,
    values: [AtomicU32; 4],
}
impl DelayControl {
    pub fn new() -> Arc<Self> {
        let value = DelaySettings::default();
        Arc::new(Self {
            sequence: AtomicU64::new(0),
            values: [
                AtomicU32::new(0),
                AtomicU32::new(u32::from(value.time_ms)),
                AtomicU32::new(value.feedback.to_bits()),
                AtomicU32::new(value.wet.to_bits()),
            ],
        })
    }
    // One control-plane writer, enforced by the engine's ownership.
    pub fn publish(&self, value: DelaySettings) {
        assert!(value.validate().is_ok());
        self.sequence.fetch_add(1, Ordering::SeqCst);
        for (target, value) in self.values.iter().zip([
            u32::from(value.enabled),
            u32::from(value.time_ms),
            value.feedback.to_bits(),
            value.wet.to_bits(),
        ]) {
            target.store(value, Ordering::SeqCst);
        }
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    fn read(&self) -> Option<DelaySettings> {
        let before = self.sequence.load(Ordering::SeqCst);
        if before & 1 != 0 {
            return None;
        }
        let v = self.values.each_ref().map(|v| v.load(Ordering::SeqCst));
        (before == self.sequence.load(Ordering::SeqCst)).then_some(DelaySettings {
            enabled: v[0] != 0,
            time_ms: v[1] as u16,
            feedback: f32::from_bits(v[2]),
            wet: f32::from_bits(v[3]),
        })
    }
}
pub(crate) struct DelayRing {
    samples: Vec<AtomicU32>,
    frames: usize,
    channels: usize,
}
impl DelayRing {
    pub fn new(rate: u32, channels: u16) -> Result<Arc<Self>, String> {
        let frames = usize::try_from(rate)
            .map_err(|_| "Delay sample rate exceeds address space")?
            .checked_add(1)
            .ok_or("Delay frame count overflow")?;
        let count = frames
            .checked_mul(usize::from(channels))
            .ok_or("Delay allocation overflow")?;
        if rate == 0 || channels == 0 || count > 16 * 1024 * 1024 {
            return Err("Delay buffer exceeds 64 MiB per deck".into());
        }
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(count)
            .map_err(|e| format!("Cannot reserve delay buffer: {e}"))?;
        samples.resize_with(count, || AtomicU32::new(0));
        Ok(Arc::new(Self {
            samples,
            frames,
            channels: usize::from(channels),
        }))
    }
    fn read(&self, cursor: usize, delay: usize, channel: usize) -> f32 {
        let frame = (cursor + self.frames - delay) % self.frames;
        f32::from_bits(self.samples[frame * self.channels + channel].load(Ordering::Relaxed))
    }
}
pub(crate) struct DelaySource<S> {
    source: S,
    control: Arc<DelayControl>,
    ring: Arc<DelayRing>,
    settings: DelaySettings,
    cursor: usize,
    channel: usize,
    old_tap: usize,
    new_tap: usize,
    pending_tap: usize,
    tap_fade: u32,
    ramp: u32,
    wet: f32,
    feedback: f32,
    target_wet: f32,
    target_feedback: f32,
    smoothing: u32,
    cue_controls: Option<[Arc<crate::cue_bus::DeckMixControl>; 2]>,
    cue_deck: usize,
    auxiliary_passthrough: bool,
}
impl<S: Source> DelaySource<S> {
    pub fn new(source: S, control: Arc<DelayControl>, ring: Arc<DelayRing>) -> Self {
        assert_eq!(usize::from(source.channels()), ring.channels);
        assert_eq!(u64::from(source.sample_rate()) + 1, ring.frames as u64);
        let settings = control
            .read()
            .expect("delay installed on control-plane writer");
        let tap = ((u64::from(source.sample_rate()) * u64::from(settings.time_ms)) / 1000).max(1)
            as usize;
        let wet = if settings.enabled { settings.wet } else { 0. };
        let ramp = source.sample_rate().div_ceil(200).max(1);
        Self {
            source,
            control,
            ring,
            settings,
            cursor: 0,
            channel: 0,
            old_tap: tap,
            new_tap: tap,
            pending_tap: tap,
            tap_fade: 0,
            ramp,
            wet,
            feedback: settings.feedback,
            target_wet: wet,
            target_feedback: settings.feedback,
            smoothing: 0,
            auxiliary_passthrough: false,
            cue_controls: None,
            cue_deck: 0,
        }
    }
    pub fn with_cue_controls(
        mut self,
        controls: [Arc<crate::cue_bus::DeckMixControl>; 2],
        deck: usize,
    ) -> Self {
        assert!(deck < 2);
        self.cue_controls = Some(controls);
        self.cue_deck = deck;
        self
    }
}
impl<S: Source> Iterator for DelaySource<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // A deck mixer can admit sources later; keep its fixed-format effect bus alive.
        let dry = self.source.next().unwrap_or(0.);
        if self.channel == 0 {
            self.auxiliary_passthrough = self.cue_controls.as_ref().is_some_and(|controls| {
                controls[self.cue_deck].rendered_cue() || controls.iter().any(|c| c.monitor_mode())
            });
            if let Some(settings) = self.control.read() {
                if settings != self.settings {
                    self.settings = settings;
                    self.target_wet = if settings.enabled { settings.wet } else { 0. };
                    self.target_feedback = settings.feedback;
                    self.smoothing = self.ramp;
                    self.pending_tap = ((u64::from(self.source.sample_rate())
                        * u64::from(settings.time_ms))
                        / 1000)
                        .max(1) as usize;
                }
            }
            if self.tap_fade == 0 && self.pending_tap != self.old_tap {
                self.new_tap = self.pending_tap;
                self.tap_fade = 1;
            }
            if self.smoothing != 0 {
                self.wet += (self.target_wet - self.wet) / self.smoothing as f32;
                self.feedback += (self.target_feedback - self.feedback) / self.smoothing as f32;
                self.smoothing -= 1;
                if self.smoothing == 0 {
                    self.wet = self.target_wet;
                    self.feedback = self.target_feedback;
                }
            }
        }
        let a = self.ring.read(self.cursor, self.old_tap, self.channel);
        let echo = if self.tap_fade == 0 {
            a
        } else {
            let b = self.ring.read(self.cursor, self.new_tap, self.channel);
            a + (b - a) * self.tap_fade as f32 / self.ramp as f32
        };
        // Never record the pre-fader headphone bus in master effect history.
        let written = if self.auxiliary_passthrough && self.channel >= 2 {
            0.
        } else if dry.is_finite() {
            dry + self.feedback * echo
        } else {
            0.
        };
        self.ring.samples[self.cursor * self.ring.channels + self.channel].store(
            if written.is_finite() && written.abs() >= 1e-30 {
                written.to_bits()
            } else {
                0
            },
            Ordering::Relaxed,
        );
        let output = if self.wet == 0. || (self.auxiliary_passthrough && self.channel >= 2) {
            dry
        } else {
            (1. - self.wet) * dry + self.wet * echo
        };
        self.channel += 1;
        if self.channel == self.ring.channels {
            self.channel = 0;
            self.cursor = (self.cursor + 1) % self.ring.frames;
            if self.tap_fade == self.ramp {
                self.old_tap = self.new_tap;
                self.tap_fade = 0;
            } else if self.tap_fade != 0 {
                self.tap_fade += 1;
            }
        }
        Some(output)
    }
}
impl<S: Source> Source for DelaySource<S> {
    fn channels(&self) -> u16 {
        self.source.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabling_last_cue_between_source_and_effect_cannot_record_a_cued_frame() {
        struct DisableAfterSample<S> {
            source: S,
            control: Arc<crate::cue_bus::DeckMixControl>,
        }
        impl<S: Source> Iterator for DisableAfterSample<S> {
            type Item = f32;
            fn next(&mut self) -> Option<f32> {
                let sample = self.source.next();
                self.control.set_cue(false);
                sample
            }
        }
        impl<S: Source> Source for DisableAfterSample<S> {
            fn channels(&self) -> u16 {
                self.source.channels()
            }
            fn sample_rate(&self) -> u32 {
                self.source.sample_rate()
            }
            fn current_span_len(&self) -> Option<usize> {
                None
            }
            fn total_duration(&self) -> Option<Duration> {
                None
            }
        }
        let controls = std::array::from_fn(|_| crate::cue_bus::DeckMixControl::new());
        controls[1].set_gain(0.);
        controls[1].set_cue(true);
        let mut pcm = vec![0.; 400];
        pcm[0] = 1.;
        pcm[1] = -0.5;
        let source = DisableAfterSample {
            source: crate::cue_bus::CueSend::new(
                rodio::buffer::SamplesBuffer::new(4, 1000, pcm),
                controls[1].clone(),
            ),
            control: controls[1].clone(),
        };
        let fx = DelayControl::new();
        fx.publish(DelaySettings {
            enabled: true,
            time_ms: 20,
            feedback: 0.,
            wet: 1.,
        });
        let mut output = DelaySource::new(source, fx, DelayRing::new(1000, 4).unwrap())
            .with_cue_controls(controls, 1);
        assert_eq!(
            (output.next(), output.next(), output.next(), output.next()),
            (Some(0.), Some(0.), Some(1.), Some(-0.5))
        );
        for _ in 0..300 {
            assert_eq!(output.next(), Some(0.));
        }
    }
    #[test]
    fn auxiliary_delay_remains_live_without_cue_and_never_records_headphone_audio() {
        let fx = DelayControl::new();
        fx.publish(DelaySettings {
            enabled: true,
            time_ms: 20,
            feedback: 0.,
            wet: 1.,
        });
        let controls = std::array::from_fn(|_| crate::cue_bus::DeckMixControl::new());
        let mut impulse = vec![0.; 400];
        impulse[2] = 1.;
        impulse[3] = -0.5;
        let mut legacy = DelaySource::new(
            rodio::buffer::SamplesBuffer::new(4, 1000, impulse),
            fx.clone(),
            DelayRing::new(1000, 4).unwrap(),
        )
        .with_cue_controls(controls.clone(), 1);
        for _ in 0..80 {
            assert_eq!(legacy.next(), Some(0.));
        }
        assert_eq!(
            (legacy.next(), legacy.next(), legacy.next(), legacy.next()),
            (Some(0.), Some(0.), Some(1.), Some(-0.5))
        );
        let mut data = [0., 0., 0.5, -0.25].repeat(40);
        data.extend([0.; 400]);
        controls[1].set_cue(true);
        let mut source = DelaySource::new(
            rodio::buffer::SamplesBuffer::new(4, 1000, data),
            fx,
            DelayRing::new(1000, 4).unwrap(),
        )
        .with_cue_controls(controls.clone(), 1);
        for _ in 0..40 {
            assert_eq!(
                (source.next(), source.next(), source.next(), source.next()),
                (Some(0.), Some(0.), Some(0.5), Some(-0.25))
            );
        }
        controls[0].set_cue(true);
        controls[1].set_cue(false);
        for _ in 0..160 {
            assert_eq!(source.next(), Some(0.));
        }
        controls[0].set_cue(false);
        for _ in 0..160 {
            assert_eq!(source.next(), Some(0.));
        }
    }
    #[test]
    fn delay_bypass_ramps_and_latest_time_target() {
        let control = DelayControl::new();
        control.publish(DelaySettings {
            feedback: 0.,
            wet: 1.,
            ..Default::default()
        });
        let mut source = DelaySource::new(
            rodio::buffer::SamplesBuffer::new(1, 1000, vec![1.; 1000]),
            Arc::clone(&control),
            DelayRing::new(1000, 1).unwrap(),
        );
        for _ in 0..10 {
            assert_eq!(source.next(), Some(1.));
        }
        control.publish(DelaySettings {
            enabled: true,
            feedback: 0.,
            wet: 1.,
            ..Default::default()
        });
        for expected in [0.8_f32, 0.6, 0.4, 0.2, 0.] {
            assert!((source.next().unwrap() - expected).abs() < 1e-6);
        }
        control.publish(DelaySettings {
            feedback: 0.,
            wet: 1.,
            ..Default::default()
        });
        for expected in [0.2_f32, 0.4, 0.6, 0.8, 1.] {
            assert!((source.next().unwrap() - expected).abs() < 1e-6);
        }
        let control = DelayControl::new();
        control.publish(DelaySettings {
            enabled: true,
            time_ms: 20,
            feedback: 0.,
            wet: 1.,
        });
        let mut source = DelaySource::new(
            rodio::buffer::SamplesBuffer::new(
                1,
                1000,
                (0..1000).map(|i| i as f32 * 0.01).collect::<Vec<_>>(),
            ),
            Arc::clone(&control),
            DelayRing::new(1000, 1).unwrap(),
        );
        for _ in 0..22 {
            source.next();
        }
        control.publish(DelaySettings {
            enabled: true,
            time_ms: 30,
            feedback: 0.,
            wet: 1.,
        });
        source.next();
        source.next();
        control.publish(DelaySettings {
            enabled: true,
            time_ms: 40,
            feedback: 0.,
            wet: 1.,
        });
        for _ in 24..60 {
            source.next();
        }
        assert!((source.next().unwrap() - 0.2).abs() < 1e-6);
    }
    #[test]
    fn delay_impulse_bypass_stereo_and_feedback() {
        let control = DelayControl::new();
        control.publish(DelaySettings {
            enabled: true,
            time_ms: 20,
            feedback: 0.5,
            wet: 1.,
        });
        let mut samples = vec![0.; 100];
        samples[0] = 1.;
        let mut source = DelaySource::new(
            rodio::buffer::SamplesBuffer::new(2, 1000, samples),
            control,
            DelayRing::new(1000, 2).unwrap(),
        );
        for _ in 0..20 {
            assert_eq!(source.next(), Some(0.));
            assert_eq!(source.next(), Some(0.));
        }
        assert_eq!(source.next(), Some(1.));
        assert_eq!(source.next(), Some(0.));
        for _ in 0..19 {
            source.next();
            source.next();
        }
        assert_eq!(source.next(), Some(0.5));
        assert_eq!(source.next(), Some(0.));
        let control = DelayControl::new();
        let ring = DelayRing::new(48000, 1).unwrap();
        let mut source = DelaySource::new(
            rodio::source::SineWave::new(440.),
            Arc::clone(&control),
            Arc::clone(&ring),
        );
        let stats = crate::looping::measured_heap(|| {
            for _ in 0..20000 {
                source.next();
            }
            drop(source);
        });
        assert_eq!(stats.allocations, 0);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.deallocations, 0);
        assert!(DelayRing::new(u32::MAX, 32).is_err());
    }
}
