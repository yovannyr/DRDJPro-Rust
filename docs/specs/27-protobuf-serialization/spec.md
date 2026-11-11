# Feature Specification: ProtoBuf Serialization

**Feature Branch**: `27-protobuf-serialization`  
**Created**: 2026-11-11  
**Status**: Planned  
**Input**: Serato DJ Pro uses Protocol Buffers for efficient data serialization between components and cloud-sync. DRDJPro-Rust uses serde + JSON for all serialization but lacks ProtoBuf for performance-critical paths.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Efficient Track Serialization (Priority: P1)

As an application sync engine, I want to serialize track metadata and beat-grid data using ProtoBuf so that sync bandwidth is reduced by at least 50% compared to JSON.

**Why this priority**: ProtoBuf produces smaller payloads than JSON, reducing network usage and sync time for large libraries.

**Independent Test**: Can be tested by serializing a track with full beat grid to both ProtoBuf and JSON, then comparing the byte sizes.

**Acceptance Scenarios**:

1. **Given** a track with 500 beat-grid points, **When** serialized to ProtoBuf, **Then** the serialized size is at least 40% smaller than JSON
2. **Given** a track with 500 beat-grid points, **When** serialized to ProtoBuf and deserialized, **Then** all data is preserved exactly
3. **Given** a track with 500 beat-grid points, **When** serialized to JSON and deserialized, **Then** the JSON payload is larger than ProtoBuf by at least 40%
4. **Given** a roundtrip comparison, **When** ProtoBuf and JSON produce identical objects, **Then** all fields match byte-for-byte

---

### User Story 2 - Cloud Sync Protocol (Priority: P1)

As a cloud-sync service, I want to exchange track metadata, playlists, and sync state using ProtoBuf so that synchronization is fast and reliable.

**Why this priority**: ProtoBuf provides a contract-first approach with strong typing, reducing sync errors and enabling versioning.

**Independent Test**: Can be tested by defining a sync protocol, serializing test data on one side, deserializing on the other, and verifying all fields.

**Acceptance Scenarios**:

1. **Given** a sync request with 100 track updates, **When** serialized via ProtoBuf, **Then** the payload size is under 200 KB
2. **Given** a sync response with revision numbers, **When** deserialized, **Then** all 100 updates are recovered correctly
3. **Given** a sync protocol version mismatch, **When** client requests data, **Then** the server returns a version-error response
4. **Given** a sync message with unknown fields, **When** deserialized, **Then** unknown fields are preserved and forwarded (forward compatibility)

---

### User Story 3 - Controller Plugin Data (Priority: P2)

As a MIDI controller plugin, I want to exchange pad states, loop settings, and cue positions using ProtoBuf so that communication with the main application is efficient.

**Why this priority**: Controller plugins exchange frequent, small messages; ProtoBuf reduces overhead compared to JSON parsing.

**Independent Test**: Can be tested by serializing pad-state messages and verifying roundtrip correctness for 1000 iterations.

**Acceptance Scenarios**:

1. **Given** 16 performance pad states, **When** serialized via ProtoBuf, **Then** the payload is under 1 KB
2. **Given** a loop region definition, **When** serialized and deserialized, **Then** start/end positions and quantization are preserved
3. **Given** 4 cue-point definitions, **When** serialized via ProtoBuf, **Then** all positions and colors are preserved exactly
4. **Given** high-frequency pad updates (60/s), **When** serialized via ProtoBuf, **Then** serialization time per message is under 10 µs

---

### User Story 4 - Schema Versioning (Priority: P1)

As a developer, I want ProtoBuf schema versioning so that new fields can be added without breaking existing clients.

**Why this priority**: ProtoBuf's field-numbering provides forward/backward compatibility, enabling gradual rollout of new features.

**Independent Test**: Can be tested by serializing with schema v1, deserializing with schema v2, and verifying backward compatibility.

**Acceptance Scenarios**:

1. **Given** a message serialized with schema v1 (no `key_change` field), **When** deserialized with schema v2, **Then** the `key_change` field defaults to zero
2. **Given** a message serialized with schema v2 (includes `key_change`), **When** deserialized with schema v1, **Then** the unknown field is silently ignored
3. **Given** a schema migration from v1 to v2, **When** existing serialized data is re-read, **Then** all v1 fields are still accessible
4. **Given** deprecated fields in a schema, **When** a message uses them, **Then** the parser accepts them but logs a deprecation warning

---

## Edge Cases

