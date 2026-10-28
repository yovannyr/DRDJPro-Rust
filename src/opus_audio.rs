//! Ogg Opus decoding on control workers, never in the audio callback.
use crate::cancellation::Cancellation;
use opus_pure::{OggOpusReader, OggPacket, Trim, MAX_PACKET_SAMPLES};
use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

/// Inspect only the first Ogg page: the initial packet must be OpusHead.
pub(crate) fn is_opus(path: &Path) -> Result<bool, String> {
    let mut header = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(512)
        .read_to_end(&mut header)
        .map_err(|e| e.to_string())?;
    if header.len() < 27 || &header[..4] != b"OggS" {
        return Ok(false);
    }
    let start = 27 + usize::from(header[26]);
    Ok(header.get(start..start + 8) == Some(&b"OpusHead"[..]))
}

// Check real page boundaries, not arbitrary OggS byte sequences inside payloads.
// The decoder subsequently validates each page CRC while it reads the stream.
fn terminal_page(path: &Path, cancel: &Cancellation) -> Result<(i64, bool, usize), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    let mut offset = 0_u64;
    let mut serial = None;
    let mut completed_packets = 0_usize;
    let mut first_audio_page_packets = None;
    while offset < length {
        cancel.check()?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut header = [0_u8; 27];
        file.read_exact(&mut header)
            .map_err(|e| format!("Truncated Ogg page header: {e}"))?;
        if &header[..4] != b"OggS" || header[4] != 0 {
            return Err("Invalid Ogg page boundary".into());
        }
        let current_serial = u32::from_le_bytes(header[14..18].try_into().unwrap());
        if serial.is_some_and(|s| s != current_serial) {
            return Err("Chained or multiplexed Opus streams are not supported".into());
        }
        serial = Some(current_serial);
        let segments = usize::from(header[26]);
        if segments == 0 {
            return Err("Invalid Ogg segment table".into());
        }
        let mut table = [0_u8; 255];
        file.read_exact(&mut table[..segments])
            .map_err(|e| format!("Truncated Ogg segment table: {e}"))?;
        completed_packets = completed_packets
            .checked_add(table[..segments].iter().filter(|&&n| n < 255).count())
            .ok_or("Ogg packet count overflow")?;
        if completed_packets > 2 && first_audio_page_packets.is_none() {
            first_audio_page_packets = Some(completed_packets - 2);
        }
        let payload: u64 = table[..segments].iter().map(|&n| u64::from(n)).sum();
        offset = offset
            .checked_add(27 + segments as u64 + payload)
            .ok_or("Ogg page length overflow")?;
        if offset > length {
            return Err("Truncated Ogg page payload".into());
        }
        if header[5] & 0x04 != 0 {
            if offset != length {
                return Err(
                    "Chained streams or trailing data after Ogg EOS are not supported".into(),
                );
            }
            let granule = i64::from_le_bytes(header[6..14].try_into().unwrap());
            if granule < 0 {
                return Err("Invalid final Ogg granule".into());
            }
            return Ok((
                granule,
                payload == 0,
                first_audio_page_packets.ok_or("No Opus audio packets")?,
            ));
        }
    }
    Err("Ogg stream has no end-of-stream page".into())
}

struct CancellableReader {
    file: File,
    cancel: Cancellation,
}
impl Read for CancellableReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("Operation cancelled"));
        }
        self.file.read(buffer)
    }
}
impl Seek for CancellableReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("Operation cancelled"));
        }
        self.file.seek(position)
    }
}

// RFC 7845 permits a cropped stream to start at a nonzero granule. Determine
// the offset from all packets on the first completed audio page, then rewind.
fn initial_granule_offset(
    reader: &mut OggOpusReader<BufReader<CancellableReader>>,
    cancel: &Cancellation,
    first_page_packets: usize,
) -> Result<i64, String> {
    let mut packet = OggPacket::default();
    let mut first_granule = None;
    let mut first_page_eos = false;
    let mut decoded = 0_u64;
    for _ in 0..first_page_packets {
        cancel.check()?;
        if !reader
            .read_packet_into(&mut packet)
            .map_err(|e| e.to_string())?
        {
            break;
        }
        if packet.page_granule < 0 {
            return Err("Completed Opus page has no granule position".into());
        }
        first_granule = Some(packet.page_granule);
        first_page_eos |= packet.end_of_stream;
        decoded = decoded
            .checked_add(
                opus_pure::packet::samples_48k(&packet.data).map_err(|e| e.to_string())? as u64,
            )
            .ok_or("Opus granule overflow")?;
    }
    let granule = first_granule.ok_or("No Opus audio packets")? as u64;
    if granule < decoded && !first_page_eos {
        return Err("Invalid initial Opus granule position".into());
    }
    reader.rewind().map_err(|e| e.to_string())?;
    i64::try_from(granule.saturating_sub(decoded))
        .map_err(|_| "Opus granule offset overflow".into())
}

