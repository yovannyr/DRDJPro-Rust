//! BeatGridMessage — ProtoBuf representation of beat grid.
//!
//! Maps to `beatgrid::BeatGrid` + `WarpMarker` + `TempoChange` + `MeterChange`.

use super::*;
use serde::{Deserialize, Serialize};

/// A single warp marker in the beat grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarpMarkerMessage {
    pub beat: u64,
    pub position_ms: u64,
}

/// A tempo change in the beat grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoChangeMessage {
    pub beat: u64,
    pub bpm: f64,
}

/// A meter (time signature) change in the beat grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeterChangeMessage {
    pub bar: u32,
    pub beats: u8,
    pub denominator: u8,
}

/// Wire-format BeatGridMessage (ProtoBuf v2 compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeatGridMessage {
    pub bpm: f64,
    pub anchor_ms: u64,
    pub beats_per_bar: u8,
    pub meter_denominator: u8,
    pub warp_markers: Vec<WarpMarkerMessage>,
    pub tempo_changes: Vec<TempoChangeMessage>,
    pub meter_changes: Vec<MeterChangeMessage>,
}

impl Default for BeatGridMessage {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            anchor_ms: 0,
            beats_per_bar: 4,
            meter_denominator: 4,
            warp_markers: Vec::new(),
            tempo_changes: Vec::new(),
            meter_changes: Vec::new(),
        }
    }
}

impl ProtoSerialize for BeatGridMessage {
    fn encode(&self, buf: &mut Vec<u8>) -> Result<(), ProtoError> {
        // Field 1: BPM (varint)
        buf.extend_from_slice(&encode_field_key(beatgrid_fields::BPM, WireType::Varint));
        buf.extend_from_slice(&encode_varint64((self.bpm * 1000.0) as u64));

        // Field 2: anchor_ms (varint)
        buf.extend_from_slice(&encode_field_key(beatgrid_fields::ANCHOR_MS, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.anchor_ms));

        // Field 3: beats_per_bar (varint)
        buf.extend_from_slice(&encode_field_key(beatgrid_fields::BEATS_PER_BAR, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.beats_per_bar as u64));

        // Field 4: meter_denominator (varint)
        buf.extend_from_slice(&encode_field_key(beatgrid_fields::METER_DENOMINATOR, WireType::Varint));
        buf.extend_from_slice(&encode_varint64(self.meter_denominator as u64));

        // Field 5: warp_markers (repeated nested message)
        for marker in &self.warp_markers {
            let mut sub_buf = Vec::new();
            sub_buf.extend_from_slice(&encode_field_key(1, WireType::Varint));
            sub_buf.extend_from_slice(&encode_varint64(marker.beat));
            sub_buf.extend_from_slice(&encode_field_key(2, WireType::Varint));
            sub_buf.extend_from_slice(&encode_varint64(marker.position_ms));
            buf.extend_from_slice(&encode_field_key(beatgrid_fields::WARP_MARKERS, WireType::LengthDelimited));
            buf.extend_from_slice(&encode_varint64(sub_buf.len() as u64));
            buf.extend_from_slice(&sub_buf);
        }

        // Field 6: tempo_changes (repeated nested message)
        for tc in &self.tempo_changes {
            let mut sub_buf = Vec::new();
            sub_buf.extend_from_slice(&encode_field_key(1, WireType::Varint));
            sub_buf.extend_from_slice(&encode_varint64(tc.beat));
            sub_buf.extend_from_slice(&encode_field_key(2, WireType::LengthDelimited));
            sub_buf.extend_from_slice(&encode_f64(tc.bpm));
            buf.extend_from_slice(&encode_field_key(beatgrid_fields::TEMPO_CHANGES, WireType::LengthDelimited));
            buf.extend_from_slice(&encode_varint64(sub_buf.len() as u64));
            buf.extend_from_slice(&sub_buf);
        }

        // Field 7: meter_changes (repeated nested message)
        for mc in &self.meter_changes {
            let mut sub_buf = Vec::new();
            sub_buf.extend_from_slice(&encode_field_key(1, WireType::Varint));
            sub_buf.extend_from_slice(&encode_varint64(mc.bar as u64));
            sub_buf.extend_from_slice(&encode_field_key(2, WireType::Varint));
            sub_buf.extend_from_slice(&encode_varint64(mc.beats as u64));
            sub_buf.extend_from_slice(&encode_field_key(3, WireType::Varint));
            sub_buf.extend_from_slice(&encode_varint64(mc.denominator as u64));
            buf.extend_from_slice(&encode_field_key(beatgrid_fields::METER_CHANGES, WireType::LengthDelimited));
            buf.extend_from_slice(&encode_varint64(sub_buf.len() as u64));
            buf.extend_from_slice(&sub_buf);
        }

        Ok(())
    }
}

