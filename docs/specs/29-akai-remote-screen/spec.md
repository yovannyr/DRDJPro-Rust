# Feature Specification: Generic Remote Display

**Feature Branch**: `codex/spec29-generic-remote-display`
**Created**: 2026-11-11
**Status**: Software implemented and automatically tested; native acceptance deferred

## User scope correction (2026-11-12)

The user replaces the original Akai/MPC Touch scope with a generic 128x64 remote
display. No Akai SDK, inferred VID/PID, proprietary SysEx, DisplayLink or physical
OLED protocol is claimed. The initial output adapter is a separate movable window
for an arbitrary monitor. Physical display adapters require a documented protocol.
The existing folder name is retained for traceability. Standard MIDI pads are
optional and explicitly configured; they do not imply Akai device certification.

## User stories and acceptance

1. Track: select A/B and observe loaded title, artist, BPM, key, current/total time
   and selected/playing state. A track change replaces the complete snapshot.
2. Loop: observe active start/end and remaining seconds; inactive loops show OFF.
3. Cues: eight slots preserve assigned colors and normalized positions; current
   playhead proximity highlights a slot. Empty slots remain visible placeholders.
4. Pads: configure an explicit standard-MIDI port/channel/first-note, receive 16
   note/velocity, polyphonic or channel-pressure and release events. Disconnect,
   overflow and configuration changes release state; reopening is explicit.
5. Customize: choose 128x64 or 128x32 (metadata/transport page), visible widgets
   and velocity curve. Closing the remote window leaves audio unchanged.

## Functional requirements

- **FR-001**: Explicitly open/close a generic independent display window; no vendor detection.
- **FR-002**: Display title, artist, BPM, key, current time and total duration.
- **FR-003**: Display active loop start/end/current position and remaining time.
- **FR-004**: Display eight hot-cue slots, assigned colors, positions and playhead proximity.
- **FR-005**: Decode 16 configured standard-MIDI notes with velocity 0-127.
- **FR-006**: Decode polyphonic and channel aftertouch for held pads; release clears pressure.
- **FR-007**: Schedule latest-state framebuffer rendering at 30 Hz; do not queue old frames.
- **FR-008**: Support 128x64 RGB888 and optional 128x32 with explicit page selection.
- **FR-009**: Offer linear, exponential and logarithmic velocity curves.
- **FR-010**: Handle window close and MIDI disconnect/overflow without stuck controls or autoplay.
- **FR-011**: Offer levels, EQ and crossfader widgets from existing control-plane observations.
- **FR-012**: Use standard MIDI note/pressure messages, never invented proprietary device messages.
- **FR-013**: Allow selecting visible predefined widgets in the remote-window settings.
- **FR-014**: Render on a dedicated worker; never access the audio engine from that worker.

## Measurable outcomes

- **SC-001**: Isolated renderer p99 below 33ms; native 30Hz presentation qualified separately.
- **SC-002**: Physical strike-to-action below 5ms requires joint native MIDI measurement.
- **SC-003**: Loaded metadata appears within 200ms in the native window (joint acceptance).
- **SC-004**: Loop/playhead display follows the existing playback observations; native drift check pending.
- **SC-005**: Physical velocity/force correlation above 0.95 requires calibrated hardware measurement.
- **SC-006**: All 16 mapped notes pass automated decode tests; actual pads require native acceptance.
- **SC-007**: Render/input callbacks never call the audio engine; bounded nonwaiting UI handoff.
- **SC-008**: Widget changes appear within 500ms in the native window (joint acceptance).

## Limits and test strategy

Pure rendering and MIDI state tests, worker contention/latest-state tests and
headless app binding tests precede independent review and publication. The compact
original ASCII font uppercases text, truncates at the pixel width and replaces
unsupported Unicode with '?'; original text remains accessible in the window.
Physical output adapters, manufacturer-specific firmware and force calibration are
out of scope following the generic correction. Native visual/manual/listening/
hardware tests remain deferred by user instruction. Spec 26 remains stopped.
