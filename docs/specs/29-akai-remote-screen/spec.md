# Feature Specification: Akai Remote Screen

**Feature Branch**: `29-akai-remote-screen`  
**Created**: 2026-11-11  
**Status**: Planned  
**Input**: Serato DJ Pro uses `libserato_device_akaisdk.so` (12 MB) for Akai MPC Touch and Remote Screen support. DRDJPro-Rust has `midi_controller.rs` and `midi_preset_manager.rs` for MIDI mapping but no Remote Screen support.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Track Information Display (Priority: P1)

As a DJ using an Akai MPC Touch with Remote Screen, I want track information (title, artist, BPM, key) displayed on the device's screen so that I can see critical info without looking at the main application.

**Why this priority**: The Remote Screen provides essential track information in club environments where the main display may not be visible.

**Independent Test**: Can be tested by connecting an Akai device and verifying that track information is displayed on the screen.

**Acceptance Scenarios**:

1. **Given** a track is loaded on Deck A, **When** the device screen updates, **Then** the track title and artist are displayed
2. **Given** a track is playing, **When** the screen updates, **Then** the current BPM and key are shown alongside track info
3. **Given** a track is selected but not playing, **When** the screen updates, **Then** the track info is shown with a "selected" indicator
4. **Given** the track changes, **When** the transition completes, **Then** the new track info is displayed within 200 ms

---

### User Story 2 - Loop Indicator (Priority: P1)

As a DJ setting loops, I want the loop start/end positions and duration displayed on the Akai Remote Screen so that I can monitor loop boundaries without the main UI.

**Why this priority**: Loop visualization is critical for live mixing; the Remote Screen provides a compact indicator.

**Independent Test**: Can be tested by setting a loop on a track and verifying that the loop region is displayed correctly on the Remote Screen.

**Acceptance Scenarios**:

1. **Given** a loop is set on a track, **When** the loop region is active, **Then** the loop start and end markers are displayed on the screen
2. **Given** a loop is counting down, **When** the loop time elapses, **Then** the remaining loop time is shown on the screen
3. **Given** no loop is active, **When** the loop widget is rendered, **Then** the widget shows "OFF" or an empty indicator
4. **Given** the loop length changes (e.g., 4 → 8 bars), **When** the change is made, **Then** the screen updates to show the new loop length

---

### User Story 3 - Hot Cue Grid Display (Priority: P2)

As a DJ using Hot Cues, I want the Hot Cue positions and colors displayed on the Akai Remote Screen so that I can navigate between cues without the main display.

**Why this priority**: Hot Cue visualization helps DJs navigate tracks during live performances.

**Independent Test**: Can be tested by setting Hot Cues and verifying that their positions and colors are shown on the Remote Screen.

**Acceptance Scenarios**:

1. **Given** 8 Hot Cues are set on a track, **When** the Hot Cue grid is rendered, **Then** all cue positions are shown as colored markers
2. **Given** a Hot Cue is triggered, **When** the deck jumps to that position, **Then** the corresponding marker on the screen is highlighted
3. **Given** no Hot Cues are set, **When** the grid is rendered, **Then** empty placeholder slots are shown
4. **Given** a Hot Cue color is changed, **When** the change is applied, **Then** the screen updates the marker color

---

### User Story 4 - MPC Touch Pad Integration (Priority: P1)

As a DJ using an MPC Touch pad, I want velocity-sensitive and pressure-sensitive pad events to be processed so that I can trigger samples, cues, and hot cues with nuanced control.

**Why this priority**: The MPC Touch pad is a core input device for Akai controllers; velocity and pressure sensitivity are essential for expressive performance.

**Independent Test**: Can be tested by hitting pads with varying force and verifying that velocity values are correctly reported.

**Acceptance Scenarios**:

1. **Given** a pad is struck lightly, **When** the event is received, **Then** the velocity value is in the range 0–63
2. **Given** a pad is struck hard, **When** the event is received, **Then** the velocity value is in the range 64–127
3. **Given** a pad is held down, **When** pressure changes, **Then** the pressure value is updated (aftertouch support)
4. **Given** a pad is released, **When** the release event is received, **Then** the corresponding note-off event is sent to the deck

---

## Edge Cases

