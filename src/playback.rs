//! Control-plane loading and two Rodio decks. Rodio itself is not lock-free.
use crate::deck_render::{DeckRenderer, RenderControl, RenderFormat, SeekFrame};
use crate::looping::{frame_at, frame_time, FrameLoop, PcmSource};
use crate::playback_speed::PlaybackSpeed;
use rodio::{mixer::Mixer, Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    fs::File,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckId {
    A,
    B,
}
impl DeckId {
    fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MixerLevels {
    pub crossfader: f32,
    pub channel_a: f32,
    pub channel_b: f32,
}
impl Default for MixerLevels {
    fn default() -> Self {
        Self {
            crossfader: 0.5,
            channel_a: 1.0,
            channel_b: 1.0,
        }
    }
}
impl MixerLevels {
    /// Linear curve. Invalid control input is muted rather than passed to Rodio.
    pub fn volumes(self) -> (f32, f32) {
        fn unit(v: f32) -> f32 {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        let x = unit(self.crossfader);
        (unit(self.channel_a) * (1.0 - x), unit(self.channel_b) * x)
    }
}

#[derive(Clone)]
pub struct PreparedTrack {
    pub title: String,
    path: Arc<Path>,
    samples: PcmSource,
    overview: Arc<[f32]>,
    detail: Arc<crate::waveform::DetailPeaks>,
    tempo: Option<crate::tempo::TempoEstimate>,
}
impl PreparedTrack {
    /// Decode on a worker, never inside an audio callback. Limit retained PCM to 256 MiB/track.
    pub fn open(path: &Path) -> Result<Self, String> {
        Self::open_cancellable(path, &crate::cancellation::Cancellation::default())
    }
    pub(crate) fn open_cancellable(
        path: &Path,
        cancel: &crate::cancellation::Cancellation,
    ) -> Result<Self, String> {
        Self::open_cancellable_with_limit(path, 64 * 1024 * 1024, cancel)
    }
    #[cfg(test)]
    fn open_with_limit(path: &Path, limit: usize) -> Result<Self, String> {
        Self::open_cancellable_with_limit(
            path,
            limit,
            &crate::cancellation::Cancellation::default(),
        )
    }
    fn open_cancellable_with_limit(
        path: &Path,
        limit: usize,
        cancel: &crate::cancellation::Cancellation,
    ) -> Result<Self, String> {
        cancel.check()?;
        if !path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("mp3") || s.eq_ignore_ascii_case("wav"))
        {
            return Err("Select an MP3 or WAV file".into());
        }
        let canonical = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let decoder = Decoder::try_from(File::open(&canonical).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        if channels == 0 || rate == 0 {
            return Err("Invalid audio format".into());
        }
        let hint = decoder
            .total_duration()
            .and_then(|d| {
                d.as_nanos()
                    .checked_mul(u128::from(rate))?
                    .checked_mul(u128::from(channels))
            })
            .map_or(0, |n| (n / 1_000_000_000).min(limit as u128) as usize);
        let samples = collect_pcm(decoder, hint, limit, cancel)?;
        if samples.is_empty() || samples.len() % usize::from(channels) != 0 {
            return Err("No complete audio frames decoded".into());
        }
        cancel.check()?;
        let overview = crate::waveform::peak_overview(&samples, channels).into();
        cancel.check()?;
        let detail = Arc::new(crate::waveform::DetailPeaks::new(&samples, channels, rate));
        cancel.check()?;
        let tempo = crate::tempo::estimate(&samples, channels, rate);
        cancel.check()?;
        Ok(Self {
            path: canonical.into(),
            title: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            samples: PcmSource::new(channels, rate, samples),
            overview,
            detail,
            tempo,
        })
    }
    pub fn duration(&self) -> Duration {
        self.samples.total_duration().unwrap_or_default()
    }
}

fn collect_pcm(
    samples: impl Iterator<Item = f32>,
    hint: usize,
    limit: usize,
    cancel: &crate::cancellation::Cancellation,
) -> Result<Vec<f32>, String> {
    cancel.check()?;
    // Bound speculative header reservation to8MiB. Unknown/underreported lengths grow fallibly.
    let reserve = hint.min(limit).min(2 * 1024 * 1024);
    let mut data = Vec::new();
    data.try_reserve_exact(reserve)
        .map_err(|e| format!("Cannot reserve PCM memory: {e}"))?;
    for sample in samples {
        if data.len().is_multiple_of(4096) {
            cancel.check()?;
        }
        if data.len() == limit {
            return Err("Track exceeds the 256 MiB decoded PCM limit".into());
        }
        if !sample.is_finite() {
            return Err("Non-finite audio sample".into());
        }
        if data.len() == data.capacity() {
            let next = data.capacity().saturating_mul(2).max(4096).min(limit);
            data.try_reserve_exact(next - data.len())
                .map_err(|e| format!("Cannot grow PCM memory: {e}"))?;
        }
        data.push(sample);
    }
    cancel.check()?;
    Ok(data)
}
struct Deck {
    sink: Sink,
    source_retired: Option<Arc<AtomicBool>>,
    render: Option<Arc<RenderControl>>,
    track: Option<PreparedTrack>,
    loop_in: Option<usize>,
    loop_region: Option<FrameLoop>,
    loop_active: bool,
    speed: Option<PlaybackSpeed>,
    base_factor: f64,
    nudge: i32,
    sync_factor: f64,
    grid: Option<crate::beatgrid::BeatGrid>,
}
struct SourceRetirement(Arc<AtomicBool>);
impl Drop for SourceRetirement {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
struct RetiredTrack {
    _track: PreparedTrack,
    released: Arc<AtomicBool>,
}
impl Deck {
    fn actual_speed(&self) -> f64 {
        if self.render.as_ref().is_some_and(|r| r.sync_expired()) {
            self.track.as_ref().map_or(1., |track| {
                PlaybackSpeed::nudged(self.base_factor, self.nudge, track.samples.sample_rate())
                    .unwrap()
                    .effective_factor()
            })
        } else {
            self.speed.map_or(1., PlaybackSpeed::effective_factor)
        }
    }
    fn playing(&self) -> bool {
        self.track
            .as_ref()
            .zip(self.render.as_ref())
            .is_some_and(|(track, render)| {
                !render.is_paused() && render.position() < track.duration()
            })
    }
}
// The queue starts with generic metadata. Expose the renderer's fixed device format
// for its entire lifetime, including pause/EOF, so the mixer needs no rate conversion.
struct DeckOutput {
    queue: rodio::queue::SourcesQueueOutput,
    render: Arc<RenderControl>,
    channel: u16,
    channels: u16,
    sample_rate: u32,
    // Rust drops fields in declaration order: publish only AFTER the queue's PCM Arc is gone.
    _retirement: SourceRetirement,
}
impl Iterator for DeckOutput {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 && self.render.stopped.load(Ordering::Acquire) {
            return None;
        }
        let sample = self.queue.next();
        if sample.is_some() {
            self.channel = (self.channel + 1) % self.channels;
        }
        sample
    }
}
impl Source for DeckOutput {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
pub struct DeckSnapshot {
    pub path: Option<Arc<Path>>,
    pub title: String,
    pub loaded: bool,
    pub playing: bool,
    pub position: Duration,
    pub duration: Duration,
    pub overview: Option<Arc<[f32]>>,
    pub detail: Option<Arc<crate::waveform::DetailPeaks>>,
    pub grid: Option<crate::beatgrid::BeatGrid>,
    pub tempo: Option<crate::tempo::TempoEstimate>,
    pub loop_in: Option<Duration>,
    pub loop_out: Option<Duration>,
    pub loop_active: bool,
    pub nudge_direction: i32,
    pub requested_speed: f64,
    pub effective_speed: f64,
    pub sync_factor: f64,
}
impl DeckSnapshot {
    pub fn percent(&self) -> f32 {
        if self.duration.is_zero() {
            0.0
        } else {
            (100.0 * self.position.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 100.0)
        }
    }
}

pub struct DualDeckEngine {
    decks: [RefCell<Deck>; 2],
    levels: Cell<MixerLevels>,
    mixer: Mixer,
    output_channels: u16,
    output_rate: u32,
    retired: RefCell<Vec<RetiredTrack>>,
    // Shared producer timeline; completed-frame stamps are consumed by one-shot sync.
    render_clock: Arc<crate::render_bus::RenderClock>,
    clock_observer: RefCell<crate::render_bus::ClockObserver>,
    control_clock: Box<dyn Fn() -> std::time::Instant>,
    // Drop explicitly stops the owned device before releasing track-buffer owners.
    _stream: Option<OutputStream>,
}
impl Drop for DualDeckEngine {
    fn drop(&mut self) {
        for deck in &mut self.decks {
            deck.get_mut().sink.stop();
        }
        drop(self._stream.take());
    }
}
impl DualDeckEngine {
    pub fn open_default() -> Result<Self, Box<dyn Error>> {
        let stream = OutputStreamBuilder::open_default_stream()?;
        let mut engine = Self::from_mixer(
            stream.mixer().clone(),
            stream.config().channel_count(),
            stream.config().sample_rate(),
        );
        engine._stream = Some(stream);
        Ok(engine)
    }
    /// An injected renderer on another thread must be stopped/joined before this engine drops.
    pub(crate) fn from_mixer(mixer: Mixer, output_channels: u16, output_rate: u32) -> Self {
        Self::from_mixer_with_clock(
            mixer,
            output_channels,
            output_rate,
            Box::new(std::time::Instant::now),
        )
    }
    pub(crate) fn from_mixer_with_clock(
        mixer: Mixer,
        output_channels: u16,
        output_rate: u32,
        control_clock: Box<dyn Fn() -> std::time::Instant>,
    ) -> Self {
        assert!(output_channels > 0 && output_rate > 0);
        // Both deck queues enter one fixed-format bus before the device/injected mixer.
        let (deck_mixer, deck_output) = rodio::mixer::mixer(output_channels, output_rate);
        let (bus, render_clock) = crate::render_bus::RenderBus::new(deck_output);
        mixer.add(bus);
        let mixer = deck_mixer;
        let decks = std::array::from_fn(|_| {
            let sink = Sink::connect_new(&mixer);
            sink.pause();
            RefCell::new(Deck {
                sink,
                source_retired: None,
                render: None,
                track: None,
                loop_in: None,
                loop_region: None,
                loop_active: false,
                speed: None,
                base_factor: 1.,
                nudge: 0,
                sync_factor: 1.,
                grid: None,
            })
        });
        let engine = Self {
            decks,
            levels: Cell::new(MixerLevels::default()),
            mixer,
            output_channels,
            output_rate,
            retired: RefCell::new(Vec::new()),
            render_clock,
            clock_observer: RefCell::new(crate::render_bus::ClockObserver::default()),
            control_clock,
            _stream: None,
        };
        engine.set_levels(MixerLevels::default());
        engine
    }
    /// Control-plane reclamation after the source queue acknowledges destruction.
    pub fn collect_retired(&self) {
        self.retired
            .borrow_mut()
            .retain(|track| !track.released.load(Ordering::Acquire));
    }
    /// Install prepared tracks; same-track commands reuse the existing renderer and Sink.
    pub fn install(&self, id: DeckId, track: PreparedTrack) {
        let source = track.samples.clone();
        self.replace_source(id, track, source, Duration::ZERO, false, None);
    }
    fn replace_source(
        &self,
        id: DeckId,
        track: PreparedTrack,
        source: PcmSource,
        offset: Duration,
        playing: bool,
        grid: Option<crate::beatgrid::BeatGrid>,
    ) {
        let _ = self.apply_sync_factor(id, 1.);
        let (loop_in, loop_region, speed, base_factor, nudge) = {
            let deck = self.decks[id.index()].borrow();
            if deck
                .track
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(&old.detail, &track.detail))
            {
                (
                    deck.loop_in,
                    deck.loop_region,
                    deck.speed.unwrap(),
                    deck.base_factor,
                    deck.nudge,
                )
            } else {
                (
                    None,
                    None,
                    PlaybackSpeed::new(1., track.samples.sample_rate())
                        .expect("Prepared source format was validated"),
                    1.,
                    0,
                )
            }
        };
        let region = source.loop_region();
        {
            let mut deck = self.decks[id.index()].borrow_mut();
            if deck
                .track
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(&old.detail, &track.detail))
            {
                if let Some(render) = &deck.render {
                    render.publish(
                        speed.output_rate().get(),
                        playing,
                        region,
                        Some(SeekFrame {
                            time: offset,
                            frame: source.cursor_frame(),
                        }),
                    );
                    if playing {
                        deck.sink.play();
                    } else {
                        deck.sink.pause();
                    }
                    deck.grid = grid;
                    deck.loop_active = false;
                    return;
                }
            }
            if let Some(render) = &deck.render {
                render.stopped.store(true, Ordering::Release);
            }
        }
        self.collect_retired();
        let render = RenderControl::new(
            speed.output_rate().get(),
            playing,
            region,
            SeekFrame {
                time: offset,
                frame: source.cursor_frame(),
            },
            RenderFormat {
                source_rate: track.samples.sample_rate(),
                output_rate: self.output_rate,
                frames: track.samples.frames(),
            },
        );
        let renderer = DeckRenderer::new(
            source,
            Arc::clone(&render),
            self.output_channels,
            self.output_rate,
        )
        .with_clock(Arc::clone(&self.render_clock));
        let source_retired = Arc::new(AtomicBool::new(false));
        let (sink, queue) = Sink::new();
        let output = DeckOutput {
            queue,
            render: Arc::clone(&render),
            channel: 0,
            channels: self.output_channels,
            sample_rate: self.output_rate,
            _retirement: SourceRetirement(Arc::clone(&source_retired)),
        };
        sink.pause();
        let (a, b) = self.levels.get().volumes();
        sink.set_volume(if id == DeckId::A { a } else { b });
        sink.append(renderer);
        if playing {
            sink.play();
        }
        self.mixer.add(output);

        let mut old = std::mem::replace(
            &mut *self.decks[id.index()].borrow_mut(),
            Deck {
                sink,
                source_retired: Some(source_retired),
                render: Some(render),
                track: Some(track),
                loop_in,
                loop_region,
                loop_active: false,
                speed: Some(speed),
                base_factor,
                nudge,
                sync_factor: 1.,
                grid,
            },
        );
        // Transfer ownership before old Sink::drop signals the audio path to stop.
        if let (Some(track), Some(released)) = (old.track.take(), old.source_retired.take()) {
            self.retired.borrow_mut().push(RetiredTrack {
                _track: track,
                released,
            });
        }
    }
    /// Seek the shared PCM source on the control plane, without Sink::try_seek's audio feedback wait.
    pub fn seek_percent(&self, id: DeckId, percent: f32) -> Result<(), String> {
        if !percent.is_finite() {
            return Err("Invalid seek position".into());
        }
        let duration = self.snapshot(id).duration;
        self.seek_position(
            id,
            duration.mul_f64(f64::from(percent.clamp(0.0, 100.0)) / 100.0),
        )
    }
    /// Absolute source-time seek. Hot cues must not be rounded through a GUI percentage.
    pub fn seek_position(&self, id: DeckId, offset: Duration) -> Result<(), String> {
        let (track, playing, grid) = {
            let deck = self.decks[id.index()].borrow();
            (
                deck.track.clone().ok_or("No track loaded")?,
                deck.playing(),
                deck.grid,
            )
        };
        let duration = track.duration();
        if offset > duration {
            return Err("Position is outside the loaded track".into());
        }
        let mut source = track.samples.clone();
        source.try_seek(offset).map_err(|e| e.to_string())?;
        self.reset_nudge_for_replacement(id);
        self.replace_source(
            id,
            track,
            source,
            offset,
            playing && offset < duration,
            grid,
        );
        Ok(())
    }
    /// One-shot tempo matching from confirmed constant grids; no beat-phase alignment.
    pub fn tempo_match_factor(&self, id: DeckId) -> Result<f64, String> {
        let target = self.decks[id.index()].borrow();
        let reference = self.decks[1 - id.index()].borrow();
        let track = target
            .track
            .as_ref()
            .ok_or("Load both decks before matching tempo")?;
        reference
            .track
            .as_ref()
            .ok_or("Load both decks before matching tempo")?;
        let own_grid = target
            .grid
            .ok_or("Save manual grids on both decks before matching tempo")?;
        let reference_grid = reference
            .grid
            .ok_or("Save manual grids on both decks before matching tempo")?;
        if target.nudge != 0 || reference.nudge != 0 {
            return Err("Release nudge on both decks before matching tempo".into());
        }
        let reference_speed = reference.actual_speed();
        let factor = reference_grid.bpm() * reference_speed / own_grid.bpm();
        PlaybackSpeed::new(factor, track.samples.sample_rate())?;
        Ok(factor)
    }
    fn fresh_phase_positions(&self, own: &Deck, reference: &Deck) -> Option<(Duration, Duration)> {
        self.fresh_phase_positions_at(own, reference, self.control_now(), || self.control_now())
    }
    fn fresh_phase_positions_at(
        &self,
        own: &Deck,
        reference: &Deck,
        now: std::time::Instant,
        finish: impl FnOnce() -> std::time::Instant,
    ) -> Option<(Duration, Duration)> {
        let before = self.render_clock.completed.load(Ordering::Acquire);
        if !self.clock_observer.borrow_mut().observe(before, now) {
            return None;
        }
        let own_render = own.render.as_ref()?;
        let reference_render = reference.render.as_ref()?;
        let a = own_render.rendered_stamp()?;
        let b = reference_render.rendered_stamp()?;
        let after = self.render_clock.completed.load(Ordering::Acquire);
        let frame_fresh = a.output_frame == b.output_frame
            && crate::render_bus::RenderClock::frame_is_fresh(
                a.output_frame,
                before,
                after,
                self.output_rate,
            );
        let end = finish();
        let live = self.clock_observer.borrow_mut().observe(after, end);
        let timely = end
            .checked_duration_since(now)
            .is_some_and(|elapsed| elapsed <= Duration::from_millis(5));
        (frame_fresh && live && timely).then(|| {
            (
                own_render.stamped_position(a),
                reference_render.stamped_position(b),
            )
        })
    }
    pub fn phase_sync_frame(
        &self,
        id: DeckId,
        alignment: crate::phase_sync::Alignment,
    ) -> Result<usize, String> {
        self.tempo_match_factor(id)?;
        let own = self.decks[id.index()].borrow();
        let reference = self.decks[1 - id.index()].borrow();
        if !own.playing() || !reference.playing() {
            return Err("Play both decks before phase sync".into());
        }
        if own.loop_active || reference.loop_active {
            return Err("Exit loops on both decks before phase sync".into());
        }
        let track = own.track.as_ref().unwrap();
        let own_render = own.render.as_ref().unwrap();
        let reference_render = reference.render.as_ref().unwrap();
        let positions = self
            .fresh_phase_positions(&own, &reference)
            // Explicit pending commands retain the previous one-shot control-position semantics.
            .unwrap_or_else(|| (own_render.position(), reference_render.position()));
        crate::phase_sync::target_frame_mode(
            own.grid.unwrap(),
            positions.0,
            reference.grid.unwrap(),
            positions.1,
            track.samples.sample_rate(),
            track.samples.frames(),
            alignment,
        )
    }
    pub fn phase_sync_once(
        &self,
        id: DeckId,
        alignment: crate::phase_sync::Alignment,
    ) -> Result<(), String> {
        let factor = self.tempo_match_factor(id)?;
        let frame = self.phase_sync_frame(id, alignment)?;
        let mut deck = self.decks[id.index()].borrow_mut();
        let rate = deck.track.as_ref().unwrap().samples.sample_rate();
        let speed = PlaybackSpeed::new(factor, rate)?;
        // Rate and seek become one coherent command at the next output-frame boundary.
        deck.render.as_ref().unwrap().publish(
            speed.output_rate().get(),
            true,
            None,
            Some(SeekFrame {
                time: frame_time(frame, rate),
                frame,
            }),
        );
        deck.base_factor = factor;
        deck.sync_factor = 1.;
        deck.speed = Some(speed);
        Ok(())
    }
    pub fn command_version(&self, id: DeckId) -> Option<u64> {
        self.decks[id.index()]
            .borrow()
            .render
            .as_ref()
            .map(|r| r.command_version())
    }
    pub fn control_now(&self) -> std::time::Instant {
        (self.control_clock)()
    }
    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }
    pub fn rendered_frames(&self) -> u64 {
        self.render_clock.completed.load(Ordering::Acquire)
    }
    pub fn sync_observation(
        &self,
        id: DeckId,
        alignment: crate::phase_sync::Alignment,
    ) -> Option<(f64, u64, f64)> {
        let nominal = self.tempo_match_factor(id).ok()?;
        let own = self.decks[id.index()].borrow();
        let reference = self.decks[1 - id.index()].borrow();
        if !own.playing() || !reference.playing() || own.loop_active || reference.loop_active {
            return None;
        }
        let (a, b) = self.fresh_phase_positions(&own, &reference)?;
        let error = match alignment {
            crate::phase_sync::Alignment::Beat => {
                crate::phase_sync::beat_displacement(own.grid?, a, reference.grid?, b).ok()?
            }
            crate::phase_sync::Alignment::Bar4 => crate::phase_sync::alignment_displacement(
                own.grid?,
                a,
                reference.grid?,
                b,
                alignment,
            )
            .ok()?,
        };
        Some((error, self.rendered_frames(), nominal))
    }
    pub fn apply_sync_factor(&self, id: DeckId, factor: f64) -> Result<(), String> {
        if !factor.is_finite() || !(0.98..=1.02).contains(&factor) {
            return Err("Sync correction exceeds two percent".into());
        }
        let mut deck = self.decks[id.index()].borrow_mut();
        let track = deck.track.as_ref().ok_or("Load a track first")?;
        if factor != 1. && (!deck.playing() || deck.loop_active || deck.nudge != 0) {
            return Err("Sync correction requires playing without loops or nudge".into());
        }
        let rate = track.samples.sample_rate();
        let requested = deck.base_factor * (1. + f64::from(deck.nudge) * 0.02) * factor;
        let speed = PlaybackSpeed::new(requested, rate)?;
        if deck.sync_factor == factor && factor == 1. {
            return Ok(());
        }
        let region = if deck.loop_active {
            deck.loop_region
        } else {
            None
        };
        if factor == 1. {
            deck.render.as_ref().unwrap().publish(
                speed.output_rate().get(),
                deck.playing(),
                region,
                None,
            );
        } else {
            let neutral = PlaybackSpeed::nudged(deck.base_factor, deck.nudge, rate)?;
            let until = self
                .rendered_frames()
                .checked_add(u64::from(self.output_rate).div_ceil(10))
                .ok_or("Sync clock deadline overflow")?;
            deck.render.as_ref().unwrap().publish_sync(
                speed.output_rate().get(),
                neutral.output_rate().get(),
                until,
                deck.playing(),
                region,
            );
        }
        deck.speed = Some(speed);
        deck.sync_factor = factor;
        Ok(())
    }
    pub fn set_speed(&self, id: DeckId, factor: f64) -> Result<(), String> {
        let nudge = self.decks[id.index()].borrow().nudge;
        self.set_rate_state(id, factor, nudge)
    }
    pub fn set_nudge(&self, id: DeckId, direction: i32) -> Result<(), String> {
        if !(-1..=1).contains(&direction) {
            return Err("Nudge direction must be -1, 0 or 1".into());
        }
        let base = {
            let deck = self.decks[id.index()].borrow();
            if direction == 0 && deck.track.is_none() {
                return Ok(());
            }
            if direction != 0 && !deck.playing() {
                return Err("Play the deck before nudging".into());
            }
            deck.base_factor
        };
        self.set_rate_state(id, base, direction)
    }
    fn reset_nudge_for_replacement(&self, id: DeckId) {
        let mut deck = self.decks[id.index()].borrow_mut();
        if let Some(track) = &deck.track {
            deck.speed = Some(
                PlaybackSpeed::new(deck.base_factor, track.samples.sample_rate())
                    .expect("Base speed was validated"),
            );
        }
        deck.nudge = 0;
        deck.sync_factor = 1.;
    }
    fn set_rate_state(&self, id: DeckId, factor: f64, nudge: i32) -> Result<(), String> {
        let mut deck = self.decks[id.index()].borrow_mut();
        let track = deck.track.as_ref().ok_or("Load a track first")?;
        let speed = PlaybackSpeed::nudged(factor, nudge, track.samples.sample_rate())?;
        if deck.base_factor == factor && deck.nudge == nudge && deck.sync_factor == 1. {
            return Ok(());
        }
        let region = if deck.loop_active {
            deck.loop_region
        } else {
            None
        };
        deck.render.as_ref().expect("Loaded deck renderer").publish(
            speed.output_rate().get(),
            deck.playing(),
            region,
            None,
        );
        deck.speed = Some(speed);
        deck.base_factor = factor;
        deck.nudge = nudge;
        deck.sync_factor = 1.;
        Ok(())
    }
    // Manual-loop commands run on the control plane.
    pub fn set_loop_in(&self, id: DeckId, position: Duration) -> Result<(), String> {
        let snapshot = self.snapshot(id);
        let rate = self.decks[id.index()]
            .borrow()
            .track
            .as_ref()
            .ok_or("Load a track first")?
            .samples
            .sample_rate();
        if position >= snapshot.duration {
            return Err("Loop In must be before track end".into());
        }
        if self.decks[id.index()].borrow().loop_active {
            self.exit_loop(id)?;
        }
        let mut deck = self.decks[id.index()].borrow_mut();
        deck.loop_in = Some(frame_at(position, rate));
        deck.loop_region = None;
        Ok(())
    }
    pub fn set_loop_out(&self, id: DeckId, position: Duration) -> Result<(), String> {
        let region = {
            let deck = self.decks[id.index()].borrow();
            let track = deck.track.as_ref().ok_or("Load a track first")?;
            if position > track.duration() {
                return Err("Loop Out exceeds track end".into());
            }
            // Duration can truncate a fractional nanosecond at EOF; map that exact end to all frames.
            let end = if position == track.duration() {
                track.samples.frames()
            } else {
                frame_at(position, track.samples.sample_rate())
            };
            FrameLoop::new(
                deck.loop_in.ok_or("Set Loop In first")?,
                end,
                track.samples.frames(),
            )?
        };
        if self.decks[id.index()].borrow().loop_active {
            self.exit_loop(id)?;
        }
        self.decks[id.index()].borrow_mut().loop_region = Some(region);
        Ok(())
    }
    /// Load saved boundaries atomically. Activation remains an explicit separate command.
    pub fn restore_loop(&self, id: DeckId, start: Duration, end: Duration) -> Result<(), String> {
        let region = {
            let deck = self.decks[id.index()].borrow();
            let track = deck.track.as_ref().ok_or("Load a track first")?;
            if start >= end || start >= track.duration() || end > track.duration() {
                return Err("Saved loop is outside the loaded track".into());
            }
            let end_frame = if end == track.duration() {
                track.samples.frames()
            } else {
                frame_at(end, track.samples.sample_rate())
            };
            FrameLoop::new(
                frame_at(start, track.samples.sample_rate()),
                end_frame,
                track.samples.frames(),
            )?
        };
        self.exit_loop(id)?;
        let mut deck = self.decks[id.index()].borrow_mut();
        deck.loop_in = Some(region.start);
        deck.loop_region = Some(region);
        Ok(())
    }
    pub fn auto_loop(&self, id: DeckId, eighths: u16, snap: bool) -> Result<(), String> {
        let position = self.snapshot(id).position;
        let region = {
            let deck = self.decks[id.index()].borrow();
            let track = deck.track.as_ref().ok_or("Load a track first")?;
            let grid = deck
                .grid
                .ok_or("Save a manual beatgrid before using Auto Loop")?;
            let start = if snap {
                grid.nearest_beat_frame(position, track.samples.sample_rate())?
            } else {
                deck.render.as_ref().unwrap().frame()
            };
            FrameLoop::from_beats(
                start,
                eighths,
                grid.bpm(),
                track.samples.sample_rate(),
                track.samples.frames(),
            )?
        };
        self.apply_loop_region(id, region, true, Some(region.start))
    }
    pub fn resize_loop(&self, id: DeckId, double: bool) -> Result<(), String> {
        let (region, active) = {
            let deck = self.decks[id.index()].borrow();
            let track = deck.track.as_ref().ok_or("Load a track first")?;
            (
                deck.loop_region
                    .ok_or("Set Loop In/Out first")?
                    .resized(double, track.samples.frames())?,
                deck.loop_active,
            )
        };
        self.apply_loop_region(id, region, active, None)
    }
    fn apply_loop_region(
        &self,
        id: DeckId,
        region: FrameLoop,
        active: bool,
        seek: Option<usize>,
    ) -> Result<(), String> {
        let mut deck = self.decks[id.index()].borrow_mut();
        if active {
            let track = deck.track.as_ref().ok_or("Load a track first")?;
            let target = seek.map(|frame| SeekFrame {
                time: frame_time(frame, track.samples.sample_rate()),
                frame,
            });
            let speed =
                PlaybackSpeed::nudged(deck.base_factor, deck.nudge, track.samples.sample_rate())?;
            deck.render.as_ref().unwrap().publish(
                speed.output_rate().get(),
                deck.playing(),
                Some(region),
                target,
            );
            deck.speed = Some(speed);
            deck.sync_factor = 1.;
        }
        deck.loop_in = Some(region.start);
        deck.loop_region = Some(region);
        deck.loop_active = active;
        Ok(())
    }
    pub fn activate_loop(&self, id: DeckId) -> Result<(), String> {
        self.engage_loop(id, false)
    }
    pub fn reloop(&self, id: DeckId) -> Result<(), String> {
        self.engage_loop(id, true)
    }
    fn engage_loop(&self, id: DeckId, restart: bool) -> Result<(), String> {
        let mut deck = self.decks[id.index()].borrow_mut();
        let track = deck.track.as_ref().ok_or("Load a track first")?;
        let region = deck.loop_region.ok_or("Set valid Loop In/Out first")?;
        let render = deck.render.as_ref().unwrap();
        let frame = render.frame();
        let target = (restart || frame < region.start || frame >= region.end).then(|| SeekFrame {
            time: frame_time(region.start, track.samples.sample_rate()),
            frame: region.start,
        });
        let speed =
            PlaybackSpeed::nudged(deck.base_factor, deck.nudge, track.samples.sample_rate())?;
        render.publish(
            speed.output_rate().get(),
            deck.playing(),
            Some(region),
            target,
        );
        deck.speed = Some(speed);
        deck.sync_factor = 1.;
        deck.loop_active = true;
        Ok(())
    }
    pub fn exit_loop(&self, id: DeckId) -> Result<(), String> {
        let mut deck = self.decks[id.index()].borrow_mut();
        if !deck.loop_active {
            return Ok(());
        }
        let speed = PlaybackSpeed::new(
            deck.base_factor,
            deck.track.as_ref().unwrap().samples.sample_rate(),
        )?;
        deck.render.as_ref().unwrap().publish(
            speed.output_rate().get(),
            deck.playing(),
            None,
            None,
        );
        deck.loop_active = false;
        deck.nudge = 0;
        deck.sync_factor = 1.;
        deck.speed = Some(speed);
        Ok(())
    }
    pub fn load_a(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        self.install(DeckId::A, PreparedTrack::open(path)?);
        Ok(())
    }
    pub fn load_b(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        self.install(DeckId::B, PreparedTrack::open(path)?);
        Ok(())
    }
    pub fn play(&self, id: DeckId) {
        let _ = self.apply_sync_factor(id, 1.);
        let restart = {
            let deck = self.decks[id.index()].borrow();
            deck.track.is_some()
                && deck.track.as_ref().is_some_and(|t| {
                    deck.render
                        .as_ref()
                        .is_some_and(|r| r.position() >= t.duration())
                })
        };
        if restart {
            self.cue(id);
        }
        let deck = self.decks[id.index()].borrow();
        if let Some(render) = &deck.render {
            render.publish(
                deck.speed.unwrap().output_rate().get(),
                true,
                if deck.loop_active {
                    deck.loop_region
                } else {
                    None
                },
                None,
            );
            deck.sink.play();
        }
    }
    pub fn pause(&self, id: DeckId) {
        let mut deck = self.decks[id.index()].borrow_mut();
        if let Some(track) = &deck.track {
            let speed = PlaybackSpeed::new(deck.base_factor, track.samples.sample_rate())
                .expect("Validated base rate");
            deck.render.as_ref().unwrap().publish(
                speed.output_rate().get(),
                false,
                if deck.loop_active {
                    deck.loop_region
                } else {
                    None
                },
                None,
            );
            deck.speed = Some(speed);
            deck.nudge = 0;
            deck.sync_factor = 1.;
        }
        deck.sink.pause();
    }
    pub fn cue(&self, id: DeckId) {
        let (track, grid) = {
            let deck = self.decks[id.index()].borrow();
            (deck.track.clone(), deck.grid)
        };
        if let Some(track) = track {
            let source = track.samples.clone();
            self.reset_nudge_for_replacement(id);
            self.replace_source(id, track, source, Duration::ZERO, false, grid);
        }
    }
    #[cfg(test)]
    fn set_grid_here(&self, id: DeckId, bpm: f64) -> Result<(), String> {
        let position = self.snapshot(id).position;
        let grid = crate::beatgrid::BeatGrid::new(bpm, position)?;
        self.set_grid(id, Some(grid))
    }
    pub fn set_grid(
        &self,
        id: DeckId,
        grid: Option<crate::beatgrid::BeatGrid>,
    ) -> Result<(), String> {
        let mut deck = self.decks[id.index()].borrow_mut();
        if deck.track.is_none() {
            return Err("Load a track before setting its grid".into());
        }
        if grid.is_some_and(|grid| grid.anchor() > deck.track.as_ref().unwrap().duration()) {
            return Err("Grid anchor is outside this track".into());
        }
        deck.grid = grid;
        Ok(())
    }
    #[cfg(test)]
    fn clear_grid(&self, id: DeckId) {
        self.decks[id.index()].borrow_mut().grid = None;
    }
    pub fn snapshot(&self, id: DeckId) -> DeckSnapshot {
        let deck = self.decks[id.index()].borrow();
        let duration = deck
            .track
            .as_ref()
            .map_or(Duration::ZERO, PreparedTrack::duration);
        let position = deck
            .render
            .as_ref()
            .map_or(Duration::ZERO, |r| r.position())
            .min(duration);
        DeckSnapshot {
            path: deck.track.as_ref().map(|t| Arc::clone(&t.path)),
            title: deck
                .track
                .as_ref()
                .map_or_else(|| "No track loaded".into(), |t| t.title.clone()),
            loaded: deck.track.is_some(),
            playing: deck.playing(),
            position,
            duration,
            overview: deck.track.as_ref().map(|t| Arc::clone(&t.overview)),
            detail: deck.track.as_ref().map(|t| Arc::clone(&t.detail)),
            grid: deck.grid,
            tempo: deck.track.as_ref().and_then(|track| track.tempo),
            loop_in: deck.track.as_ref().and_then(|t| {
                deck.loop_in
                    .map(|frame| frame_time(frame, t.samples.sample_rate()))
            }),
            loop_out: deck.track.as_ref().and_then(|t| {
                deck.loop_region
                    .map(|region| frame_time(region.end, t.samples.sample_rate()).min(t.duration()))
            }),
            loop_active: deck.loop_active,
            nudge_direction: deck.nudge,
            requested_speed: deck.base_factor,
            effective_speed: deck.actual_speed(),
            sync_factor: if deck.render.as_ref().is_some_and(|r| r.sync_expired()) {
                1.
            } else {
                deck.sync_factor
            },
        }
    }
    /// Returns the latest complete producer window, not device/current freshness.
    pub fn output_peak(&self) -> Option<f32> {
        crate::peak_meter::PeakAccumulator::read_peak(&self.render_clock.peak_window)
    }
    #[cfg(test)]
    pub fn output_volumes(&self) -> (f32, f32) {
        (
            self.decks[0].borrow().sink.volume(),
            self.decks[1].borrow().sink.volume(),
        )
    }
    pub fn set_levels(&self, levels: MixerLevels) {
        self.levels.set(levels);
        let (a, b) = levels.volumes();
        self.decks[0].borrow().sink.set_volume(a);
        self.decks[1].borrow().sink.set_volume(b);
    }
    pub fn wait_for_end(&self) {
        // EOF renderers remain installed for race-free seeks/restart. Their queues never end.
        // This is a control-thread wait for source frames handed to the output graph;
        // physical device drain/latency is not inferred from producer completion.
        while self.decks.iter().any(|deck| {
            let deck = deck.borrow();
            deck.track
                .as_ref()
                .zip(deck.render.as_ref())
                .is_some_and(|(track, render)| render.position() < track.duration())
        }) {
            self.collect_retired();
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

pub fn play_file(path: &Path) -> Result<(), Box<dyn Error>> {
    let engine = DualDeckEngine::open_default()?;
    engine.load_a(path)?;
    engine.set_levels(MixerLevels {
        crossfader: 0.0,
        ..MixerLevels::default()
    });
    engine.play(DeckId::A);
    engine.wait_for_end();
    Ok(())
}
pub fn mix_files(a: &Path, b: &Path) -> Result<(), Box<dyn Error>> {
    let engine = DualDeckEngine::open_default()?;
    engine.load_a(a)?;
    engine.load_b(b)?;
    engine.play(DeckId::A);
    engine.play(DeckId::B);
    engine.wait_for_end();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fixture, wav_file};
    #[test]
    fn continuous_bar_observation_detects_whole_beat_offset_without_mutating_decks() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let clock = std::rc::Rc::new(std::cell::Cell::new(std::time::Instant::now()));
        let source_clock = std::rc::Rc::clone(&clock);
        let engine = DualDeckEngine::from_mixer_with_clock(
            mixer,
            1,
            48000,
            Box::new(move || source_clock.get()),
        );
        let file = wav_file(4096, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        for id in [DeckId::A, DeckId::B] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine.play(id);
        }
        engine
            .seek_position(DeckId::A, Duration::from_millis(500))
            .unwrap();
        let beat = crate::phase_sync::Alignment::Beat;
        let bar = crate::phase_sync::Alignment::Bar4;
        assert!(engine.sync_observation(DeckId::A, beat).is_none());
        for _ in 0..1000 {
            output.next();
        }
        clock.set(clock.get() + Duration::from_millis(1));
        let a = engine.snapshot(DeckId::A);
        let b = engine.snapshot(DeckId::B);
        assert!(engine.sync_observation(DeckId::A, beat).unwrap().0.abs() < 1e-8);
        assert!((engine.sync_observation(DeckId::A, bar).unwrap().0 + 1.).abs() < 1e-8);
        assert!((engine.sync_observation(DeckId::B, bar).unwrap().0 - 1.).abs() < 1e-8);
        assert_eq!(engine.snapshot(DeckId::A).position, a.position);
        assert_eq!(engine.snapshot(DeckId::B).position, b.position);
        assert_eq!(
            engine.snapshot(DeckId::A).requested_speed,
            a.requested_speed
        );
        assert_eq!(
            engine.snapshot(DeckId::B).requested_speed,
            b.requested_speed
        );
    }
    #[test]
    fn bar_phase_sync_seeks_matching_bar_and_renders_expected_pcm_without_touching_reference() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        let samples: Vec<i16> = (0..192000).map(|i| (i % 10000) as i16).collect();
        let a = crate::test_support::wav_samples(&samples, 48000, 1);
        let b = wav_file(0, 192000);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        for id in [DeckId::A, DeckId::B] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine.play(id);
        }
        engine.set_levels(MixerLevels {
            channel_a: 1.,
            channel_b: 0.,
            crossfader: 0.,
        });
        engine
            .seek_position(DeckId::A, Duration::from_secs(1))
            .unwrap();
        engine
            .phase_sync_once(DeckId::A, crate::phase_sync::Alignment::Beat)
            .unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));
        engine
            .phase_sync_once(DeckId::A, crate::phase_sync::Alignment::Bar4)
            .unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(2));
        assert_eq!(engine.snapshot(DeckId::B).position, Duration::ZERO);
        assert_eq!(engine.snapshot(DeckId::B).requested_speed, 1.);
        assert!((output.next().unwrap() - 6000. / 32768.).abs() < 1e-7);
    }
    #[test]
    fn sync_correction_is_separate_bounded_and_expires_without_control_ticks() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        let file = wav_file(8192, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        engine.set_speed(DeckId::A, 1.1).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let position = engine.snapshot(DeckId::A).position;
        engine.apply_sync_factor(DeckId::A, 1.02).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.1);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        assert!((engine.snapshot(DeckId::A).effective_speed - 1.122).abs() < 1e-9);
        assert_eq!(engine.snapshot(DeckId::B).effective_speed, 1.);
        assert!(engine.apply_sync_factor(DeckId::A, 1.021).is_err());
        for _ in 0..4800 {
            output.next();
        }
        assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.02);
        output.next();
        assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
        assert!((engine.snapshot(DeckId::A).effective_speed - 1.1).abs() < 1e-9);
        let render = engine.decks[0].borrow().render.as_ref().unwrap().clone();
        let before = render.rendered_stamp().unwrap().phase_units;
        output.next();
        assert_eq!(render.rendered_stamp().unwrap().phase_units - before, 52800);
        engine.apply_sync_factor(DeckId::A, 0.99).unwrap();
        engine.set_speed(DeckId::A, 1.1).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
        engine.apply_sync_factor(DeckId::A, 1.01).unwrap();
        engine.pause(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.play(DeckId::A);
        engine.apply_sync_factor(DeckId::A, 1.01).unwrap();
        engine.set_nudge(DeckId::A, 1).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 1);
        engine.set_nudge(DeckId::A, 0).unwrap();
        engine.set_speed(DeckId::A, 2.).unwrap();
        assert!(engine.apply_sync_factor(DeckId::A, 1.02).is_err());
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 2.);
        assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
    }
    #[test]
    fn fresh_pair_rechecks_wall_time_after_a_suspended_read_with_stalled_bus() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        let file = wav_file(4096, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let start = std::time::Instant::now();
        let pair = |begin, end| {
            engine.fresh_phase_positions_at(
                &engine.decks[0].borrow(),
                &engine.decks[1].borrow(),
                begin,
                || end,
            )
        };
        assert!(pair(start, start).is_none());
        output.next();
        assert!(pair(
            start + Duration::from_millis(1),
            start + Duration::from_millis(1)
        )
        .is_some());
        // Still-current stamps cannot disguise a long suspended control-plane read.
        assert!(pair(
            start + Duration::from_millis(2),
            start + Duration::from_millis(103)
        )
        .is_none());
        assert!(pair(
            start + Duration::from_millis(104),
            start + Duration::from_millis(104)
        )
        .is_none());
        output.next();
        assert!(pair(
            start + Duration::from_millis(105),
            start + Duration::from_millis(110)
        )
        .is_some());
        assert!(pair(
            start + Duration::from_millis(111),
            start + Duration::from_millis(117)
        )
        .is_none());
    }
    #[test]
    fn fresh_phase_pairs_require_live_bus_equal_frames_and_acknowledged_commands() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 2, 48000);
        let file = wav_file(4096, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let start = std::time::Instant::now();
        let pair = |now| {
            engine.fresh_phase_positions_at(
                &engine.decks[0].borrow(),
                &engine.decks[1].borrow(),
                now,
                || now,
            )
        };
        assert!(pair(start).is_none());
        output.next();
        output.next();
        assert!(pair(start + Duration::from_millis(1)).is_some());
        // A half stereo frame retains the last complete, matching and still-fresh pair.
        output.next();
        assert!(pair(start + Duration::from_millis(2)).is_some());
        output.next();
        assert!(pair(start + Duration::from_millis(3)).is_some());
        engine.set_speed(DeckId::A, 1.1).unwrap();
        assert!(pair(start + Duration::from_millis(4)).is_none());
        output.next();
        assert!(pair(start + Duration::from_millis(5)).is_none());
        output.next();
        assert!(pair(start + Duration::from_millis(6)).is_some());
        assert!(pair(start + Duration::from_millis(106)).is_some());
        assert!(
            pair(start + Duration::from_millis(107)).is_none(),
            "stalled bus must expire even if stamp frames match"
        );
        output.next();
        output.next();
        assert!(pair(start + Duration::from_millis(108)).is_some());
        // A long observation gap must reacquire progress, not revive a newly-seen old pair.
        assert!(pair(start + Duration::from_millis(500)).is_none());
        assert!(pair(start + Duration::from_millis(501)).is_none());
        output.next();
        output.next();
        assert!(pair(start + Duration::from_millis(502)).is_some());
        engine
            .seek_position(DeckId::A, Duration::from_secs(1))
            .unwrap();
        assert!(pair(start + Duration::from_millis(503)).is_none());
        output.next();
        output.next();
        assert!(pair(start + Duration::from_millis(504)).is_some());
        engine.pause(DeckId::A);
        assert!(pair(start + Duration::from_millis(505)).is_none());
        engine.load_a(file.path()).unwrap();
        assert!(pair(start + Duration::from_millis(506)).is_none());
    }
    #[test]
    fn coherent_render_stamps_follow_full_frames_and_invalidate_on_commands_and_replacement() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 2, 48000);
        let file = wav_file(4096, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let a = engine.decks[0].borrow().render.as_ref().unwrap().clone();
        let b = engine.decks[1].borrow().render.as_ref().unwrap().clone();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        assert!(a.rendered_stamp().is_none());
        output.next();
        assert!(
            a.rendered_stamp().is_none(),
            "half stereo frame must not publish a stamp"
        );
        output.next();
        let first = a.rendered_stamp().unwrap();
        assert_eq!(first, b.rendered_stamp().unwrap());
        assert_eq!(first.output_frame, 1);
        assert_eq!(first.phase_units, 48000);
        assert_eq!(engine.render_clock.completed.load(Ordering::Acquire), 1);
        engine
            .seek_position(DeckId::A, Duration::from_secs(1))
            .unwrap();
        assert!(a.rendered_stamp().is_none());
        output.next();
        assert!(a.rendered_stamp().is_none());
        output.next();
        let sought = a.rendered_stamp().unwrap();
        assert_eq!(sought.output_frame, 2);
        assert_eq!(sought.phase_units, 48001 * 48000);
        engine.set_speed(DeckId::A, 1.25).unwrap();
        assert!(a.rendered_stamp().is_none());
        output.next();
        output.next();
        let sped = a.rendered_stamp().unwrap();
        assert_eq!(sped.phase_units, sought.phase_units + 60000);
        engine.pause(DeckId::A);
        assert!(a.rendered_stamp().is_none());
        engine.play(DeckId::A);
        for _ in 0..1000 {
            output.next();
        }
        assert!(a.rendered_stamp().is_some());
        engine.seek_percent(DeckId::A, 100.).unwrap();
        assert!(a.rendered_stamp().is_none());
        for _ in 0..1000 {
            output.next();
        }
        let end = a.rendered_stamp().unwrap();
        assert_eq!(end.phase_units, 192000 * 48000);
        assert!(end.output_frame <= engine.render_clock.completed.load(Ordering::Acquire));
        // Explicit EOF pauses the Sink, so its last real render stamp remains frozen.
        for _ in 0..1000 {
            output.next();
        }
        assert_eq!(a.rendered_stamp().unwrap(), end);
        engine.play(DeckId::A);
        for _ in 0..1000 {
            output.next();
        }
        assert!(a.rendered_stamp().unwrap().output_frame > end.output_frame);
        engine.pause(DeckId::A);
        engine.load_a(file.path()).unwrap();
        assert!(
            a.rendered_stamp().is_none(),
            "stopped source rejects its old stamp"
        );
        let next = engine.decks[0].borrow().render.as_ref().unwrap().clone();
        assert!(!Arc::ptr_eq(&a, &next));
        assert!(next.rendered_stamp().is_none());
    }
    #[test]
    fn concurrent_render_stamp_reads_are_coherent() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        let file = wav_file(4096, 192000);
        engine.load_a(file.path()).unwrap();
        engine.play(DeckId::A);
        let render = engine.decks[0].borrow().render.as_ref().unwrap().clone();
        for _ in 0..1000 {
            output.next();
        }
        let (start, wait) = std::sync::mpsc::channel();
        let finished = Arc::new(AtomicBool::new(false));
        let done = Arc::clone(&finished);
        let worker = std::thread::spawn(move || {
            wait.recv().unwrap();
            for _ in 0..100000 {
                output.next();
            }
            done.store(true, Ordering::Release);
            // Return ownership so the injected audio source drops before engine buffer owners.
            output
        });
        start.send(()).unwrap();
        let mut accepted = 0;
        let mut mismatch = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !finished.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            if let Some(stamp) = render.rendered_stamp() {
                if stamp.phase_units != stamp.output_frame * 48000 {
                    mismatch = Some(stamp);
                    break;
                }
                accepted += 1;
            }
        }
        let output = worker.join().unwrap();
        drop(output);
        assert_eq!(mismatch, None);
        assert!(accepted > 0);
    }
    #[test]
    fn shared_bus_clock_survives_late_loading_pause_eof_and_track_replacement() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 2, 48000);
        for _ in 0..20 {
            assert_eq!(output.next(), Some(0.));
        }
        assert_eq!(engine.render_clock.completed.load(Ordering::Acquire), 10);
        let track = wav_file(8192, 4800);
        engine.load_a(track.path()).unwrap();
        engine.load_b(track.path()).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        for _ in 0..200 {
            output.next();
        }
        assert_eq!(engine.render_clock.completed.load(Ordering::Acquire), 110);
        let a = engine.snapshot(DeckId::A).position;
        let b = engine.snapshot(DeckId::B).position;
        assert_eq!(a, b);
        engine.pause(DeckId::A);
        // Sink pause is a Rodio control callback; settle its existing 5ms control tick.
        for _ in 0..1000 {
            output.next();
        }
        let paused = engine.snapshot(DeckId::A).position;
        for _ in 0..1000 {
            output.next();
        }
        assert_eq!(engine.snapshot(DeckId::A).position, paused);
        assert!(engine.snapshot(DeckId::B).position > b);
        engine.seek_percent(DeckId::B, 100.).unwrap();
        for _ in 0..1000 {
            output.next();
        }
        assert!(!engine.snapshot(DeckId::B).playing);
        engine.load_b(track.path()).unwrap();
        engine.play(DeckId::B);
        let before = engine.render_clock.completed.load(Ordering::Acquire);
        for _ in 0..1000 {
            output.next();
        }
        assert_eq!(
            engine.render_clock.completed.load(Ordering::Acquire),
            before + 500
        );
        assert!(engine.snapshot(DeckId::B).position > Duration::ZERO);
        engine.collect_retired();
        assert!(engine.retired.borrow().is_empty());
    }
    #[test]
    fn one_shot_phase_sync_publishes_rate_and_seek_together_and_renders_target_samples() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        let ramp: Vec<i16> = (0..192000).map(|i| (i % 10000) as i16).collect();
        let a = crate::test_support::wav_samples(&ramp, 48000, 1);
        let b = wav_file(0, 192000);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        for (id, bpm) in [(DeckId::A, 120.), (DeckId::B, 128.)] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(bpm, Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine.play(id);
        }
        engine.set_levels(MixerLevels {
            channel_a: 1.,
            channel_b: 0.,
            crossfader: 0.,
        });
        engine
            .seek_position(DeckId::A, Duration::from_millis(100))
            .unwrap();
        engine
            .seek_position(DeckId::B, Duration::from_nanos(117187500))
            .unwrap();
        let renderer = engine.decks[0].borrow().render.as_ref().unwrap().clone();
        let reference = engine.snapshot(DeckId::B);
        engine
            .phase_sync_once(DeckId::A, crate::phase_sync::Alignment::Beat)
            .unwrap();
        let snapshot = engine.snapshot(DeckId::A);
        assert_eq!(snapshot.position, Duration::from_millis(125));
        assert_eq!(snapshot.requested_speed, 128. / 120.);
        assert!(snapshot.playing);
        assert!(Arc::ptr_eq(
            &renderer,
            engine.decks[0].borrow().render.as_ref().unwrap()
        ));
        assert_eq!(engine.snapshot(DeckId::B).position, reference.position);
        assert_eq!(
            engine.snapshot(DeckId::B).requested_speed,
            reference.requested_speed
        );
        assert!((output.next().unwrap() - 6000. / 32768.).abs() < 1e-7);
        assert!((output.next().unwrap() - (6001. + 1. / 15.) / 32768.).abs() < 1e-7);
        engine
            .restore_loop(DeckId::A, Duration::ZERO, Duration::from_secs(1))
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        let before = engine.snapshot(DeckId::A);
        assert!(engine
            .phase_sync_once(DeckId::A, crate::phase_sync::Alignment::Beat)
            .is_err());
        assert!(engine
            .phase_sync_once(DeckId::B, crate::phase_sync::Alignment::Beat)
            .is_err());
        assert_eq!(engine.snapshot(DeckId::A).position, before.position);
        assert_eq!(
            engine.snapshot(DeckId::A).requested_speed,
            before.requested_speed
        );
        engine.exit_loop(DeckId::A).unwrap();
        engine.pause(DeckId::B);
        assert!(engine
            .phase_sync_once(DeckId::A, crate::phase_sync::Alignment::Beat)
            .is_err());
        assert!(!engine.snapshot(DeckId::B).playing);
        engine.play(DeckId::B);
        engine.seek_percent(DeckId::A, 100.).unwrap();
        assert!(engine
            .phase_sync_once(DeckId::A, crate::phase_sync::Alignment::Beat)
            .is_err());
        assert!(!engine.snapshot(DeckId::A).playing);
    }
    #[test]
    fn tempo_matching_uses_manual_grids_effective_rate_and_rejects_invalid_state() {
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        assert!(engine.tempo_match_factor(DeckId::A).is_err());
        let a = wav_file(4096, 192000);
        let b = crate::test_support::wav_samples(&vec![4096; 176400], 44100, 1);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        let grid = |bpm| Some(crate::beatgrid::BeatGrid::new(bpm, Duration::ZERO).unwrap());
        engine.set_grid(DeckId::A, grid(120.)).unwrap();
        assert!(engine.tempo_match_factor(DeckId::A).is_err());
        engine.set_grid(DeckId::B, grid(127.)).unwrap();
        engine.set_speed(DeckId::B, 1.03123456).unwrap();
        let expected_bpm = 127. * engine.snapshot(DeckId::B).effective_speed;
        let factor = engine.tempo_match_factor(DeckId::A).unwrap();
        assert_eq!(factor, expected_bpm / 120.);
        engine.set_speed(DeckId::A, factor).unwrap();
        assert!(
            (120. * engine.snapshot(DeckId::A).effective_speed - expected_bpm).abs()
                <= 120. / 48000. / 2. + 1e-10
        );
        engine.play(DeckId::B);
        engine.set_nudge(DeckId::B, 1).unwrap();
        assert!(engine.tempo_match_factor(DeckId::A).is_err());
        assert!(engine.tempo_match_factor(DeckId::B).is_err());
        engine.set_nudge(DeckId::B, 0).unwrap();
        engine.set_grid(DeckId::A, grid(40.)).unwrap();
        engine.set_grid(DeckId::B, grid(240.)).unwrap();
        let before = engine.snapshot(DeckId::A).requested_speed;
        assert!(engine.tempo_match_factor(DeckId::A).is_err());
        assert!(engine.tempo_match_factor(DeckId::B).is_err());
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, before);
        engine.set_grid(DeckId::A, grid(120.)).unwrap();
        engine.set_grid(DeckId::B, grid(240.)).unwrap();
        engine.set_speed(DeckId::B, 1.).unwrap();
        assert_eq!(engine.tempo_match_factor(DeckId::A).unwrap(), 2.);
        engine.set_speed(DeckId::A, 1.).unwrap();
        assert_eq!(engine.tempo_match_factor(DeckId::B).unwrap(), 0.5);
    }
    #[test]
    fn loop_enable_preserves_inside_phase_reloop_restarts_and_both_preserve_transport() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        let samples: Vec<i16> = (0..192000).map(|i| (i % 10000) as i16).collect();
        let a = crate::test_support::wav_samples(&samples, 48000, 1);
        let b = wav_file(4096, 192000);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        engine.set_levels(MixerLevels {
            crossfader: 0.,
            channel_a: 1.,
            channel_b: 1.,
        });
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        engine
            .restore_loop(DeckId::A, Duration::from_secs(1), Duration::from_secs(3))
            .unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_secs(2))
            .unwrap();
        let before = engine.snapshot(DeckId::A).position;
        let volumes = engine.output_volumes();
        engine.activate_loop(DeckId::A).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, before);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!((output.next().unwrap() - 6000. / 32768.).abs() < 1e-6);
        engine.reloop(DeckId::A).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));
        assert!((output.next().unwrap() - 8000. / 32768.).abs() < 1e-6);
        engine.pause(DeckId::A);
        engine
            .seek_position(DeckId::A, Duration::from_millis(2250))
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        assert_eq!(
            engine.snapshot(DeckId::A).position,
            Duration::from_millis(2250)
        );
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.reloop(DeckId::A).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.seek_percent(DeckId::A, 100.).unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        assert!(
            !engine.snapshot(DeckId::A).playing,
            "enabling at EOF must not resume"
        );
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.output_volumes(), volumes);
    }
    #[test]
    fn pcm_decode_reserves_bounded_memory_and_shared_transfer_avoids_a_second_buffer() {
        let cancel = crate::cancellation::Cancellation::default();
        let mut source = None;
        let stats = crate::looping::measured_heap(|| {
            let pcm = collect_pcm(
                std::iter::repeat_n(0.25, 1000000),
                1000000,
                2000000,
                &cancel,
            )
            .unwrap();
            source = Some(PcmSource::new(1, 48000, pcm));
        });
        assert_eq!(source.unwrap().frames(), 1000000);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.allocations, 2);
        assert!(
            stats.peak_bytes >= 4000000 && stats.peak_bytes <= 4001024,
            "{stats:?}"
        );
        assert_eq!(stats.deallocations, 0);
        let small = collect_pcm(std::iter::repeat_n(0.5, 3), usize::MAX, 10, &cancel).unwrap();
        assert_eq!(small.len(), 3);
        assert!(small.capacity() <= 10);
        assert!(collect_pcm(std::iter::repeat_n(0.5, 11), 0, 10, &cancel).is_err());
        println!("isolated1M-sample decode/shared ownership: {stats:?}");
    }
    #[test]
    fn cancelled_decode_stops_midstream_and_never_opens_pre_cancelled_paths() {
        let cancel = crate::cancellation::Cancellation::default();
        let consumed = Cell::new(0);
        let iterator = (0..100000).map(|i| {
            consumed.set(i + 1);
            if i == 5000 {
                cancel.request();
            }
            0.25
        });
        assert!(collect_pcm(iterator, 0, 100000, &cancel)
            .unwrap_err()
            .contains("cancelled"));
        assert!((5001..=9096).contains(&consumed.get()));
        assert!(
            PreparedTrack::open_cancellable(Path::new("does-not-exist.wav"), &cancel)
                .err()
                .unwrap()
                .contains("cancelled")
        );
    }
    #[test]
    fn cli_completion_wait_uses_render_eof_while_installed_queues_remain_alive() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        engine.wait_for_end(); // No loaded decks must not block.
        let a = wav_file(8192, 480);
        let b = wav_file(4096, 960);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        engine.set_speed(DeckId::A, 2.).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let returned = Arc::new(AtomicBool::new(false));
        let observe = Arc::clone(&returned);
        let (done, wait) = std::sync::mpsc::channel();
        let audio = std::thread::spawn(move || {
            for _ in 0..240 {
                output.next();
            }
            assert!(!observe.load(Ordering::Acquire), "B is still rendering");
            for _ in 0..2000 {
                output.next();
            }
            // Keep the output graph alive until wait returns; a Sink EOF wait would time out.
            let completed = wait.recv_timeout(Duration::from_secs(5)).is_ok();
            if completed {
                Some(output)
            } else {
                drop(output);
                None
            }
        });
        engine.wait_for_end();
        returned.store(true, Ordering::Release);
        for id in [DeckId::A, DeckId::B] {
            let state = engine.snapshot(id);
            assert_eq!(state.position, state.duration);
            assert!(!state.playing);
            assert!(!engine.decks[id.index()].borrow().sink.empty());
        }
        done.send(()).unwrap();
        let output = audio.join().unwrap();
        assert!(output.is_some(), "completion depended on queue destruction");
        drop(output);
    }
    #[test]
    fn loop_commands_preserve_nonintegral_frame_and_fractional_render_phase() {
        fn track(rate: u32) -> PreparedTrack {
            let samples: Vec<_> = (0..1024).map(|i| i as f32 / 1000.).collect();
            PreparedTrack {
                path: Arc::from(Path::new("frame-loop.wav")),
                title: "frame loop".into(),
                overview: crate::waveform::peak_overview(&samples, 1).into(),
                detail: Arc::new(crate::waveform::DetailPeaks::new(&samples, 1, rate)),
                tempo: None,
                samples: PcmSource::new(1, rate, samples),
            }
        }
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        engine.install(DeckId::A, track(48000));
        engine.set_levels(MixerLevels {
            crossfader: 0.,
            channel_a: 1.,
            channel_b: 1.,
        });
        engine.play(DeckId::A);
        output.next();
        output.next();
        let at_two = engine.snapshot(DeckId::A).position;
        assert_eq!(at_two, frame_time(2, 48000));
        engine.set_loop_in(DeckId::A, at_two).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).loop_in, Some(at_two));
        output.next();
        engine
            .set_loop_out(DeckId::A, engine.snapshot(DeckId::A).position)
            .unwrap();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_out,
            Some(frame_time(3, 48000))
        );
        engine
            .restore_loop(DeckId::A, Duration::ZERO, frame_time(8, 48000))
            .unwrap();
        engine.reloop(DeckId::A).unwrap();
        for _ in 0..2 {
            output.next();
        }
        engine.resize_loop(DeckId::A, false).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, at_two);
        assert!((output.next().unwrap() - 0.002).abs() < 1e-8);
        engine.resize_loop(DeckId::A, true).unwrap();
        engine.reloop(DeckId::A).unwrap();
        for _ in 0..6 {
            output.next();
        }
        engine.resize_loop(DeckId::A, false).unwrap();
        engine.resize_loop(DeckId::A, true).unwrap();
        assert_eq!(
            engine.snapshot(DeckId::A).position,
            at_two,
            "back-to-back controls retain mapped pending phase"
        );
        assert!((output.next().unwrap() - 0.002).abs() < 1e-8);
        engine.install(DeckId::A, track(44100));
        engine
            .restore_loop(DeckId::A, Duration::ZERO, frame_time(128, 44100))
            .unwrap();
        engine.reloop(DeckId::A).unwrap();
        engine.play(DeckId::A);
        for _ in 0..37 {
            output.next();
        }
        let fractional = engine.snapshot(DeckId::A).position;
        engine.resize_loop(DeckId::A, false).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, fractional);
        let expected = 37. * 44100. / 48000. / 1000.;
        assert!(
            (f64::from(output.next().unwrap()) - expected).abs() < 1e-8,
            "resize must not floor fractional source frame"
        );
        let after = engine.snapshot(DeckId::A).position;
        engine.exit_loop(DeckId::A).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, after);
        engine.pause(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).position, after);
        for _ in 0..1000 {
            output.next();
        }
        assert_eq!(engine.snapshot(DeckId::A).position, after);
    }
    #[test]
    fn short_nudge_flanks_preserve_rendered_ramp_and_exact_output_frame_phase() {
        for (source_rate, output_rate, channels) in
            [(44100, 48000, 2), (48000, 48000, 1), (96000, 44100, 2)]
        {
            let (mixer, mut output) = rodio::mixer::mixer(channels, output_rate);
            let engine = DualDeckEngine::from_mixer(mixer, channels, output_rate);
            let samples: Vec<_> = (0..source_rate)
                .flat_map(|frame| {
                    let v = frame as f32 / 100000.;
                    [v, -v]
                })
                .collect();
            engine.install(
                DeckId::A,
                PreparedTrack {
                    path: Arc::from(Path::new("ramp-nudge.wav")),
                    title: "ramp".into(),
                    overview: crate::waveform::peak_overview(&samples, 2).into(),
                    detail: Arc::new(crate::waveform::DetailPeaks::new(&samples, 2, source_rate)),
                    tempo: None,
                    samples: PcmSource::new(2, source_rate, samples),
                },
            );
            engine.set_levels(MixerLevels {
                crossfader: 0.,
                channel_a: 1.,
                channel_b: 1.,
            });
            engine.play(DeckId::A);
            let installed = Arc::clone(engine.decks[0].borrow().render.as_ref().unwrap());
            let mut phase = 0u128;
            for step in 0..50 {
                let before = engine.snapshot(DeckId::A).position;
                engine
                    .set_nudge(DeckId::A, [1, 0, -1, 0][step % 4])
                    .unwrap();
                if step == 25 {
                    engine.set_speed(DeckId::A, 1.1).unwrap();
                }
                assert_eq!(engine.snapshot(DeckId::A).position, before);
                let rate = engine.decks[0].borrow().speed.unwrap().output_rate().get();
                for _ in 0..37 {
                    let expected = (phase as f64 / f64::from(output_rate) / 100000.) as f32;
                    let left = output.next().unwrap();
                    assert!(
                        (left - expected).abs() < 0.0000002,
                        "source{source_rate} output{output_rate} step{step}: {left} vs {expected}"
                    );
                    if channels == 2 {
                        assert!((output.next().unwrap() + left).abs() < 0.0000002);
                    }
                    phase += u128::from(rate);
                    let expected_ns = (phase * 1_000_000_000)
                        .div_ceil(u128::from(source_rate) * u128::from(output_rate))
                        as u64;
                    assert_eq!(
                        engine.snapshot(DeckId::A).position,
                        Duration::from_nanos(expected_ns)
                    );
                }
                assert!(Arc::ptr_eq(
                    &installed,
                    engine.decks[0].borrow().render.as_ref().unwrap()
                ));
                assert!(
                    engine.retired.borrow().is_empty(),
                    "rate gestures must not replace source/sink"
                );
            }
            engine.set_nudge(DeckId::A, 0).unwrap();
            assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.1);
        }
    }
    #[test]
    fn replaced_pcm_survives_audio_queue_drop_until_control_plane_reclamation() {
        for warmup in [0, 1000] {
            let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
            let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
            let file = wav_file(8192, 48000);
            let next = wav_file(4096, 48000);
            let track = PreparedTrack::open(file.path()).unwrap();
            let storage = track.samples.storage_weak();
            engine.install(DeckId::A, track);
            engine.play(DeckId::A);
            for _ in 0..warmup {
                output.next();
            }
            engine.load_a(next.path()).unwrap();
            engine.collect_retired();
            assert_eq!(engine.retired.borrow().len(), 1);
            assert!(storage.strong_count() >= 2);
            let observe = storage.clone();
            let audio = std::thread::spawn(move || {
                for _ in 0..1000 {
                    output.next();
                }
                // Destruction of the mixer also exercises retirement of a still queued source.
                drop(output);
                assert_eq!(
                    observe.strong_count(),
                    1,
                    "control-plane owner must retain PCM after audio queue destruction"
                );
                std::thread::current().id()
            });
            assert_ne!(audio.join().unwrap(), std::thread::current().id());
            assert_eq!(storage.strong_count(), 1);
            assert!(engine.retired.borrow()[0].released.load(Ordering::Acquire));
            engine.collect_retired();
            assert_eq!(storage.strong_count(), 0);
            assert!(engine.retired.borrow().is_empty());
        }
    }
    #[test]
    fn nudge_release_tracks_latest_base_and_transport_cancels_without_state_loss() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        engine.set_nudge(DeckId::A, 0).unwrap();
        assert!(engine.set_nudge(DeckId::A, 1).is_err());
        let file = wav_file(8192, 48000 * 8);
        let other = wav_file(4096, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        engine.set_speed(DeckId::A, 1.1).unwrap();
        assert!(engine.set_nudge(DeckId::A, 1).is_err());
        let grid = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        engine.set_levels(MixerLevels {
            channel_a: 0.6,
            channel_b: 0.8,
            crossfader: 0.3,
        });
        let volumes = engine.output_volumes();
        engine
            .restore_loop(
                DeckId::A,
                Duration::from_millis(500),
                Duration::from_millis(1500),
            )
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        engine.set_nudge(DeckId::A, 1).unwrap();
        let active = engine.snapshot(DeckId::A);
        assert_eq!(active.requested_speed, 1.1);
        assert_eq!(active.nudge_direction, 1);
        assert!((active.effective_speed - 1.122).abs() < 1e-9);
        assert!(active.loop_active && active.playing);
        output.by_ref().take(12000).for_each(drop);
        assert!((engine.snapshot(DeckId::A).position.as_secs_f64() - 0.7805).abs() < 0.025);
        let phase = engine.snapshot(DeckId::A).position;
        engine.set_speed(DeckId::A, 1.2).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, phase);
        assert!((engine.snapshot(DeckId::A).effective_speed - 1.224).abs() < 1e-9);
        let before = engine.snapshot(DeckId::A);
        assert!(engine.set_speed(DeckId::A, 2.).is_err());
        assert!(engine.set_nudge(DeckId::A, 2).is_err());
        assert_eq!(
            engine.snapshot(DeckId::A).requested_speed,
            before.requested_speed
        );
        assert_eq!(engine.snapshot(DeckId::A).position, before.position);
        engine.set_nudge(DeckId::A, 0).unwrap();
        let released = engine.snapshot(DeckId::A);
        assert_eq!(released.requested_speed, 1.2);
        assert_eq!(released.effective_speed, 1.2);
        assert_eq!(released.position, phase);
        assert!(released.loop_active && released.playing);
        engine.set_nudge(DeckId::A, -1).unwrap();
        engine.pause(DeckId::A);
        let paused = engine.snapshot(DeckId::A);
        assert_eq!(paused.nudge_direction, 0);
        assert_eq!(paused.effective_speed, 1.2);
        assert!(paused.loop_active && !paused.playing);
        engine.play(DeckId::A);
        engine.set_nudge(DeckId::A, -1).unwrap();
        engine.seek_percent(DeckId::A, 50.).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(4));
        engine.set_nudge(DeckId::A, 1).unwrap();
        engine.cue(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.2);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.play(DeckId::A);
        engine.set_nudge(DeckId::A, -1).unwrap();
        assert!(engine.load_a(Path::new("missing-nudge.wav")).is_err());
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, -1);
        engine.load_a(other.path()).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).requested_speed, 1.);
        assert_eq!(engine.output_volumes(), volumes);
        engine.seek_percent(DeckId::A, 99.).unwrap();
        engine.play(DeckId::A);
        engine.set_nudge(DeckId::A, 1).unwrap();
        output.by_ref().take(24000).for_each(drop);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.set_nudge(DeckId::A, 0).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(8));
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.play(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
    }
    #[test]
    fn nudge_steady_signal_pitch_returns_to_updated_base_on_release() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 2, 48000);
        let samples: Vec<_> = (0..96000)
            .flat_map(|frame| {
                let value = 0.25 * (std::f32::consts::TAU * 440. * frame as f32 / 48000.).sin();
                [value, -value]
            })
            .collect();
        engine.install(
            DeckId::A,
            PreparedTrack {
                path: Arc::from(Path::new("nudge-tone.wav")),
                title: "nudge tone".into(),
                overview: crate::waveform::peak_overview(&samples, 2).into(),
                detail: Arc::new(crate::waveform::DetailPeaks::new(&samples, 2, 48000)),
                tempo: None,
                samples: PcmSource::new(2, 48000, samples),
            },
        );
        engine.set_levels(MixerLevels {
            channel_a: 1.,
            channel_b: 1.,
            crossfader: 0.,
        });
        engine.play(DeckId::A);
        for (direction, base) in [(1, 1.), (0, 1.), (-1, 1.), (1, 1.05), (0, 1.05)] {
            engine.set_speed(DeckId::A, base).unwrap();
            engine.set_nudge(DeckId::A, direction).unwrap();
            output.by_ref().take(8192).for_each(drop);
            let rendered: Vec<_> = output.by_ref().take(19200).collect();
            assert_eq!(rendered.len(), 19200);
            let left: Vec<_> = rendered
                .as_chunks::<2>()
                .0
                .iter()
                .map(|frame| {
                    assert!((frame[0] + frame[1]).abs() < 1e-5);
                    frame[0]
                })
                .collect();
            let crossings = left.windows(2).filter(|w| w[0] <= 0. && w[1] > 0.).count();
            let measured = crossings as f64 * 48000. / left.len() as f64;
            let expected = 440. * engine.snapshot(DeckId::A).effective_speed;
            assert!(
                (measured - expected).abs() <= 5.1,
                "direction{direction}, base{base}, measured{measured}"
            );
            assert_eq!(engine.snapshot(DeckId::A).requested_speed, base);
        }
    }
    #[test]
    fn changed_rate_loop_wraps_and_resizes_render_only_region_samples_after_settling() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 2, 48000);
        let samples: Vec<_> = (0..48000)
            .flat_map(|frame| {
                let value = if (12000..24000).contains(&frame) {
                    0.4
                } else {
                    0.8
                };
                [value, -value]
            })
            .collect();
        engine.install(
            DeckId::A,
            PreparedTrack {
                path: Arc::from(Path::new("speed-loop.wav")),
                title: "speed loop".into(),
                overview: crate::waveform::peak_overview(&samples, 2).into(),
                detail: Arc::new(crate::waveform::DetailPeaks::new(&samples, 2, 48000)),
                tempo: None,
                samples: PcmSource::new(2, 48000, samples),
            },
        );
        engine.set_levels(MixerLevels {
            channel_a: 0.5,
            channel_b: 1.,
            crossfader: 0.,
        });
        engine
            .restore_loop(
                DeckId::A,
                Duration::from_millis(250),
                Duration::from_millis(500),
            )
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        engine.set_speed(DeckId::A, 1.5).unwrap();
        engine.play(DeckId::A);
        for stage in 0..3 {
            if stage == 1 {
                engine.resize_loop(DeckId::A, false).unwrap();
            }
            if stage == 2 {
                engine.set_speed(DeckId::A, 0.75).unwrap();
            }
            // Control-plane replacement has an existing queue/control settling interval.
            // Steady-state assertions deliberately do not qualify the transition itself.
            output.by_ref().take(8192).for_each(drop);
            let rendered: Vec<_> = output.by_ref().take(48000 * 2 * 2).collect();
            assert_eq!(rendered.len(), 48000 * 2 * 2);
            for frame in rendered.as_chunks::<2>().0 {
                assert!((frame[0] - 0.2).abs() < 1e-6, "stage{stage}: {frame:?}");
                assert!((frame[1] + 0.2).abs() < 1e-6);
            }
            let snapshot = engine.snapshot(DeckId::A);
            assert!(snapshot.playing && snapshot.loop_active);
            assert!((snapshot.loop_in.unwrap()..snapshot.loop_out.unwrap())
                .contains(&snapshot.position));
        }
    }
    #[test]
    fn speed_offline_render_measures_pitch_duration_stereo_and_source_time() {
        for (base, factor) in [
            (48000, 0.5),
            (48000, 1.),
            (48000, 1.25),
            (48000, 2.),
            (44100, 1.333333),
        ] {
            let effective = (f64::from(base) * factor).round() / f64::from(base);
            let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
            let engine = DualDeckEngine::from_mixer(mixer, 2, 48000);
            let samples: Vec<_> = (0..base)
                .flat_map(|frame| {
                    let value =
                        0.25 * (std::f32::consts::TAU * 440. * frame as f32 / base as f32).sin();
                    [value, -value]
                })
                .collect();
            engine.install(
                DeckId::A,
                PreparedTrack {
                    path: Arc::from(Path::new("speed-tone.wav")),
                    title: "speed tone".into(),
                    overview: crate::waveform::peak_overview(&samples, 2).into(),
                    detail: Arc::new(crate::waveform::DetailPeaks::new(&samples, 2, base)),
                    tempo: None,
                    samples: PcmSource::new(2, base, samples),
                },
            );
            engine.set_levels(MixerLevels {
                channel_a: 1.,
                channel_b: 1.,
                crossfader: 0.,
            });
            engine.set_speed(DeckId::A, factor).unwrap();
            engine.play(DeckId::A);
            let mut rendered: Vec<_> = output.by_ref().take(19200).collect();

            let left: Vec<_> = rendered
                .as_chunks::<2>()
                .0
                .iter()
                .map(|frame| {
                    assert!((frame[0] + frame[1]).abs() < 1e-5);
                    frame[0]
                })
                .collect();
            let first_signal = left.iter().position(|sample| sample.abs() > 1e-5).unwrap();
            let signal = &left[first_signal..];
            let expected_source_time = signal.len() as f64 / 48000. * effective;
            // Sink publishes its counter every 5 ms; compare source time after measured
            // queue lead-in, allowing that control tick and two source-frame roundings.
            let position = engine.snapshot(DeckId::A).position.as_secs_f64();
            assert!(
                (position - expected_source_time).abs() <= 0.005 * effective + 2. / f64::from(base),
                "factor {factor}: source time {position}, expected near {expected_source_time}"
            );
            let crossings = signal
                .windows(2)
                .filter(|w| w[0] <= 0. && w[1] > 0.)
                .count();
            let frequency = crossings as f64 * 48000. / signal.len() as f64;
            // A finite zero-crossing window has one-crossing frequency resolution.
            let resolution = 48000. / signal.len() as f64;
            assert!(
                (frequency - 440. * effective).abs() <= resolution + 0.01,
                "factor {factor}, measured {frequency} Hz"
            );
            rendered.extend(output.by_ref().take(48000 * 6));
            let last = rendered
                .as_chunks::<2>()
                .0
                .iter()
                .rposition(|frame| frame[0].abs() > 1e-5)
                .unwrap()
                + 1;
            let first = rendered
                .as_chunks::<2>()
                .0
                .iter()
                .position(|frame| frame[0].abs() > 1e-5)
                .unwrap();
            let span = last - first;
            assert!(
                (span as f64 - 48000. / effective).abs() <= 4.,
                "factor {factor}, signal span {span}, leading silent frames {first}"
            );
            eprintln!(
                "source {base} Hz speed {factor} (effective {effective}): leading silence {first} output frames, signal span {span} frames"
            );
            assert!(!engine.snapshot(DeckId::A).playing);
            assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));
        }
    }
    #[test]
    fn speed_controls_preserve_loop_phase_transport_grid_gains_and_reset_on_new_track() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        assert!(engine.set_speed(DeckId::A, 1.25).is_err());
        let file = wav_file(8192, 48000 * 8);
        let other = wav_file(4096, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        let grid = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        engine.set_levels(MixerLevels {
            channel_a: 0.6,
            channel_b: 0.8,
            crossfader: 0.3,
        });
        let volumes = engine.output_volumes();
        engine
            .seek_position(DeckId::A, Duration::from_secs(2))
            .unwrap();
        engine.set_speed(DeckId::A, 1.25).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(2));
        assert!(!engine.snapshot(DeckId::A).playing);
        engine
            .restore_loop(
                DeckId::A,
                Duration::from_millis(500),
                Duration::from_millis(1500),
            )
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        output.by_ref().take(24000).for_each(drop);
        let before = engine.snapshot(DeckId::A);
        assert!((before.position.as_secs_f64() - 1.125).abs() < 0.02);
        engine.set_speed(DeckId::A, 0.5).unwrap();
        let after = engine.snapshot(DeckId::A);
        assert_eq!(after.position, before.position);
        assert_eq!(after.loop_out, before.loop_out);
        assert!(after.playing && after.loop_active);
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(grid));
        assert_eq!(engine.output_volumes(), volumes);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).requested_speed, 1.);
        for value in [f64::NAN, f64::INFINITY, 0., 0.49, 2.01] {
            assert!(engine.set_speed(DeckId::A, value).is_err());
            assert_eq!(engine.snapshot(DeckId::A).position, after.position);
            assert_eq!(engine.snapshot(DeckId::A).requested_speed, 0.5);
        }
        engine.pause(DeckId::A);
        let paused = engine.snapshot(DeckId::A).position;
        output.by_ref().take(4096).for_each(drop);
        assert_eq!(engine.snapshot(DeckId::A).position, paused);
        engine.seek_percent(DeckId::A, 50.).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(4));
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 0.5);
        engine.cue(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 0.5);
        assert!(engine.load_a(Path::new("missing-speed.wav")).is_err());
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 0.5);
        engine.load_a(other.path()).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert_eq!(engine.output_volumes(), volumes);
    }
    #[test]
    fn aligned_auto_loop_is_optional_and_validated_before_active_state_changes() {
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        let file = wav_file(8192, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        assert!(engine.auto_loop(DeckId::A, 8, true).is_err());
        let grid = crate::beatgrid::BeatGrid::new(120., Duration::from_millis(250)).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_millis(850))
            .unwrap();
        engine.auto_loop(DeckId::A, 8, false).unwrap();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_in,
            Some(Duration::from_millis(850))
        );
        engine
            .seek_position(DeckId::A, Duration::from_millis(850))
            .unwrap();
        engine.auto_loop(DeckId::A, 8, true).unwrap();
        let snapshot = engine.snapshot(DeckId::A);
        assert_eq!(snapshot.loop_in, Some(Duration::from_millis(750)));
        assert_eq!(snapshot.loop_out, Some(Duration::from_millis(1250)));
        assert!(snapshot.loop_active);
        assert!(!snapshot.playing);
        let volumes = engine.output_volumes();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        engine
            .restore_loop(
                DeckId::A,
                Duration::from_millis(7400),
                Duration::from_millis(7900),
            )
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        let before = engine.snapshot(DeckId::A);
        assert!(engine.auto_loop(DeckId::A, 32, true).is_err());
        let after = engine.snapshot(DeckId::A);
        assert_eq!(after.position, before.position);
        assert_eq!(after.loop_in, before.loop_in);
        assert_eq!(after.loop_out, before.loop_out);
        assert!(after.playing && after.loop_active);
        assert_eq!(after.grid, Some(grid));
        assert_eq!(engine.output_volumes(), volumes);
        assert!(engine.snapshot(DeckId::B).playing);
        assert!(engine.snapshot(DeckId::B).loop_in.is_none());
        engine.seek_percent(DeckId::A, 100.).unwrap();
        assert!(engine.auto_loop(DeckId::A, 1, true).is_err());
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(8));
    }
    #[test]
    fn auto_loop_resize_preserves_play_state_phase_grid_gains_and_other_deck() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48000);
        assert!(engine.auto_loop(DeckId::A, 32, false).is_err());
        let file = wav_file(8192, 48000 * 12);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        assert!(engine.auto_loop(DeckId::A, 32, false).is_err());
        let grid = crate::beatgrid::BeatGrid::new(120., Duration::from_millis(250)).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_millis(1250))
            .unwrap();
        engine.auto_loop(DeckId::A, 32, false).unwrap();
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(
            engine.snapshot(DeckId::A).loop_in,
            Some(Duration::from_millis(1250))
        );
        assert_eq!(
            engine.snapshot(DeckId::A).loop_out,
            Some(Duration::from_millis(3250))
        );
        engine.set_levels(MixerLevels {
            channel_a: 0.6,
            channel_b: 0.8,
            crossfader: 0.3,
        });
        let volumes = engine.output_volumes();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        output.by_ref().take(48000 * 3 / 2).for_each(drop);
        let before_half = engine.snapshot(DeckId::A).position;
        let start = Duration::from_millis(1250);
        assert!(before_half > Duration::from_millis(2250));
        let expected =
            start + Duration::from_nanos(((before_half - start).as_nanos() % 1_000_000_000) as u64);
        engine.resize_loop(DeckId::A, false).unwrap();
        let snapshot = engine.snapshot(DeckId::A);
        assert!(snapshot.playing && snapshot.loop_active);
        assert!(
            (Duration::from_millis(1250)..Duration::from_millis(2250)).contains(&snapshot.position)
        );
        assert_eq!(
            snapshot.position, expected,
            "Halving must wrap phase, not reset to In"
        );
        assert_ne!(snapshot.position, start);
        engine.resize_loop(DeckId::A, true).unwrap();
        assert_eq!(
            engine.snapshot(DeckId::A).position,
            expected,
            "Doubling must retain the mapped phase"
        );
        assert_eq!(
            engine.snapshot(DeckId::A).loop_out,
            Some(Duration::from_millis(3250))
        );
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(grid));
        assert_eq!(engine.output_volumes(), volumes);
        assert!(engine.snapshot(DeckId::B).playing);
        assert!(engine.snapshot(DeckId::B).loop_in.is_none());
        let before = engine.snapshot(DeckId::A);
        assert!(engine.auto_loop(DeckId::A, 256, false).is_err());
        let after = engine.snapshot(DeckId::A);
        assert_eq!(before.loop_out, after.loop_out);
        assert_eq!(before.position, after.position);
        assert!(after.loop_active);
        engine.exit_loop(DeckId::A).unwrap();
        let before = engine.snapshot(DeckId::A).position;
        engine.resize_loop(DeckId::A, false).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, before);
        assert!(!engine.snapshot(DeckId::A).loop_active);
        engine.pause(DeckId::A);
        engine
            .restore_loop(
                DeckId::A,
                Duration::ZERO,
                crate::looping::frame_time(1, 48000),
            )
            .unwrap();
        assert!(engine.resize_loop(DeckId::A, false).is_err());
    }
    #[test]
    fn saved_bounds_validation_is_atomic_and_frame_aligned_eof_round_trips() {
        let (mixer, _output) = rodio::mixer::mixer(1, 44100);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 44100);
        assert!(engine
            .restore_loop(DeckId::A, Duration::ZERO, Duration::from_secs(1))
            .is_err());
        let file = wav_file(8192, 44101);
        engine.load_a(file.path()).unwrap();
        let end = engine.snapshot(DeckId::A).duration;
        engine.restore_loop(DeckId::A, Duration::ZERO, end).unwrap();
        let saved = engine.snapshot(DeckId::A);
        assert_eq!(saved.loop_out, Some(end));
        engine
            .restore_loop(DeckId::A, saved.loop_in.unwrap(), saved.loop_out.unwrap())
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        engine.play(DeckId::A);
        for (start, end) in [
            (Duration::ZERO, end + Duration::from_nanos(1)),
            (Duration::from_nanos(1), Duration::from_nanos(2)),
            (end, Duration::ZERO),
        ] {
            assert!(engine.restore_loop(DeckId::A, start, end).is_err());
            assert!(engine.snapshot(DeckId::A).loop_active);
            assert_eq!(engine.snapshot(DeckId::A).loop_in, saved.loop_in);
        }
        engine.restore_loop(DeckId::A, Duration::ZERO, end).unwrap();
        assert!(!engine.snapshot(DeckId::A).loop_active);
        assert!(engine.snapshot(DeckId::A).playing);
    }
    #[test]
    fn loop_lifecycle_offline_audio_position_exit_seek_and_track_replacement() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48_000);
        let engine = DualDeckEngine::from_mixer(mixer, 2, 48_000);
        assert!(engine.set_loop_in(DeckId::A, Duration::ZERO).is_err());
        let samples: Vec<_> = (0..48_000)
            .flat_map(|frame| {
                let v = if frame < 12_000 {
                    0.1
                } else if frame < 24_000 {
                    0.4
                } else {
                    0.8
                };
                [v, -v]
            })
            .collect();
        engine.install(
            DeckId::A,
            PreparedTrack {
                path: Arc::from(Path::new("loop-test.wav")),
                title: "loop test".into(),
                overview: crate::waveform::peak_overview(&samples, 2).into(),
                detail: Arc::new(crate::waveform::DetailPeaks::new(&samples, 2, 48_000)),
                tempo: None,
                samples: PcmSource::new(2, 48_000, samples),
            },
        );
        let other = wav_file(4096, 48_000 * 4);
        engine.load_b(other.path()).unwrap();
        engine.set_levels(MixerLevels {
            crossfader: 0.,
            channel_a: 0.5,
            channel_b: 0.8,
        });
        let volumes = engine.output_volumes();
        let grid = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        assert!(engine.activate_loop(DeckId::A).is_err());
        assert!(engine
            .set_loop_out(DeckId::A, Duration::from_millis(500))
            .is_err());
        engine
            .set_loop_in(DeckId::A, Duration::from_millis(250))
            .unwrap();
        assert!(engine
            .set_loop_out(DeckId::A, Duration::from_millis(250))
            .is_err());
        assert!(engine
            .set_loop_out(DeckId::A, Duration::from_secs(2))
            .is_err());
        engine
            .set_loop_out(DeckId::A, Duration::from_millis(500))
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(
            engine.snapshot(DeckId::A).position,
            Duration::from_millis(250)
        );
        engine.play(DeckId::A);
        for _ in 0..5 {
            // More than the entire one-second track: every emitted frame must stay in the loop.
            for frame in output
                .by_ref()
                .take(48_000)
                .collect::<Vec<_>>()
                .as_chunks::<2>()
                .0
            {
                assert!((frame[0] - 0.2).abs() < 1e-6);
                assert!((frame[1] + 0.2).abs() < 1e-6);
            }
            let position = engine.snapshot(DeckId::A).position;
            assert!((Duration::from_millis(250)..Duration::from_millis(500)).contains(&position));
        }
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(grid));
        assert_eq!(engine.output_volumes(), volumes);
        assert_eq!(engine.snapshot(DeckId::B).position, Duration::ZERO);
        engine.pause(DeckId::A);
        let paused = engine.snapshot(DeckId::A).position;
        output.by_ref().take(4096).for_each(drop);
        assert_eq!(engine.snapshot(DeckId::A).position, paused);
        assert!(engine
            .load_a(Path::new("missing-loop-fixture.wav"))
            .is_err());
        assert!(engine.decks[0].borrow().loop_active);
        engine.play(DeckId::A);
        let before = engine.snapshot(DeckId::A).position;
        engine.exit_loop(DeckId::A).unwrap();
        assert!(!engine.decks[0].borrow().loop_active);
        assert_eq!(engine.snapshot(DeckId::A).position, before);
        assert!(engine.snapshot(DeckId::A).playing);
        output.by_ref().take(96_000).for_each(drop);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.activate_loop(DeckId::A).unwrap();
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.seek_percent(DeckId::A, 75.).unwrap();
        assert!(!engine.decks[0].borrow().loop_active);
        engine.activate_loop(DeckId::A).unwrap();
        engine.cue(DeckId::A);
        assert!(!engine.decks[0].borrow().loop_active);
        assert!(engine.decks[0].borrow().loop_region.is_some());
        engine.activate_loop(DeckId::A).unwrap();
        let original = engine.decks[0].borrow().loop_region;
        assert!(engine
            .set_loop_in(DeckId::A, Duration::from_secs(1))
            .is_err());
        assert_eq!(engine.decks[0].borrow().loop_region, original);
        assert!(engine.decks[0].borrow().loop_active);
        engine.set_loop_in(DeckId::A, Duration::ZERO).unwrap();
        assert!(engine.decks[0].borrow().loop_region.is_none());
        engine
            .set_loop_out(DeckId::A, Duration::from_secs(1))
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        engine.load_a(other.path()).unwrap();
        assert!(engine.decks[0].borrow().loop_region.is_none());
        assert!(!engine.decks[0].borrow().loop_active);
    }
    #[test]
    fn absolute_cue_seek_preserves_transport_grid_other_deck_and_rejects_outside_source() {
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48_000);
        assert!(engine.seek_position(DeckId::A, Duration::ZERO).is_err());
        let file = wav_file(8192, 48_000 * 4);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        engine.set_levels(MixerLevels {
            crossfader: 0.3,
            channel_a: 0.6,
            channel_b: 0.8,
        });
        let gains = engine.output_volumes();
        let grid = crate::beatgrid::BeatGrid::new(128.5, Duration::ZERO).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        let target = Duration::new(1, 123456789);
        engine.seek_position(DeckId::A, target).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, target);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        engine.seek_position(DeckId::A, target).unwrap();
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).position, Duration::ZERO);
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(grid));
        assert_eq!(engine.output_volumes(), gains);
        assert!(engine
            .seek_position(DeckId::A, Duration::from_secs(5))
            .is_err());
        assert_eq!(engine.snapshot(DeckId::A).position, target);
        assert!(engine.snapshot(DeckId::A).playing);
    }
    #[test]
    fn manual_grid_belongs_to_its_track_and_survives_transport_but_not_reload() {
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48_000);
        assert!(engine.set_grid_here(DeckId::A, 120.0).is_err());
        let wav = wav_file(8192, 48_000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(wav.path()).unwrap();
        engine.seek_percent(DeckId::A, 50.0).unwrap();
        engine.set_grid_here(DeckId::A, 120.0).unwrap();
        engine.set_grid_here(DeckId::B, 90.0).unwrap();
        let left = engine.snapshot(DeckId::A).grid;
        let right = engine.snapshot(DeckId::B).grid;
        engine.play(DeckId::A);
        assert!(engine.set_grid_here(DeckId::A, f64::NAN).is_err());
        assert_eq!(engine.snapshot(DeckId::A).grid, left);
        assert!(engine.snapshot(DeckId::A).playing);
        engine.seek_percent(DeckId::A, 75.0).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).grid, left);
        engine.cue(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).grid, left);
        engine.seek_percent(DeckId::A, 100.0).unwrap();
        engine.play(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).grid, left);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
        assert!(engine.load_a(Path::new("missing.wav")).is_err());
        assert_eq!(engine.snapshot(DeckId::A).grid, left);
        engine.load_a(wav.path()).unwrap();
        assert!(engine.snapshot(DeckId::A).grid.is_none());
        assert_eq!(engine.snapshot(DeckId::B).grid, right);
        engine.clear_grid(DeckId::B);
        assert!(engine.snapshot(DeckId::B).grid.is_none());
    }
    #[test]
    fn seek_preserves_state_clamps_bounds_and_rejects_nonfinite_input() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48_000);
        assert!(engine.seek_percent(DeckId::A, 25.0).is_err());
        let wav = wav_file(8192, 48_000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(&fixture()).unwrap();
        engine.play(DeckId::B);
        let start = std::time::Instant::now();
        // No audio consumer is running: seek must complete without audio-thread feedback.
        engine.seek_percent(DeckId::A, 50.0).unwrap();
        eprintln!("control-plane PCM seek completed in {:?}", start.elapsed());
        assert_eq!(
            engine.snapshot(DeckId::A).position,
            Duration::from_millis(500)
        );
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).position, Duration::ZERO);
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(engine.seek_percent(DeckId::A, invalid).is_err());
            assert_eq!(engine.snapshot(DeckId::A).percent(), 50.0);
        }
        engine.seek_percent(DeckId::A, -25.0).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
        engine.play(DeckId::A);
        engine.seek_percent(DeckId::A, 125.0).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).percent(), 100.0);
        assert!(!engine.snapshot(DeckId::A).playing);
        // Play must restart even before an audio consumer observes the empty end source.
        engine.play(DeckId::A);
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).percent(), 0.0);
        output.by_ref().take(50_000).for_each(drop);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.seek_percent(DeckId::A, 25.0).unwrap();
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).percent(), 25.0);
        engine.cue(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
        engine.seek_percent(DeckId::A, 50.0).unwrap();
        engine.load_a(wav.path()).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
        engine.seek_percent(DeckId::B, 70.0).unwrap();
        assert!(engine.snapshot(DeckId::B).playing);
        assert!((engine.snapshot(DeckId::B).percent() - 70.0).abs() < 0.001);
    }

    #[test]
    fn seeking_changes_rendered_stereo_samples_and_keeps_gain_and_position_offset() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48_000);
        let engine = DualDeckEngine::from_mixer(mixer, 2, 48_000);
        let samples: Vec<_> = (0..48_000)
            .flat_map(|frame| {
                let value = if frame < 24_000 { 0.25 } else { 0.5 };
                [value, -value]
            })
            .collect();
        engine.install(
            DeckId::A,
            PreparedTrack {
                path: Arc::from(Path::new("synthetic-test.wav")),
                title: "stereo step".into(),
                overview: crate::waveform::peak_overview(&samples, 2).into(),
                detail: Arc::new(crate::waveform::DetailPeaks::new(&samples, 2, 48_000)),
                tempo: None,
                samples: PcmSource::new(2, 48_000, samples),
            },
        );
        engine.set_levels(MixerLevels {
            crossfader: 0.0,
            channel_a: 0.6,
            channel_b: 0.8,
        });
        engine.play(DeckId::A);
        engine.seek_percent(DeckId::A, 75.0).unwrap();
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(
            engine.snapshot(DeckId::A).position,
            Duration::from_millis(750)
        );
        let rendered: Vec<_> = output.by_ref().take(4800).collect();
        assert_eq!(rendered.len(), 4800);
        for frame in rendered.as_chunks::<2>().0 {
            assert!((frame[0] - 0.3).abs() < 1e-6, "frame: {frame:?}");
            assert!((frame[1] + 0.3).abs() < 1e-6, "frame: {frame:?}");
        }
        assert!((79.0..=81.0).contains(&engine.snapshot(DeckId::A).percent()));
        engine.seek_percent(DeckId::A, 25.0).unwrap();
        // Allow old sink controls to settle before inspecting the replacement source.
        output.by_ref().take(1000).for_each(drop);
        let rendered: Vec<_> = output.by_ref().take(1000).collect();
        for frame in rendered.as_chunks::<2>().0 {
            assert!((frame[0] - 0.15).abs() < 1e-6);
            assert!((frame[1] + 0.15).abs() < 1e-6);
        }
        engine.pause(DeckId::A);
        engine.seek_percent(DeckId::A, 60.0).unwrap();
        output.by_ref().take(2000).for_each(drop);
        let paused = engine.snapshot(DeckId::A).position;
        output.by_ref().take(2000).for_each(drop);
        assert_eq!(engine.snapshot(DeckId::A).position, paused);
        assert_eq!(paused, Duration::from_millis(600));
    }
    #[test]
    fn gains_are_linear_clamped_and_finite() {
        for (x, expected) in [(0.0, (1.0, 0.0)), (0.5, (0.5, 0.5)), (1.0, (0.0, 1.0))] {
            let actual = MixerLevels {
                crossfader: x,
                ..MixerLevels::default()
            }
            .volumes();
            assert_eq!(actual, expected);
        }
        assert_eq!(
            MixerLevels {
                crossfader: 2.0,
                channel_a: -1.0,
                channel_b: 0.25
            }
            .volumes(),
            (0.0, 0.25)
        );
        assert_eq!(
            MixerLevels {
                crossfader: -2.0,
                channel_a: 2.0,
                channel_b: 1.0
            }
            .volumes(),
            (1.0, 0.0)
        );
        assert_eq!(
            MixerLevels {
                crossfader: f32::NAN,
                channel_a: f32::INFINITY,
                channel_b: f32::NAN
            }
            .volumes(),
            (0.0, 0.0)
        );
    }
    #[test]
    fn loads_mp3_wav_and_rejects_invalid_missing_and_oversized_tracks() {
        let wav = wav_file(8192, 4800);
        let track = PreparedTrack::open(wav.path()).unwrap();
        assert_eq!(track.duration(), Duration::from_millis(100));
        assert_eq!(&*track.overview, &[0.25; crate::waveform::OVERVIEW_BINS]);
        let mp3 = PreparedTrack::open(&fixture()).unwrap();
        assert!(
            mp3.tempo.is_none(),
            "steady MP3 tone must not invent a tempo"
        );
        assert!((8.9..9.2).contains(&mp3.duration().as_secs_f64()));
        assert_eq!(mp3.overview.len(), crate::waveform::OVERVIEW_BINS);
        assert!(mp3
            .overview
            .iter()
            .all(|p| p.is_finite() && *p > 0.0 && *p < 0.2));
        assert!(PreparedTrack::open_with_limit(wav.path(), 10).is_err());
        assert!(PreparedTrack::open(Path::new("missing.wav")).is_err());
        assert!(PreparedTrack::open(Path::new("unsupported.txt")).is_err());
        let bad = crate::test_support::TempFile::new("mp3", b"invalid");
        assert!(PreparedTrack::open(bad.path()).is_err());
    }
    #[test]
    fn offline_render_checks_real_sink_mix_and_position() {
        for (x, a, b, expected) in [
            (0.0, 1.0, 1.0, 0.25),
            (1.0, 1.0, 1.0, -0.125),
            (0.5, 1.0, 1.0, 0.0625),
            (0.5, 0.4, 0.8, 0.0),
            (0.5, 0.0, 0.0, 0.0),
        ] {
            let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
            let engine = DualDeckEngine::from_mixer(mixer, 1, 48_000);
            let left = wav_file(8192, 4800);
            let right = wav_file(-4096, 4800);
            engine.load_a(left.path()).unwrap();
            engine.load_b(right.path()).unwrap();
            // Playback must no longer depend on the files remaining open/present.
            drop(left);
            drop(right);
            engine.set_levels(MixerLevels {
                crossfader: x,
                channel_a: a,
                channel_b: b,
            });
            engine.play(DeckId::A);
            engine.play(DeckId::B);
            let start = std::time::Instant::now();
            let rendered: Vec<_> = output.by_ref().take(2400).collect();
            assert_eq!(rendered.len(), 2400);
            assert!(rendered.iter().all(|s| (s - expected).abs() < 1e-5));
            let snapshot = engine.snapshot(DeckId::A);
            assert!((40.0..=55.0).contains(&snapshot.percent()));
            eprintln!("offline 50ms mix rendered in {:?}", start.elapsed());
            engine.pause(DeckId::A);
            engine.pause(DeckId::B);
            let position = engine.snapshot(DeckId::A).position;
            // Allow Rodio's 5 ms control propagation, then a paused position must freeze.
            output.by_ref().take(1000).for_each(drop);
            let paused = engine.snapshot(DeckId::A).position;
            output.by_ref().take(1000).for_each(drop);
            assert_eq!(engine.snapshot(DeckId::A).position, paused);
            assert!(paused >= position);
        }
    }
    #[test]
    fn reload_cue_eof_and_restart_preserve_other_deck_and_gains() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48_000);
        let wav = wav_file(8192, 4800);
        engine.play(DeckId::A);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(wav.path()).unwrap();
        engine.set_levels(MixerLevels {
            crossfader: 0.25,
            channel_a: 0.8,
            channel_b: 0.4,
        });
        engine.play(DeckId::B);
        engine.load_a(wav.path()).unwrap(); // No audio consumer needed for reload/cue.
        engine.cue(DeckId::A);
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::A).percent(), 0.0);
        assert!((engine.decks[0].borrow().sink.volume() - 0.6).abs() < 1e-6);
        assert_eq!(engine.decks[1].borrow().sink.volume(), 0.1);
        let previous = engine.snapshot(DeckId::A).title;
        assert!(engine.load_a(Path::new("missing.wav")).is_err());
        assert_eq!(engine.snapshot(DeckId::A).title, previous);
        engine.play(DeckId::A);
        output.by_ref().take(10_000).for_each(drop);
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).percent(), 100.0);
        engine.play(DeckId::A);
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).percent(), 0.0);
        engine.cue(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
    }
    #[test]
    fn explicit_grid_commit_preserves_source_transport_and_mixer_levels() {
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = DualDeckEngine::from_mixer(mixer, 1, 48_000);
        let file = crate::test_support::wav_file(8192, 48_000 * 2);
        engine.load_a(file.path()).unwrap();
        let levels = MixerLevels {
            crossfader: 0.3,
            channel_a: 0.6,
            channel_b: 0.8,
        };
        engine.set_levels(levels);
        engine.seek_percent(DeckId::A, 25.).unwrap();
        engine.play(DeckId::A);
        let before = engine.snapshot(DeckId::A);
        let grid = crate::beatgrid::BeatGrid::new(128.5, Duration::from_millis(510)).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        let after = engine.snapshot(DeckId::A);
        assert_eq!(after.position, before.position);
        assert!(after.playing);
        assert!(Arc::ptr_eq(
            after.detail.as_ref().unwrap(),
            before.detail.as_ref().unwrap()
        ));
        assert_eq!(after.path, before.path);
        assert_eq!(engine.levels.get().volumes(), levels.volumes());
        assert!(engine.snapshot(DeckId::B).grid.is_none());
        assert!(engine
            .set_grid(
                DeckId::A,
                Some(crate::beatgrid::BeatGrid::new(120., Duration::from_secs(3)).unwrap())
            )
            .is_err());
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(grid));
        engine.set_grid(DeckId::A, None).unwrap();
        assert!(engine.snapshot(DeckId::A).playing);
    }
}