/// Decode a single Ogg Opus stream with RFC 7845 pre-skip and end trimming.
/// `limit` is measured in interleaved f32 samples, not packets or frames.
pub(crate) fn decode(
    path: &Path,
    limit: usize,
    cancel: &Cancellation,
) -> Result<(u16, u32, Vec<f32>), String> {
    cancel.check()?;
    let (final_granule, empty_eos, first_page_packets) = terminal_page(path, cancel)?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    let source = CancellableReader {
        file,
        cancel: cancel.clone(),
    };
    let mut reader = OggOpusReader::new(BufReader::new(source)).map_err(|e| e.to_string())?;
    let granule_offset = initial_granule_offset(&mut reader, cancel, first_page_packets)?;
    let head = reader.head().clone();
    let channels = usize::from(head.channel_count);
    if !(1..=2).contains(&channels) {
        return Err("Only mono or stereo Opus is supported".into());
    }
    const RATE: i32 = 48_000;
    let mut decoder = head.decoder(RATE).map_err(|e| e.to_string())?;
    let mut trim = Trim::new(&head, RATE, channels).map_err(|e| e.to_string())?;
    let mut block = vec![0.0_f32; MAX_PACKET_SAMPLES * channels];
    let mut packet = OggPacket::default();
    let mut next = OggPacket::default();
    let mut samples = Vec::new();
    if !reader
        .read_packet_into(&mut packet)
        .map_err(|e| e.to_string())?
    {
        return Err("No Opus audio packets".into());
    }
    loop {
        cancel.check()?;
        let has_next = reader
            .read_packet_into(&mut next)
            .map_err(|e| e.to_string())?;
        if !has_next && empty_eos {
            packet.end_of_stream = true;
            packet.page_granule = final_granule;
        }
        if packet.page_granule >= 0 {
            packet.page_granule = packet
                .page_granule
                .checked_sub(granule_offset)
                .filter(|value| *value >= 0)
                .ok_or("Invalid Opus granule offset")?;
        }
        let frames = decoder
            .decode(&packet.data, MAX_PACKET_SAMPLES, &mut block)
            .map_err(|e| e.to_string())?;
        if frames > MAX_PACKET_SAMPLES {
            return Err("Invalid Opus packet length".into());
        }
        let trimmed = trim.keep(&packet, &block[..frames * channels]);
        let new_len = samples
            .len()
            .checked_add(trimmed.len())
            .ok_or("PCM sample count overflow")?;
        if new_len > limit {
            return Err("Track exceeds the 256 MiB decoded PCM limit".into());
        }
        if trimmed.iter().any(|sample| !sample.is_finite()) {
            return Err("Non-finite audio sample".into());
        }
        if new_len > samples.capacity() {
            let target = samples.capacity().saturating_mul(2).max(new_len).min(limit);
            samples
                .try_reserve_exact(target - samples.len())
                .map_err(|e| format!("Cannot grow PCM memory: {e}"))?;
        }
        samples.extend_from_slice(trimmed);
        if !has_next {
            break;
        }
        std::mem::swap(&mut packet, &mut next);
    }
    cancel.check()?;
    if samples.is_empty() || samples.len() % channels != 0 {
        return Err("No complete audio frames decoded".into());
    }
    Ok((channels as u16, RATE as u32, samples))
}

