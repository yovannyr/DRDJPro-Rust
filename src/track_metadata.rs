//! Header/tag inspection on library/load workers; no PCM decode or tag writes.
use std::{fs::File, path::Path};
use symphonia::core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{Limit, MetadataOptions, MetadataRevision, StandardTagKey},
    probe::Hint,
};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: Option<u64>,
    pub duration_exact: bool,
    pub scanned: bool,
    pub error: Option<String>,
}
pub const MAX_TEXT: usize = 512;
pub const MAX_DURATION_MS: u64 = 7 * 24 * 60 * 60 * 1000;
impl TrackMetadata {
    pub fn validate(&self) -> bool {
        [&self.title, &self.artist, &self.album]
            .iter()
            .all(|s| s.chars().count() <= MAX_TEXT)
            && self
                .error
                .as_ref()
                .is_none_or(|s| s.chars().count() <= MAX_TEXT)
            && self.duration_ms.is_none_or(|v| v <= MAX_DURATION_MS)
            && (!self.duration_exact || self.duration_ms.is_some())
    }
    pub fn duration_text(&self) -> String {
        self.duration_ms.map_or_else(
            || "—".into(),
            |ms| {
                format!(
                    "{}{:02}:{:02}",
                    if self.duration_exact { "" } else { "~" },
                    ms / 60000,
                    (ms / 1000) % 60
                )
            },
        )
    }
}
fn text(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXT)
        .collect()
}
fn tags(output: &mut TrackMetadata, revision: &MetadataRevision) {
    for tag in revision.tags() {
        let target = match tag.std_key {
            Some(StandardTagKey::TrackTitle) => &mut output.title,
            Some(StandardTagKey::Artist) => &mut output.artist,
            Some(StandardTagKey::Album) => &mut output.album,
            _ => continue,
        };
        let value = text(&tag.value.to_string());
        if !value.is_empty() {
            *target = value;
        }
    }
}
pub fn read(path: &Path) -> TrackMetadata {
    let result = (|| -> Result<TrackMetadata, Box<dyn std::error::Error>> {
        let stream = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
            hint.with_extension(extension);
        }
        let mut probed = symphonia::default::get_probe().format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions {
                limit_metadata_bytes: Limit::Maximum(512 * 1024),
                limit_visual_bytes: Limit::Maximum(0),
            },
        )?;
        let mut output = TrackMetadata {
            scanned: true,
            ..Default::default()
        };
        if let Some(metadata) = probed.metadata.get() {
            if let Some(revision) = metadata.current() {
                tags(&mut output, revision);
            }
        }
        if let Some(revision) = probed.format.metadata().current() {
            tags(&mut output, revision);
        }
        let track = probed
            .format
            .default_track()
            .ok_or("No default audio track")?;
        let params = &track.codec_params;
        if let (Some(frames), Some(rate)) = (params.n_frames, params.sample_rate) {
            if rate > 0 {
                output.duration_ms = frames
                    .checked_mul(1000)
                    .map(|v| v / u64::from(rate))
                    .filter(|v| *v <= MAX_DURATION_MS);
            }
        }
        Ok(output)
    })();
    result.unwrap_or_else(|e| TrackMetadata {
        scanned: true,
        error: Some(text(&e.to_string())),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{tagged_mp3, tagged_wav, wav_file, TempFile};
    #[test]
    fn reads_utf8_mp3_tags_and_header_duration_without_pcm_decode() {
        let file = tagged_mp3("Canción 日本", "DJ Ñ", "Album 2026");
        let meta = read(file.path());
        assert_eq!(meta.title, "Canción 日本");
        assert_eq!(meta.artist, "DJ Ñ");
        assert_eq!(meta.album, "Album 2026");
        assert!(meta.error.is_none());
        assert!(meta.scanned);
        assert!((8900..9300).contains(&meta.duration_ms.unwrap()));
    }
    #[test]
    fn reads_wav_info_tags_and_handles_missing_tags_without_inventing_values() {
        let file = tagged_wav("WAV title", "WAV artist", "WAV album");
        let meta = read(file.path());
        assert_eq!(meta.title, "WAV title");
        assert_eq!(meta.artist, "WAV artist");
        assert_eq!(meta.album, "WAV album");
        assert_eq!(meta.duration_ms, Some(1000));
        assert!(meta.error.is_none());
        let blank = wav_file(1, 480);
        let meta = read(blank.path());
        assert_eq!(meta.title, "");
        assert_eq!(meta.artist, "");
        assert_eq!(meta.album, "");
        assert_eq!(meta.duration_ms, Some(10));
        assert!(meta.error.is_none());
    }
    #[test]
    fn invalid_or_missing_media_has_unknown_duration_and_explicit_error() {
        let file = TempFile::new("mp3", b"not audio");
        let meta = read(file.path());
        assert!(meta.error.is_some());
        assert_eq!(meta.duration_ms, None);
        assert_eq!(meta.duration_text(), "—");
        assert!(read(Path::new("missing-track.wav")).error.is_some());
    }
    #[test]
    fn text_and_duration_bounds_are_enforced_and_control_characters_are_removed() {
        let file = tagged_mp3(&format!("  {}\n", "É".repeat(600)), "", "");
        let meta = read(file.path());
        assert_eq!(meta.title.chars().count(), MAX_TEXT);
        assert!(meta.validate());
        let invalid = TrackMetadata {
            duration_ms: Some(u64::MAX),
            ..Default::default()
        };
        assert!(!invalid.validate());
        assert_eq!(text("  One\nTwo  "), "OneTwo");
    }
}
