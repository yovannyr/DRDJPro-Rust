//! Lossless native track snapshots. Run on library/sync workers, not audio.
use super::Track;
use crate::{beatgrid::BeatGrid, hot_cues::HotCue, saved_loops::SavedLoop};
use drdj_proto::{
    AnalysisMessage, BeatGridMessage, CueMessage, LoopMessage, NativeMetadataMessage,
    NativeTrackMessage,
};
use std::{ffi::OsString, path::PathBuf, time::Duration};

fn encode_path(path: &std::path::Path) -> (Vec<u8>, String) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        (
            path.as_os_str()
                .encode_wide()
                .flat_map(u16::to_le_bytes)
                .collect(),
            "windows-utf16-le".into(),
        )
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        (path.as_os_str().as_bytes().to_vec(), "unix-bytes".into())
    }
}

fn decode_path(bytes: Vec<u8>, encoding: &str) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        if encoding != "windows-utf16-le" || !bytes.len().is_multiple_of(2) {
            return Err("native_path: incompatible encoding or odd UTF16 byte length".into());
        }
        let units: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        if units.contains(&0) {
            return Err("native_path: NUL is not allowed".into());
        }
        Ok(OsString::from_wide(&units).into())
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        if encoding != "unix-bytes" || bytes.contains(&0) {
            return Err("native_path: incompatible encoding or NUL".into());
        }
        Ok(OsString::from_vec(bytes).into())
    }
}

impl From<&BeatGrid> for BeatGridMessage {
    fn from(grid: &BeatGrid) -> Self {
        Self {
            bpm: grid.bpm(),
            anchor_ns: grid.anchor().as_nanos() as u64,
            beats_per_bar: u32::from(grid.beats_per_bar()),
            meter_denominator: u32::from(grid.meter_denominator()),
            warp_markers: grid
                .warp_markers()
                .iter()
                .map(|m| drdj_proto::WarpMarkerMessage {
                    beat: m.beat,
                    position_ns: m.position_ns,
                })
                .collect(),
            tempo_changes: grid
                .tempo_changes()
                .iter()
                .map(|m| drdj_proto::TempoChangeMessage {
                    beat: m.beat,
                    bpm: m.bpm,
                })
                .collect(),
            meter_changes: grid
                .meter_changes()
                .iter()
                .map(|m| drdj_proto::MeterChangeMessage {
                    bar: m.bar,
                    beats: u32::from(m.beats),
                    denominator: u32::from(m.denominator),
                })
                .collect(),
            points: Vec::new(),
        }
    }
}

impl TryFrom<BeatGridMessage> for BeatGrid {
    type Error = String;
    fn try_from(value: BeatGridMessage) -> Result<Self, Self::Error> {
        if !value.points.is_empty() {
            return Err("grid.points: native grid cannot represent sampled points".into());
        }
        if value
            .warp_markers
            .windows(2)
            .any(|p| p[0].beat >= p[1].beat)
            || value
                .tempo_changes
                .windows(2)
                .any(|p| p[0].beat >= p[1].beat)
            || value.meter_changes.windows(2).any(|p| p[0].bar >= p[1].bar)
        {
            return Err("grid: duplicate or unsorted changes".into());
        }
        let byte = |n| u8::try_from(n).map_err(|_| "grid.meter: value exceeds u8".to_owned());
        let mut grid = BeatGrid::new(value.bpm, Duration::from_nanos(value.anchor_ns))?
            .with_time_signature(byte(value.beats_per_bar)?, byte(value.meter_denominator)?)?;
        for m in value.meter_changes {
            grid = grid.with_time_signature_change(m.bar, byte(m.beats)?, byte(m.denominator)?)?;
        }
        for m in value.tempo_changes {
            grid = grid.with_tempo_change(m.beat, m.bpm)?;
        }
        for m in value.warp_markers {
            grid = grid.with_warp_marker(m.beat, Duration::from_nanos(m.position_ns))?;
        }
        Ok(grid)
    }
}

fn encode_cues(cues: &crate::hot_cues::HotCues) -> Vec<CueMessage> {
    cues.slots()
        .iter()
        .enumerate()
        .filter_map(|(slot, cue)| {
            cue.as_ref().map(|cue| CueMessage {
                position_ms: cue.position().as_millis() as u64,
                label: cue.name().into(),
                color: cue.rgb(),
                point_index: slot as u32,
                position_ns: Some(cue.position().as_nanos() as u64),
                slot: Some(slot as u32),
            })
        })
        .collect()
}

