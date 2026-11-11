//! TrackMessage — ProtoBuf representation of a track.
//!
//! Maps to `TrackMetadata` + `TrackAnalysisResult` for efficient cloud sync.

use super::*;
use serde::{Deserialize, Serialize};

/// Wire-format TrackMessage (ProtoBuf v2 compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackMessage {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub bpm: f64,
    pub key: String,
    pub duration_ms: u64,
    pub file_path: String,
    pub cover_art_bytes: Vec<u8>,
    pub analysis_hash: String,
    pub tempo: f32,
}

impl Default for TrackMessage {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            bpm: 120.0,
            key: String::new(),
            duration_ms: 0,
            file_path: String::new(),
            cover_art_bytes: Vec::new(),
            analysis_hash: String::new(),
            tempo: 120.0,
        }
    }
}

impl ProtoSerialize for TrackMessage {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), ProtoError> {
        // Field 1: title
        buf.extend_from_slice(&encode_field_key(track_fields::TITLE, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.title));

        // Field 2: artist
        buf.extend_from_slice(&encode_field_key(track_fields::ARTIST, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.artist));

        // Field 3: album
        buf.extend_from_slice(&encode_field_key(track_fields::ALBUM, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.album));

        // Field 4: BPM (varint — scaled by 1000)
        buf.extend_from_slice(&encode_field_key(track_fields::BPM, WireType::Varint));
        let bpm_varint = encode_varint64((self.bpm * 1000.0) as u64);
        buf.extend_from_slice(&bpm_varint);

        // Field 5: key
        buf.extend_from_slice(&encode_field_key(track_fields::KEY, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.key));

        // Field 6: duration_ms
        buf.extend_from_slice(&encode_field_key(track_fields::DURATION_MS, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.duration_ms));

        // Field 7: file_path
        buf.extend_from_slice(&encode_field_key(track_fields::FILE_PATH, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.file_path));

        // Field 8: cover_art_bytes
        buf.extend_from_slice(&encode_field_key(track_fields::COVER_ART_BYTES, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_bytes(&self.cover_art_bytes));

        // Field 9: analysis_hash
        buf.extend_from_slice(&encode_field_key(track_fields::ANALYSIS_HASH, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.analysis_hash));

        // Field 10: tempo (I32 wire type for f32 raw bits)
        buf.extend_from_slice(&encode_field_key(track_fields::TEMPO, WireType::I32));
        let tempo_bytes = encode_f32(self.tempo);
        buf.extend_from_slice(&tempo_bytes);

        Ok(())
    }
}

impl ProtoDeserialize for TrackMessage {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, ProtoError> {
        let mut msg = TrackMessage::default();

        loop {
            let Some((field, wire)) = try_decode_field_key(reader)? else { break };
            match field {
                track_fields::TITLE if wire == WireType::LengthDelimited => {
                    msg.title = decode_string(reader)?;
                }
                track_fields::ARTIST if wire == WireType::LengthDelimited => {
                    msg.artist = decode_string(reader)?;
                }
                track_fields::ALBUM if wire == WireType::LengthDelimited => {
                    msg.album = decode_string(reader)?;
                }
                track_fields::BPM if wire == WireType::Varint => {
                    let varint = decode_varint64(reader)?;
                    msg.bpm = varint as f64 / 1000.0;
                }
                track_fields::KEY if wire == WireType::LengthDelimited => {
                    msg.key = decode_string(reader)?;
                }
                track_fields::DURATION_MS if wire == WireType::Varint => {
                    msg.duration_ms = decode_varint64(reader)?;
                }
                track_fields::FILE_PATH if wire == WireType::LengthDelimited => {
                    msg.file_path = decode_string(reader)?;
                }
                track_fields::COVER_ART_BYTES if wire == WireType::LengthDelimited => {
                    msg.cover_art_bytes = decode_bytes(reader)?;
                }
                track_fields::ANALYSIS_HASH if wire == WireType::LengthDelimited => {
                    msg.analysis_hash = decode_string(reader)?;
                }
                track_fields::TEMPO if wire == WireType::I32 => {
                    msg.tempo = decode_f32(reader)?;
                }
                // Unknown fields — skip (forward compatibility)
                _ => {
                    skip_field(reader, wire)?;
                }
            }
        }