- What happens when a ProtoBuf message exceeds 4 MB (the default limit)?
- How are floating-point precision differences between platforms handled?
- What happens when a field is removed from the schema mid-deployment?
- How are binary fields (e.g., cover-art thumbnails) serialized efficiently?
- What happens when the schema changes between the client and server?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST define ProtoBuf schema for Track, Playlist, BeatGrid, Loop, Cue, and SyncState messages
- **FR-002**: System MUST generate Rust types from `.proto` files using `prost-build` at compile time
- **FR-003**: System MUST implement `serde` traits for all generated ProtoBuf types
- **FR-004**: System MUST convert between existing Rust domain types and ProtoBuf message types transparently
- **FR-005**: System MUST serialize track data to ProtoBuf with at least 40% size reduction compared to JSON
- **FR-006**: System MUST deserialize ProtoBuf messages with all fields preserved exactly
- **FR-007**: System MUST support schema versioning with field numbers (not names) for stability
- **FR-008**: System MUST handle unknown fields gracefully (forward compatibility)
- **FR-009**: System MUST support optional and repeated fields in all messages
- **FR-010**: System MUST support serialization of binary fields (e.g., cover-art thumbnails) as `bytes`
- **FR-011**: System MUST provide `Into<Track>` and `From<Track>` conversions for ProtoBuf types
- **FR-012**: System MUST support encoding/decoding in both binary and JSON formats (for debugging)
- **FR-013**: System MUST validate message required fields before serialization
- **FR-014**: System MUST log serialization/deserialization failures with field-level detail

### Key Entities

- **TrackMessage**: ProtoBuf representation of a track (title, artist, album, BPM, key, duration, file path)
- **BeatGridMessage**: ProtoBuf representation of beat positions with timestamps and confidence values
- **LoopMessage**: ProtoBuf representation of loop regions (start, end, quantization)
- **CueMessage**: ProtoBuf representation of cue points (position, color, label)
- **PlaylistMessage**: ProtoBuf representation of a playlist with ordered track references
- **SyncStateMessage**: ProtoBuf representation of sync state (revision, last_sync, pending changes)
- **SyncRequest/Response**: ProtoBuf messages for cloud-sync protocol

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: ProtoBuf serialization produces payloads at least 40% smaller than JSON for all message types
- **SC-002**: ProtoBuf serialization time is under 50 µs per track (500 beat-grid points)
- **SC-003**: ProtoBuf deserialization time is under 100 µs per track
- **SC-004**: Roundtrip serialization/deserialization preserves 100% of field values
- **SC-005**: Schema version v2 can deserialize messages from v1 without data loss (backward compatibility)
- **SC-006**: Schema version v1 can ignore new fields from v2 messages (forward compatibility)
- **SC-007**: 1000 consecutive serialization/deserialization cycles produce identical results
- **SC-008**: All ProtoBuf messages compile without warnings using `prost-build` with strict mode

## Dependencies

- `prost` crate (0.13) for ProtoBuf code generation and serialization
- `prost-types` crate (0.13) for standard ProtoBuf types (Timestamp, Duration)
- `tonic` crate (0.12) for gRPC support (optional, for future cloud-sync)
- `serde` crate for JSON compatibility layer
- `prost-build` for build-time `.proto` → Rust generation

## Assumptions

- ProtoBuf schemas are versioned and stored in the `proto/` directory
- Binary ProtoBuf format is preferred for network transmission; JSON format is used for debugging
- Schema changes are backward compatible unless explicitly documented as breaking
- All timestamps use UTC and ProtoBuf `google.protobuf.Timestamp`
- Binary fields (e.g., thumbnails) are base64-encoded in JSON format, raw bytes in binary format

## Technical Constraints

- `.proto` files must be compiled at build time via `build.rs` using `prost-build`
- Generated Rust code must not exceed 500 lines per message type
- ProtoBuf messages must not use `oneof` for more than 8 variants (to keep generated code manageable)
- Wire format must be ProtoBuf v2 (no ProtoBuf v3 extensions that require different tooling)
- Maximum message size is 4 MB (Prost default); larger payloads require chunking

## Out of Scope

- ProtoBuf schema evolution tooling (migration scripts, schema diffing)
- ProtoBuf streaming for large file transfers (use HTTP chunking instead)
- Integration with third-party ProtoBuf services (e.g., Protocol Buffers registry)
- ProtoBuf for UI-to-renderer communication (Slint handles this natively)
- ProtoBuf for audio data transport (PCM data uses raw binary)
