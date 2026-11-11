//! SyncStateMessage — ProtoBuf representation of sync state.

use super::*;
use serde::{Deserialize, Serialize};

/// Wire-format SyncStateMessage (ProtoBuf v2 compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncStateMessage {
    pub revision: u64,
    pub last_sync: u64,
    pub pending_changes: Vec<u8>, // serialized change set
    pub server_version: u64,
}

impl Default for SyncStateMessage {
    fn default() -> Self {
        Self {
            revision: 0,
            last_sync: 0,
            pending_changes: Vec::new(),
            server_version: 0,
        }
    }
}

impl ProtoSerialize for SyncStateMessage {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), ProtoError> {
        buf.extend_from_slice(&encode_field_key(sync_fields::REVISION, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.revision));

        buf.extend_from_slice(&encode_field_key(sync_fields::LAST_SYNC, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.last_sync));

        buf.extend_from_slice(&encode_field_key(sync_fields::PENDING_CHANGES, WireType::LengthDelimited));
        buf.extend_from_slice(&encode_bytes(&self.pending_changes));

        buf.extend_from_slice(&encode_field_key(sync_fields::SERVER_VERSION, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.server_version));

        Ok(())
    }
}

impl ProtoDeserialize for SyncStateMessage {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, ProtoError> {
        let mut msg = SyncStateMessage::default();
        loop {
            let Some((field, wire)) = try_decode_field_key(reader)? else { break };
            match field {
                sync_fields::REVISION if wire == WireType::Varint => {
                    msg.revision = decode_varint64(reader)?;
                }
                sync_fields::LAST_SYNC if wire == WireType::Varint => {
                    msg.last_sync = decode_varint64(reader)?;
                }
                sync_fields::PENDING_CHANGES if wire == WireType::LengthDelimited => {
                    msg.pending_changes = decode_bytes(reader)?;
                }
                sync_fields::SERVER_VERSION if wire == WireType::Varint => {
                    msg.server_version = decode_varint64(reader)?;
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
    fn test_syncstate_roundtrip() {
        let original = SyncStateMessage {
            revision: 42,
            last_sync: 1700000000,
            pending_changes: vec![0x01, 0x02, 0x03],
            server_version: 100,
        };
        let encoded = encode(&original).expect("encode");
        let decoded: SyncStateMessage = decode(&encoded).expect("decode");
        assert_eq!(original.revision, decoded.revision);
        assert_eq!(original.last_sync, decoded.last_sync);
        assert_eq!(original.pending_changes, decoded.pending_changes);
        assert_eq!(original.server_version, decoded.server_version);
    }
}