pub(crate) fn metadata(
    path: &Path,
    cancel: &Cancellation,
) -> Result<crate::track_metadata::TrackMetadata, String> {
    cancel.check()?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    let source = CancellableReader {
        file,
        cancel: cancel.clone(),
    };
    let reader = OggOpusReader::new(BufReader::new(source)).map_err(|e| e.to_string())?;
    cancel.check()?;
    let tag = |name| {
        reader
            .tags()
            .get(name)
            .map(crate::track_metadata::text)
            .unwrap_or_default()
    };
    Ok(crate::track_metadata::TrackMetadata {
        title: tag("TITLE"),
        artist: tag("ARTIST"),
        album: tag("ALBUM"),
        scanned: true,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn trimmed_stereo_output_matches_ffmpeg_reference_and_limits_are_enforced() {
        let path = fixture("format-opus.opus");
        assert!(is_opus(&path).unwrap());
        let cancel = Cancellation::default();
        let (channels, rate, pcm) = decode(&path, 100_000, &cancel).unwrap();
        assert_eq!((channels, rate, pcm.len()), (2, 48_000, 96_000));
        // Reference samples were generated by FFmpeg/libopus, not this decoder.
        for (frame, expected) in [
            (0, 0.00102567_f32),
            (1, 0.00571955),
            (10, 0.04918844),
            (100, -0.04471633),
            (999, 0.07301649),
            (1000, 0.07562286),
            (12000, 0.00033275),
            (24000, 0.00059517),
            (47998, -0.00949738),
            (47999, -0.00363460),
        ] {
            assert!(
                (pcm[frame * 2] - expected).abs() < 0.002,
                "frame {frame}: {} vs {expected}",
                pcm[frame * 2]
            );
        }
        assert!(decode(&path, 100, &cancel).unwrap_err().contains("limit"));
        cancel.request();
        assert!(decode(&path, 100_000, &cancel).is_err());
        let invalid = crate::test_support::TempFile::new("opus", b"not an ogg file");
        assert!(!is_opus(invalid.path()).unwrap());
        assert!(decode(invalid.path(), 100_000, &Cancellation::default()).is_err());
    }

    fn page_crc(page: &mut [u8]) {
        page[22..26].fill(0);
        let mut crc = 0_u32;
        for &byte in page.iter() {
            crc ^= u32::from(byte) << 24;
            for _ in 0..8 {
                crc = if crc & 0x8000_0000 != 0 {
                    (crc << 1) ^ 0x04c1_1db7
                } else {
                    crc << 1
                };
            }
        }
        page[22..26].copy_from_slice(&crc.to_le_bytes());
    }

    #[test]
    fn end_page_and_cancellation_are_validated_without_rejecting_empty_eos() {
        let original = std::fs::read(fixture("format-opus.opus")).unwrap();
        let mut offset = 0_usize;
        let last = loop {
            assert_eq!(&original[offset..offset + 4], b"OggS");
            let count = usize::from(original[offset + 26]);
            let payload: usize = original[offset + 27..offset + 27 + count]
                .iter()
                .map(|&x| usize::from(x))
                .sum();
            let end = offset + 27 + count + payload;
            if end == original.len() {
                break offset;
            }
            offset = end;
        };
        let mut no_eos = original.clone();
        no_eos[last + 5] &= !0x04;
        page_crc(&mut no_eos[last..]);
        let truncated = crate::test_support::TempFile::new("opus", &no_eos);
        assert!(decode(truncated.path(), 100_000, &Cancellation::default())
            .unwrap_err()
            .contains("end-of-stream"));

        let mut corrupt = original.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        let corrupt = crate::test_support::TempFile::new("opus", &corrupt);
        assert!(decode(corrupt.path(), 100_000, &Cancellation::default()).is_err());
        let short = crate::test_support::TempFile::new("opus", &original[..original.len() - 1]);
        assert!(decode(short.path(), 100_000, &Cancellation::default()).is_err());

        // A valid final page with an empty packet is allowed by Ogg Opus.
        let mut empty_eos = no_eos;
        let mut page = vec![0_u8; 28];
        page[..4].copy_from_slice(b"OggS");
        page[5] = 0x04;
        page[6..14].copy_from_slice(&original[last + 6..last + 14]);
        page[14..18].copy_from_slice(&original[last + 14..last + 18]);
        let seq = u32::from_le_bytes(original[last + 18..last + 22].try_into().unwrap()) + 1;
        page[18..22].copy_from_slice(&seq.to_le_bytes());
        page[26] = 1;
        page_crc(&mut page);
        empty_eos.extend_from_slice(&page);
        let empty_eos = crate::test_support::TempFile::new("opus", &empty_eos);
        let expected = decode(
            &fixture("format-opus.opus"),
            100_000,
            &Cancellation::default(),
        )
        .unwrap()
        .2;
        let actual = decode(empty_eos.path(), 100_000, &Cancellation::default())
            .unwrap()
            .2;
        assert_eq!(actual, expected);

        let cancel = Cancellation::default();
        let mut source = CancellableReader {
            file: File::open(fixture("format-opus.opus")).unwrap(),
            cancel: cancel.clone(),
        };
        let mut header = [0_u8; 4];
        assert_eq!(source.read(&mut header).unwrap(), 4);
        cancel.request();
        assert!(source.read(&mut header).is_err());
        assert!(terminal_page(&fixture("format-opus.opus"), &cancel).is_err());
    }

    #[test]
    fn nonzero_initial_granule_preserves_exact_pcm_and_duration() {
        let mut bytes = std::fs::read(fixture("format-opus.opus")).unwrap();
        let mut offset = 0_usize;
        let mut pages_changed = 0;
        while offset < bytes.len() {
            let segments = usize::from(bytes[offset + 26]);
            let payload: usize = bytes[offset + 27..offset + 27 + segments]
                .iter()
                .map(|&x| usize::from(x))
                .sum();
            let end = offset + 27 + segments + payload;
            let granule = i64::from_le_bytes(bytes[offset + 6..offset + 14].try_into().unwrap());
            if granule > 0 {
                bytes[offset + 6..offset + 14].copy_from_slice(&(granule + 96_000).to_le_bytes());
                page_crc(&mut bytes[offset..end]);
                pages_changed += 1;
            }
            offset = end;
        }
        assert!(pages_changed >= 2);
        let cropped = crate::test_support::TempFile::new("opus", &bytes);
        let expected = decode(
            &fixture("format-opus.opus"),
            100_000,
            &Cancellation::default(),
        )
        .unwrap()
        .2;
        let actual = decode(cropped.path(), 100_000, &Cancellation::default())
            .unwrap()
            .2;
        assert_eq!(actual.len(), 96_000);
        assert_eq!(actual, expected);
    }

    #[test]
    fn equal_granules_on_distinct_audio_pages_trim_the_last_packet() {
        let mut bytes = std::fs::read(fixture("format-opus.opus")).unwrap();
        let mut offset = 0;
        let mut changed = 0;
        while offset < bytes.len() {
            let segments = usize::from(bytes[offset + 26]);
            let payload: usize = bytes[offset + 27..offset + 27 + segments]
                .iter()
                .map(|&x| usize::from(x))
                .sum();
            let end = offset + 27 + segments + payload;
            let granule = i64::from_le_bytes(bytes[offset + 6..offset + 14].try_into().unwrap());
            if granule > 0 {
                bytes[offset + 6..offset + 14].copy_from_slice(&144_000_i64.to_le_bytes());
                page_crc(&mut bytes[offset..end]);
                changed += 1;
            }
            offset = end;
        }
        assert_eq!(changed, 2);
        let file = crate::test_support::TempFile::new("opus", &bytes);
        let pcm = decode(file.path(), 100_000, &Cancellation::default())
            .unwrap()
            .2;
        assert_eq!(pcm.len(), 47_688 * 2);
    }

    #[test]
    fn mono_opus_and_opus_with_ogg_extension_are_recognized() {
        let mono = fixture("format-opus-mono.opus");
        let (channels, rate, pcm) = decode(&mono, 50_000, &Cancellation::default()).unwrap();
        assert_eq!((channels, rate, pcm.len()), (1, 48_000, 48_000));
        let bytes = std::fs::read(fixture("format-opus.opus")).unwrap();
        let ogg = crate::test_support::TempFile::new("ogg", &bytes);
        assert!(is_opus(ogg.path()).unwrap());
        assert_eq!(
            crate::audio::analyze_file(ogg.path()).unwrap().frames,
            48_000
        );
        assert!(crate::playback::PreparedTrack::open(ogg.path()).is_ok());
        let meta = crate::track_metadata::read(ogg.path());
        assert_eq!(meta.title, "Fixture Tone");
        assert_eq!(meta.artist, "Fixture Artist");
    }
}