fn encode_loops(loops: &crate::saved_loops::SavedLoops) -> Vec<LoopMessage> {
    loops
        .slots()
        .iter()
        .enumerate()
        .filter_map(|(slot, region)| {
            region.as_ref().map(|region| LoopMessage {
                start_ns: Some(region.start().as_nanos() as u64),
                end_ns: Some(region.end().as_nanos() as u64),
                name: Some(region.name().into()),
                slot: Some(slot as u32),
                ..Default::default()
            })
        })
        .collect()
}

impl From<&Track> for NativeTrackMessage {
    fn from(track: &Track) -> Self {
        let metadata = &track.metadata;
        let (native_path, path_encoding) = encode_path(&track.path);
        Self {
            id: track.id,
            name: track.name.clone(),
            format: track.format.clone(),
            available: track.available,
            metadata: NativeMetadataMessage {
                prefer_imported_tags: metadata.prefer_imported_tags.then_some(true),
                title: metadata.title.clone(),
                artist: metadata.artist.clone(),
                album: metadata.album.clone(),
                duration_ms: metadata.duration_ms,
                duration_exact: metadata.duration_exact,
                scanned: metadata.scanned,
                error: metadata.error.clone(),
                analysis: metadata.analysis.as_ref().map(|a| AnalysisMessage {
                    file_path: a.file_path.clone(),
                    bpm: a.bpm,
                    musical_key: a.musical_key.clone(),
                    gain: a.gain,
                    waveform_overview: a.waveform_overview.clone(),
                    is_corrupt: a.is_corrupt,
                }),
            },
            native_path,
            path_encoding,
            grid: track.grid.as_ref().map(Into::into),
            hot_cues: encode_cues(&track.hot_cues),
            saved_loops: encode_loops(&track.saved_loops),
        }
    }
}

impl From<Track> for NativeTrackMessage {
    fn from(track: Track) -> Self {
        let (native_path, path_encoding) = encode_path(&track.path);
        let hot_cues = encode_cues(&track.hot_cues);
        let saved_loops = encode_loops(&track.saved_loops);
        let m = track.metadata;
        Self {
            id: track.id,
            name: track.name,
            format: track.format,
            available: track.available,
            native_path,
            path_encoding,
            hot_cues,
            saved_loops,
            grid: track.grid.as_ref().map(Into::into),
            metadata: NativeMetadataMessage {
                prefer_imported_tags: m.prefer_imported_tags.then_some(true),
                title: m.title,
                artist: m.artist,
                album: m.album,
                duration_ms: m.duration_ms,
                duration_exact: m.duration_exact,
                scanned: m.scanned,
                error: m.error,
                analysis: m.analysis.map(|a| AnalysisMessage {
                    file_path: a.file_path,
                    bpm: a.bpm,
                    musical_key: a.musical_key,
                    gain: a.gain,
                    waveform_overview: a.waveform_overview,
                    is_corrupt: a.is_corrupt,
                }),
            },
        }
    }
}

impl TryFrom<NativeTrackMessage> for Track {
    type Error = String;
    fn try_from(value: NativeTrackMessage) -> Result<Self, Self::Error> {
        drdj_proto::Contract::validate(&value).map_err(|error| error.to_string())?;
        if value.id <= 0 || value.id == i32::MAX {
            return Err("id: invalid native track identity".into());
        }
        let path = decode_path(value.native_path, &value.path_encoding)?;
        if !path.is_absolute() {
            return Err("native_path: must be absolute".into());
        }
        let m = value.metadata;
        let metadata = crate::track_metadata::TrackMetadata {
            prefer_imported_tags: m.prefer_imported_tags.unwrap_or(false),
            title: m.title,
            artist: m.artist,
            album: m.album,
            duration_ms: m.duration_ms,
            duration_exact: m.duration_exact,
            scanned: m.scanned,
            error: m.error,
            analysis: m
                .analysis
                .map(|a| crate::audio_analyzer::TrackAnalysisResult {
                    file_path: a.file_path,
                    bpm: a.bpm,
                    musical_key: a.musical_key,
                    gain: a.gain,
                    waveform_overview: a.waveform_overview,
                    is_corrupt: a.is_corrupt,
                }),
        };
        if !metadata.validate() {
            return Err("metadata: native validation failed".into());
        }
        let mut track = Track::new(value.id, path, value.available);
        track.name = value.name;
        track.format = value.format;
        track.metadata = metadata;
        track.grid = value
            .grid
            .map(TryInto::try_into)
            .transpose()
            .map_err(|e: String| format!("grid: {e}"))?;
        let mut slots = 0u8;
        for cue in value.hot_cues {
            let slot = cue.slot.ok_or("hot_cues.slot: required")?;
            if slot >= 8 || slots & (1 << slot) != 0 {
                return Err("hot_cues.slot: duplicate or outside 0..7".into());
            }
            slots |= 1 << slot;
            let ns = cue.position_ns.ok_or("hot_cues.position_ns: required")?;
            if cue.position_ms != ns / 1_000_000 || cue.point_index != slot {
                return Err("hot_cues: inconsistent legacy position or slot".into());
            }
            track.hot_cues.set(
                slot as usize,
                Some(HotCue::new(Duration::from_nanos(ns), cue.label, cue.color)?),
            )?;
        }
        slots = 0;
        for region in value.saved_loops {
            let slot = region.slot.ok_or("saved_loops.slot: required")?;
            if slot >= 8 || slots & (1 << slot) != 0 {
                return Err("saved_loops.slot: duplicate or outside 0..7".into());
            }
            slots |= 1 << slot;
            let start = region.start_ns.ok_or("saved_loops.start_ns: required")?;
            let end = region.end_ns.ok_or("saved_loops.end_ns: required")?;
            track.saved_loops.set(
                slot as usize,
                Some(SavedLoop::new(
                    Duration::from_nanos(start),
                    Duration::from_nanos(end),
                    region.name.ok_or("saved_loops.name: required")?,
                )?),
            )?;
        }
        track.refresh_search();
        Ok(track)
    }
}