- What happens when the Remote Screen resolution is different from the expected 128×64?
- How are rapid screen updates handled when multiple decks change simultaneously?
- What happens when the Akai device is disconnected during a live performance?
- How are pad events handled when the pad is physically damaged (stuck values)?
- What happens when the application is processing a heavy effect chain (screen updates may lag)?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect Akai MPC Touch and Remote Screen devices by USB VID/PID (vendor: `0x27dd`, product: `0x0017`)
- **FR-002**: System MUST render track information (title, artist, BPM, key, duration) on the Remote Screen
- **FR-003**: System MUST render a loop indicator showing start/end positions, current position, and remaining loop time
- **FR-004**: System MUST render a Hot Cue grid showing cue positions with their assigned colors
- **FR-005**: System MUST process MPC Touch pad events with velocity sensitivity (0–127)
- **FR-006**: System MUST support pressure sensitivity (aftertouch) on held pads
- **FR-007**: System MUST update the Remote Screen at a minimum of 30 Hz refresh rate
- **FR-008**: System MUST support a configurable screen resolution (default 128×64, optional 128×32)
- **FR-009**: System MUST provide a velocity curve configuration (linear, exponential, logarithmic)
- **FR-010**: System MUST handle device disconnection gracefully and remove the screen widget from the UI
- **FR-011**: System MUST support displaying device-specific widgets (level meter, EQ bars, crossfader position)
- **FR-012**: System MUST encode MIDI messages for Akai-specific protocol (SysEx, proprietary messages)
- **FR-013**: System MUST allow users to customize which widgets are displayed on the Remote Screen
- **FR-014**: System MUST isolate screen rendering in a dedicated task to prevent blocking audio processing

### Key Entities

- **Remote Screen**: Rendered display on the Akai device with configurable resolution and widgets
- **Widget**: A visual component on the Remote Screen (track info, loop indicator, Hot Cue grid, level meter)
- **Pad Event**: Touch pad input event with x/y position, velocity, pressure, and action (down/move/up)
- **Akai Device Profile**: Configuration for a specific Akai device (VID/PID, screen resolution, supported widgets)
- **Screen Buffer**: Frame buffer for rendering widgets before sending to the device

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Remote Screen updates complete within 33 ms (30 Hz refresh rate) for 99% of frame updates
- **SC-002**: Pad event latency from physical strike to MIDI message is under 5 ms
- **SC-003**: Track information is displayed on the Remote Screen within 200 ms of track load
- **SC-004**: Loop indicator updates are synchronized with playback (no visible drift)
- **SC-005**: Velocity sensitivity accurately reflects strike force (correlation > 0.95 with measured force)
- **SC-006**: 100% of Akai MPC Touch pads report events correctly
- **SC-007**: Screen buffer rendering does not block the audio processing thread
- **SC-008**: Users can customize screen widgets and see changes applied within 500 ms

## Dependencies

- `midir` crate (0.10) for MIDI I/O (existing)
- `slint` crate (1.18) for widget rendering (existing)
- `image` crate (0.25) for bitmap manipulation
- `serde` crate for device profile serialization
- USB backend drivers (libusb for device detection, as defined in Spec 28)

## Assumptions

- Akai MPC Touch devices use the standard MIDI over USB protocol for pad events
- Screen data is sent via MIDI SysEx messages or vendor-specific control transfers
- The screen resolution is 128×64 pixels with 1-bit monochrome display (default)
- Velocity sensitivity follows the standard MIDI velocity range (0–127)
- The device supports aftertouch (pressure sensitivity on held pads)

## Technical Constraints

- Screen rendering must not block the audio processing thread
- MIDI SysEx messages are limited to 64 KB per message (OS-dependent)
- Pad events must be processed in real time (no buffering beyond the OS MIDI driver)
- Device communication must be isolated in a separate task to prevent blocking
- Screen updates must be throttled to 30 Hz to avoid unnecessary USB traffic

## Out of Scope

- Firmware updates for Akai devices
- Custom widget creation by users (only predefined widgets are supported)
- Support for other Akai devices beyond MPC Touch (e.g., MPD series, LPD series) in this version
- Akai-specific DSP processing (only I/O is handled)
- Multi-device screen mirroring (one screen per device)
