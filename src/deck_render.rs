//! Fixed output-format PCM renderer. Controls are atomic; the audio path never waits for a writer.
use crate::looping::{FrameLoop, PcmSource};
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Clone, Copy)]
pub(crate) struct SeekFrame {
    pub time: Duration,
    pub frame: usize,
}
#[derive(Clone, Copy)]
pub(crate) struct RenderFormat {
    pub source_rate: u32,
    pub output_rate: u32,
    pub frames: usize,
}
impl RenderFormat {
    fn duration_ns(self) -> u64 {
        (self.frames as u128 * 1_000_000_000 / u128::from(self.source_rate)) as u64
    }
}
#[derive(Clone, Copy)]
struct Settings {
    sequence: u64,
    rate: u32,
    neutral_rate: u32,
    sync_until: u64,
    paused: bool,
    region: Option<FrameLoop>,
    seek_serial: u64,
    seek_ns: u64,
    seek_units: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RenderStamp {
    pub phase_units: u64,
    pub output_frame: u64,
}
pub(crate) struct RenderControl {
    format: RenderFormat,
    sequence: AtomicU64,
    rate: AtomicU32,
    neutral_rate: AtomicU32,
    sync_until: AtomicU64,
    expired_command: AtomicU64,
    paused: AtomicBool,
    start: AtomicUsize,
    end: AtomicUsize,
    seek_serial: AtomicU64,
    seek_ns: AtomicU64,
    applied_seek: AtomicU64,
    seek_units: AtomicU64,
    phase_units: AtomicU64,
    stamp_sequence: AtomicU64,
    stamp_phase: AtomicU64,
    stamp_frame: AtomicU64,
    stamp_command: AtomicU64,
    pub stopped: AtomicBool,
}
impl RenderControl {
    pub fn new(
        rate: u32,
        playing: bool,
        region: Option<FrameLoop>,
        seek: SeekFrame,
        format: RenderFormat,
    ) -> Arc<Self> {
        assert!(format.source_rate > 0 && format.output_rate > 0);
        assert!(format.frames as u128 * u128::from(format.output_rate) <= u128::from(u64::MAX));
        Arc::new(Self {
            format,
            sequence: AtomicU64::new(0),
            rate: AtomicU32::new(rate),
            neutral_rate: AtomicU32::new(rate),
            sync_until: AtomicU64::new(0),
            expired_command: AtomicU64::new(u64::MAX),
            paused: AtomicBool::new(!playing),
            start: AtomicUsize::new(region.map_or(0, |r| r.start)),
            end: AtomicUsize::new(region.map_or(0, |r| r.end)),
            seek_serial: AtomicU64::new(1),
            seek_ns: AtomicU64::new(seek.time.as_nanos() as u64),
            seek_units: AtomicU64::new(
                (seek.frame as u128 * u128::from(format.output_rate)) as u64,
            ),
            applied_seek: AtomicU64::new(0),
            phase_units: AtomicU64::new(0),
            stamp_sequence: AtomicU64::new(0),
            stamp_phase: AtomicU64::new(0),
            stamp_frame: AtomicU64::new(0),
            stamp_command: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        })
    }
    // Single control-plane writer (the engine is !Send/!Sync). All payload fields are atomic.
    pub fn publish(
        &self,
        rate: u32,
        playing: bool,
        region: Option<FrameLoop>,
        seek: Option<SeekFrame>,
    ) {
        self.publish_guarded(rate, playing, region, seek, None);
    }
    pub fn publish_sync(
        &self,
        rate: u32,
        neutral_rate: u32,
        until: u64,
        playing: bool,
        region: Option<FrameLoop>,
    ) {
        self.publish_guarded(rate, playing, region, None, Some((neutral_rate, until)));
    }
    fn publish_guarded(
        &self,
        rate: u32,
        playing: bool,
        region: Option<FrameLoop>,
        seek: Option<SeekFrame>,
        expiry: Option<(u32, u64)>,
    ) {
        let target = seek
            .map(|s| {
                (
                    s.time,
                    s.frame as u128 * u128::from(self.format.output_rate),
                )
            })
            .or_else(|| {
                let current = self.read(None).expect("Sole control-plane writer");
                if current.region == region {
                    return None;
                }
                let phase = self.current_units();
                let mapped = wrap_units(phase, region, self.format);
                (mapped != phase).then(|| (self.time_at(mapped), mapped))
            });
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.rate.store(rate, Ordering::SeqCst);
        self.neutral_rate
            .store(expiry.map_or(rate, |e| e.0), Ordering::SeqCst);
        self.sync_until
            .store(expiry.map_or(0, |e| e.1), Ordering::SeqCst);
        self.paused.store(!playing, Ordering::SeqCst);
        self.start
            .store(region.map_or(0, |r| r.start), Ordering::SeqCst);
        self.end
            .store(region.map_or(0, |r| r.end), Ordering::SeqCst);
        if let Some((time, units)) = target {
            self.seek_ns.store(time.as_nanos() as u64, Ordering::SeqCst);
            self.seek_units.store(units as u64, Ordering::SeqCst);
            self.seek_serial.fetch_add(1, Ordering::SeqCst);
        }
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    fn read(&self, previous: Option<u64>) -> Option<Settings> {
        let first = self.sequence.load(Ordering::SeqCst);
        if first & 1 != 0 || previous == Some(first) {
            return None;
        }
        let end = self.end.load(Ordering::SeqCst);
        let settings = Settings {
            sequence: first,
            rate: self.rate.load(Ordering::SeqCst),
            neutral_rate: self.neutral_rate.load(Ordering::SeqCst),
            sync_until: self.sync_until.load(Ordering::SeqCst),
            paused: self.paused.load(Ordering::SeqCst),
            region: (end != 0).then(|| FrameLoop {
                start: self.start.load(Ordering::SeqCst),
                end,
            }),
            seek_serial: self.seek_serial.load(Ordering::SeqCst),
            seek_ns: self.seek_ns.load(Ordering::SeqCst),
            seek_units: self.seek_units.load(Ordering::SeqCst),
        };
        // Do not spin on the real-time path: an overlapping command is retried next frame.
        (self.sequence.load(Ordering::SeqCst) == first).then_some(settings)
    }
    /// One bounded read: overlapping render publication or a pending command returns None.
    pub fn rendered_stamp(&self) -> Option<RenderStamp> {
        let first = self.stamp_sequence.load(Ordering::SeqCst);
        if first == 0 || first & 1 != 0 || self.stopped.load(Ordering::Acquire) {
            return None;
        }
        let stamp = RenderStamp {
            phase_units: self.stamp_phase.load(Ordering::SeqCst),
            output_frame: self.stamp_frame.load(Ordering::SeqCst),
        };
        let command = self.stamp_command.load(Ordering::SeqCst);
        (self.stamp_sequence.load(Ordering::SeqCst) == first
            && self.sequence.load(Ordering::SeqCst) == command)
            .then_some(stamp)
    }
    pub fn stamped_position(&self, stamp: RenderStamp) -> Duration {
        self.time_at(u128::from(stamp.phase_units))
    }
    fn current_units(&self) -> u128 {
        let requested = self.seek_serial.load(Ordering::Acquire);
        let phase = if self.applied_seek.load(Ordering::Acquire) != requested {
            u128::from(self.seek_units.load(Ordering::Relaxed))
        } else {
            u128::from(self.phase_units.load(Ordering::Relaxed))
        };
        let region = self.read(None).and_then(|s| s.region);
        wrap_units(phase, region, self.format)
    }
    pub fn sync_expired(&self) -> bool {
        self.sync_until.load(Ordering::SeqCst) != 0
            && self.expired_command.load(Ordering::SeqCst) == self.sequence.load(Ordering::SeqCst)
    }
    pub fn command_version(&self) -> u64 {
        self.sequence.load(Ordering::SeqCst)
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
    pub fn frame(&self) -> usize {
        (self.current_units() / u128::from(self.format.output_rate)) as usize
    }
    fn time_at(&self, units: u128) -> Duration {
        let ns = (units * 1_000_000_000)
            .div_ceil(u128::from(self.format.source_rate) * u128::from(self.format.output_rate));
        Duration::from_nanos((ns as u64).min(self.format.duration_ns()))
    }
    pub fn position(&self) -> Duration {
        if let Some(settings) = self.read(None) {
            if self.applied_seek.load(Ordering::Acquire) != settings.seek_serial {
                return Duration::from_nanos(settings.seek_ns);
            }
        }
        self.time_at(self.current_units())
    }
}

fn wrap_units(phase: u128, region: Option<FrameLoop>, format: RenderFormat) -> u128 {
    if let Some(region) = region {
        let start = region.start as u128 * u128::from(format.output_rate);
        let length = (region.end - region.start) as u128 * u128::from(format.output_rate);
        if phase < start {
            start
        } else {
            start + (phase - start) % length
        }
    } else {
        phase.min(format.frames as u128 * u128::from(format.output_rate))
    }
}
pub(crate) struct DeckRenderer {
    samples: PcmSource,
    control: Arc<RenderControl>,
    settings: Settings,
    output_channels: u16,
    output_rate: u32,
    channel: u16,
    // Source-frame position * output_rate. Integer accumulation avoids repeated-rate drift.
    phase: u128,
    applied_seek: u64,
    clock: Option<Arc<crate::render_bus::RenderClock>>,
}
impl DeckRenderer {
    pub fn new(samples: PcmSource, control: Arc<RenderControl>, channels: u16, rate: u32) -> Self {
        assert!(channels > 0 && rate > 0);
        let settings = control
            .read(None)
            .expect("Renderer is constructed by the sole writer");
        assert_eq!(control.format.source_rate, samples.sample_rate());
        assert_eq!(control.format.output_rate, rate);
        assert_eq!(control.format.frames, samples.frames());
        Self {
            samples,
            control,
            settings,
            output_channels: channels,
            output_rate: rate,
            channel: 0,
            phase: 0,
            applied_seek: 0,
            clock: None,
        }
    }
    pub fn with_clock(mut self, clock: Arc<crate::render_bus::RenderClock>) -> Self {
        self.clock = Some(clock);
        self
    }
    fn publish_stamp(&self) {
        let Some(clock) = &self.clock else {
            return;
        };
        // The bus increments completed only after this full frame returns from its inner mixer.
        let frame = clock.completed.load(Ordering::Acquire).wrapping_add(1);
        self.control.stamp_sequence.fetch_add(1, Ordering::SeqCst);
        self.control
            .stamp_phase
            .store(self.phase as u64, Ordering::SeqCst);
        self.control.stamp_frame.store(frame, Ordering::SeqCst);
        self.control
            .stamp_command
            .store(self.settings.sequence, Ordering::SeqCst);
        self.control.stamp_sequence.fetch_add(1, Ordering::SeqCst);
    }
    fn wrap(&self, phase: u128) -> u128 {
        wrap_units(phase, self.settings.region, self.control.format)
    }
    fn publish_position(&self) {
        self.control
            .phase_units
            .store(self.phase as u64, Ordering::Relaxed);
        self.control
            .applied_seek
            .store(self.applied_seek, Ordering::Release);
    }
    fn prepare_frame(&mut self) {
        let prior_seek = self.applied_seek;
        let prior_phase = self.phase;
        if let Some(settings) = self.control.read(Some(self.settings.sequence)) {
            self.settings = settings;
        }
        let settings = self.settings;
        if self.applied_seek != settings.seek_serial {
            self.phase = u128::from(settings.seek_units);
            self.applied_seek = settings.seek_serial;
        }
        self.phase = self.wrap(self.phase);
        if self.phase != prior_phase || self.applied_seek != prior_seek {
            self.publish_position();
        }
    }
}
impl Iterator for DeckRenderer {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 {
            self.prepare_frame();
        }
        let expired = self.settings.sync_until != 0
            && self.clock.as_ref().is_some_and(|clock| {
                clock.completed.load(Ordering::Acquire) >= self.settings.sync_until
            });
        let applied_rate = if expired {
            self.control
                .expired_command
                .store(self.settings.sequence, Ordering::SeqCst);
            self.settings.neutral_rate
        } else {
            self.settings.rate
        };
        let end = self.samples.frames() as u128 * u128::from(self.output_rate);
        let finished = self.settings.region.is_none() && self.phase >= end;
        let value = if self.settings.paused || finished {
            0.
        } else {
            // Match Rodio's channel mapping: discard surplus inputs, duplicate mono to stereo,
            // and zero-fill other additional outputs. Device/headphone routing is separate.
            let input_channel = if self.channel < self.samples.channels() {
                Some(self.channel)
            } else if self.channel == 1 && self.samples.channels() == 1 {
                Some(0)
            } else {
                None
            };
            input_channel.map_or(0., |channel| {
                let frame = (self.phase / u128::from(self.output_rate)) as usize;
                let next =
                    self.settings
                        .region
                        .map_or((frame + 1).min(self.samples.frames() - 1), |r| {
                            if frame + 1 >= r.end {
                                r.start
                            } else {
                                frame + 1
                            }
                        });
                let fraction = (self.phase % u128::from(self.output_rate)) as f64
                    / f64::from(self.output_rate);
                let a = f64::from(self.samples.sample_at(frame, channel));
                let b = f64::from(self.samples.sample_at(next, channel));
                (a + (b - a) * fraction) as f32
            })
        };
        self.channel += 1;
        if self.channel == self.output_channels {
            self.channel = 0;
            if !self.settings.paused && !finished {
                self.phase = self.wrap(self.phase + u128::from(applied_rate));
                self.publish_position();
            }
            self.publish_stamp();
        }
        // Keep EOF silent but installed: explicit seek/restart cannot race queue exhaustion.
        Some(value)
    }
}
impl Source for DeckRenderer {
    fn channels(&self) -> u16 {
        self.output_channels
    }
    fn sample_rate(&self) -> u32 {
        self.output_rate
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
    fn stamped_renderer_publication_adds_no_allocations_for_complete_stereo_frames() {
        let pcm = PcmSource::new(2, 44100, vec![0.125; 400000]);
        let control = RenderControl::new(
            44100,
            true,
            None,
            SeekFrame {
                time: Duration::ZERO,
                frame: 0,
            },
            RenderFormat {
                source_rate: 44100,
                output_rate: 48000,
                frames: 200000,
            },
        );
        let clock = Arc::new(crate::render_bus::RenderClock::default());
        let mut renderer =
            DeckRenderer::new(pcm, Arc::clone(&control), 2, 48000).with_clock(Arc::clone(&clock));
        let allocations = crate::looping::counted_allocations(|| {
            for frame in 0..50000 {
                assert_eq!(renderer.next(), Some(0.125));
                assert_eq!(renderer.next(), Some(0.125));
                clock.completed.store(frame + 1, Ordering::Release);
            }
        });
        assert_eq!(allocations, 0);
        let stamp = control.rendered_stamp().unwrap();
        assert_eq!(stamp.output_frame, 50000);
        assert_eq!(stamp.phase_units, 50000 * 44100);
        control.stamp_sequence.fetch_add(1, Ordering::SeqCst);
        assert!(
            control.rendered_stamp().is_none(),
            "in-progress publication must return without waiting"
        );
        control.stamp_sequence.fetch_add(1, Ordering::SeqCst);
    }
    #[test]
    fn explicit_frame_seeks_keep_canonical_nonintegral_nanosecond_boundaries() {
        for rate in [44100, 48000, 96000] {
            let pcm = PcmSource::new(1, rate, (0..100).map(|i| i as f32).collect());
            let format = RenderFormat {
                source_rate: rate,
                output_rate: rate,
                frames: 100,
            };
            let target = SeekFrame {
                time: crate::looping::frame_time(1, rate),
                frame: 1,
            };
            let control = RenderControl::new(rate, true, None, target, format);
            let mut renderer = DeckRenderer::new(pcm, Arc::clone(&control), 1, rate);
            assert_eq!(
                renderer.next(),
                Some(1.),
                "canonical frame1 must not become fractional"
            );
            assert_eq!(control.frame(), 2);
            assert_eq!(crate::looping::frame_at(control.position(), rate), 2);
            assert_eq!(control.position(), crate::looping::frame_time(2, rate));
            control.publish(
                rate,
                true,
                None,
                Some(SeekFrame {
                    time: crate::looping::frame_time(7, rate),
                    frame: 7,
                }),
            );
            assert_eq!(renderer.next(), Some(7.));
            assert_eq!(control.frame(), 8);
            assert_eq!(control.position(), crate::looping::frame_time(8, rate));
        }
    }
    #[test]
    fn frame_commands_stereo_pause_and_busy_writer_are_bounded_and_allocate_nothing() {
        let samples: Vec<_> = (0..10000).flat_map(|i| [i as f32, -(i as f32)]).collect();
        let pcm = PcmSource::new(2, 48000, samples);
        let control = RenderControl::new(
            48000,
            true,
            None,
            SeekFrame {
                time: Duration::ZERO,
                frame: 0,
            },
            RenderFormat {
                source_rate: 48000,
                output_rate: 48000,
                frames: 10000,
            },
        );
        let mut render = DeckRenderer::new(pcm, Arc::clone(&control), 2, 48000);
        assert_eq!(render.next(), Some(0.));
        assert_eq!(render.next(), Some(0.));
        assert_eq!(render.next(), Some(1.));
        control.publish(96000, true, None, None);
        assert_eq!(render.next(), Some(-1.));
        assert_eq!(render.next(), Some(2.));
        assert_eq!(render.next(), Some(-2.));
        assert_eq!(render.next(), Some(4.));
        assert_eq!(render.next(), Some(-4.));
        control.publish(48000, false, None, None);
        let position = control.position();
        for _ in 0..1000 {
            assert_eq!(render.next(), Some(0.));
        }
        assert_eq!(control.position(), position);
        control.publish(
            48000,
            true,
            Some(FrameLoop { start: 2, end: 8 }),
            Some(SeekFrame {
                time: Duration::from_micros(100),
                frame: 4,
            }),
        );
        // Emulate an in-progress writer. Audio continues using its last coherent state, not a spin.
        control.sequence.fetch_add(1, Ordering::SeqCst);
        let allocations = crate::looping::counted_allocations(|| {
            for _ in 0..1000 {
                assert!(render.next().is_some());
            }
        });
        assert_eq!(allocations, 0);
        control.sequence.fetch_add(1, Ordering::SeqCst);
        assert!(render.next().unwrap() > 0.);
        assert!(control.position() < Duration::from_millis(1));
        let allocations = crate::looping::counted_allocations(|| {
            for _ in 0..100000 {
                assert!(render.next().is_some());
            }
        });
        assert_eq!(allocations, 0);
    }
    #[test]
    fn concurrent_command_snapshots_never_mix_payload_fields() {
        let a = FrameLoop { start: 2, end: 10 };
        let b = FrameLoop { start: 20, end: 40 };
        let control = RenderControl::new(
            44100,
            true,
            Some(a),
            SeekFrame {
                time: Duration::from_nanos(100),
                frame: 2,
            },
            RenderFormat {
                source_rate: 44100,
                output_rate: 48000,
                frames: 100,
            },
        );
        let writer = Arc::clone(&control);
        let task = std::thread::spawn(move || {
            for _ in 0..10000 {
                writer.publish(
                    48000,
                    false,
                    Some(b),
                    Some(SeekFrame {
                        time: Duration::from_nanos(200),
                        frame: 20,
                    }),
                );
                writer.publish(
                    44100,
                    true,
                    Some(a),
                    Some(SeekFrame {
                        time: Duration::from_nanos(100),
                        frame: 2,
                    }),
                );
            }
        });
        let mut coherent = 0;
        for _ in 0..100000 {
            if let Some(s) = control.read(None) {
                if s.rate == 44100 {
                    assert!(!s.paused);
                    assert_eq!(s.region, Some(a));
                    assert_eq!(s.seek_ns, 100);
                    assert_eq!(s.seek_units, 2 * 48000);
                } else {
                    assert_eq!(s.rate, 48000);
                    assert!(s.paused);
                    assert_eq!(s.region, Some(b));
                    assert_eq!(s.seek_ns, 200);
                    assert_eq!(s.seek_units, 20 * 48000);
                }
                coherent += 1;
            }
        }
        task.join().unwrap();
        assert!(coherent > 0);
    }
}
