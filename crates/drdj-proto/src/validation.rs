use crate::*;
use prost_reflect::{Cardinality, ReflectMessage, Value};

/// A generated contract with semantic validation. Use the checked codecs at
/// application boundaries; the low-level wire codec also accepts arbitrary bits.
pub trait Contract: Message + Default {
    /// Fully qualified schema name.
    const NAME: &'static str;
    /// Check application values before encoding, without filesystem or audio I/O.
    fn validate(&self) -> Result<(), ProtoError>;
}

fn invalid(field: &str, reason: &'static str) -> ProtoError {
    ProtoError::Validation {
        field: field.into(),
        reason,
    }
}

fn finite(value: f64, field: &str) -> Result<(), ProtoError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(invalid(field, "must be finite"))
    }
}

fn prefix(error: ProtoError, parent: &str) -> ProtoError {
    match error {
        ProtoError::Validation { field, reason } => ProtoError::Validation {
            field: format!("{parent}.{field}"),
            reason,
        },
        other => other,
    }
}

fn log_result<T>(result: Result<T, ProtoError>) -> Result<T, ProtoError> {
    result.inspect_err(crate::diagnostic)
}

/// Validate application values before writing debugging JSON.
pub fn to_json_checked<T: Contract + serde::Serialize>(message: &T) -> Result<String, ProtoError> {
    log_result(message.validate().and_then(|()| to_json(message)))
}

/// Decode debugging JSON and validate application values.
pub fn from_json_checked<T: Contract + serde::de::DeserializeOwned>(
    json: &str,
) -> Result<T, ProtoError> {
    log_result((|| {
        let message: T = from_json(json)?;
        message.validate()?;
        Ok(message)
    })())
}

/// Validate values, then encode a bounded binary message. No filesystem or I/O.
pub fn encode_checked<T: Contract>(message: &T) -> Result<Vec<u8>, ProtoError> {
    log_result(message.validate().and_then(|()| encode(message)))
}

/// Validate wire presence (including nested required fields) and values.
/// Prost's generated `required` scalars alone do not enforce wire presence.
pub fn decode_checked<T: Contract>(bytes: &[u8]) -> Result<T, ProtoError> {
    log_result((|| {
        crate::presence::validate::<T>(bytes)?;
        let message: T = decode(bytes)?;
        message.validate()?;
        Ok(message)
    })())
}