/// Validated input with an infallible conversion to the native Track.
/// Arbitrary wire values must first pass the fallible constructor.
pub struct ValidatedTrackMessage(Track);
impl TryFrom<NativeTrackMessage> for ValidatedTrackMessage {
    type Error = String;
    fn try_from(value: NativeTrackMessage) -> Result<Self, String> {
        Track::try_from(value).map(Self)
    }
}
impl From<ValidatedTrackMessage> for Track {
    fn from(value: ValidatedTrackMessage) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rich_track() -> Track {
        let path = std::env::temp_dir().join("DJ_🎵.flac");
        let mut track = Track::new(42, path, false);
        track.metadata.title = "Entrada 🎵".into();
        track.metadata.duration_ms = Some(0);
        track.metadata.error = Some(String::new());
        track.metadata.duration_exact = true;
        track.metadata.scanned = true;
        track.metadata.prefer_imported_tags = true;
        track.metadata.analysis = Some(crate::audio_analyzer::TrackAnalysisResult {
            file_path: "analysis-source".into(),
            bpm: 128.125,
            musical_key: "8A".into(),
            gain: 0.75,
            waveform_overview: vec![0, 255, 1],
            is_corrupt: true,
        });
        track.grid = Some(
            BeatGrid::new(128.123456789, Duration::from_nanos(123))
                .unwrap()
                .with_time_signature(3, 8)
                .unwrap()
                .with_time_signature_change(2, 5, 4)
                .unwrap()
                .with_tempo_change(8, 124.987654321)
                .unwrap()
                .with_warp_marker(10, Duration::from_secs(5))
                .unwrap(),
        );
        track
            .hot_cues
            .set(
                7,
                Some(HotCue::new(Duration::from_nanos(101), "Fin".into(), 0x123456).unwrap()),
            )
            .unwrap();
        track
            .saved_loops
            .set(
                5,
                Some(
                    SavedLoop::new(
                        Duration::from_nanos(1),
                        Duration::from_nanos(2),
                        "Micro".into(),
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
        track
    }
    #[test]
    fn native_track_roundtrip_preserves_all_fields_and_sparse_nanosecond_slots() {
        let track = rich_track();
        let original = NativeTrackMessage::from(&track);
        assert_eq!(
            drdj_proto::from_json_checked::<NativeTrackMessage>(
                &drdj_proto::to_json_checked(&original).unwrap()
            )
            .unwrap(),
            original
        );
        let bytes = drdj_proto::encode_checked(&original).unwrap();
        let decoded = drdj_proto::decode_checked::<NativeTrackMessage>(&bytes).unwrap();
        let restored: Track = ValidatedTrackMessage::try_from(decoded).unwrap().into();
        assert_eq!(NativeTrackMessage::from(&restored), original);
        assert_eq!(restored.path, track.path);
        assert_eq!(restored.metadata, track.metadata);
        assert_eq!(restored.grid, track.grid);
        assert_eq!(restored.hot_cues, track.hot_cues);
        assert_eq!(restored.saved_loops, track.saved_loops);
        assert!(!restored.available);
        let mut absent = original;
        absent.metadata.duration_ms = None;
        absent.metadata.duration_exact = false;
        absent.metadata.error = None;
        absent.metadata.analysis = None;
        let restored = Track::try_from(absent.clone()).unwrap();
        assert_eq!(NativeTrackMessage::from(restored), absent);
    }
    #[test]
    fn malformed_snapshots_do_not_silently_overwrite_native_slots_or_grid_changes() {
        let original = NativeTrackMessage::from(rich_track());
        let mut duplicate = original.clone();
        duplicate.hot_cues.push(duplicate.hot_cues[0].clone());
        assert!(Track::try_from(duplicate)
            .err()
            .unwrap()
            .contains("hot_cues["));
        let mut duplicate = original.clone();
        duplicate.saved_loops[0].slot = Some(8);
        assert!(Track::try_from(duplicate)
            .err()
            .unwrap()
            .contains("saved_loops["));
        let mut duplicate = original.clone();
        let grid = duplicate.grid.as_mut().unwrap();
        grid.tempo_changes.push(grid.tempo_changes[0]);
        assert!(Track::try_from(duplicate).is_err());
        let mut wrong_encoding = original;
        wrong_encoding.path_encoding = "foreign-platform".into();
        assert!(Track::try_from(wrong_encoding)
            .err()
            .unwrap()
            .contains("path_encoding"));
    }
    #[test]
    fn native_tracks_and_existing_playlists_cross_the_sync_contract_together() {
        use drdjpro_ui_spike::preparation::playlist_manager::{Playlist, PlaylistType};
        let original = NativeTrackMessage::from(rich_track());
        let playlist = Playlist::new(17, "Live".into(), PlaylistType::User);
        let playlist = drdj_proto::PlaylistMessage::try_from(&playlist).unwrap();
        let request = drdj_proto::SyncRequest {
            schema_version: drdj_proto::SCHEMA_VERSION,
            native_updates: vec![original.clone()],
            playlists: vec![playlist.clone()],
            ..Default::default()
        };
        let request =
            drdj_proto::decode_checked(&drdj_proto::encode_checked(&request).unwrap()).unwrap();
        let response = drdj_proto::respond(request);
        let response: drdj_proto::SyncResponse =
            drdj_proto::decode_checked(&drdj_proto::encode_checked(&response).unwrap()).unwrap();
        assert_eq!(response.native_updates, vec![original.clone()]);
        assert_eq!(response.playlists, vec![playlist]);
        let track = Track::try_from(response.native_updates.into_iter().next().unwrap()).unwrap();
        assert_eq!(NativeTrackMessage::from(track), original);
        assert_eq!(
            Playlist::try_from(response.playlists.into_iter().next().unwrap())
                .unwrap()
                .id,
            17
        );
    }
    #[test]
    fn malformed_native_snapshots_fail_both_checked_encoding_and_native_import() {
        let original = NativeTrackMessage::from(rich_track());
        let cases: Vec<NativeTrackMessage> = vec![
            {
                let mut v = original.clone();
                v.metadata.duration_ms = None;
                v
            },
            {
                let mut v = original.clone();
                v.metadata.duration_ms = Some(604_800_001);
                v
            },
            {
                let mut v = original.clone();
                v.metadata.error = Some("x".repeat(513));
                v
            },
            {
                let mut v = original.clone();
                v.metadata.analysis.as_mut().unwrap().gain = 4.1;
                v
            },
            {
                let mut v = original.clone();
                v.hot_cues[0].color = 0xffff_ffff;
                v
            },
            {
                let mut v = original.clone();
                v.hot_cues[0].position_ns = None;
                v
            },
            {
                let mut v = original.clone();
                v.saved_loops[0].name = Some("invalid\nname".into());
                v
            },
            {
                let mut v = original.clone();
                (v.native_path, v.path_encoding) =
                    encode_path(std::path::Path::new("relative.wav"));
                v
            },
            {
                let mut v = original;
                v.path_encoding = "foreign".into();
                v
            },
        ];
        for value in cases {
            assert!(drdj_proto::encode_checked(&value).is_err());
            assert!(Track::try_from(value).is_err());
        }
    }
    #[test]
    fn native_path_encoding_preserves_non_unicode_code_units() {
        #[cfg(windows)]
        let path = {
            use std::os::windows::ffi::OsStringExt;
            PathBuf::from(OsString::from_wide(&[67, 58, 92, 0xd800, 46, 119, 97, 118]))
        };
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(OsString::from_vec(vec![b'/', 0xff, b'.', b'w', b'a', b'v']))
        };
        let (bytes, encoding) = encode_path(&path);
        assert_eq!(decode_path(bytes, &encoding).unwrap(), path);
    }
}
