//! Fixed-format deck bus with one output-frame clock. This is a render clock, not a device clock.
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct RenderClock {
    pub completed: AtomicU64,
    pub peak_window: AtomicU64,
}
const MAX_FRAME_AGE_MS: u64 = 5;
const MAX_PROGRESS_GAP: Duration = Duration::from_millis(100);
impl RenderClock {
    /// Bound render-coordinate age and frame advancement during sampling; wall time is separate.
    pub fn frame_is_fresh(frame: u64, before: u64, after: u64, rate: u32) -> bool {
        if rate == 0 || frame == 0 {
            return false;
        }
        let budget = (u64::from(rate) * MAX_FRAME_AGE_MS).div_ceil(1000);
        after
            .checked_sub(before)
            .is_some_and(|elapsed| elapsed <= budget)
            && after.checked_sub(frame).is_some_and(|age| age <= budget)
    }
}
#[derive(Default)]
pub(crate) struct ClockObserver {
    last: Option<(u64, Instant)>,
    progress: Option<Instant>,
}
impl ClockObserver {
    /// Control-plane observation only. Acquisition needs progress between timely observations.
    pub fn observe(&mut self, frame: u64, now: Instant) -> bool {
        if let Some((previous, at)) = self.last {
            let timely = now
                .checked_duration_since(at)
                .is_some_and(|gap| gap <= MAX_PROGRESS_GAP);
            if !timely || frame < previous {
                self.progress = None;
            } else if frame > previous {
                self.progress = Some(now);
            }
        }
        self.last = Some((frame, now));
        self.progress.is_some_and(|at| {
            now.checked_duration_since(at)
                .is_some_and(|age| age <= MAX_PROGRESS_GAP)
        })
    }
}
pub(crate) struct RenderBus<S> {
    source: S,
    clock: Arc<RenderClock>,
    channels: u16,
    rate: u32,
    channel: u16,
    peak: crate::peak_meter::PeakAccumulator,
    cue_controls: Option<[Arc<crate::cue_bus::DeckMixControl>; 2]>,
    stereo_meter: bool,
}
impl<S: Source> RenderBus<S> {
    pub fn new(source: S) -> (Self, Arc<RenderClock>) {
        let channels = source.channels();
        let rate = source.sample_rate();
        assert!(channels > 0 && rate > 0);
        let clock = Arc::new(RenderClock::default());
        (
            Self {
                source,
                clock: Arc::clone(&clock),
                channels,
                rate,
                channel: 0,
                peak: crate::peak_meter::PeakAccumulator::new(rate),
                stereo_meter: false,
                cue_controls: None,
            },
            clock,
        )
    }
    pub fn with_cue_meter(mut self, controls: [Arc<crate::cue_bus::DeckMixControl>; 2]) -> Self {
        self.cue_controls = Some(controls);
        self
    }
}
impl<S: Source> Iterator for RenderBus<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // MixerSource can admit late sources after returning None; retain the bus at silence.
        let sample = self.source.next().unwrap_or(0.);
        if self.channel == 0 {
            self.stereo_meter = self
                .cue_controls
                .as_ref()
                .is_some_and(|controls| controls.iter().any(|c| c.cue()));
        }
        if !self.stereo_meter || self.channel < 2 {
            self.peak.sample(sample);
        }
        self.channel += 1;
        if self.channel == self.channels {
            self.channel = 0;
            self.peak.complete_frame(&self.clock.peak_window);
            self.clock.completed.fetch_add(1, Ordering::Release);
        }
        Some(sample)
    }
}
impl<S: Source> Source for RenderBus<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_freshness_rejects_future_old_and_slow_pair_reads_at_exact_limits() {
        for (rate, budget) in [(44100, 221), (48000, 240), (96000, 480)] {
            assert!(RenderClock::frame_is_fresh(
                1000 - budget,
                1000 - budget,
                1000,
                rate
            ));
            assert!(!RenderClock::frame_is_fresh(
                1000 - budget - 1,
                1000,
                1000,
                rate
            ));
            assert!(!RenderClock::frame_is_fresh(
                1000,
                1000 - budget - 1,
                1000,
                rate
            ));
            assert!(!RenderClock::frame_is_fresh(1001, 1000, 1000, rate));
            assert!(!RenderClock::frame_is_fresh(1000, 1001, 1000, rate));
        }
        assert!(!RenderClock::frame_is_fresh(0, 1000, 1000, 48000));
        assert!(!RenderClock::frame_is_fresh(1000, 1000, 1000, 0));
    }
    #[test]
    fn clock_observer_requires_timely_progress_and_expires_stalls_and_poll_gaps() {
        let start = Instant::now();
        let mut observer = ClockObserver::default();
        assert!(!observer.observe(10, start));
        assert!(!observer.observe(10, start + Duration::from_millis(10)));
        assert!(observer.observe(11, start + Duration::from_millis(20)));
        assert!(observer.observe(11, start + Duration::from_millis(120)));
        assert!(!observer.observe(11, start + Duration::from_millis(121)));
        assert!(observer.observe(12, start + Duration::from_millis(122)));
        assert!(!observer.observe(100, start + Duration::from_millis(300)));
        assert!(!observer.observe(100, start + Duration::from_millis(301)));
        assert!(observer.observe(101, start + Duration::from_millis(302)));
        assert!(!observer.observe(1, start + Duration::from_millis(303)));
        assert!(observer.observe(2, start + Duration::from_millis(304)));
        assert!(!observer.observe(3, start));
    }
    #[test]
    fn bus_counts_complete_frames_during_silence_and_admits_late_stereo_sources() {
        let (input, source) = rodio::mixer::mixer(2, 48000);
        let (mut bus, clock) = RenderBus::new(source);
        assert_eq!(bus.next(), Some(0.));
        assert_eq!(clock.completed.load(Ordering::Acquire), 0);
        assert_eq!(bus.next(), Some(0.));
        assert_eq!(clock.completed.load(Ordering::Acquire), 1);
        // Insert mid-frame: Rodio delays admission to its next complete channel boundary.
        assert_eq!(bus.next(), Some(0.));
        input.add(rodio::buffer::SamplesBuffer::new(
            2,
            48000,
            vec![0.25, -0.5, 0.5, -0.75],
        ));
        assert_eq!(bus.next(), Some(0.));
        assert_eq!(bus.next(), Some(0.25));
        assert_eq!(bus.next(), Some(-0.5));
        assert_eq!(clock.completed.load(Ordering::Acquire), 3);
        assert_eq!(bus.next(), Some(0.5));
        assert_eq!(bus.next(), Some(-0.75));
        for _ in 0..8 {
            assert_eq!(bus.next(), Some(0.));
        }
        assert_eq!(clock.completed.load(Ordering::Acquire), 8);
        assert_eq!(
            (bus.channels(), bus.sample_rate(), bus.current_span_len()),
            (2, 48000, None)
        );
    }
    #[test]
    fn bus_peak_uses_complete_windows_all_channels_and_preserves_output_bits() {
        let samples: Vec<f32> = (0..100)
            .map(|i| {
                if i == 99 {
                    -1.25
                } else if i % 2 == 0 {
                    -0.25
                } else {
                    0.5
                }
            })
            .collect();
        let source = rodio::buffer::SamplesBuffer::new(2, 1000, samples.clone());
        let (mut bus, clock) = RenderBus::new(source);
        for sample in &samples[..99] {
            assert_eq!(bus.next().unwrap().to_bits(), sample.to_bits());
            assert_eq!(
                crate::peak_meter::PeakAccumulator::read_peak(&clock.peak_window),
                None
            );
        }
        assert_eq!(clock.completed.load(Ordering::Acquire), 49);
        assert_eq!(bus.next().unwrap().to_bits(), samples[99].to_bits());
        assert_eq!(
            crate::peak_meter::PeakAccumulator::read_peak(&clock.peak_window),
            Some(1.25)
        );
        for _ in 0..100 {
            assert_eq!(bus.next(), Some(0.));
        }
        assert_eq!(
            crate::peak_meter::PeakAccumulator::read_peak(&clock.peak_window),
            Some(0.)
        );
        assert_eq!(clock.completed.load(Ordering::Acquire), 100);
    }

    #[test]
    fn warmed_bus_pull_adds_no_allocations_and_counts_frames_at_supported_formats() {
        for (channels, rate) in [(1, 44100), (2, 48000), (4, 96000)] {
            let (input, source) = rodio::mixer::mixer(channels, rate);
            input.add(rodio::buffer::SamplesBuffer::new(
                channels,
                rate,
                vec![0.125; 200000],
            ));
            let (mut bus, clock) = RenderBus::new(source);
            for _ in 0..16 {
                bus.next();
            }
            let allocations = crate::looping::counted_allocations(|| {
                for _ in 0..100000 {
                    assert_eq!(bus.next(), Some(0.125));
                }
            });
            assert_eq!(allocations, 0);
            assert_eq!(
                crate::peak_meter::PeakAccumulator::read_peak(&clock.peak_window),
                Some(0.125)
            );
            assert_eq!(
                clock.completed.load(Ordering::Acquire),
                100016 / u64::from(channels)
            );
        }
    }
}
