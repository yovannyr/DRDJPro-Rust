//! Offline decode verification for real DJ-library input files.
//! This module does not open an audio device or perform playback.
use std::{error::Error, path::Path};

use symphonia::{
    core::{
        audio::SampleBuffer, codecs::DecoderOptions, errors::Error as SymphoniaError,
        meta::MetadataOptions,
    },
    default::get_codecs,
};

#[derive(Debug, Clone)]
pub struct AudioAnalysis {
    pub sample_rate: u32,
    pub channels: usize,
    pub frames: u64,
    pub peak: f32,
    pub rms: f32,
}

impl AudioAnalysis {
    pub fn duration_seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.sample_rate)
    }
}

/// Decode the first decodable audio track and validate its samples.
pub fn analyze_file(path: &Path) -> Result<AudioAnalysis, Box<dyn Error>> {
    if crate::opus_audio::is_opus(path)? {
        let (channels, rate, samples) = crate::opus_audio::decode(
            path,
            64 * 1024 * 1024,
            &crate::cancellation::Cancellation::default(),
        )?;
        let sum = samples
            .iter()
            .map(|&s| f64::from(s) * f64::from(s))
            .sum::<f64>();
        let peak = samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
        return Ok(AudioAnalysis {
            sample_rate: rate,
            channels: usize::from(channels),
            frames: (samples.len() / usize::from(channels)) as u64,
            peak,
            rms: (sum / samples.len() as f64).sqrt() as f32,
        });
    }
    let (probed, _) = crate::media_input::probe(path, &MetadataOptions::default())?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("no default audio track")?;
    let track_id = track.id;
    let mut decoder = get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut result: Option<AudioAnalysis> = None;
    let mut sum_squares = 0_f64;
    let mut sample_count = 0_u64;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => return Err("decoder reset required".into()),
            Err(err) => return Err(err.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(audio) => audio,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => return Err(err.into()),
        };
        let spec = *audio.spec();
        let channels = spec.channels.count();
        if spec.rate == 0 || channels == 0 {
            return Err("invalid audio channel layout or sample rate".into());
        }
        if let Some(previous) = &result {
            if previous.sample_rate != spec.rate || previous.channels != channels {
                return Err("audio format changed within track".into());
            }
        }
        let mut samples = SampleBuffer::<f32>::new(audio.capacity() as u64, spec);
        samples.copy_interleaved_ref(audio);
        let output = samples.samples();
        let analysis = result.get_or_insert(AudioAnalysis {
            sample_rate: spec.rate,
            channels,
            frames: 0,
            peak: 0.0,
            rms: 0.0,
        });
        for &sample in output {
            if !sample.is_finite() {
                return Err("non-finite decoded sample".into());
            }
            analysis.peak = analysis.peak.max(sample.abs());
            sum_squares += f64::from(sample) * f64::from(sample);
        }
        sample_count += output.len() as u64;
        analysis.frames += (output.len() / channels) as u64;
    }
    let mut analysis = result.ok_or("no audio frames decoded")?;
    if sample_count == 0 || analysis.frames == 0 {
        return Err("empty audio track".into());
    }
    analysis.rms = (sum_squares / sample_count as f64).sqrt() as f32;
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    // Minimal deterministic PCM test file, generated without downloads.
    fn pcm_wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    fn temporary_file(ext: &str, bytes: &[u8]) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("drdj-audio-{}-{stamp}.{ext}", std::process::id()));
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn decodes_stereo_pcm_and_computes_basic_metrics() {
        let samples = [16384_i16, -16384_i16].repeat(480);
        let path = temporary_file("wav", &pcm_wav(48_000, 2, &samples));
        let result = analyze_file(&path);
        fs::remove_file(&path).unwrap();
        let result = result.unwrap();
        assert_eq!(result.sample_rate, 48_000);
        assert_eq!(result.channels, 2);
        assert_eq!(result.frames, 480);
        assert!((result.duration_seconds() - 0.01).abs() < 1e-6);
        assert!(result.peak > 0.49 && result.peak < 0.51);
        assert!(result.rms > 0.49 && result.rms < 0.51);
    }

    #[test]
    fn rejects_invalid_music_data() {
        let path = temporary_file("mp3", b"this is not an MP3");
        let result = analyze_file(&path);
        fs::remove_file(&path).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn decodes_bundled_mp3_fixture() {
        let path = std::env::var_os("DRDJ_AUDIO_TEST_FILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(crate::test_support::fixture);
        let result = analyze_file(Path::new(&path)).expect("decode real music sample");
        assert!(result.sample_rate >= 8_000);
        assert!(result.channels >= 1 && result.channels <= 8);
        assert!(result.duration_seconds() > 8.0 && result.duration_seconds() < 11.0);
        assert!(result.peak > 0.001);
        assert!(result.peak <= 1.1);
        assert!(result.rms > 0.0001);
    }
}
