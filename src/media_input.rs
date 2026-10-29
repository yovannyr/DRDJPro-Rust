//! Worker-only diagnostics and bounded ID3/MPEG-in-WAVE recovery. Never rewrites media.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};
use symphonia::core::{
    errors::Error,
    formats::FormatOptions,
    io::{MediaSource, MediaSourceStream},
    meta::MetadataOptions,
    probe::{Hint, ProbeResult},
};

// Bound untrusted tag offsets and the number of consecutive leading tags.
const MAX_TAG_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TAGS: usize = 8;
pub(crate) const TAG_WARNING: &str = "ID3 tags skipped; audio readable: ";
pub(crate) const WAVE_WARNING: &str = "MPEG audio read from WAV data; container tags omitted";

fn explain_failure(path: &Path, error: Error) -> Error {
    let detail = if std::fs::metadata(path).is_ok_and(|m| m.len() == 0) {
        Some("Audio file is empty (0 bytes)".to_owned())
    } else if let Ok(source) = AudioWindow::after_id3(path) {
        (source.len <= 128).then(|| {
            format!(
                "No audio decoded after leading ID3 tags ({} remaining bytes)",
                source.len
            )
        })
    } else {
        None
    };
    let detail = detail.or_else(|| match &error {
        Error::Unsupported("core (probe): no suitable format reader found") => {
            Some("Audio format not recognized or damaged; no suitable format reader found".into())
        }
        Error::Unsupported("wav: unsupported wave format") => Some(
            "Unsupported or invalid WAVE container; no validated MPEG Layer-3 data found".into(),
        ),
        _ => None,
    });
    detail.map_or(error, |message| {
        Error::IoError(io::Error::new(io::ErrorKind::InvalidData, message))
    })
}

fn eligible(path: &Path, error: &Error) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
        && matches!(
            error,
            Error::DecodeError("id3v2: invalid text encoding")
                | Error::Unsupported("id3v2: compressed frames are not supported")
        )
}

fn raw_probe(
    path: &Path,
    source: impl MediaSource + 'static,
    options: &MetadataOptions,
) -> Result<ProbeResult, Error> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|x| x.to_str()) {
        hint.with_extension(ext);
    }
    symphonia::default::get_probe().format(
        &hint,
        MediaSourceStream::new(Box::new(source), Default::default()),
        &FormatOptions::default(),
        options,
    )
}

fn require_mp3(result: ProbeResult) -> Result<ProbeResult, Error> {
    if result
        .format
        .default_track()
        .is_none_or(|track| track.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_MP3)
    {
        return Err(Error::Unsupported("WAVE data is not MPEG Layer 3"));
    }
    Ok(result)
}

pub(crate) fn probe(
    path: &Path,
    options: &MetadataOptions,
) -> Result<(ProbeResult, Option<String>), Error> {
    match raw_probe(path, File::open(path)?, options) {
        Ok(result) => Ok((result, None)),
        Err(error) => {
            if let Ok(source) = mpeg_wave_source(path) {
                let result = raw_probe(Path::new("payload.mp3"), source, options)
                    .and_then(require_mp3)
                    .map_err(|error| explain_failure(path, error))?;
                Ok((result, Some(WAVE_WARNING.into())))
            } else if eligible(path, &error) {
                let source = AudioWindow::after_id3(path)?;
                let result = raw_probe(path, source, options)?;
                Ok((result, Some(format!("{TAG_WARNING}{error}"))))
            } else {
                Err(explain_failure(path, error))
            }
        }
    }
}

/// Rodio hides the parser error; independently validate the recovery source.
pub(crate) fn recovery_source(path: &Path) -> Result<AudioWindow, Error> {
    match raw_probe(path, File::open(path)?, &MetadataOptions::default()) {
        Err(error) => {
            if let Ok(source) = mpeg_wave_source(path) {
                require_mp3(raw_probe(
                    Path::new("payload.mp3"),
                    source,
                    &MetadataOptions::default(),
                )?)?;
                mpeg_wave_source(path)
            } else if eligible(path, &error) {
                Ok(AudioWindow::after_id3(path)?)
            } else {
                Err(explain_failure(path, error))
            }
        }
        Ok(_) => Err(Error::Unsupported("original parser accepted source")),
    }
}