pub(crate) fn required(message: &DynamicMessage) -> Result<(), ProtoError> {
    for field in message.descriptor().fields() {
        if field.cardinality() == Cardinality::Required && !message.has_field(&field) {
            return Err(invalid(field.name(), "required field missing"));
        }
    }
    for (field, value) in message.fields() {
        match value {
            Value::Message(child) => {
                required(child).map_err(|error| prefix(error, field.name()))?
            }
            Value::List(children) => {
                for (index, child) in children.iter().enumerate() {
                    if let Value::Message(child) = child {
                        required(child).map_err(|error| {
                            prefix(error, &format!("{}[{index}]", field.name()))
                        })?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

macro_rules! contract {
    ($name:ident, $this:ident, $body:block) => {
        impl Contract for $name {
            const NAME: &'static str = concat!("drdj.v1.", stringify!($name));
            fn validate(&$this) -> Result<(), ProtoError> $body
        }
    };
}

contract!(TrackMessage, self, {
    if !self.analysis_hash.is_empty() {
        log::warn!(target: "drdj_proto", "deprecated field TrackMessage.analysis_hash (tag 9)");
    }
    if self.file_path.is_empty() {
        return Err(invalid("file_path", "must not be empty"));
    }
    finite(self.bpm, "bpm")?;
    finite(f64::from(self.tempo), "tempo")?;
    if let Some(change) = self.key_change {
        finite(change, "key_change")?;
    }
    if let Some(grid) = &self.beat_grid {
        grid.validate().map_err(|e| prefix(e, "beat_grid"))?;
    }
    for (index, cue) in self.cues.iter().enumerate() {
        cue.validate()
            .map_err(|e| prefix(e, &format!("cues[{index}]")))?;
    }
    for (index, region) in self.loops.iter().enumerate() {
        region
            .validate()
            .map_err(|e| prefix(e, &format!("loops[{index}]")))?;
    }
    Ok(())
});
contract!(BeatGridMessage, self, {
    finite(self.bpm, "bpm")?;
    if self.bpm <= 0.0 {
        return Err(invalid("bpm", "must be positive"));
    }
    if self.beats_per_bar == 0 || !matches!(self.meter_denominator, 2 | 4 | 8 | 16) {
        return Err(invalid("meter_denominator", "invalid time signature"));
    }
    for (index, point) in self.points.iter().enumerate() {
        point
            .validate()
            .map_err(|e| prefix(e, &format!("points[{index}]")))?;
    }
    for (index, change) in self.tempo_changes.iter().enumerate() {
        change
            .validate()
            .map_err(|e| prefix(e, &format!("tempo_changes[{index}]")))?;
    }
    for (index, change) in self.meter_changes.iter().enumerate() {
        change
            .validate()
            .map_err(|e| prefix(e, &format!("meter_changes[{index}]")))?;
    }
    Ok(())
});
contract!(BeatPointMessage, self, {
    if !(0.0..=1.0).contains(&self.confidence) {
        return Err(invalid(
            "confidence",
            "must be finite and between zero and one",
        ));
    }
    Ok(())
});
contract!(TempoChangeMessage, self, {
    finite(self.bpm, "bpm")?;
    if self.bpm <= 0.0 {
        return Err(invalid("bpm", "must be positive"));
    }
    Ok(())
});
contract!(MeterChangeMessage, self, {
    if self.beats == 0 || !matches!(self.denominator, 2 | 4 | 8 | 16) {
        return Err(invalid("denominator", "invalid time signature"));
    }
    Ok(())
});
contract!(WarpMarkerMessage, self, { Ok(()) });
contract!(CueMessage, self, { Ok(()) });
contract!(LoopMessage, self, {
    match (self.start_ns, self.end_ns) {
        (Some(start), Some(end)) if start < end => {}
        (None, None) => {}
        _ => {
            return Err(invalid(
                "end_ns",
                "nanosecond loop requires start before end",
            ))
        }
    }
    if self.end_beat < self.start_beat || self.end_bar < self.start_bar {
        return Err(invalid("end_beat", "loop end precedes start"));
    }
    Ok(())
});

contract!(AnalysisMessage, self, {
    finite(f64::from(self.bpm), "bpm")?;
    finite(f64::from(self.gain), "gain")?;
    if !(0.0..=240.0).contains(&self.bpm) {
        return Err(invalid("bpm", "native BPM must be zero through 240"));
    }
    if !(0.1..=4.0).contains(&self.gain) {
        return Err(invalid("gain", "native gain must be 0.1 through 4"));
    }
    if !self.musical_key.is_empty()
        && !matches!(
            self.musical_key.as_bytes(),
            [b'1'..=b'9', b'A' | b'B'] | [b'1', b'0'..=b'2', b'A' | b'B']
        )
    {
        return Err(invalid("musical_key", "invalid Camelot key"));
    }
    if self.file_path.chars().count() > 32768 {
        return Err(invalid("file_path", "native analysis path too long"));
    }
    if self.waveform_overview.len() > 4096 {
        return Err(invalid(
            "waveform_overview",
            "native overview exceeds 4096 bytes",
        ));
    }
    Ok(())
});
contract!(NativeMetadataMessage, self, {
    if self.duration_ms.is_some_and(|v| v > 604_800_000)
        || (self.duration_exact && self.duration_ms.is_none())
    {
        return Err(invalid(
            "duration_ms",
            "native duration must exist when exact and not exceed seven days",
        ));
    }
    if self.error.as_ref().is_some_and(|e| e.chars().count() > 512) {
        return Err(invalid("error", "native text exceeds 512 characters"));
    }
    for (field, value) in [
        ("title", &self.title),
        ("artist", &self.artist),
        ("album", &self.album),
    ] {
        if value.chars().count() > 512 {
            return Err(invalid(field, "native text exceeds 512 characters"));
        }
    }
    if let Some(analysis) = &self.analysis {
        analysis.validate().map_err(|e| prefix(e, "analysis"))?;
    }
    Ok(())
});
contract!(NativeTrackMessage, self, {
    if self.id <= 0 || self.id == i32::MAX {
        return Err(invalid("id", "invalid native track identity"));
    }
    if self.native_path.is_empty() {
        return Err(invalid("native_path", "must not be empty"));
    }
    if !matches!(
        self.path_encoding.as_str(),
        "windows-utf16-le" | "unix-bytes"
    ) {
        return Err(invalid("path_encoding", "unknown native path encoding"));
    }
    match self.path_encoding.as_str() {
        "windows-utf16-le" => {
            if !self.native_path.len().is_multiple_of(2)
                || self.native_path.as_chunks::<2>().0.contains(&[0, 0])
            {
                return Err(invalid(
                    "native_path",
                    "invalid UTF16 path byte length or NUL",
                ));
            }
            if !windows_absolute(&self.native_path) {
                return Err(invalid("native_path", "Windows path must be absolute"));
            }
        }
        "unix-bytes" => {
            if self.native_path[0] != b'/' || self.native_path.contains(&0) {
                return Err(invalid(
                    "native_path",
                    "Unix path must be absolute and contain no NUL",
                ));
            }
        }
        _ => return Err(invalid("path_encoding", "unknown native path encoding")),
    }
    self.metadata
        .validate()
        .map_err(|e| prefix(e, "metadata"))?;
    if let Some(grid) = &self.grid {
        grid.validate().map_err(|e| prefix(e, "grid"))?;
        native_grid(grid).map_err(|e| prefix(e, "grid"))?;
    }
    let mut seen = 0u8;
    for (index, cue) in self.hot_cues.iter().enumerate() {
        let slot = cue.slot.ok_or_else(|| {
            invalid(
                &format!("hot_cues[{index}].slot"),
                "required native slot missing",
            )
        })?;
        if slot >= 8 || seen & (1 << slot) != 0 {
            return Err(invalid(
                &format!("hot_cues[{index}].slot"),
                "duplicate or out of range native slot",
            ));
        }
        seen |= 1 << slot;
        if cue.position_ns.is_none() {
            return Err(invalid(
                &format!("hot_cues[{index}].position_ns"),
                "required native nanoseconds missing",
            ));
        }
        let ns = cue.position_ns.unwrap_or_default();
        if ns > 604_800_000_000_000
            || cue.position_ms != ns / 1_000_000
            || cue.point_index != slot
            || cue.color > 0xff_ffff
            || !native_label(&cue.label)
        {
            return Err(invalid(
                &format!("hot_cues[{index}]"),
                "invalid native cue position, identity, color or label",
            ));
        }
    }
    seen = 0;
    for (index, region) in self.saved_loops.iter().enumerate() {
        let slot = region.slot.ok_or_else(|| {
            invalid(
                &format!("saved_loops[{index}].slot"),
                "required native slot missing",
            )
        })?;
        if slot >= 8 || seen & (1 << slot) != 0 {
            return Err(invalid(
                &format!("saved_loops[{index}].slot"),
                "duplicate or out of range native slot",
            ));
        }
        seen |= 1 << slot;
        if region.start_ns.is_none() || region.end_ns.is_none() || region.name.is_none() {
            return Err(invalid(
                &format!("saved_loops[{index}]"),
                "required native fields missing",
            ));
        }
        region
            .validate()
            .map_err(|e| prefix(e, &format!("saved_loops[{index}]")))?;
        if region.end_ns.is_some_and(|end| end > 604_800_000_000_000)
            || !native_label(region.name.as_deref().unwrap_or_default())
        {
            return Err(invalid(
                &format!("saved_loops[{index}]"),
                "invalid native loop bounds or name",
            ));
        }
    }
    Ok(())
});

fn windows_absolute(bytes: &[u8]) -> bool {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        std::path::Path::new(&std::ffi::OsString::from_wide(&units)).is_absolute()
    }
    #[cfg(not(windows))]
    {
        // Foreign snapshots can be relayed; native import rejects an OS mismatch.
        let sep = |unit| matches!(unit, 47 | 92);
        (units.len() >= 3
            && units[0] <= 127
            && (units[0] as u8).is_ascii_alphabetic()
            && units[1] == 58
            && sep(units[2]))
            || (units.len() >= 5
                && sep(units[0])
                && sep(units[1])
                && units[2..]
                    .iter()
                    .position(|&u| sep(u))
                    .is_some_and(|i| i > 0 && i + 3 < units.len()))
    }
}

fn native_label(value: &str) -> bool {
    value.chars().count() <= 128 && !value.chars().any(char::is_control)
}

fn native_grid(grid: &BeatGridMessage) -> Result<(), ProtoError> {
    if !(40.0..=240.0).contains(&grid.bpm)
        || grid.anchor_ns > 604_800_000_000_000
        || grid.beats_per_bar > 16
        || !grid.points.is_empty()
    {
        return Err(invalid("bpm", "grid exceeds native model bounds"));
    }
    if grid.warp_markers.len() > 64
        || grid.tempo_changes.len() > 64
        || grid.meter_changes.len() > 16
    {
        return Err(invalid("changes", "native grid capacity exceeded"));
    }
    if grid
        .warp_markers
        .windows(2)
        .any(|p| p[0].beat >= p[1].beat || p[0].position_ns >= p[1].position_ns)
        || grid
            .tempo_changes
            .windows(2)
            .any(|p| p[0].beat >= p[1].beat)
        || grid.meter_changes.windows(2).any(|p| p[0].bar >= p[1].bar)
    {
        return Err(invalid(
            "changes",
            "native changes must be strictly sorted and unique",
        ));
    }
    if grid.warp_markers.iter().any(|m| {
        !(1..=16_000_000).contains(&m.beat)
            || m.position_ns <= grid.anchor_ns
            || m.position_ns > 604_800_000_000_000
    }) || grid
        .tempo_changes
        .iter()
        .any(|m| !(1..=16_000_000).contains(&m.beat) || !(40.0..=240.0).contains(&m.bpm))
        || grid
            .meter_changes
            .iter()
            .any(|m| !(2..=1_000_000).contains(&m.bar) || m.beats > 16)
    {
        return Err(invalid("changes", "native change bounds exceeded"));
    }
    Ok(())
}

fn timestamp(value: &Option<pbjson_types::Timestamp>, field: &str) -> Result<(), ProtoError> {
    if value.as_ref().is_some_and(|v| {
        !(-62_135_596_800..=253_402_300_799).contains(&v.seconds)
            || !(0..1_000_000_000).contains(&v.nanos)
    }) {
        Err(invalid(field, "timestamp outside protobuf UTC range"))
    } else {
        Ok(())
    }
}
contract!(PlaylistMessage, self, {
    if self.name.is_empty() {
        return Err(invalid("name", "must not be empty"));
    }
    if let Some(native) = &self.native {
        native.validate().map_err(|e| prefix(e, "native"))?;
    }
    timestamp(&self.created_at, "created_at")?;
    timestamp(&self.modified_at, "modified_at")
});
contract!(PlaylistEntryMessage, self, {
    timestamp(&Some(self.added_at), "added_at")
});
contract!(NativePlaylistMessage, self, {
    if PlaylistKind::try_from(self.kind)
        .ok()
        .filter(|kind| *kind != PlaylistKind::Unknown)
        .is_none()
    {
        return Err(invalid("kind", "unknown playlist kind"));
    }
    for (index, entry) in self.entries.iter().enumerate() {
        entry
            .validate()
            .map_err(|e| prefix(e, &format!("entries[{index}]")))?;
    }
    Ok(())
});
contract!(SyncStateMessage, self, {
    timestamp(&self.last_sync, "last_sync")
});
contract!(SyncRequest, self, {
    for (index, track) in self.native_updates.iter().enumerate() {
        track
            .validate()
            .map_err(|e| prefix(e, &format!("native_updates[{index}]")))?;
    }
    for (index, playlist) in self.playlists.iter().enumerate() {
        playlist
            .validate()
            .map_err(|e| prefix(e, &format!("playlists[{index}]")))?;
    }
    for (index, track) in self.updates.iter().enumerate() {
        track
            .validate()
            .map_err(|e| prefix(e, &format!("updates[{index}]")))?;
    }
    if let Some(state) = &self.state {
        state.validate().map_err(|e| prefix(e, "state"))?;
    }
    Ok(())
});
contract!(SyncResponse, self, {
    for (index, track) in self.native_updates.iter().enumerate() {
        track
            .validate()
            .map_err(|e| prefix(e, &format!("native_updates[{index}]")))?;
    }
    for (index, playlist) in self.playlists.iter().enumerate() {
        playlist
            .validate()
            .map_err(|e| prefix(e, &format!("playlists[{index}]")))?;
    }
    for (index, track) in self.updates.iter().enumerate() {
        track
            .validate()
            .map_err(|e| prefix(e, &format!("updates[{index}]")))?;
    }
    if let Some(state) = &self.state {
        state.validate().map_err(|e| prefix(e, "state"))?;
    }
    Ok(())
});
contract!(PadStateMessage, self, {
    if self.index >= 16 {
        return Err(invalid("index", "pad index must be zero through fifteen"));
    }
    Ok(())
});
contract!(PadBankMessage, self, {
    let mut seen = 0u16;
    for (index, pad) in self.pads.iter().enumerate() {
        pad.validate()
            .map_err(|e| prefix(e, &format!("pads[{index}]")))?;
        let bit = 1u16 << pad.index;
        if seen & bit != 0 {
            return Err(invalid(
                &format!("pads[{index}].index"),
                "duplicate pad index",
            ));
        }
        seen |= bit;
    }
    Ok(())
});
