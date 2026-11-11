//! LoopMessage — ProtoBuf representation of loop regions.

use super::*;
use serde::{Deserialize, Serialize};

/// Wire-format LoopMessage (ProtoBuf v2 compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoopMessage {
    pub start_bar: u32,
    pub end_bar: u32,
    pub start_beat: u64,
    pub end_beat: u64,
    pub quantization: String,
}

impl Default for LoopMessage {
    fn default() -> Self {
        Self {
            start_bar: 0,
            end_bar: 0,
            start_beat: 0,
            end_beat: 0,
            quantization: "Bar".to_string(),
        }
    }
}

impl ProtoSerialize for LoopMessage {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), ProtoError> {
        buf.extend_from_slice(&encode_field_key(loop_fields::START_BAR, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.start_bar as u64));

        buf.extend_from_slice(&encode_field_key(loop_fields::END_BAR, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.end_bar as u64));

        buf.extend_from_slice(&encode_field_key(loop_fields::START_BEAT, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.start_beat));

        buf.extend_from_slice(&encode_field_key(loop_fields::END_BEAT, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.end_beat));

        buf.extend_from_slice(&encode_field_key(loop_fields::QUANTIZATION, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_string(&self.quantization));

        Ok(())
    }
}

impl ProtoDeserialize for LoopMessage {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, ProtoError> {
        let mut msg = LoopMessage::default();
        loop {
            let Some((field, wire)) = try_decode_field_key(reader)? else { break };
            match field {
                loop_fields::START_BAR if wire == WireType::Varint => {
                    msg.start_bar = decode_varint64(reader)? as u32;
                }
                loop_fields::END_BAR if wire == WireType::Varint => {
                    msg.end_bar = decode_varint64(reader)? as u32;
                }
                loop_fields::START_BEAT if wire == WireType::Varint => {
                    msg.start_beat = decode_varint64(reader)?;
                }
                loop_fields::END_BEAT if wire == WireType::Varint => {
                    msg.end_beat = decode_varint64(reader)?;
                }
                loop_fields::QUANTIZATION if wire == WireType::LengthDelimited => {
                    msg.quantization = decode_string(reader)?;
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
    fn test_loop_roundtrip() {
        let original = LoopMessage {
            start_bar: 1,
            end_bar: 8,
            start_beat: 0,
            end_beat: 32,
            quantization: "Bar".to_string(),
        };
        let encoded = encode(&original).expect("encode");
        let decoded: LoopMessage = decode(&encoded).expect("decode");
        assert_eq!(original.start_bar, decoded.start_bar);
        assert_eq!(original.end_bar, decoded.end_bar);
        assert_eq!(original.quantization, decoded.quantization);
    }
}
