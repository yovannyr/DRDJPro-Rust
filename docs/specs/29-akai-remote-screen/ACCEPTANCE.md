# Spec 29 acceptance ledger

Status: software implemented; final automated checks passed. Native visual,
manual, listening and hardware acceptance is deferred by user instruction.
The user-corrected generic scope is specified in [spec.md](spec.md).

| Requirement | Implemented evidence | Remaining acceptance |
|---|---|---|
| FR-001 | Explicit independent REMOTE window, headless close/reopen test | Native monitor positioning/usability |
| FR-002 | Live A/B title/artist/BPM/key/current/total snapshot and accessible Unicode text | Native readability; tiny bitmap uses uppercase ASCII fallback |
| FR-003 | Active loop start/end/playhead/remaining renderer; loop OFF | Native playback drift/calibration |
| FR-004 | Eight slots, positions, assigned RGB colors and playhead-proximity marker | Native colors/readability; proximity is not a historical trigger latch |
| FR-005 | Sixteen configured standard-MIDI notes and velocity, A/B cue dispatch | Physical controller; velocity is displayed, cue gain stays unchanged |
| FR-006 | Poly and channel pressure on held pads; release/disconnect reset | Hardware aftertouch |
| FR-007 | Dedicated latest-state renderer and UI publisher with 30Hz deadlines | OS/native presentation frequency/p99 |
| FR-008 | Fixed 128x64/32 RGB888 buffer; explicit half-height page | Native scaling/readability |
| FR-009 | Three monotonic bounded velocity curves with endpoint tests | Physical force calibration |
| FR-010 | Loss/overflow/stale/timeout/close release, epochs, explicit reopen | Physical unplug timing; sub-scan unplug/replug can be missed |
| FR-011 | Existing levels/EQ/crossfader observations and pure widgets | Native visual checks |
| FR-012 | Standard MIDI input only; no invented SysEx, VID/PID or Akai protocol | Physical adapter requires a documented protocol; excluded by generic scope |
| FR-013 | Six widget switches, selected deck, size/page and curve controls | Native interaction timing |
| FR-014 | Dedicated render worker; bounded nonwaiting UI handoff, no engine reference | OS scheduling and complete native stress session |

| Success criterion | Evidence / open measurement |
|---|---|
| SC-001 | Isolated debug renderer p99 138.5us in the latest 500-frame debug probe; 33ms native presentation target unmeasured |
| SC-002 | Callback queue/action routing tested; physical strike-to-action5ms unmeasured |
| SC-003 | Snapshot replacement/binding tested; native track-to-window200ms unmeasured |
| SC-004 | Actual UI software playhead/loop bounds used; native drift check deferred |
| SC-005 | Curve monotonicity/endpoints tested; force correlation0.95 requires calibrated hardware |
| SC-006 | All16notes and release/pressure tested; physical pad availability unmeasured |
| SC-007 | Render service has no audio-engine access; callback queues fixed capacity, no waits; renderer may allocate on its own worker |
| SC-008 | Widget configuration binds to framebuffer snapshot; native500ms qualification deferred |

No real MIDI input was opened for these tests. No visual or listening acceptance
is inferred from headless tests. Five-second held-pad cutoff is deliberate; use
normal deck transport for sustained playback. Existing MC4000 mappings/presets
remain intact. Spec26 remains stopped.


