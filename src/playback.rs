//! Control-plane loading and two Rodio decks. Rodio itself is not lock-free.
use crate::looping::{frame_at, frame_time, FrameLoop, PcmSource};
use crate::playback_speed::PlaybackSpeed;
use rodio::{mixer::Mixer, Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    fs::File,
    path::Path,
    sync::Arc,
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
        Self::open_with_limit(path, 64 * 1024 * 1024)
    }
    fn open_with_limit(path: &Path, limit: usize) -> Result<Self, String> {
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
        let mut samples = Vec::new();
        for sample in decoder {
            if samples.len() == limit {
                return Err("Track exceeds the 256 MiB decoded PCM limit".into());
            }
            if !sample.is_finite() {
                return Err("Non-finite audio sample".into());
            }
            samples.push(sample);
        }
        if samples.is_empty() || samples.len() % usize::from(channels) != 0 {
            return Err("No complete audio frames decoded".into());
        }
        let overview = crate::waveform::peak_overview(&samples, channels).into();
        let detail = Arc::new(crate::waveform::DetailPeaks::new(&samples, channels, rate));
        let tempo = crate::tempo::estimate(&samples, channels, rate);
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

struct Deck {
    sink: Sink,
    track: Option<PreparedTrack>,
    offset: Duration,
    loop_in: Option<usize>,
    loop_region: Option<FrameLoop>,
    loop_active: bool,
    speed: Option<PlaybackSpeed>,
    grid: Option<crate::beatgrid::BeatGrid>,
}
// A fresh Rodio queue initially reports mono/44.1 kHz even when stereo PCM is queued.
// Each replacement sink contains exactly one fixed-format track. Report that format
// throughout its lifetime so the output converter cannot misinterpret initial samples.
struct DeckOutput {
    queue: rodio::queue::SourcesQueueOutput,
    channels: u16,
    sample_rate: u32,
}
impl Iterator for DeckOutput {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.queue.next()
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
    pub requested_speed: f64,
    pub effective_speed: f64,
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
    // Drop the stream only after the sinks.
    _stream: Option<OutputStream>,
}
impl DualDeckEngine {
    pub fn open_default() -> Result<Self, Box<dyn Error>> {
        let stream = OutputStreamBuilder::open_default_stream()?;
        let mut engine = Self::from_mixer(stream.mixer().clone());
        engine._stream = Some(stream);
        Ok(engine)
    }
    pub(crate) fn from_mixer(mixer: Mixer) -> Self {
        let decks = std::array::from_fn(|_| {
            let sink = Sink::connect_new(&mixer);
            sink.pause();
            RefCell::new(Deck {
                sink,
                track: None,
                offset: Duration::ZERO,
                loop_in: None,
                loop_region: None,
                loop_active: false,
                speed: None,
                grid: None,
            })
        });
        let engine = Self {
            decks,
            levels: Cell::new(MixerLevels::default()),
            mixer,
            _stream: None,
        };
        engine.set_levels(MixerLevels::default());
        engine
    }
    /// Replace only after preparation succeeded. Fresh sinks avoid stop/append waiting for audio.
    pub fn install(&self, id: DeckId, track: PreparedTrack) {
        let source = track.samples.clone();
        self.replace_source(id, track, source, Duration::ZERO, false, None);
    }
    fn replace_source(
        &self,
        id: DeckId,
        track: PreparedTrack,
        mut source: PcmSource,
        offset: Duration,
        playing: bool,
        grid: Option<crate::beatgrid::BeatGrid>,
    ) {
        let (loop_in, loop_region, speed) = {
            let deck = self.decks[id.index()].borrow();
            if deck
                .track
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(&old.detail, &track.detail))
            {
                (deck.loop_in, deck.loop_region, deck.speed.unwrap())
            } else {
                (
                    None,
                    None,
                    PlaybackSpeed::new(1., track.samples.sample_rate())
                        .expect("Prepared source format was validated"),
                )
            }
        };
        source.set_playback_rate(speed.output_rate());
        let (sink, queue) = Sink::new();
        let output = DeckOutput {
            queue,
            channels: source.channels(),
            sample_rate: source.sample_rate(),
        };
        sink.pause();
        let (a, b) = self.levels.get().volumes();
        sink.set_volume(if id == DeckId::A { a } else { b });
        sink.append(source);
        if playing {
            sink.play();
        }
        self.mixer.add(output);

        *self.decks[id.index()].borrow_mut() = Deck {
            sink,
            track: Some(track),
            offset,
            loop_in,
            loop_region,
            loop_active: false,
            speed: Some(speed),
            grid,
        };
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
                !deck.sink.is_paused() && !deck.sink.empty(),
                deck.grid,
            )
        };
        let duration = track.duration();
        if offset > duration {
            return Err("Position is outside the loaded track".into());
        }
        let mut source = track.samples.clone();
        source.try_seek(offset).map_err(|e| e.to_string())?;
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
    pub fn set_speed(&self, id: DeckId, factor: f64) -> Result<(), String> {
        let (track, playing, grid, region, speed) = {
            let deck = self.decks[id.index()].borrow();
            let track = deck.track.clone().ok_or("Load a track first")?;
            let speed = PlaybackSpeed::new(factor, track.samples.sample_rate())?;
            if deck
                .speed
                .is_some_and(|current| current.requested() == speed.requested())
            {
                return Ok(());
            }
            (
                track,
                !deck.sink.is_paused() && !deck.sink.empty(),
                deck.grid,
                if deck.loop_active {
                    deck.loop_region
                } else {
                    None
                },
                speed,
            )
        };
        let position = self.snapshot(id).position;
        let mut source = track.samples.clone();
        if let Some(region) = region {
            source.set_loop(region)?;
        }
        source.try_seek(position).map_err(|e| e.to_string())?;
        // Source preparation/validation succeeded. Replacement is synchronous on this control plane.
        self.decks[id.index()].borrow_mut().speed = Some(speed);
        self.replace_source(id, track, source, position, playing, grid);
        self.decks[id.index()].borrow_mut().loop_active = region.is_some();
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
                frame_at(position, track.samples.sample_rate())
            };
            FrameLoop::from_beats(
                start,
                eighths,
                grid.bpm(),
                track.samples.sample_rate(),
                track.samples.frames(),
            )?
        };
        let rate = self.decks[id.index()]
            .borrow()
            .track
            .as_ref()
            .unwrap()
            .samples
            .sample_rate();
        self.apply_loop_region(id, region, true, frame_time(region.start, rate))
    }
    pub fn resize_loop(&self, id: DeckId, double: bool) -> Result<(), String> {
        let position = self.snapshot(id).position;
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
        self.apply_loop_region(id, region, active, position)
    }
    fn apply_loop_region(
        &self,
        id: DeckId,
        region: FrameLoop,
        active: bool,
        position: Duration,
    ) -> Result<(), String> {
        if active {
            let (track, playing, grid) = {
                let deck = self.decks[id.index()].borrow();
                (
                    deck.track.clone().ok_or("Load a track first")?,
                    !deck.sink.is_paused() && !deck.sink.empty(),
                    deck.grid,
                )
            };
            let mut source = track.samples.clone();
            source.set_loop(region)?;
            let frame = region.wrap(frame_at(position, source.sample_rate()));
            let offset = frame_time(frame, source.sample_rate());
            source.try_seek(offset).map_err(|e| e.to_string())?;
            self.replace_source(id, track, source, offset, playing, grid);
        }
        let mut deck = self.decks[id.index()].borrow_mut();
        deck.loop_in = Some(region.start);
        deck.loop_region = Some(region);
        deck.loop_active = active;
        Ok(())
    }
    pub fn activate_loop(&self, id: DeckId) -> Result<(), String> {
        let (track, region, playing, grid) = {
            let deck = self.decks[id.index()].borrow();
            (
                deck.track.clone().ok_or("Load a track first")?,
                deck.loop_region.ok_or("Set valid Loop In/Out first")?,
                !deck.sink.is_paused() && !deck.sink.empty(),
                deck.grid,
            )
        };
        let mut source = track.samples.clone();
        source.set_loop(region)?;
        let offset = frame_time(region.start, source.sample_rate());
        self.replace_source(id, track, source, offset, playing, grid);
        self.decks[id.index()].borrow_mut().loop_active = true;
        Ok(())
    }
    pub fn exit_loop(&self, id: DeckId) -> Result<(), String> {
        if !self.decks[id.index()].borrow().loop_active {
            return Ok(());
        }
        self.seek_position(id, self.snapshot(id).position)
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
        let restart = {
            let deck = self.decks[id.index()].borrow();
            deck.track.is_some()
                && (deck.sink.empty()
                    || deck
                        .track
                        .as_ref()
                        .is_some_and(|t| deck.offset >= t.duration()))
        };
        if restart {
            self.cue(id);
        }
        let deck = self.decks[id.index()].borrow();
        if deck.track.is_some() {
            deck.sink.play();
        }
    }
    pub fn pause(&self, id: DeckId) {
        // Freeze the wrapped control-plane position instead of allowing Rodio's pending
        // periodic position update to move it across a loop boundary after Pause.
        let active = self.decks[id.index()].borrow().loop_active;
        if active {
            let position = self.snapshot(id).position;
            let (track, region, grid) = {
                let deck = self.decks[id.index()].borrow();
                (
                    deck.track.clone().unwrap(),
                    deck.loop_region.unwrap(),
                    deck.grid,
                )
            };
            let mut source = track.samples.clone();
            source
                .set_loop(region)
                .expect("Installed loop was validated");
            source
                .try_seek(position)
                .expect("In-memory seek cannot fail");
            self.replace_source(id, track, source, position, false, grid);
            self.decks[id.index()].borrow_mut().loop_active = true;
        } else {
            self.decks[id.index()].borrow().sink.pause();
        }
    }
    pub fn cue(&self, id: DeckId) {
        let (track, grid) = {
            let deck = self.decks[id.index()].borrow();
            (deck.track.clone(), deck.grid)
        };
        if let Some(track) = track {
            let source = track.samples.clone();
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
        let source_elapsed = deck
            .sink
            .get_pos()
            .mul_f64(deck.speed.map_or(1., PlaybackSpeed::effective_factor));
        let absolute = deck.offset.saturating_add(source_elapsed);
        let position = if deck.track.is_some() && deck.sink.empty() {
            duration
        } else {
            absolute.min(duration)
        };
        let position = if deck.loop_active {
            let track = deck.track.as_ref().unwrap();
            frame_time(
                deck.loop_region
                    .unwrap()
                    .wrap(frame_at(absolute, track.samples.sample_rate())),
                track.samples.sample_rate(),
            )
        } else {
            position
        };
        DeckSnapshot {
            path: deck.track.as_ref().map(|t| Arc::clone(&t.path)),
            title: deck
                .track
                .as_ref()
                .map_or_else(|| "No track loaded".into(), |t| t.title.clone()),
            loaded: deck.track.is_some(),
            playing: !deck.sink.is_paused() && !deck.sink.empty(),
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
            requested_speed: deck.speed.map_or(1., PlaybackSpeed::requested),
            effective_speed: deck.speed.map_or(1., PlaybackSpeed::effective_factor),
        }
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
        for deck in &self.decks {
            deck.borrow().sink.sleep_until_end();
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
    fn changed_rate_loop_wraps_and_resizes_render_only_region_samples_after_settling() {
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = DualDeckEngine::from_mixer(mixer);
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
            let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        assert_eq!(engine.decks[1].borrow().speed.unwrap().requested(), 1.);
        for value in [f64::NAN, f64::INFINITY, 0., 0.49, 2.01] {
            assert!(engine.set_speed(DeckId::A, value).is_err());
            assert_eq!(engine.snapshot(DeckId::A).position, after.position);
            assert_eq!(engine.decks[0].borrow().speed.unwrap().requested(), 0.5);
        }
        engine.pause(DeckId::A);
        let paused = engine.snapshot(DeckId::A).position;
        output.by_ref().take(4096).for_each(drop);
        assert_eq!(engine.snapshot(DeckId::A).position, paused);
        engine.seek_percent(DeckId::A, 50.).unwrap();
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(4));
        assert_eq!(engine.decks[0].borrow().speed.unwrap().requested(), 0.5);
        engine.cue(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
        assert_eq!(engine.decks[0].borrow().speed.unwrap().requested(), 0.5);
        assert!(engine.load_a(Path::new("missing-speed.wav")).is_err());
        assert_eq!(engine.decks[0].borrow().speed.unwrap().requested(), 0.5);
        engine.load_a(other.path()).unwrap();
        assert_eq!(engine.decks[0].borrow().speed.unwrap().requested(), 1.);
        assert_eq!(engine.output_volumes(), volumes);
    }
    #[test]
    fn aligned_auto_loop_is_optional_and_validated_before_active_state_changes() {
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
            let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
        let engine = DualDeckEngine::from_mixer(mixer);
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
