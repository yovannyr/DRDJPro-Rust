//! CueMessage — ProtoBuf representation of cue points.

use super::*;
use serde::{Deserialize, Serialize};

/// Wire-format CueMessage (ProtoBuf v2 compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueMessage {
    pub position_ms: u64,
    pub label: String,
    pub color: u32, // RGBA
    pub point_index: u32,
}

impl Default for CueMessage {
    fn default() -> Self {
        Self {
            position_ms: 0,
            label: String::new(),
            color: 0xFFFFFFFF,
            point_index: 0,
        }
    }
}

impl ProtoSerialize for CueMessage {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), ProtoError> {
        buf.extend_from_slice(&encode_field_key(cue_fields::POSITION_MS, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.position_ms));

        buf.extend_from_slice(&encode_field_key(cue_fields::LABEL, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.label));

        buf.extend_from_slice(&encode_field_key(cue_fields::COLOR, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.color as u64));

        buf.extend_from_slice(&encode_field_key(cue_fields::POINT_INDEX, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.point_index as u64));

        Ok(())
    }
}

impl ProtoDeserialize for CueMessage {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, ProtoError> {
        let mut msg = CueMessage::default();
        loop {
            let Some((field, wire)) = try_decode_field_key(reader)? else { break };
            match field {
                cue_fields::POSITION_MS if wire == WireType::Varint => {
                    msg.position_ms = decode_varint64(reader)?;
                }
                cue_fields::LABEL if wire == WireType::LengthDelimited => {
                    msg.label = decode_string(reader)?;
                }
                cue_fields::COLOR if wire == WireType::Varint => {
                    msg.color = decode_varint64(reader)? as u32;
                }
                cue_fields::POINT_INDEX if wire == WireType::Varint => {
                    msg.point_index = decode_varint64(reader)? as u32;
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
    fn test_cue_roundtrip() {
        let original = CueMessage {
            position_ms: 12345,
            label: "Hot Cue 1".to_string(),
            color: 0xFF00FF00,
            point_index: 1,
        };
        let encoded = encode(&original).expect("encode");
        let decoded: CueMessage = decode(&encoded).expect("decode");
        assert_eq!(original.position_ms, decoded.position_ms);
        assert_eq!(original.label, decoded.label);
        assert_eq!(original.color, decoded.color);
    }
}
