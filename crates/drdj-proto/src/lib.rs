//! Generated Protocol Buffers. Use [`ForwardMessage`] when relaying binary
//! messages: typed Prost decoding intentionally discards unknown fields.
//! These allocating APIs belong on workers, never in audio callbacks.

use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage};
use std::sync::OnceLock;
use thiserror::Error;

include!(concat!(env!("OUT_DIR"), "/drdj.v1.rs"));
include!(concat!(env!("OUT_DIR"), "/drdj.v1.serde.rs"));

/// Explicit application limit, checked before allocating an encoded message.
pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
/// Binary descriptors compiled from the checked-in schema.
pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/descriptor.bin"));

/// Codec failures, with Prost's field context for malformed wire data.
#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("message size {actual} exceeds {MAX_MESSAGE_BYTES} bytes")]
    TooLarge { actual: usize },
    #[error("invalid protobuf: {0}")]
    Decode(#[from] prost::DecodeError),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown message type {0}")]
    UnknownMessage(String),
}

fn check_size(actual: usize) -> Result<(), ProtoError> {
    if actual > MAX_MESSAGE_BYTES {
        Err(ProtoError::TooLarge { actual })
    } else {
        Ok(())
    }
}

/// Encode to binary without rounding floating-point fields.
pub fn encode<T: Message>(message: &T) -> Result<Vec<u8>, ProtoError> {
    check_size(message.encoded_len())?;
    Ok(message.encode_to_vec())
}

/// Decode a typed message. Use [`ForwardMessage`] to retain unknown fields.
pub fn decode<T: Message + Default>(bytes: &[u8]) -> Result<T, ProtoError> {
    check_size(bytes.len())?;
    Ok(T::decode(bytes)?)
}

/// Encode Protobuf JSON (base64 bytes, decimal-string u64, UTC timestamps).
pub fn to_json<T: serde::Serialize>(message: &T) -> Result<String, ProtoError> {
    Ok(serde_json::to_string(message)?)
}

/// Decode debugging JSON with an explicit input size limit.
pub fn from_json<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, ProtoError> {
    check_size(json.len())?;
    Ok(serde_json::from_str(json)?)
}

/// Immutable binary relay retaining unknown fields, including nested ones.
/// JSON is not an unknown-field archive.
#[derive(Clone, Debug)]
pub struct ForwardMessage(DynamicMessage);

impl ForwardMessage {
    /// Decode using a fully qualified name, e.g. `drdj.v1.TrackMessage`.
    pub fn decode(name: &str, bytes: &[u8]) -> Result<Self, ProtoError> {
        check_size(bytes.len())?;
        static POOL: OnceLock<DescriptorPool> = OnceLock::new();
        let pool = POOL.get_or_init(|| {
            DescriptorPool::decode(FILE_DESCRIPTOR_SET)
                .expect("build-generated descriptor must be valid")
        });
        let descriptor = pool
            .get_message_by_name(name)
            .ok_or_else(|| ProtoError::UnknownMessage(name.to_owned()))?;
        Ok(Self(DynamicMessage::decode(descriptor, bytes)?))
    }

    /// Encode without discarding fields introduced by newer producers.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        encode(&self.0)
    }
}
