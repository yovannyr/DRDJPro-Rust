//! Two independently controlled playback decks routed to one output mixer.
//! Proof-of-concept only: not a low-latency DJ audio engine.
use std::{error::Error, fs::File, path::Path};

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};

#[derive(Debug, Clone, Copy)]
pub struct MixerLevels {
    pub crossfader: f32,
    pub channel_a: f32,
    pub channel_b: f32,
}

impl MixerLevels {
    /// Linear crossfader, values clamped to 0..1.
    pub fn volumes(self) -> (f32, f32) {
        let x = self.crossfader.clamp(0.0, 1.0);
        (
            self.channel_a.clamp(0.0, 1.0) * (1.0 - x),
            self.channel_b.clamp(0.0, 1.0) * x,
        )
    }
}

pub struct DualDeckEngine {
    // Output stream must live as long as the sinks.
    _stream: OutputStream,
    deck_a: Sink,
    deck_b: Sink,
}

impl DualDeckEngine {
    pub fn open_default() -> Result<Self, Box<dyn Error>> {
        let stream = OutputStreamBuilder::open_default_stream()?;
        let deck_a = Sink::connect_new(stream.mixer());
        let deck_b = Sink::connect_new(stream.mixer());
        deck_a.pause();
        deck_b.pause();
        Ok(Self {
            _stream: stream,
            deck_a,
            deck_b,
        })
    }

    pub fn load_a(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let source = Decoder::try_from(File::open(path)?)?;
        self.deck_a.stop();
        self.deck_a.append(source);
        self.deck_a.pause();
        Ok(())
    }

    pub fn load_b(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let source = Decoder::try_from(File::open(path)?)?;
        self.deck_b.stop();
        self.deck_b.append(source);
        self.deck_b.pause();
        Ok(())
    }

    pub fn play_a(&self) {
        self.deck_a.play();
    }
    pub fn play_b(&self) {
        self.deck_b.play();
    }
    pub fn pause_a(&self) {
        self.deck_a.pause();
    }
    pub fn pause_b(&self) {
        self.deck_b.pause();
    }
    pub fn stop_a(&self) {
        self.deck_a.stop();
    }
    pub fn stop_b(&self) {
        self.deck_b.stop();
    }

    pub fn set_levels(&self, levels: MixerLevels) {
        let (a, b) = levels.volumes();
        self.deck_a.set_volume(a);
        self.deck_b.set_volume(b);
    }

    pub fn is_a_paused(&self) -> bool {
        self.deck_a.is_paused()
    }

    pub fn is_b_paused(&self) -> bool {
        self.deck_b.is_paused()
    }

    pub fn cue_a(&self) -> Result<(), rodio::source::SeekError> {
        self.deck_a.try_seek(std::time::Duration::ZERO)?;
        self.deck_a.pause();
        Ok(())
    }

    pub fn cue_b(&self) -> Result<(), rodio::source::SeekError> {
        self.deck_b.try_seek(std::time::Duration::ZERO)?;
        self.deck_b.pause();
        Ok(())
    }

    pub fn wait_for_end(&self) {
        self.deck_a.sleep_until_end();
        self.deck_b.sleep_until_end();
    }
}

pub fn play_file(path: &Path) -> Result<(), Box<dyn Error>> {
    let engine = DualDeckEngine::open_default()?;
    engine.load_a(path)?;
    engine.set_levels(MixerLevels {
        crossfader: 0.0,
        channel_a: 1.0,
        channel_b: 1.0,
    });
    engine.play_a();
    engine.wait_for_end();
    engine.stop_a();
    Ok(())
}

/// Load two files, start both, and mix through one physical output.
/// For interactive controls use DualDeckEngine directly.
pub fn mix_files(a: &Path, b: &Path) -> Result<(), Box<dyn Error>> {
    let engine = DualDeckEngine::open_default()?;
    engine.load_a(a)?;
    engine.load_b(b)?;
    engine.set_levels(MixerLevels {
        crossfader: 0.5,
        channel_a: 1.0,
        channel_b: 1.0,
    });
    engine.pause_a();
    engine.pause_b();
    engine.play_a();
    engine.play_b();
    engine.wait_for_end();
    engine.stop_a();
    engine.stop_b();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfader_extremes_and_midpoint() {
        let mut mixer = MixerLevels {
            crossfader: 0.0,
            channel_a: 1.0,
            channel_b: 1.0,
        };
        assert_eq!(mixer.volumes(), (1.0, 0.0));
        mixer.crossfader = 1.0;
        assert_eq!(mixer.volumes(), (0.0, 1.0));
        mixer.crossfader = 0.5;
        assert_eq!(mixer.volumes(), (0.5, 0.5));
    }

    #[test]
    fn channel_gain_and_crossfader_are_clamped() {
        let mixer = MixerLevels {
            crossfader: 2.0,
            channel_a: 2.0,
            channel_b: 0.25,
        };
        assert_eq!(mixer.volumes(), (0.0, 0.25));
        let mixer = MixerLevels {
            crossfader: -1.0,
            channel_a: -3.0,
            channel_b: 1.0,
        };
        assert_eq!(mixer.volumes(), (0.0, 0.0));
    }

    #[test]
    fn two_headless_sinks_have_independent_transport() {
        let (deck_a, _a_queue) = Sink::new();
        let (deck_b, _b_queue) = Sink::new();
        deck_a.pause();
        deck_b.pause();
        deck_a.play();
        assert!(!deck_a.is_paused());
        assert!(deck_b.is_paused());
        deck_b.play();
        deck_a.pause();
        assert!(deck_a.is_paused());
        assert!(!deck_b.is_paused());
        deck_a.set_volume(0.2);
        deck_b.set_volume(0.8);
        assert!((deck_a.volume() - 0.2).abs() < f32::EPSILON);
        assert!((deck_b.volume() - 0.8).abs() < f32::EPSILON);
        deck_a.stop();
        assert!(deck_a.empty());
        assert!(!deck_b.is_paused());
    }
}
