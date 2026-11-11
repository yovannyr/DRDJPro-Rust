//! DrDJ ProtoBuf-like binary serialization.
//!
//! Provides efficient, schema-versioned binary encoding/decoding for domain
//! types — Track, BeatGrid, Loop, Cue, Playlist, SyncState.
//!
//! Wire format mimics ProtoBuf v2 varint + length-delimited encoding so that
//! generated types are compatible with a future `prost`-based code generator.
//!
//! Design goals:
//! - ≥40 % smaller payloads than JSON
//! - Forward-compatible (unknown fields are preserved)
//! - Serato Cloud-Sync, controller plugin, and cross-process transport

mod track;
mod beatgrid;
mod loop_msg;
mod cue;
mod playlist;
mod sync_state;

pub use track::*;
pub use beatgrid::*;
pub use loop_msg::*;
pub use cue::*;
pub use playlist::*;
pub use sync_state::*;

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{BufReader, Cursor, Read, Write};
use thiserror::Error;

// ── Wire type tags (ProtoBuf v2 compatible) ──────────────────────────────

/// ProtoBuf wire-type constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WireType {
    Varint = 0,
    I64 = 1,
    LengthDelimited = 2,
    StartGroup = 3,
    EndGroup = 4,
    I32 = 5,
}

// ── Errors ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("truncated message — expected {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },
    #[error("unknown wire type {wire} in field {field}")]
    UnknownWireType { field: u32, wire: u8 },
    #[error("field {0} is required but missing")]
    MissingField(u32),
    #[error("invalid enum discriminant {value} for {type_name}")]
    InvalidEnum { value: i64, type_name: &'static str },
}

// ── Helper: encode/decode field key ──────────────────────────────────────

/// Encode a field tag + wire type into a varint.
#[inline]
pub fn encode_field_key(field_number: u32, wire_type: WireType) -> Vec<u8> {
    let tag = ((field_number as u64) << 3) | (wire_type as u64);
    encode_varint64(tag)
}

/// Decode a field key into (field_number, wire_type).
/// Returns `None` on EOF (used to stop nested decode loops gracefully).
#[inline]
pub fn decode_field_key<R: Read>(reader: &mut R) -> Result<(u32, WireType), ProtoError> {
    let raw = decode_varint64(reader)?;
    Ok(decode_field_key_from_raw(raw))
}

/// Decode a field key from a raw varint value (internal helper).
#[inline]
fn decode_field_key_from_raw(raw: u64) -> (u32, WireType) {
    let field = (raw >> 3) as u32;
    match raw & 0x7 {
        0 => (field, WireType::Varint),
        1 => (field, WireType::I64),
        2 => (field, WireType::LengthDelimited),
        5 => (field, WireType::I32),
        _w => (field, WireType::Varint), // treat unknown as skip-safe
    }
}

