//! Device-backed preview playback; not a low-latency DJ engine.
//! Never called from the GUI render loop or a real-time audio callback.
use std::{error::Error, fs::File, path::Path};

use rodio::{Decoder, OutputStreamBuilder, Sink};

pub fn play_file(path: &Path) -> Result<(), Box<dyn Error>> {
    // Decode first: avoid opening an output device for an invalid file.
    let source = Decoder::try_from(File::open(path)?)?;
    let stream = OutputStreamBuilder::open_default_stream()?;
    let sink = Sink::connect_new(stream.mixer());
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}

/// Creates a headless sink, useful for testing transport without hardware.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_sink_pause_resume_volume_and_stop() {
        let (sink, _queue) = Sink::new();
        sink.set_volume(0.25);
        assert!((sink.volume() - 0.25).abs() < f32::EPSILON);
        sink.pause();
        assert!(sink.is_paused());
        sink.play();
        assert!(!sink.is_paused());
        sink.stop();
        assert!(sink.empty());
    }
}
