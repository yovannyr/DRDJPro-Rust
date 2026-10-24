//! Control-plane loading and two Rodio decks. Rodio itself is not lock-free.
use rodio::{
    buffer::SamplesBuffer, mixer::Mixer, Decoder, OutputStream, OutputStreamBuilder, Sink, Source,
};
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
    samples: SamplesBuffer,
    overview: Arc<[f32]>,
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
        let decoder = Decoder::try_from(File::open(path).map_err(|e| e.to_string())?)
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
        Ok(Self {
            title: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            samples: SamplesBuffer::new(channels, rate, samples),
            overview,
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
    pub title: String,
    pub loaded: bool,
    pub playing: bool,
    pub position: Duration,
    pub duration: Duration,
    pub overview: Option<Arc<[f32]>>,
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
        self.replace_source(id, track, source, Duration::ZERO, false);
    }
    fn replace_source(
        &self,
        id: DeckId,
        track: PreparedTrack,
        source: SamplesBuffer,
        offset: Duration,
        playing: bool,
    ) {
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
        };
    }
    /// Seek the shared PCM source on the control plane, without Sink::try_seek's audio feedback wait.
    pub fn seek_percent(&self, id: DeckId, percent: f32) -> Result<(), String> {
        if !percent.is_finite() {
            return Err("Invalid seek position".into());
        }
        let (track, playing) = {
            let deck = self.decks[id.index()].borrow();
            (
                deck.track.clone().ok_or("No track loaded")?,
                !deck.sink.is_paused() && !deck.sink.empty(),
            )
        };
        let duration = track.duration();
        let offset = duration.mul_f64(f64::from(percent.clamp(0.0, 100.0)) / 100.0);
        let mut source = track.samples.clone();
        source.try_seek(offset).map_err(|e| e.to_string())?;
        self.replace_source(id, track, source, offset, playing && offset < duration);
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
        let restart = {
            let deck = self.decks[id.index()].borrow();
            if deck.sink.empty()
                || deck
                    .track
                    .as_ref()
                    .is_some_and(|t| deck.offset >= t.duration())
            {
                deck.track.clone()
            } else {
                None
            }
        };
        if let Some(track) = restart {
            self.install(id, track);
        }
        let deck = self.decks[id.index()].borrow();
        if deck.track.is_some() {
            deck.sink.play();
        }
    }
    pub fn pause(&self, id: DeckId) {
        self.decks[id.index()].borrow().sink.pause();
    }
    pub fn cue(&self, id: DeckId) {
        let track = self.decks[id.index()].borrow().track.clone();
        if let Some(track) = track {
            self.install(id, track);
        }
    }
    pub fn snapshot(&self, id: DeckId) -> DeckSnapshot {
        let deck = self.decks[id.index()].borrow();
        let duration = deck
            .track
            .as_ref()
            .map_or(Duration::ZERO, PreparedTrack::duration);
        let position = if deck.track.is_some() && deck.sink.empty() {
            duration
        } else {
            deck.offset
                .saturating_add(deck.sink.get_pos())
                .min(duration)
        };
        DeckSnapshot {
            title: deck
                .track
                .as_ref()
                .map_or_else(|| "No track loaded".into(), |t| t.title.clone()),
            loaded: deck.track.is_some(),
            playing: !deck.sink.is_paused() && !deck.sink.empty(),
            position,
            duration,
            overview: deck.track.as_ref().map(|t| Arc::clone(&t.overview)),
        }
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
                title: "stereo step".into(),
                overview: crate::waveform::peak_overview(&samples, 2).into(),
                samples: SamplesBuffer::new(2, 48_000, samples),
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
}
