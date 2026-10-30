//! Frame-boundary selection of a stereo output pair; no callback allocation or locks.
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorSettings {
    pub gain: f32,
    pub mix: f32,
}
impl Default for MonitorSettings {
    fn default() -> Self {
        Self { gain: 1., mix: 0. }
    }
}
impl MonitorSettings {
    pub fn validate(self) -> Result<(), String> {
        if [self.gain, self.mix]
            .into_iter()
            .all(|v| v.is_finite() && (0. ..=1.).contains(&v))
        {
            Ok(())
        } else {
            Err("Monitor gain and mix must be finite values between 0 and 1".into())
        }
    }
    fn bits(self) -> u64 {
        u64::from(self.gain.to_bits()) | (u64::from(self.mix.to_bits()) << 32)
    }
    fn from_bits(bits: u64) -> Self {
        Self {
            gain: f32::from_bits(bits as u32),
            mix: f32::from_bits((bits >> 32) as u32),
        }
    }
}
pub struct RoutingControl {
    channels: u16,
    requested: AtomicU32,
    monitor: AtomicU64,
}
impl RoutingControl {
    pub fn set_monitor(&self, settings: MonitorSettings) {
        self.monitor.store(settings.bits(), Ordering::Release);
    }
    pub fn pair(&self) -> u16 {
        self.requested.load(Ordering::Acquire) as u16
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
    pub fn set_pair(&self, pair: u16) -> Result<(), String> {
        if pair != 0 && u32::from(pair) * 2 + 1 >= u32::from(self.channels) {
            return Err("Output pair is unavailable on the current audio device".into());
        }
        // One control-plane writer; route and reservation publish together.
        let state = self.requested.load(Ordering::Acquire);
        if state & (1 << 16) != 0 && pair == 1 {
            return Err("Output pair 3/4 is reserved for active headphone cue".into());
        }
        self.requested
            .store((state & (1 << 16)) | u32::from(pair), Ordering::Release);
        Ok(())
    }
    pub fn set_cue_enabled(&self, enabled: bool) -> Result<(), String> {
        if enabled && self.channels < 4 {
            return Err("Independent headphone cue requires at least four output channels".into());
        }
        let state = self.requested.load(Ordering::Acquire);
        if enabled && state as u16 == 1 {
            return Err("Select a master pair other than 3/4 before enabling headphone cue".into());
        }
        self.requested.store(
            (state & 0xffff) | (u32::from(enabled) << 16),
            Ordering::Release,
        );
        Ok(())
    }
}
pub struct PairRouter<S> {
    source: S,
    control: Arc<RoutingControl>,
    channel: u16,
    pair: u16,
    stereo: [f32; 2],
    cue: bool,
    monitor: MonitorSettings,
    target: MonitorSettings,
    ramp: u32,
    remaining: u32,
    monitor_tail: bool,
}
impl<S: Source> PairRouter<S> {
    pub fn new(source: S) -> (Self, Arc<RoutingControl>) {
        assert!(source.channels() > 0);
        let control = Arc::new(RoutingControl {
            channels: source.channels(),
            requested: AtomicU32::new(0),
            monitor: AtomicU64::new(MonitorSettings::default().bits()),
        });
        let ramp = source.sample_rate().div_ceil(200).max(1);
        (
            Self {
                source,
                control: Arc::clone(&control),
                channel: 0,
                pair: 0,
                stereo: [0.; 2],
                cue: false,
                monitor: MonitorSettings::default(),
                target: MonitorSettings::default(),
                ramp,
                remaining: 0,
                monitor_tail: false,
            },
            control,
        )
    }
}
impl<S: Source> Iterator for PairRouter<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 {
            let was_monitoring = self.cue || self.monitor_tail;
            let state = self.control.requested.load(Ordering::Acquire);
            self.pair = state as u16;
            self.cue = state & (1 << 16) != 0;
            let target = MonitorSettings::from_bits(self.control.monitor.load(Ordering::Acquire));
            if target != self.target {
                self.target = target;
                self.remaining = self.ramp;
            }
            if self.remaining > 0 {
                self.monitor.gain += (self.target.gain - self.monitor.gain) / self.remaining as f32;
                self.monitor.mix += (self.target.mix - self.monitor.mix) / self.remaining as f32;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.monitor = self.target;
                }
            }
            // Finish parameter ramps after the last cue/preview request is disabled.
            // An explicit master reroute to 3/4 takes precedence over this tail.
            self.monitor_tail = !self.cue
                && was_monitoring
                && (self.remaining > 0 || self.monitor.mix > 0.)
                && self.pair != 1;
        }
        let input = self.source.next()?;
        if self.channel < 2 {
            self.stereo[usize::from(self.channel)] = input;
        }
        let output = if (self.cue || self.monitor_tail) && self.channel / 2 == 1 {
            self.monitor.gain
                * ((1. - self.monitor.mix) * if self.cue { input } else { 0. }
                    + self.monitor.mix * self.stereo[usize::from(self.channel % 2)])
        } else if self.pair == 0 && (!self.cue || self.channel < 2) {
            // Keep the original mapping, including mono and existing multichannel sources.
            input
        } else if self.channel / 2 == self.pair {
            self.stereo[usize::from(self.channel % 2)]
        } else {
            0.
        };
        self.channel = (self.channel + 1) % self.control.channels;
        Some(output)
    }
}
impl<S: Source> Source for PairRouter<S> {
    fn channels(&self) -> u16 {
        self.source.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headphone_gain_mix_ramp_is_stereo_coherent_and_leaves_master_untouched() {
        let (mut source, control) = PairRouter::new(rodio::buffer::SamplesBuffer::new(
            4,
            1000,
            [0.2, -0.4, 0.8, -0.6].repeat(100),
        ));
        control.set_cue_enabled(true).unwrap();
        control.set_monitor(MonitorSettings { gain: 0., mix: 1. });
        for frame in 1..=5 {
            let gain = 1. - frame as f32 / 5.;
            let mix = frame as f32 / 5.;
            let actual: Vec<_> = source.by_ref().take(4).collect();
            assert_eq!(&actual[..2], &[0.2, -0.4]);
            assert!((actual[2] - gain * ((1. - mix) * 0.8 + mix * 0.2)).abs() < 1e-6);
            assert!((actual[3] - gain * ((1. - mix) * -0.6 + mix * -0.4)).abs() < 1e-6);
        }
        assert_eq!(
            source.by_ref().take(4).collect::<Vec<_>>(),
            vec![0.2, -0.4, 0., 0.]
        );
    }
    #[test]
    fn master_preview_fades_out_when_last_monitor_request_is_disabled() {
        let (mut source, control) = PairRouter::new(rodio::buffer::SamplesBuffer::new(
            4,
            1000,
            [1., -0.5, 0., 0.].repeat(30),
        ));
        control.set_cue_enabled(true).unwrap();
        control.set_monitor(MonitorSettings { gain: 1., mix: 1. });
        source.by_ref().take(24).for_each(drop);
        control.set_cue_enabled(false).unwrap();
        // Force the engine's publication window: route changes before mix arrives.
        assert_eq!(
            source.by_ref().take(4).collect::<Vec<_>>(),
            vec![1., -0.5, 1., -0.5]
        );
        control.set_monitor(MonitorSettings { gain: 1., mix: 0. });
        for step in 1..=5 {
            let frame: Vec<_> = source.by_ref().take(4).collect();
            assert_eq!(&frame[..2], &[1., -0.5]);
            assert!((frame[2] - (1. - step as f32 / 5.)).abs() < 1e-6);
            assert!((frame[3] + 0.5 * (1. - step as f32 / 5.)).abs() < 1e-6);
        }
    }
    #[test]
    fn cue_pair_is_separate_from_remapped_master_and_route_changes_are_frame_bound() {
        let (mut source, control) = PairRouter::new(rodio::buffer::SamplesBuffer::new(
            6,
            48000,
            [1., -1., 0.25, -0.5, 0., 0.].repeat(3),
        ));
        control.set_pair(2).unwrap();
        control.set_cue_enabled(true).unwrap();
        assert!(control.set_pair(1).is_err());
        assert_eq!(source.next(), Some(0.));
        control.set_pair(0).unwrap();
        assert_eq!(
            source.by_ref().take(5).collect::<Vec<_>>(),
            vec![0., 0.25, -0.5, 1., -1.]
        );
        assert_eq!(
            source.by_ref().take(6).collect::<Vec<_>>(),
            vec![1., -1., 0.25, -0.5, 0., 0.]
        );
    }
    #[test]
    fn routes_complete_stereo_frames_and_preserves_default_mapping() {
        let data = [1., -1., 7., 8., 2., -2., 9., 10., 3., -3., 11., 12.];
        let (mut router, control) =
            PairRouter::new(rodio::buffer::SamplesBuffer::new(4, 48000, data.to_vec()));
        assert_eq!(router.next(), Some(1.));
        control.set_pair(1).unwrap();
        assert_eq!(
            router.by_ref().take(3).collect::<Vec<_>>(),
            vec![-1., 7., 8.]
        );
        assert_eq!(router.next(), Some(0.));
        control.set_pair(0).unwrap();
        assert_eq!(
            router.by_ref().take(3).collect::<Vec<_>>(),
            vec![0., 2., -2.]
        );
        assert_eq!(router.collect::<Vec<_>>(), vec![3., -3., 11., 12.]);
        assert!(control.set_pair(2).is_err());
        assert_eq!(control.pair(), 0);
        for channels in [1, 2, 3] {
            let (router, control) = PairRouter::new(rodio::buffer::SamplesBuffer::new(
                channels,
                48000,
                vec![0.25; usize::from(channels) * 2],
            ));
            assert!(control.set_pair(1).is_err());
            assert_eq!(
                router.collect::<Vec<_>>(),
                vec![0.25; usize::from(channels) * 2]
            );
        }
        for channels in [4, 6, 8] {
            let mut frame = vec![0.; usize::from(channels)];
            frame[0] = 0.25;
            frame[1] = -0.5;
            let (router, control) =
                PairRouter::new(rodio::buffer::SamplesBuffer::new(channels, 48000, frame));
            let pair = channels / 2 - 1;
            control.set_pair(pair).unwrap();
            let actual = router.collect::<Vec<_>>();
            for (channel, value) in actual.into_iter().enumerate() {
                let expected = if channel == usize::from(pair) * 2 {
                    0.25
                } else if channel == usize::from(pair) * 2 + 1 {
                    -0.5
                } else {
                    0.
                };
                assert_eq!(value, expected);
            }
        }
    }
}
