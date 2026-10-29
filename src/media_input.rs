//! Worker-only recovery for malformed leading MP3 ID3 tags. Never rewrites media.
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

pub(crate) fn probe(
    path: &Path,
    options: &MetadataOptions,
) -> Result<(ProbeResult, Option<String>), Error> {
    match raw_probe(path, File::open(path)?, options) {
        Ok(result) => Ok((result, None)),
        Err(error) if eligible(path, &error) => {
            let source = AudioWindow::after_id3(path)?;
            let result = raw_probe(path, source, options)?;
            Ok((result, Some(format!("{TAG_WARNING}{error}"))))
        }
        Err(error) => Err(error),
    }
}

/// Rodio hides the parser error; independently confirm the narrow recovery condition.
pub(crate) fn recovery_source(path: &Path) -> Result<AudioWindow, Error> {
    match raw_probe(path, File::open(path)?, &MetadataOptions::default()) {
        Err(error) if eligible(path, &error) => Ok(AudioWindow::after_id3(path)?),
        Err(error) => Err(error),
        Ok(_) => Err(Error::Unsupported("not an ID3 recovery case")),
    }
}

/// A seekable view whose byte zero is the validated end of the leading ID3 tags.
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
            if end >= len || end > MAX_TAG_BYTES {
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
        file.read_exact(&mut next)?;
        if &next == b"ID3" {
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
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid or excessive leading ID3 tag bounds",
    )
}
impl Read for AudioWindow {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
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
        assert!(AudioWindow::after_id3(empty_audio.path()).is_err());
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