impl ProtoDeserialize for BeatGridMessage {
    fn decode<R: Read>(reader: &mut R) -> Result<Self, ProtoError> {
        let mut msg = BeatGridMessage::default();

        loop {
            let Some((field, wire)) = try_decode_field_key(reader)? else { break };
            match field {
                beatgrid_fields::BPM if wire == WireType::Varint => {
                    msg.bpm = decode_varint64(reader)? as f64 / 1000.0;
                }
                beatgrid_fields::ANCHOR_MS if wire == WireType::Varint => {
                    msg.anchor_ms = decode_varint64(reader)?;
                }
                beatgrid_fields::BEATS_PER_BAR if wire == WireType::Varint => {
                    msg.beats_per_bar = decode_varint64(reader)? as u8;
                }
                beatgrid_fields::METER_DENOMINATOR if wire == WireType::Varint => {
                    msg.meter_denominator = decode_varint64(reader)? as u8;
                }
                beatgrid_fields::WARP_MARKERS if wire == WireType::LengthDelimited => {
                    let marker = decode_nested_warp_marker(reader)?;
                    msg.warp_markers.push(marker);
                }
                beatgrid_fields::TEMPO_CHANGES if wire == WireType::LengthDelimited => {
                    let tc = decode_tempo_change(reader)?;
                    msg.tempo_changes.push(tc);
                }
                beatgrid_fields::METER_CHANGES if wire == WireType::LengthDelimited => {
                    let mc = decode_meter_change(reader)?;
                    msg.meter_changes.push(mc);
                }
                _ => skip_field(reader, wire)?,
            }
        }

        Ok(msg)
    }
}

fn decode_nested_warp_marker<R: Read>(reader: &mut R) -> Result<WarpMarkerMessage, ProtoError> {
    let len = decode_varint64(reader)? as usize;
    let mut sub_buf = vec![0u8; len];
    reader.read_exact(&mut sub_buf)?;
    let mut sub_cursor = Cursor::new(sub_buf);
    let mut marker = WarpMarkerMessage { beat: 0, position_ms: 0 };
    loop {
        let Some((field, wire)) = try_decode_field_key(&mut sub_cursor)? else { break };
        match field {
            1 if wire == WireType::Varint => marker.beat = decode_varint64(&mut sub_cursor)?,
            2 if wire == WireType::Varint => marker.position_ms = decode_varint64(&mut sub_cursor)?,
            _ => skip_field(&mut sub_cursor, wire)?,
        }
    }
    Ok(marker)
}

fn decode_tempo_change<R: Read>(reader: &mut R) -> Result<TempoChangeMessage, ProtoError> {
    let len = decode_varint64(reader)? as usize;
    let mut sub_buf = vec![0u8; len];
    reader.read_exact(&mut sub_buf)?;
    let mut sub_cursor = Cursor::new(sub_buf);
    let mut tc = TempoChangeMessage { beat: 0, bpm: 0.0 };
    loop {
        let Some((field, wire)) = try_decode_field_key(&mut sub_cursor)? else { break };
        match field {
            1 if wire == WireType::Varint => tc.beat = decode_varint64(&mut sub_cursor)?,
            2 if wire == WireType::LengthDelimited => tc.bpm = decode_f64(&mut sub_cursor)?,
            _ => skip_field(&mut sub_cursor, wire)?,
        }
    }
    Ok(tc)
}

fn decode_meter_change<R: Read>(reader: &mut R) -> Result<MeterChangeMessage, ProtoError> {
    let len = decode_varint64(reader)? as usize;
    let mut sub_buf = vec![0u8; len];
    reader.read_exact(&mut sub_buf)?;
    let mut sub_cursor = Cursor::new(sub_buf);
    let mut mc = MeterChangeMessage {
        bar: 0, beats: 0, denominator: 4,
    };
    loop {
        let Some((field, wire)) = try_decode_field_key(&mut sub_cursor)? else { break };
        match field {
            1 if wire == WireType::Varint => mc.bar = decode_varint64(&mut sub_cursor)? as u32,
            2 if wire == WireType::Varint => mc.beats = decode_varint64(&mut sub_cursor)? as u8,
            3 if wire == WireType::Varint => mc.denominator = decode_varint64(&mut sub_cursor)? as u8,
            _ => skip_field(&mut sub_cursor, wire)?,
        }
    }
    Ok(mc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beatgrid_roundtrip() {
        let grid = BeatGridMessage {
            bpm: 128.5,
            anchor_ms: 0,
            beats_per_bar: 4,
            meter_denominator: 4,
            warp_markers: vec![
                WarpMarkerMessage { beat: 0, position_ms: 0 },
                WarpMarkerMessage { beat: 1000, position_ms: 468750 },
            ],
            tempo_changes: vec![
                TempoChangeMessage { beat: 0, bpm: 128.0 },
                TempoChangeMessage { beat: 500, bpm: 130.0 },
            ],
            meter_changes: vec![
                MeterChangeMessage { bar: 1, beats: 4, denominator: 4 },
            ],
        };

        let encoded = encode(&grid).expect("encode failed");
        let decoded: BeatGridMessage = decode(&encoded).expect("decode failed");

        assert!((decoded.bpm - grid.bpm).abs() < 0.01);
        assert_eq!(decoded.beats_per_bar, grid.beats_per_bar);
        assert_eq!(decoded.warp_markers.len(), grid.warp_markers.len());
        assert_eq!(decoded.tempo_changes.len(), grid.tempo_changes.len());
        assert_eq!(decoded.meter_changes.len(), grid.meter_changes.len());
    }

    #[test]
    fn test_beatgrid_empty() {
        let grid = BeatGridMessage::default();
        let encoded = encode(&grid).expect("encode failed");
        let decoded: BeatGridMessage = decode(&encoded).expect("decode failed");
        assert_eq!(decoded.bpm, 120.0);
        assert_eq!(decoded.warp_markers.len(), 0);
    }
}
