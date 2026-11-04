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
    censor: bool,
    censor_serial: u64,
    sequence: u64,
    rate: u32,
    neutral_rate: u32,
    sync_until: u64,
    paused: bool,
    region: Option<FrameLoop>,
    seek_serial: u64,
    seek_ns: u64,
    seek_units: u64,
    trigger_serial: u64,
    trigger_at: u64,
    trigger_units: u64,
    scratch: bool,
    scratch_serial: u64,
    scratch_target: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RenderStamp {
    pub phase_units: u64,
    pub output_frame: u64,
}
pub(crate) struct RenderControl {
    censor: AtomicBool,
    censor_serial: AtomicU64,
    scratch: AtomicBool,
    scratch_serial: AtomicU64,
    scratch_target: AtomicU64,
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
    trigger_serial: AtomicU64,
    completed_trigger: AtomicU64,
    trigger_at: AtomicU64,
    trigger_units: AtomicU64,
    phase_units: AtomicU64,
    stamp_sequence: AtomicU64,
    stamp_phase: AtomicU64,
    stamp_frame: AtomicU64,
    stamp_command: AtomicU64,
    pub stopped: AtomicBool,
    last_sample: Arc<[AtomicU32; 2]>,
}
impl RenderControl {
    #[cfg(test)]
    pub fn new(
        rate: u32,
        playing: bool,
        region: Option<FrameLoop>,
        seek: SeekFrame,
        format: RenderFormat,
    ) -> Arc<Self> {
        Self::new_with_history(rate, playing, region, seek, format, None)
    }
    pub fn new_with_history(
        rate: u32,
        playing: bool,
        region: Option<FrameLoop>,
        seek: SeekFrame,
        format: RenderFormat,
        previous: Option<&Self>,
    ) -> Arc<Self> {
        assert!(format.source_rate > 0 && format.output_rate > 0);
        assert!(format.frames as u128 * u128::from(format.output_rate) <= u128::from(u64::MAX));
        Arc::new(Self {
            censor: AtomicBool::new(false),
            censor_serial: AtomicU64::new(0),
            scratch: AtomicBool::new(false),
            scratch_serial: AtomicU64::new(0),
            scratch_target: AtomicU64::new(0),
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
            trigger_serial: AtomicU64::new(0),
            completed_trigger: AtomicU64::new(0),
            trigger_at: AtomicU64::new(0),
            trigger_units: AtomicU64::new(0),
            phase_units: AtomicU64::new(0),
            stamp_sequence: AtomicU64::new(0),
            stamp_phase: AtomicU64::new(0),
            stamp_frame: AtomicU64::new(0),
            stamp_command: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            last_sample: previous.map_or_else(
                || Arc::new([const { AtomicU32::new(0) }; 2]),
                |old| Arc::clone(&old.last_sample),
            ),
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
    pub fn cancel_trigger(&self) {
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.trigger_at.store(0, Ordering::SeqCst);
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    /// Control-plane query. A consumed trigger is acknowledged once per execution;
    /// cancellation leaves no pending trigger even before the renderer observes it.
    pub fn trigger_pending(&self) -> bool {
        self.trigger_at.load(Ordering::SeqCst) != 0
            && self.completed_trigger.load(Ordering::Acquire)
                != self.trigger_serial.load(Ordering::SeqCst)
    }
    /// Single control-plane writer; signed source-time motion, no audio-thread allocation.
    pub fn scratch_touch(&self, held: bool) {
        let target = if !held && self.scratching() {
            self.scratch_target.load(Ordering::SeqCst)
        } else {
            self.current_units() as u64
        };
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.scratch.store(held, Ordering::SeqCst);
        self.scratch_target.store(target, Ordering::SeqCst);
        self.scratch_serial.fetch_add(1, Ordering::SeqCst);
        self.trigger_at.store(0, Ordering::SeqCst);
        if !held {
            self.seek_units.store(target, Ordering::SeqCst);
            self.seek_ns.store(
                self.time_at(u128::from(target)).as_nanos() as u64,
                Ordering::SeqCst,
            );
            self.seek_serial.fetch_add(1, Ordering::SeqCst);
        }
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    /// Accumulate relative source time; clamp to the loaded PCM, not loop bounds.
    pub fn scratch_move(&self, source_seconds: f64) {
        let target = self.scratch_target.load(Ordering::SeqCst) as f64
            + source_seconds
                * f64::from(self.format.source_rate)
                * f64::from(self.format.output_rate);
        let end = self.format.frames as u64 * u64::from(self.format.output_rate);
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.scratch_target
            .store(target.clamp(0., end as f64) as u64, Ordering::SeqCst);
        self.scratch_serial.fetch_add(1, Ordering::SeqCst);
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    /// Control-plane touch state, independent of the transport pause flag.
    pub fn scratching(&self) -> bool {
        self.scratch.load(Ordering::SeqCst)
    }
    /// Momentary reverse overlay; the published source timeline keeps moving forward.
    pub fn censor(&self, held: bool) {
        if self.censoring() == held {
            return;
        }
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.censor.store(held, Ordering::SeqCst);
        if held {
            self.censor_serial.fetch_add(1, Ordering::SeqCst);
        }
        self.trigger_at.store(0, Ordering::SeqCst);
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    pub fn censoring(&self) -> bool {
        self.censor.load(Ordering::SeqCst)
    }
    pub fn schedule_trigger(&self, at_frame: usize, target_frame: usize) {
        self.sequence.fetch_add(1, Ordering::SeqCst);
        self.trigger_at.store(
            at_frame as u64 * u64::from(self.format.output_rate),
            Ordering::SeqCst,
        );
        self.trigger_units.store(
            target_frame as u64 * u64::from(self.format.output_rate),
            Ordering::SeqCst,
        );
        self.trigger_serial.fetch_add(1, Ordering::SeqCst);
        self.sequence.fetch_add(1, Ordering::SeqCst);
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
        self.scratch.store(false, Ordering::SeqCst);
        self.censor.store(false, Ordering::SeqCst);
        self.trigger_at.store(0, Ordering::SeqCst);
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
            censor: self.censor.load(Ordering::SeqCst),
            censor_serial: self.censor_serial.load(Ordering::SeqCst),
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
            trigger_serial: self.trigger_serial.load(Ordering::SeqCst),
            trigger_at: self.trigger_at.load(Ordering::SeqCst),
            trigger_units: self.trigger_units.load(Ordering::SeqCst),
            scratch: self.scratch.load(Ordering::SeqCst),
            scratch_serial: self.scratch_serial.load(Ordering::SeqCst),
            scratch_target: self.scratch_target.load(Ordering::SeqCst),
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
        let region = self
            .read(None)
            .and_then(|s| if s.scratch { None } else { s.region });
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
struct Handoff {
    previous: Arc<RenderControl>,
    offsets: [f32; 2],
    elapsed: u32,
    frames: u32,
    seek_serial: u64,
}
pub(crate) struct DeckRenderer {
    censor_serial: u64,
    // Audible reverse head is private to the render thread. `phase` stays authoritative.
    reverse_phase: u128,
    reverse_exhausted: bool,
    scratch_serial: u64,
    scratch_remaining: u32,
    handoff: Option<Handoff>,
    keylock: Option<Arc<drdj_keylock::PreparedKeylock>>,
    keylock_rate: u32,
    micro: drdj_keylock::micro::MicroStretch,
    keylock_frame: [f32; 2],
    samples: PcmSource,
    control: Arc<RenderControl>,
    settings: Settings,
    output_channels: u16,
    output_rate: u32,
    channel: u16,
    // Source-frame position * output_rate. Integer accumulation avoids repeated-rate drift.
    phase: u128,
    applied_seek: u64,
    applied_trigger: u64,
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
            censor_serial: 0,
            reverse_phase: 0,
            reverse_exhausted: false,
            scratch_serial: 0,
            scratch_remaining: 0,
            handoff: None,
            keylock: None,
            keylock_rate: rate,
            micro: drdj_keylock::micro::MicroStretch::new(rate),
            keylock_frame: [0.; 2],
            samples,
            control,
            settings,
            output_channels: channels,
            output_rate: rate,
            channel: 0,
            phase: 0,
            applied_seek: 0,
            applied_trigger: 0,
            clock: None,
        }
    }
    /// Five-ms dezipper: retain the small old control, never its decoded PCM.
    pub fn with_handoff(mut self, previous: Option<Arc<RenderControl>>) -> Self {
        self.handoff = previous.map(|previous| Handoff {
            previous,
            offsets: [0.; 2],
            elapsed: 0,
            frames: (self.output_rate / 200).max(2),
            seek_serial: self.settings.seek_serial,
        });
        self
    }
    pub fn with_keylock(mut self, clip: Option<(Arc<drdj_keylock::PreparedKeylock>, u32)>) -> Self {
        if let Some((clip, rate)) = clip {
            self.keylock = Some(clip);
            self.keylock_rate = rate;
        }
        self
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
        let prior_censor = self.settings.censor;
        if let Some(settings) = self.control.read(Some(self.settings.sequence)) {
            self.settings = settings;
        }
        let settings = self.settings;
        if settings.scratch_serial != self.scratch_serial {
            self.scratch_serial = settings.scratch_serial;
            self.scratch_remaining = (self.output_rate / 30).max(1);
        }
        if self.applied_seek != settings.seek_serial {
            if let Some(handoff) = &mut self.handoff {
                if settings.seek_serial != handoff.seek_serial {
                    handoff.elapsed = handoff.frames;
                }
            }
            self.phase = u128::from(settings.seek_units);
            self.applied_seek = settings.seek_serial;
        }
        if !settings.paused
            && !settings.scratch
            && settings.trigger_at != 0
            && self.applied_trigger != settings.trigger_serial
            && self.phase >= u128::from(settings.trigger_at)
        {
            self.phase = u128::from(settings.trigger_units);
            self.applied_trigger = settings.trigger_serial;
            self.publish_position();
            self.control
                .completed_trigger
                .store(settings.trigger_serial, Ordering::Release);
        }
        if !settings.scratch {
            self.phase = self.wrap(self.phase);
        }
        if settings.censor != prior_censor || settings.censor_serial != self.censor_serial {
            self.censor_serial = settings.censor_serial;
            self.reverse_phase = self.phase;
            self.reverse_exhausted = false;
            self.micro.reset();
        }
        if self.phase != prior_phase || self.applied_seek != prior_seek {
            self.micro.reset();
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
        let scratch = self.settings.scratch;
        let censor = self.settings.censor && !scratch;
        let audible_phase = if censor {
            self.reverse_phase
        } else {
            self.phase
        };
        let scratch_moving = scratch
            && self.scratch_remaining > 0
            && self.phase != u128::from(self.settings.scratch_target);
        let paused = if scratch {
            !scratch_moving
        } else {
            self.settings.paused
        };
        let finished = (scratch || self.settings.region.is_none()) && self.phase >= end;
        if self.channel == 0 && !paused && !finished && !scratch && !censor {
            if let Some(clip) = &self.keylock {
                let region = self.settings.region;
                self.keylock_frame = self.micro.frame(
                    self.phase as f64 / f64::from(self.output_rate),
                    f64::from(self.keylock_rate) / f64::from(self.output_rate),
                    applied_rate != self.keylock_rate,
                    self.samples.channels(),
                    |frame, channel| {
                        let frame = region.map_or(frame, |r| {
                            r.start as f64
                                + (frame - r.start as f64).rem_euclid((r.end - r.start) as f64)
                        });
                        clip.sample_at_source(frame, channel)
                    },
                );
            }
        }
        let value = if paused || finished || (censor && self.reverse_exhausted) {
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
                if let Some(clip) = self.keylock.as_ref().filter(|_| !scratch) {
                    if censor {
                        return clip.sample_at_source(
                            audible_phase as f64 / f64::from(self.output_rate),
                            channel,
                        );
                    }
                    return self.keylock_frame[usize::from(channel)];
                }
                let frame = (audible_phase / u128::from(self.output_rate)) as usize;
                let next = self.settings.region.filter(|_| !scratch).map_or(
                    (frame + 1).min(self.samples.frames() - 1),
                    |r| {
                        if frame + 1 >= r.end {
                            r.start
                        } else {
                            frame + 1
                        }
                    },
                );
                let fraction = (audible_phase % u128::from(self.output_rate)) as f64
                    / f64::from(self.output_rate);
                let a = f64::from(self.samples.sample_at(frame, channel));
                let b = f64::from(self.samples.sample_at(next, channel));
                (a + (b - a) * fraction) as f32
            })
        };
        let mut value = value;
        if let Some(handoff) = &mut self.handoff {
            if handoff.elapsed < handoff.frames && !paused && !finished && !scratch {
                if let Some(offset) = handoff.offsets.get_mut(usize::from(self.channel)) {
                    if handoff.elapsed == 0 {
                        let previous = f32::from_bits(
                            handoff.previous.last_sample[usize::from(self.channel)]
                                .load(Ordering::Relaxed),
                        );
                        *offset = previous - value;
                    }
                    let weight = 1. - handoff.elapsed as f32 / (handoff.frames - 1) as f32;
                    value += *offset * weight;
                }
            } else if paused || finished || scratch {
                // Silence is authoritative; never inject an old signal into a paused/EOF deck.
                handoff.elapsed = handoff.frames;
            }
        }
        if let Some(last) = self.control.last_sample.get(usize::from(self.channel)) {
            last.store(value.to_bits(), Ordering::Relaxed);
        }
        self.channel += 1;
        if self.channel == self.output_channels {
            if let Some(handoff) = &mut self.handoff {
                handoff.elapsed = handoff.elapsed.saturating_add(1).min(handoff.frames);
            }
            self.channel = 0;
            if scratch {
                if self.scratch_remaining > 0 {
                    let target = u128::from(self.settings.scratch_target);
                    let remaining = u128::from(self.scratch_remaining);
                    if target >= self.phase {
                        self.phase += (target - self.phase).div_ceil(remaining);
                    } else {
                        self.phase -= (self.phase - target).div_ceil(remaining);
                    }
                    self.scratch_remaining -= 1;
                    self.publish_position();
                }
            } else if !self.settings.paused && !finished {
                if censor {
                    let step = u128::from(applied_rate);
                    if let Some(region) = self.settings.region {
                        let start = region.start as u128 * u128::from(self.output_rate);
                        let length =
                            (region.end - region.start) as u128 * u128::from(self.output_rate);
                        self.reverse_phase =
                            start + (self.reverse_phase - start + length - step % length) % length;
                    } else if self.reverse_phase >= step {
                        self.reverse_phase -= step;
                    } else {
                        self.reverse_phase = 0;
                        self.reverse_exhausted = true;
                    }
                }
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
    fn censor_renderer(
        rate: u32,
        step: u32,
        region: Option<FrameLoop>,
        start: usize,
    ) -> (Arc<RenderControl>, DeckRenderer) {
        let pcm = PcmSource::new(
            2,
            rate,
            (0..12000)
                .flat_map(|i| {
                    let v = i as f32 / 12000.;
                    [v, -v]
                })
                .collect(),
        );
        let control = RenderControl::new(
            step,
            true,
            region,
            SeekFrame {
                time: Duration::from_secs_f64(start as f64 / f64::from(rate)),
                frame: start,
            },
            RenderFormat {
                source_rate: rate,
                output_rate: 4800,
                frames: 12000,
            },
        );
        let renderer = DeckRenderer::new(pcm, control.clone(), 2, 4800);
        (control, renderer)
    }
    #[test]
    fn censor_exact_reverse_forward_timeline_and_frame_coherent_release() {
        for step in [2400, 3000, 3750] {
            let (control, mut renderer) = censor_renderer(3000, step, None, 4000);
            renderer.by_ref().take(2).for_each(drop);
            let start = control.current_units();
            control.censor(true);
            control.censor(true); // repeated NoteOn must not restart the reverse head
            for i in 0..64u128 {
                let expected = (start - u128::from(step) * i) as f64 / 4800. / 12000.;
                let a = renderer.next().unwrap();
                if i == 63 {
                    control.censor(false);
                } // mid-frame release keeps L/R coherent
                let b = renderer.next().unwrap();
                assert!((f64::from(a) - expected).abs() < 1e-7);
                assert_eq!(a, -b);
                assert_eq!(control.current_units(), start + u128::from(step) * (i + 1));
            }
            let resumed = control.current_units();
            let a = renderer.next().unwrap();
            renderer.next();
            assert!((f64::from(a) - resumed as f64 / 4800. / 12000.).abs() < 1e-7);
            control.censor(true);
            renderer.by_ref().take(10).for_each(drop);
            control.censor(false);
            control.censor(true); // new gesture between frames must start at the current timeline
            let restart = control.current_units();
            assert!(
                (f64::from(renderer.next().unwrap()) - restart as f64 / 4800. / 12000.).abs()
                    < 1e-7
            );
        }
    }
    #[test]
    fn censor_loop_boundaries_pause_seek_and_eof_are_authoritative() {
        let region = FrameLoop {
            start: 2000,
            end: 2010,
        };
        let (control, mut renderer) = censor_renderer(3000, 3750, Some(region), 2003);
        renderer.by_ref().take(2).for_each(drop);
        let start = control.current_units();
        control.censor(true);
        let base = 2000 * 4800u128;
        let length = 10 * 4800u128;
        for i in 0..80u128 {
            let reverse = base + (start - base + length - (i * 3750) % length) % length;
            let frame = reverse / 4800;
            let next = if frame == 2009 { 2000 } else { frame + 1 };
            let expected = (frame as f64
                + (next as f64 - frame as f64) * (reverse % 4800) as f64 / 4800.)
                / 12000.;
            assert!((f64::from(renderer.next().unwrap()) - expected).abs() < 1e-7);
            renderer.next();
            assert_eq!(
                control.current_units(),
                base + (start - base + (i + 1) * 3750) % length
            );
        }
        control.publish(3750, false, Some(region), None);
        let paused = control.current_units();
        assert!(!control.censoring());
        assert!(renderer.by_ref().take(40).all(|v| v == 0.));
        control.censor(false);
        assert_eq!(control.current_units(), paused);
        control.publish(
            4800,
            true,
            None,
            Some(SeekFrame {
                time: Duration::from_secs(1),
                frame: 3000,
            }),
        );
        renderer.by_ref().take(2).for_each(drop);
        assert_eq!(control.frame(), 3001);

        for (start, frames) in [(11995, 5), (1, 12)] {
            let (control, mut renderer) = censor_renderer(3000, 4800, None, start);
            control.censor(true);
            for _ in 0..frames {
                renderer.next();
                renderer.next();
            }
            if start == 11995 {
                assert_eq!(control.frame(), 12000);
            }
            assert!(renderer.by_ref().take(16).all(|v| v == 0.));
            control.censor(false);
            let normal = control.frame();
            assert_eq!(
                renderer.next().unwrap(),
                if normal == 12000 {
                    0.
                } else {
                    normal as f32 / 12000.
                }
            );
        }
    }
    #[test]
    fn censor_realtime_heap_and_optimized_deadline() {
        let pcm = PcmSource::new(2, 48000, vec![0.25; 48000 * 8]);
        let control = RenderControl::new(
            48000,
            true,
            None,
            SeekFrame {
                time: Duration::from_secs(2),
                frame: 96000,
            },
            RenderFormat {
                source_rate: 48000,
                output_rate: 48000,
                frames: 48000 * 4,
            },
        );
        let mut renderer = DeckRenderer::new(pcm, control.clone(), 2, 48000);
        renderer.by_ref().take(2).for_each(drop);
        control.censor(true);
        let start = std::time::Instant::now();
        let heap = crate::looping::measured_heap(|| {
            for i in 0..9600 {
                if i == 4000 {
                    control.censor(false);
                }
                if i == 6000 {
                    control.censor(true);
                }
                assert_eq!(renderer.next(), Some(0.25));
            }
        });
        let elapsed = start.elapsed();
        assert_eq!(
            (heap.allocations, heap.reallocations, heap.deallocations),
            (0, 0, 0)
        );
        if !cfg!(debug_assertions) {
            assert!(elapsed < Duration::from_millis(100), "{elapsed:?}");
        }
        eprintln!("Censor 100ms stereo/48kHz offline block: {elapsed:?}; heap {heap:?}");
    }
    #[test]
    fn platter_reverse_forward_boundaries_and_idle_are_allocation_free() {
        let rate = 3000;
        let pcm = PcmSource::new(
            2,
            rate,
            (0..6000)
                .flat_map(|i| {
                    let value = i as f32 / 6000.;
                    [value, -value]
                })
                .collect(),
        );
        let control = RenderControl::new(
            rate,
            false,
            Some(FrameLoop {
                start: 2000,
                end: 4000,
            }),
            SeekFrame {
                time: Duration::from_secs(1),
                frame: 3000,
            },
            RenderFormat {
                source_rate: rate,
                output_rate: rate,
                frames: 6000,
            },
        );
        let mut renderer = DeckRenderer::new(pcm, control.clone(), 2, rate);
        control.scratch_touch(true);
        assert!(renderer.by_ref().take(10).all(|v| v == 0.));
        let mut maximum = Duration::ZERO;
        let allocations = crate::looping::counted_allocations(|| {
            for (seconds, expected, ascending) in [(0.05, 3150, true), (-0.1, 2850, false)] {
                control.scratch_move(seconds);
                let mut previous = if ascending { 0. } else { 1. };
                let start = std::time::Instant::now();
                for _ in 0..100 {
                    let a = renderer.next().unwrap();
                    let b = renderer.next().unwrap();
                    assert!((a + b).abs() < 1e-6);
                    assert!(if ascending {
                        a >= previous
                    } else {
                        a <= previous
                    });
                    previous = a;
                }
                maximum = maximum.max(start.elapsed());
                assert_eq!(control.frame(), expected);
                assert!(renderer.by_ref().take(200).all(|v| v == 0.));
            }
            control.scratch_move(-100.);
            for _ in 0..200 {
                renderer.next();
            }
            assert_eq!(control.frame(), 0);
            control.scratch_move(100.);
            for _ in 0..200 {
                renderer.next();
            }
            assert_eq!(control.frame(), 6000);
            control.scratch_move(-0.05);
            for _ in 0..200 {
                renderer.next();
            }
            assert_eq!(control.frame(), 5850);
        });
        assert_eq!(allocations, 0);
        assert!(
            maximum < Duration::from_millis(33),
            "scratch block missed its audio deadline: {maximum:?}"
        );
        eprintln!(
            "scratch 100 stereo frames at 3kHz: maximum {maximum:?}, allocations {allocations}"
        );
        control.scratch_touch(false);
        assert!(control.is_paused());
        assert!(renderer.by_ref().take(200).all(|v| v == 0.));
    }
    #[test]
    fn platter_same_tick_release_commits_motion_and_transport_cancels_touch() {
        let pcm = PcmSource::new(1, 3000, vec![0.25; 6000]);
        let control = RenderControl::new(
            3000,
            true,
            None,
            SeekFrame {
                time: Duration::from_secs(1),
                frame: 3000,
            },
            RenderFormat {
                source_rate: 3000,
                output_rate: 3000,
                frames: 6000,
            },
        );
        let mut renderer = DeckRenderer::new(pcm, control.clone(), 1, 3000);
        control.scratch_touch(true);
        control.scratch_move(0.05);
        control.scratch_move(-0.02);
        control.scratch_touch(false);
        assert_eq!(control.frame(), 3090);
        assert!(!control.is_paused());
        assert_eq!(renderer.next(), Some(0.25));
        assert_eq!(control.frame(), 3091);
        control.scratch_touch(true);
        control.publish(3000, false, None, None);
        assert!(!control.scratching());
        assert_eq!(renderer.next(), Some(0.));
    }
    #[test]
    fn platter_never_executes_scheduled_cues_during_motion() {
        let pcm = PcmSource::new(1, 3000, vec![0.25; 6000]);
        let control = RenderControl::new(
            3000,
            true,
            None,
            SeekFrame {
                time: Duration::from_secs(1),
                frame: 3000,
            },
            RenderFormat {
                source_rate: 3000,
                output_rate: 3000,
                frames: 6000,
            },
        );
        let mut renderer = DeckRenderer::new(pcm, control.clone(), 1, 3000);
        control.scratch_touch(true);
        control.scratch_move(0.05);
        control.schedule_trigger(3030, 4500);
        for _ in 0..100 {
            renderer.next();
        }
        assert_eq!(control.frame(), 3150);
        control.scratch_touch(false);
        renderer.next();
        assert_eq!(control.frame(), 3151);
    }
    #[test]
    fn platter_keylock_bypass_cross_rate_preserves_pitch_and_pause_settings() {
        for (rate, output_rate) in [(44100, 48000), (48000, 44100)] {
            let data: Vec<_> = (0..rate * 2)
                .flat_map(|i| {
                    let value = i as f32 / (rate * 2) as f32;
                    [value, -value]
                })
                .collect();
            let clip =
                Arc::new(drdj_keylock::PreparedKeylock::prepare(&data, 2, rate, 1.).unwrap());
            let pcm = PcmSource::new(2, rate, data);
            let control = RenderControl::new(
                (rate as f64 * 1.1) as u32,
                true,
                None,
                SeekFrame {
                    time: Duration::from_secs(1),
                    frame: rate as usize,
                },
                RenderFormat {
                    source_rate: rate,
                    output_rate,
                    frames: (rate * 2) as usize,
                },
            );
            let mut renderer = DeckRenderer::new(pcm, control.clone(), 2, output_rate)
                .with_keylock(Some((clip, rate)));
            control.scratch_touch(true);
            control.scratch_move(-0.05);
            let count = output_rate / 30;
            let allocations = crate::looping::counted_allocations(|| {
                let mut previous = 0.5;
                for _ in 0..count {
                    let left = renderer.next().unwrap();
                    let right = renderer.next().unwrap();
                    assert!((left + right).abs() < 1e-6 && left <= previous);
                    previous = left;
                }
                assert!(renderer.by_ref().take(200).all(|v| v == 0.));
                control.scratch_touch(false);
                assert!(renderer.by_ref().take(200).all(f32::is_finite));
            });
            assert_eq!(allocations, 0);
            assert!(renderer.keylock.is_some());
            assert!(!control.is_paused());
            assert_eq!(control.read(None).unwrap().rate, (rate as f64 * 1.1) as u32);
        }
    }
    #[test]
    fn keylock_micro_render_is_stereo_coherent_and_allocation_free() {
        for (rate, output_rate) in [(48000, 48000), (44100, 48000)] {
            let data: Vec<_> = (0..rate * 2)
                .flat_map(|i| {
                    let a = (std::f32::consts::TAU * 440. * i as f32 / rate as f32).sin() * 0.2;
                    [a, -a]
                })
                .collect();
            let clip =
                Arc::new(drdj_keylock::PreparedKeylock::prepare(&data, 2, rate, 1.).unwrap());
            let pcm = PcmSource::new(2, rate, data);
            let control = RenderControl::new(
                rate,
                true,
                None,
                SeekFrame {
                    time: Duration::ZERO,
                    frame: 0,
                },
                RenderFormat {
                    source_rate: rate,
                    output_rate,
                    frames: (rate * 2) as usize,
                },
            );
            let mut renderer = DeckRenderer::new(pcm, control.clone(), 2, output_rate)
                .with_keylock(Some((clip, rate)));
            let mut maximum = Duration::ZERO;
            let allocations = crate::looping::counted_allocations(|| {
                for block in 0..80 {
                    if block % 20 == 0 {
                        control.publish(
                            (f64::from(rate) * if block % 40 == 0 { 1.02 } else { 0.98 }).round()
                                as u32,
                            true,
                            None,
                            None,
                        );
                    }
                    let start = std::time::Instant::now();
                    for _ in 0..512 {
                        let a = renderer.next().unwrap();
                        let b = renderer.next().unwrap();
                        assert!(a.is_finite() && (a + b).abs() < 1e-6);
                    }
                    maximum = maximum.max(start.elapsed());
                }
            });
            assert_eq!(allocations, 0);
            println!("Keylock micro source={rate} output={output_rate}: 80 stereo blocks x512 frames; max block {maximum:?}; nominal budget {:.3}ms; allocations={allocations}", 512000. / f64::from(output_rate));
            control.publish(rate, false, None, None);
            assert!(renderer.by_ref().take(200).all(|v| v == 0.));
            control.publish(
                rate,
                true,
                None,
                Some(SeekFrame {
                    time: Duration::from_millis(500),
                    frame: (rate / 2) as usize,
                }),
            );
            assert!(renderer.by_ref().take(200).all(f32::is_finite));
        }
    }
    #[test]
    fn handoff_dezips_stereo_for_five_ms_and_respects_silence_and_seek() {
        fn renderer(values: [f32; 2], playing: bool) -> (Arc<RenderControl>, DeckRenderer) {
            let pcm = PcmSource::new(2, 2000, values.repeat(200));
            let control = RenderControl::new(
                2000,
                playing,
                None,
                SeekFrame {
                    time: Duration::ZERO,
                    frame: 0,
                },
                RenderFormat {
                    source_rate: 2000,
                    output_rate: 2000,
                    frames: 200,
                },
            );
            let render = DeckRenderer::new(pcm, control.clone(), 2, 2000);
            (control, render)
        }
        let (old, mut previous) = renderer([0.75, -0.75], true);
        assert_eq!(previous.next(), Some(0.75));
        assert_eq!(previous.next(), Some(-0.75));
        let (_, next) = renderer([-0.25, 0.25], true);
        let values: Vec<_> = next.with_handoff(Some(old.clone())).take(24).collect();
        for (frame, pair) in values.as_chunks::<2>().0.iter().enumerate() {
            let expected = 0.75 - (frame.min(9) as f32 / 9.);
            assert!((pair[0] - expected).abs() < 1e-6);
            assert!((pair[0] + pair[1]).abs() < 1e-6);
        }
        let (early_control, early) = renderer([-0.25, 0.25], true);
        let mut early = early.with_handoff(Some(old.clone()));
        early_control.publish(
            2000,
            true,
            None,
            Some(SeekFrame {
                time: Duration::from_millis(50),
                frame: 100,
            }),
        );
        assert_eq!(early.next(), Some(-0.25));
        assert_eq!(early.next(), Some(0.25));
        let (_, paused) = renderer([-0.25, 0.25], false);
        assert!(paused
            .with_handoff(Some(old.clone()))
            .take(24)
            .all(|s| s == 0.));
        let (control, next) = renderer([-0.25, 0.25], true);
        let mut next = next.with_handoff(Some(old));
        assert_eq!(next.next(), Some(0.75));
        assert_eq!(next.next(), Some(-0.75));
        control.publish(
            2000,
            true,
            None,
            Some(SeekFrame {
                time: Duration::from_millis(50),
                frame: 100,
            }),
        );
        assert_eq!(next.next(), Some(-0.25));
        assert_eq!(next.next(), Some(0.25));
    }
    #[test]
    fn scheduled_trigger_executes_once_at_frame_boundary_and_commands_cancel_it() {
        let pcm = PcmSource::new(2, 4, (0..16).map(|i| i as f32).collect());
        let control = RenderControl::new(
            4,
            true,
            None,
            SeekFrame {
                time: Duration::ZERO,
                frame: 0,
            },
            RenderFormat {
                source_rate: 4,
                output_rate: 4,
                frames: 8,
            },
        );
        let mut renderer = DeckRenderer::new(pcm, control.clone(), 2, 4);
        assert_eq!(renderer.next(), Some(0.0));
        control.schedule_trigger(2, 6);
        assert_eq!(renderer.next(), Some(1.0));
        assert_eq!(renderer.next(), Some(2.0));
        assert_eq!(renderer.next(), Some(3.0));
        assert_eq!(renderer.next(), Some(12.0));
        assert_eq!(renderer.next(), Some(13.0));
        assert_eq!(renderer.next(), Some(14.0));
        assert_eq!(renderer.next(), Some(15.0));
        control.publish(
            4,
            true,
            None,
            Some(SeekFrame {
                time: Duration::ZERO,
                frame: 0,
            }),
        );
        renderer.next();
        control.schedule_trigger(2, 6);
        control.publish(4, true, None, None);
        assert_eq!(renderer.next(), Some(1.0));
        assert_eq!(renderer.next(), Some(2.0));
        assert_eq!(renderer.next(), Some(3.0));
        assert_eq!(renderer.next(), Some(4.0));
    }

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
