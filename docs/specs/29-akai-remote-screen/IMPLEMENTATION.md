# Spec 29 implementation and acceptance

## Slice 1: generic rendering and standard MIDI state

User scope replaces Akai/MPC Touch, DisplayLink and guessed device protocols with
a generic display. The initial output is an independent window; no physical
OLED/HID receiver protocol has been supplied. This crate does not send USB/SysEx.

`crates/drdj-remote` has no external dependencies. Its pure renderer produces
128x64 or 128x32 RGB888 frames with track, loop, eight colored cue slots, levels,
EQ and crossfader widgets. Half-height output has explicit metadata/transport
pages. Text is bounded; the original compact ASCII font uppercases and substitutes
unsupported Unicode. The app adapter must retain original accessible text.

A dedicated 30Hz worker keeps only the latest bounded snapshot and frame. UI
submission/retrieval uses try_lock, never waits, and retries newest data next tick.
Drop signals stop without joining the UI thread. Audio callbacks never receive
this service. Scheduling is a target, not native presentation qualification.

Standard MIDI pad profiles explicitly select channel and a contiguous 16-note
range. Fixed state decodes note-on/off, velocity-zero release, poly pressure and
channel pressure on held pads, plus release-all. Three monotonic velocity curves
preserve 0/127 endpoints and nonzero strikes. Duplicate held strikes do not
retrigger. App port lifecycle and action dispatch follow in the next slice.

Ten core tests pass. Isolated standalone debug renderer probe (500 frames):
p50 44.1us, p99 86.1us; no native UI/MIDI latency inferred. Independent review:
GPT-6.1 Sol, medium; both findings (bounded formatting for direct renderer callers
and legal try_submit contention in a test) fixed and follow-up accepted.
Native visual/manual/listening/hardware acceptance remains deferred.