/// A bounded seekable view of a file, stripped ID3 prefix or WAVE data payload.
pub(crate) struct AudioWindow {
    file: File,
    start: u64,
    len: u64,
}
impl AudioWindow {
    pub(crate) fn whole(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            file,
            start: 0,
            len,
        })
    }
    pub(crate) fn len(&self) -> u64 {
        self.len
    }
    fn after_id3(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut start = 0;
        for _ in 0..MAX_TAGS {
            let mut header = [0; 10];
            file.seek(SeekFrom::Start(start))?;
            if len.saturating_sub(start) < 10 {
                break;
            }
            file.read_exact(&mut header)?;
            if &header[..3] != b"ID3" {
                break;
            }
            let allowed = match header[3] {
                2 => 0xc0,
                3 => 0xe0,
                4 => 0xf0,
                _ => return Err(invalid()),
            };
            if header[4] == 255
                || header[5] & !allowed != 0
                || header[6..10].iter().any(|v| v & 128 != 0)
            {
                return Err(invalid());
            }
            let size = header[6..10]
                .iter()
                .fold(0_u64, |sum, v| (sum << 7) | u64::from(*v));
            let footer = header[3] == 4 && header[5] & 0x10 != 0;
            let end = start + 10 + size + if footer { 10 } else { 0 };
            if end > len || end > MAX_TAG_BYTES {
                return Err(invalid());
            }
            if footer {
                let mut tail = [0; 10];
                file.seek(SeekFrom::Start(end - 10))?;
                file.read_exact(&mut tail)?;
                if &tail[..3] != b"3DI" || tail[3..] != header[3..] {
                    return Err(invalid());
                }
            }
            start = end;
        }
        if start == 0 {
            return Err(invalid());
        }
        file.seek(SeekFrom::Start(start))?;
        let mut next = [0; 3];
        let count = file.read(&mut next)?;
        if count == 3 && &next == b"ID3" {
            return Err(invalid());
        }
        file.seek(SeekFrom::Start(start))?;
        Ok(Self {
            file,
            start,
            len: len - start,
        })
    }
}

/// Read only a verified MPEG Layer-3 WAVE data chunk. Never scan unbounded chunks.
fn mpeg_wave_source(path: &Path) -> Result<AudioWindow, Error> {
    let mut source = AudioWindow::whole(path)?;
    let mut prefix = [0; 3];
    source.read_exact(&mut prefix)?;
    if &prefix == b"ID3" {
        source = AudioWindow::after_id3(path)?;
    }
    source.seek(SeekFrom::Start(0))?;
    let mut header = [0; 12];
    source.read_exact(&mut header)?;
    let end = 8 + u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap()));
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" || end < 12 || end > source.len {
        return Err(Error::DecodeError("invalid MPEG WAVE bounds"));
    }
    let mut position = 12;
    let mut format = false;
    let mut data = None;
    for _ in 0..1024 {
        if position == end {
            let (start, len) = data.ok_or(Error::DecodeError("MPEG WAVE has no audio data"))?;
            if !format {
                return Err(Error::Unsupported("WAVE is not MPEG Layer 3"));
            }
            source.start += start;
            source.len = len;
            source.seek(SeekFrom::Start(0))?;
            return Ok(source);
        }
        if position + 8 > end {
            return Err(Error::DecodeError("truncated MPEG WAVE chunk header"));
        }
        source.seek(SeekFrom::Start(position))?;
        let mut chunk = [0; 8];
        source.read_exact(&mut chunk)?;
        let len = u64::from(u32::from_le_bytes(chunk[4..].try_into().unwrap()));
        let start = position + 8;
        let next = start + len;
        if next > end {
            return Err(Error::DecodeError("MPEG WAVE chunk exceeds container"));
        }
        match &chunk[..4] {
            b"fmt " => {
                if format || len < 16 {
                    return Err(Error::DecodeError("invalid MPEG WAVE format chunk"));
                }
                let mut code = [0; 2];
                source.read_exact(&mut code)?;
                if u16::from_le_bytes(code) != 0x55 {
                    return Err(Error::Unsupported("WAVE is not MPEG Layer 3"));
                }
                format = true;
            }
            b"data" => {
                if data.is_some() || len == 0 {
                    return Err(Error::DecodeError("empty or duplicate MPEG WAVE data"));
                }
                data = Some((start, len));
            }
            _ => (),
        }
        // A terminal odd payload needs no byte outside the declared RIFF extent.
        position = if next == end { next } else { next + len % 2 };
    }
    Err(Error::DecodeError("MPEG WAVE chunk limit exceeded"))
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or excessive leading ID3 tag bounds",
    )
}
impl Read for AudioWindow {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let position = self.file.stream_position()?.saturating_sub(self.start);
        let remaining = self.len.saturating_sub(position);
        let count = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        self.file.read(&mut buf[..count])
    }
}
impl Seek for AudioWindow {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::End(n) => i128::from(self.len) + i128::from(n),
            SeekFrom::Current(n) => {
                i128::from(self.file.stream_position()? - self.start) + i128::from(n)
            }
        };
        let target = u64::try_from(target).map_err(|_| invalid())?;
        let absolute = self.start.checked_add(target).ok_or_else(invalid)?;
        self.file.seek(SeekFrom::Start(absolute))?;
        Ok(target)
    }
}
impl MediaSource for AudioWindow {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{tagged_mp3, TempFile};

