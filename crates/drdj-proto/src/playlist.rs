//! PlaylistMessage — ProtoBuf representation of a playlist with ordered track references.

use super::*;
use serde::{Deserialize, Serialize};

/// Wire-format PlaylistMessage (ProtoBuf v2 compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistMessage {
    pub name: String,
    pub track_order: Vec<String>, // track UUIDs
    pub created_at: u64,
    pub modified_at: u64,
}

impl Default for PlaylistMessage {
    fn default() -> Self {
        Self {
            name: String::new(),
            track_order: Vec::new(),
            created_at: 0,
            modified_at: 0,
        }
    }
}

impl ProtoSerialize for PlaylistMessage {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), ProtoError> {
        buf.extend_from_slice(&encode_field_key(playlist_fields::NAME, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.name));

        for uuid in &self.track_order {
            buf.extend_from_slice(&encode_field_key(playlist_fields::TRACK_ORDER, WireType::LengthDelimited));
            buf.extend_from_slice(&encode_string(uuid));
        }

        buf.extend_from_slice(&encode_field_key(playlist_fields::CREATED_AT, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.created_at));

        buf.extend_from_slice(&encode_field_key(playlist_fields::MODIFIED_AT, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.modified_at));

        Ok(())
    }
}

impl ProtoDeserialize for PlaylistMessage {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, ProtoError> {
        let mut msg = PlaylistMessage::default();
        loop {
            let Some((field, wire)) = try_decode_field_key(reader)? else { break };
            match field {
                playlist_fields::NAME if wire == WireType::LengthDelimited => {
                    msg.name = decode_string(reader)?;
                }
                playlist_fields::TRACK_ORDER if wire == WireType::LengthDelimited => {
                    msg.track_order.push(decode_string(reader)?);
                }
                playlist_fields::CREATED_AT if wire == WireType::Varint => {
                    msg.created_at = decode_varint64(reader)?;
                }
                playlist_fields::MODIFIED_AT if wire == WireType::Varint => {
                    msg.modified_at = decode_varint64(reader)?;
                }
                _ => skip_field(reader, wire)?,
            }
        }
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playlist_roundtrip() {
        let original = PlaylistMessage {
            name: "My Playlist".to_string(),
            track_order: vec!["uuid-1".to_string(), "uuid-2".to_string(), "uuid-3".to_string()],
            created_at: 1700000000,
            modified_at: 1700100000,
        };
        let encoded = encode(&original).expect("encode");
        let decoded: PlaylistMessage = decode(&encoded).expect("decode");
        assert_eq!(original.name, decoded.name);
        assert_eq!(original.track_order, decoded.track_order);
        assert_eq!(original.created_at, decoded.created_at);
    }
}
