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

## Slice 2: independent app window and optional standard MIDI port

The REMOTE footer action creates a movable independent window on demand. No MIDI
backend starts until REFRESH/OPEN PADS. The live A/B snapshot includes actual
metadata, duration, software playhead, active loop bounds, cue colors/positions,
channel meters, selected-deck EQ and crossfader. A 128x32 display explicitly selects
metadata or transport; six checkboxes choose widgets. Original Unicode metadata
remains in the accessible window description even when the tiny bitmap uses '?'.

The renderer and UI publisher use accumulated 30Hz deadlines and skip catch-up
bursts after stalls. Latest-state handoff has no audio-engine reference. UI frame
conversion and actual native painting are outside the isolated renderer probe.

OPEN PADS explicitly chooses a standard MIDI input, channel 1-16, first note0-112
and curve. Names and native handles come from one port snapshot; duplicate names
are rejected. Notes1-8 map to deckA cues1-8, notes9-16 to deckB cues1-8 using the
existing cue-preview callback and reserved input15 (MC4000 owns16). Velocity and
pressure are retained and displayed; cue actions remain transport triggers, with
no velocity-dependent change to deck gain. No MIDI output or proprietary messages.

The native connection lives on a worker. Its callback copies at most3bytes into
a fixed256-packet queue without waiting. New sessions clear the old queue only
after destroying the previous connection; packets carry epochs and receipt time.
UI dispatch polls every2ms, at most64packets. Loss/overflow/stale packets over100ms,
configuration changes and closing release held controls; after5seconds a held pad
is forcibly released and requires explicit reopen. This deliberate stuck-pad limit
also bounds intentional long previews. Physical disconnect discovery polls1s;
a very short unplug/replug between scans is not guaranteed detectable.

Main-window close releases/hides the remote window and quits the event loop;
an auxiliary window cannot leave audio running by itself. Native handle cleanup
can block its own worker; it never joins on the UI thread. Explicit failure reasons
persist until user action. No automatic reconnect or playback restart.

Independent GPT-6.1-Sol review (medium) found and follow-up accepted fixes for
port-snapshot mismatch, overflow recovery and disappearing failure explanations.
Headless tests cover frame bindings, all16pad routes, ownership15/16, loss/old
session rejection, queue reset, stale input/timeout and close/reopen. Final results
and command outcomes are recorded below after completion.

## Final checks (2026-11-12)

- `cargo test -p drdj-remote --locked`: ten tests plus doctests passed.
- `cargo clippy -p drdj-remote --all-targets --locked -- -D warnings`: passed.
- Root strict Clippy of all targets, both with `dvs-xwax` and default features:
  passed without warning suppression.
- `cargo build -p drdjpro-ui-spike --release --features dvs-xwax --locked`:
  passed in3m56s with the final remote-window/input code.
- Core/targeted Rust formatting and `git diff --check`: passed.
- Latest isolated debug render probe under concurrent checks: p50 72.8us,
  p99 138.5us; no native presentation or physical input latency claim.

The full root regression uses `--test-threads=1`, `--no-fail-fast` and the CLI-only
`profile.test.package.drdjpro-ui-spike.opt-level=0` override. Production/dev profiles
are unchanged. The final aggregate is appended after that run completes.

Full root regression passed: 1,683 library tests +562 app tests +24 integration
tests =2,269passed, six existing qualification tests ignored, doctests passed.
The app test portion took340.05s sequentially. This includes the real controller
input15/16 separation and duration assertions, plus all eight new app/MIDI tests.
Final independent GPT-6.1-Sol review (medium), including scheduling and actual
headless close/reopen additions, reports no further actionable findings.
Spec26 stayed stopped; no paused worktree files were changed.