    fn wave(payload: &[u8]) -> Vec<u8> {
        let mut body =
            b"WAVEfmt \x10\0\0\0\x55\0\x02\0\x44\xac\0\0\x80\x3e\0\0\x01\0\0\0data".to_vec();
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend(body);
        bytes
    }

    #[test]
    fn mpeg_wave_matches_reference_analysis_and_both_decks() {
        let reference_path = crate::test_support::fixture();
        let payload = std::fs::read(&reference_path).unwrap();
        let reference = crate::audio::analyze_file(&reference_path).unwrap();
        let duration = crate::playback::PreparedTrack::open(&reference_path)
            .unwrap()
            .duration();
        for prefix in [false, true] {
            let mut bytes = if prefix {
                b"ID3\x03\0\0\0\0\0\0".to_vec()
            } else {
                Vec::new()
            };
            bytes.extend(wave(&payload));
            // Bytes outside RIFF are never passed to the MPEG decoder.
            bytes.extend_from_slice(b"outside-container");
            let file = TempFile::new(if prefix { "mp3" } else { "wav" }, &bytes);
            let mut window = mpeg_wave_source(file.path()).unwrap();
            let mut extracted = Vec::new();
            window.read_to_end(&mut extracted).unwrap();
            assert_eq!(extracted, payload);
            let metadata = crate::track_metadata::read(file.path());
            assert_eq!(metadata.error.as_deref(), Some(WAVE_WARNING));
            assert!(metadata.duration_ms.is_some());
            let actual = crate::audio::analyze_file(file.path()).unwrap();
            assert_eq!(actual.frames, reference.frames);
            assert_eq!(actual.peak, reference.peak);
            assert_eq!(actual.rms, reference.rms);
            let (mixer, _output) = rodio::mixer::mixer(2, 48000);
            let engine = crate::playback::DualDeckEngine::from_mixer(mixer, 2, 48000);
            for deck in [crate::playback::DeckId::A, crate::playback::DeckId::B] {
                let track = crate::playback::PreparedTrack::open(file.path()).unwrap();
                assert_eq!(track.duration(), duration);
                engine.install(deck, track);
            }
            assert_eq!(std::fs::read(file.path()).unwrap(), bytes);
        }
    }