/// Try to decode a field key, returning `Ok(None)` on EOF and `Err` for other IO errors.
#[inline]
pub fn try_decode_field_key<R: Read>(reader: &mut R) -> Result<Option<(u32, WireType)>, ProtoError> {
    match decode_varint64(reader) {
        Ok(raw) => Ok(Some(decode_field_key_from_raw(raw))),
        Err(ProtoError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

// ── Varint helpers ───────────────────────────────────────────────────────

/// Encode a u64 as a ProtoBuf varint.
pub fn encode_varint64(mut value: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    loop {
        if value > 0x7F {
            buf.push((value as u8) | 0x80);
            value >>= 7;
        } else {
            buf.push(value as u8);
            break;
        }
    }
    buf
}

/// Decode a u64 varint from a reader.
pub fn decode_varint64<R: Read>(reader: &mut R) -> Result<u64, ProtoError> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = reader.read_u8()?;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 63 {
            return Err(ProtoError::Truncated { expected: 10, actual: 0 });
        }
    }
}

/// Encode a fixed 32-bit float (stored as little-endian raw bits).
pub fn encode_f32(value: f32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

/// Decode a fixed 32-bit float.
pub fn decode_f32<R: Read>(reader: &mut R) -> Result<f32, ProtoError> {
    Ok(reader.read_f32::<LittleEndian>()?)
}

/// Encode a fixed 64-bit float.
pub fn encode_f64(value: f64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

/// Decode a fixed 64-bit float.
pub fn decode_f64<R: Read>(reader: &mut R) -> Result<f64, ProtoError> {
    Ok(reader.read_f64::<LittleEndian>()?)
}

// ── Encode/decode length-delimited strings ───────────────────────────────

/// Encode a UTF-8 string (length-prefixed).
pub fn encode_string(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let len = encode_varint64(bytes.len() as u64);
    let mut buf = Vec::with_capacity(len.len() + bytes.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(bytes);
    buf
}

/// Decode a length-delimited string.
pub fn decode_string<R: Read>(reader: &mut R) -> Result<String, ProtoError> {
    let len = decode_varint64(reader)? as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| ProtoError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData, e,
    )))
}

// ── Encode/decode Vec<u8> (bytes) ───────────────────────────────────────

/// Encode raw bytes.
pub fn encode_bytes(value: &[u8]) -> Vec<u8> {
    let len = encode_varint64(value.len() as u64);
    let mut buf = Vec::with_capacity(len.len() + value.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(value);
    buf
}

/// Decode raw bytes.
pub fn decode_bytes<R: Read>(reader: &mut R) -> Result<Vec<u8>, ProtoError> {
    let len = decode_varint64(reader)? as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Skip a length-delimited or varint field (forward compatibility).
pub fn skip_field<R: Read>(reader: &mut R, wire: WireType) -> Result<(), ProtoError> {
    match wire {
        WireType::Varint => {
            decode_varint64(reader)?;
        }
        WireType::I64 => {
            reader.read_u64::<LittleEndian>()?;
        }
        WireType::LengthDelimited => {
            let len = decode_varint64(reader)? as usize;
            let mut skip = vec![0u8; len];
            reader.read_exact(&mut skip)?;
        }
        WireType::I32 => {
            reader.read_u32::<LittleEndian>()?;
        }
        _ => {
            return Err(ProtoError::UnknownWireType {
                field: 0,
                wire: wire as u8,
            });
        }
    }
    Ok(())
}

// ── Generic encode/decode trait ─────────────────────────────────────────

/// A type that can be serialized to/from ProtoBuf wire format.
pub trait ProtoSerialize {
    /// Serialize this value into ProtoBuf wire format.
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), ProtoError>;
}

/// A type that can be deserialized from ProtoBuf wire format.
pub trait ProtoDeserialize: Sized {
    /// Deserialize from a reader in ProtoBuf wire format.
    fn decode<R: Read>(reader: &mut R) -> Result<Self, ProtoError>;
}

/// Encode a message to bytes.
pub fn encode<T: ProtoSerialize>(msg: &T) -> Result<Vec<u8>, ProtoError> {
    let mut buf = Vec::new();
    msg.encode(&mut buf)?;
    Ok(buf)
}

/// Encode a message to bytes and measure size.
pub fn encode_to_vec<T: ProtoSerialize>(msg: &T) -> Result<Vec<u8>, ProtoError> {
    encode(msg)
}

/// Decode bytes into a message.
pub fn decode<T: ProtoDeserialize>(data: &[u8]) -> Result<T, ProtoError> {
    let mut reader = BufReader::new(Cursor::new(data));
    T::decode(&mut reader)
}

/// Encode a message to a writer.
pub fn encode_to_writer<W: Write, T: ProtoSerialize>(
    writer: &mut W,
    msg: &T,
) -> Result<(), ProtoError> {
    let buf = encode(msg)?;
    writer.write_all(&buf)?;
    Ok(())
}

/// Decode a message from a reader.
pub fn decode_from_reader<R: Read, T: ProtoDeserialize>(reader: &mut R) -> Result<T, ProtoError> {
    T::decode(reader)
}

// ── Size comparison helpers ──────────────────────────────────────────────

/// Compare binary size with JSON-serialized form.
/// Returns (binary_bytes, json_bytes, reduction_percent).
pub fn size_comparison<T>(msg: &T, json_bytes: usize) -> (usize, usize, f64)
where
    T: ProtoSerialize,
{
    let binary = encode(msg).unwrap_or_default();
    let reduction = if json_bytes > 0 {
        (1.0 - binary.len() as f64 / json_bytes as f64) * 100.0
    } else {
        0.0
    };
    (binary.len(), json_bytes, reduction)
}

// ── Known field numbers (for documentation) ──────────────────────────────

/// Field numbers used in TrackMessage.
pub mod track_fields {
    pub const TITLE: u32 = 1;
    pub const ARTIST: u32 = 2;
    pub const ALBUM: u32 = 3;
    pub const BPM: u32 = 4;
    pub const KEY: u32 = 5;
    pub const DURATION_MS: u32 = 6;
    pub const FILE_PATH: u32 = 7;
    pub const COVER_ART_BYTES: u32 = 8;
    pub const ANALYSIS_HASH: u32 = 9;
    pub const TEMPO: u32 = 10;
}

/// Field numbers used in BeatGridMessage.
pub mod beatgrid_fields {
    pub const BPM: u32 = 1;
    pub const ANCHOR_MS: u32 = 2;
    pub const BEATS_PER_BAR: u32 = 3;
    pub const METER_DENOMINATOR: u32 = 4;
    pub const WARP_MARKERS: u32 = 5;
    pub const TEMPO_CHANGES: u32 = 6;
    pub const METER_CHANGES: u32 = 7;
}

/// Field numbers used in LoopMessage.
pub mod loop_fields {
    pub const START_BAR: u32 = 1;
    pub const END_BAR: u32 = 2;
    pub const START_BEAT: u32 = 3;
    pub const END_BEAT: u32 = 4;
    pub const QUANTIZATION: u32 = 5;
}

/// Field numbers used in CueMessage.
pub mod cue_fields {
    pub const POSITION_MS: u32 = 1;
    pub const LABEL: u32 = 2;
    pub const COLOR: u32 = 3;
    pub const POINT_INDEX: u32 = 4;
}

/// Field numbers used in PlaylistMessage.
pub mod playlist_fields {
    pub const NAME: u32 = 1;
    pub const TRACK_ORDER: u32 = 2;
    pub const CREATED_AT: u32 = 3;
    pub const MODIFIED_AT: u32 = 4;
}

/// Field numbers used in SyncStateMessage.
pub mod sync_fields {
    pub const REVISION: u32 = 1;
    pub const LAST_SYNC: u32 = 2;
    pub const PENDING_CHANGES: u32 = 3;
    pub const SERVER_VERSION: u32 = 4;
}