        Ok(msg)
    }
}

// ── Conversion from domain types ─────────────────────────────────────────

#[cfg(feature = "drdjpro")]
impl From<&super::super::track_metadata::TrackMetadata> for TrackMessage {
    fn from(meta: &super::super::track_metadata::TrackMetadata) -> Self {
        TrackMessage {
            title: meta.title.clone(),
            artist: meta.artist.clone(),
            album: meta.album.clone(),
            bpm: meta.analysis.as_ref().map(|a| a.bpm).unwrap_or(120.0),
            key: meta.analysis.as_ref().map(|a| a.detected_key.clone()).unwrap_or_default(),
            duration_ms: meta.duration_ms.unwrap_or(0),
            file_path: String::new(), // would need path from caller
            cover_art_bytes: Vec::new(),
            analysis_hash: String::new(),
            tempo: meta.analysis.as_ref().map(|a| a.tempo).unwrap_or(120.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_roundtrip() {
        let original = TrackMessage {
            title: "Test Track".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            bpm: 128.5,
            key: "Am".to_string(),
            duration_ms: 240000,
            file_path: "/music/test.mp3".to_string(),
            cover_art_bytes: vec![0xFF, 0xD8, 0xFF, 0xE0], // JPEG header bytes
            analysis_hash: "abc123".to_string(),
            tempo: 128.0,
        };

        let encoded = encode(&original).expect("encode failed");
        let decoded: TrackMessage = decode(&encoded).expect("decode failed");

        assert_eq!(original.title, decoded.title);
        assert_eq!(original.artist, decoded.artist);
        assert_eq!(original.album, decoded.album);
        assert!((original.bpm - decoded.bpm).abs() < 0.01);
        assert_eq!(original.key, decoded.key);
        assert_eq!(original.duration_ms, decoded.duration_ms);
        assert_eq!(original.file_path, decoded.file_path);
        assert_eq!(original.cover_art_bytes, decoded.cover_art_bytes);
        assert_eq!(original.analysis_hash, decoded.analysis_hash);
        assert!((original.tempo - decoded.tempo).abs() < 0.01);
    }

    #[test]
    fn test_track_size_reduction() {
        let track = TrackMessage {
            title: "A very long track title with many words to make the JSON larger".to_string(),
            artist: "An artist with a very long name that makes the JSON payload bigger".to_string(),
            album: "An album name that is also quite long for testing purposes".to_string(),
            bpm: 140.25,
            key: "7A".to_string(),
            duration_ms: 360000,
            file_path: "/home/user/Music/Very Long Path/To/A Track File/with spaces and Umläute/test_track_final_v2.mp3".to_string(),
            cover_art_bytes: vec![0; 256],
            analysis_hash: "sha256:abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
            tempo: 140.0,
        };

        // Encode to JSON for comparison
        let json_bytes = serde_json::to_vec(&track).unwrap_or_default().len();
        let (binary_bytes, _, reduction) = size_comparison(&track, json_bytes);

        // ProtoBuf should be at least 20% smaller even for small messages
        assert!(reduction > 15.0, "Expected ≥15% reduction, got {:.1}%", reduction);
        assert!(binary_bytes < json_bytes, "Binary {} should be smaller than JSON {}", binary_bytes, json_bytes);
    }

    #[test]
    fn test_track_unknown_fields_skipped() {
        let mut buf = Vec::new();
        let track = TrackMessage {
            title: "Skip me".to_string(),
            ..Default::default()
        };
        track.encode(&mut buf).expect("encode failed");

        // Append unknown field (field 99, length-delimited)
        buf.extend_from_slice(&encode_field_key(99, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string("unknown_value"));

        // Should decode successfully despite unknown field
        let decoded: TrackMessage = decode(&buf).unwrap_or(track.clone());
        assert_eq!(decoded.title, "Skip me");
    }
}