    #[test]
    fn mpeg_wave_rejects_bad_bounds_and_limits_reads_to_data() {
        let bytes = wave(b"odd");
        let mut extended = bytes.clone();
        extended.extend_from_slice(b"trailing");
        let file = TempFile::new("wav", &extended);
        let mut source = mpeg_wave_source(file.path()).unwrap();
        let mut read = Vec::new();
        source.read_to_end(&mut read).unwrap();
        assert_eq!(read, b"odd");
        source.seek(SeekFrom::End(5)).unwrap();
        assert_eq!(source.read(&mut [0; 8]).unwrap(), 0);
        source.seek(SeekFrom::End(-2)).unwrap();
        let mut read = Vec::new();
        source.read_to_end(&mut read).unwrap();
        assert_eq!(read, b"dd");
        for offset in [4, 16, 40] {
            let mut bad = bytes.clone();
            bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            let file = TempFile::new("wav", &bad);
            assert!(mpeg_wave_source(file.path()).is_err());
        }
        let mut duplicate = bytes[..36].to_vec();
        duplicate.extend_from_slice(&bytes[12..]);
        let len = duplicate.len() as u32 - 8;
        duplicate[4..8].copy_from_slice(&len.to_le_bytes());
        assert!(mpeg_wave_source(TempFile::new("wav", &duplicate).path()).is_err());
        assert!(mpeg_wave_source(TempFile::new("wav", &wave(b"")).path()).is_err());
        let mut chunks = b"RIFF\0\0\0\0WAVE".to_vec();
        for _ in 0..1025 {
            chunks.extend_from_slice(b"JUNK\0\0\0\0");
        }
        let len = chunks.len() as u32 - 8;
        chunks[4..8].copy_from_slice(&len.to_le_bytes());
        assert!(mpeg_wave_source(TempFile::new("wav", &chunks).path()).is_err());
        // A lying fmt code must not admit another codec through auto-detection.
        let pcm = crate::test_support::wav_file(1, 100);
        let disguised = TempFile::new("wav", &wave(&std::fs::read(pcm.path()).unwrap()));
        assert!(probe(disguised.path(), &MetadataOptions::default()).is_err());
        assert!(crate::playback::PreparedTrack::open(disguised.path()).is_err());
    }

    #[test]
    fn empty_and_tag_limited_sources_have_consistent_diagnostics() {
        for (bytes, expected) in [
            (Vec::new(), "Audio file is empty (0 bytes)"),
            (
                b"ID3\x03\0\0\0\0\0\0".to_vec(),
                "No audio decoded after leading ID3 tags (0 remaining bytes)",
            ),
            (
                [b"ID3\x03\0\0\0\0\0\0".as_slice(), &[0; 128]].concat(),
                "No audio decoded after leading ID3 tags (128 remaining bytes)",
            ),
        ] {
            let file = TempFile::new("mp3", &bytes);
            assert!(crate::track_metadata::read(file.path())
                .error
                .unwrap()
                .contains(expected));
            assert!(crate::audio::analyze_file(file.path())
                .err()
                .unwrap()
                .to_string()
                .contains(expected));
            assert!(crate::playback::PreparedTrack::open(file.path())
                .err()
                .unwrap()
                .contains(expected));
        }
    }

    #[test]
    #[ignore = "requires DRDJ_WAVE_REPORT referencing real read-only collection diagnostics"]
    fn real_collection_mpeg_wave_loads_audio() {
        let report = std::env::var_os("DRDJ_WAVE_REPORT").expect("DRDJ_WAVE_REPORT");
        let report: serde_json::Value =
            serde_json::from_reader(File::open(report).unwrap()).unwrap();
        let mut count = 0;
        for entry in report["failures"].as_array().unwrap() {
            if entry["error"].as_str() != Some("unsupported feature: wav: unsupported wave format")
            {
                continue;
            }
            let path = Path::new(entry["path"].as_str().unwrap());
            let before = std::fs::metadata(path).unwrap();
            assert_eq!(
                crate::track_metadata::read(path).error.as_deref(),
                Some(WAVE_WARNING)
            );
            let analysis = crate::audio::analyze_file(path).unwrap();
            let track = crate::playback::PreparedTrack::open(path).unwrap();
            assert!(analysis.frames > 0);
            assert!((track.duration().as_secs_f64() - analysis.duration_seconds()).abs() < 0.1);
            let after = std::fs::metadata(path).unwrap();
            assert_eq!(before.len(), after.len());
            assert_eq!(before.modified().unwrap(), after.modified().unwrap());
            count += 1;
            eprintln!("MPEG WAVE loaded/analyzed: {count}");
        }
        assert_eq!(count, 6);
    }

    #[test]
    #[ignore = "requires DRDJ_ID3_REPORT referencing real read-only collection diagnostics"]
    fn real_collection_id3_recovery_loads_audio() {
        let report = std::env::var_os("DRDJ_ID3_REPORT").expect("DRDJ_ID3_REPORT");
        let report: serde_json::Value =
            serde_json::from_reader(File::open(report).unwrap()).unwrap();
        let mut recovered = 0;
        for entry in report["failures"].as_array().unwrap() {
            let error = entry["error"].as_str().unwrap();
            if !error.contains("id3v2:") {
                continue;
            }
            let path = Path::new(entry["path"].as_str().unwrap());
            let before = std::fs::metadata(path).unwrap();
            let metadata = crate::track_metadata::read(path);
            assert!(
                metadata
                    .error
                    .as_deref()
                    .is_some_and(|e| e.starts_with(TAG_WARNING)),
                "case {recovered}: {:?}",
                metadata.error
            );
            let analysis = crate::audio::analyze_file(path).unwrap();
            let track = crate::playback::PreparedTrack::open(path).unwrap();
            assert!(analysis.frames > 0);
            // Rodio removes encoder priming/padding; raw analysis retains those frames.
            let trim_limit = 3. * 1152. / f64::from(analysis.sample_rate);
            assert!(
                (track.duration().as_secs_f64() - analysis.duration_seconds()).abs() <= trim_limit
            );
            let after = std::fs::metadata(path).unwrap();
            assert_eq!(before.len(), after.len());
            assert_eq!(before.modified().unwrap(), after.modified().unwrap());
            recovered += 1;
            eprintln!("ID3 recovery: {recovered} files loaded and analyzed");
        }
        assert!(recovered > 0);
        println!("real ID3 cases recovered: {recovered}");
    }

    #[test]
    fn id3_recovery_preserves_audio_loading_and_reports_skipped_tags() {
        let reference = crate::audio::analyze_file(&crate::test_support::fixture()).unwrap();
        let reference_duration =
            crate::playback::PreparedTrack::open(&crate::test_support::fixture())
                .unwrap()
                .duration();
        for compressed in [false, true] {
            let tagged = tagged_mp3("broken title", "artist", "album");
            let mut bytes = std::fs::read(tagged.path()).unwrap();
            if compressed {
                bytes[19] = 8;
            } else {
                bytes[20] = 255;
            }
            let file = TempFile::new("mp3", &bytes);
            let original = raw_probe(
                file.path(),
                File::open(file.path()).unwrap(),
                &MetadataOptions::default(),
            )
            .err()
            .unwrap();
            assert!(eligible(file.path(), &original));
            let metadata = crate::track_metadata::read(file.path());
            assert!(metadata.error.as_deref().unwrap().starts_with(TAG_WARNING));
            assert!(metadata.duration_ms.is_some());
            assert!(metadata.title.is_empty());
            let actual = crate::audio::analyze_file(file.path()).unwrap();
            assert_eq!(actual.frames, reference.frames);
            assert_eq!(actual.sample_rate, reference.sample_rate);
            assert_eq!(actual.channels, reference.channels);
            assert_eq!(actual.peak, reference.peak);
            assert_eq!(actual.rms, reference.rms);
            let (mixer, _output) = rodio::mixer::mixer(2, 48000);
            let engine = crate::playback::DualDeckEngine::from_mixer(mixer, 2, 48000);
            for deck in [crate::playback::DeckId::A, crate::playback::DeckId::B] {
                let track = crate::playback::PreparedTrack::open(file.path()).unwrap();
                assert_eq!(track.duration(), reference_duration);
                engine.install(deck, track);
            }
            assert_eq!(std::fs::read(file.path()).unwrap(), bytes);
        }
    }

    #[test]
    fn id3_window_validates_bounds_and_relative_seek() {
        let tagged = tagged_mp3("title", "artist", "album");
        let original = std::fs::read(tagged.path()).unwrap();
        let mut window = AudioWindow::after_id3(tagged.path()).unwrap();
        let mut actual = Vec::new();
        window.read_to_end(&mut actual).unwrap();
        // The fixture may itself carry a leading valid ID3 tag; strip the same prefix.
        assert!(original.ends_with(&actual));
        assert_eq!(window.seek(SeekFrom::End(-3)).unwrap(), window.len() - 3);
        let mut end = [0; 3];
        window.read_exact(&mut end).unwrap();
        assert_eq!(end, actual[actual.len() - 3..]);
        assert!(window.seek(SeekFrom::Current(-i64::MAX)).is_err());
        assert_eq!(window.seek(SeekFrom::Start(0)).unwrap(), 0);
        for (index, value) in [(3, 9), (5, 1), (6, 128), (6, 127)] {
            let mut bad = original.clone();
            bad[index] = value;
            let file = TempFile::new("mp3", &bad);
            assert!(AudioWindow::after_id3(file.path()).is_err());
        }
        let empty_audio = TempFile::new("mp3", b"ID3\x04\0\0\0\0\0\0");
        assert_eq!(AudioWindow::after_id3(empty_audio.path()).unwrap().len(), 0);
        assert!(
            recovery_source(tagged.path()).is_err(),
            "valid tags must not be skipped"
        );
        let bad = TempFile::new("mp3", b"not audio");
        assert!(probe(bad.path(), &MetadataOptions::default()).is_err());
    }

    #[test]
    fn id3_window_checks_footer_tag_count_and_size_cap() {
        let footer_header = *b"ID3\x04\0\x10\0\0\0\0";
        let mut bytes = footer_header.to_vec();
        bytes.extend_from_slice(b"3DI");
        bytes.extend_from_slice(&footer_header[3..]);
        bytes.extend_from_slice(b"audio");
        let file = TempFile::new("mp3", &bytes);
        let mut window = AudioWindow::after_id3(file.path()).unwrap();
        let mut audio = Vec::new();
        window.read_to_end(&mut audio).unwrap();
        assert_eq!(audio, b"audio");
        bytes[10] = b'X';
        let bad = TempFile::new("mp3", &bytes);
        assert!(AudioWindow::after_id3(bad.path()).is_err());
        for count in [8, 9] {
            let mut bytes = b"ID3\x04\0\0\0\0\0\0".repeat(count);
            bytes.extend_from_slice(b"audio");
            let file = TempFile::new("mp3", &bytes);
            assert_eq!(
                AudioWindow::after_id3(file.path()).is_ok(),
                count == MAX_TAGS
            );
        }
        // A sparse file exercises the cap without allocating a large tag buffer.
        for end in [MAX_TAG_BYTES, MAX_TAG_BYTES + 1] {
            let size = end - 10;
            let mut header = *b"ID3\x04\0\0\0\0\0\0";
            for (i, byte) in header[6..10].iter_mut().enumerate() {
                *byte = ((size >> (21 - 7 * i)) & 127) as u8;
            }
            let file = TempFile::new("mp3", &header);
            let mut writer = std::fs::OpenOptions::new()
                .write(true)
                .open(file.path())
                .unwrap();
            writer.set_len(end + 5).unwrap();
            writer.seek(SeekFrom::Start(end)).unwrap();
            std::io::Write::write_all(&mut writer, b"audio").unwrap();
            drop(writer);
            assert_eq!(
                AudioWindow::after_id3(file.path()).is_ok(),
                end == MAX_TAG_BYTES
            );
        }
    }
}
