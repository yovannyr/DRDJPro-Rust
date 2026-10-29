# Feature catalog and migration traceability (v0.1)

Legend: **Specified** = arc42/roadmap requirement, not verified implementation. **Integrated** = documented UI/backend wiring, not independently tested here. **Partial** = explicitly documented limitation. **Deferred** = later-phase scope. Status must be verified against code and tests.

| Area | Capability | Source status | Rust priority |
| --- | --- | --- | --- |
| Deck | Load/play/pause/cue, tempo/pitch, key shift, reverse | Specified; reverse consolidated to backend API in feature summary | P0 |
| Deck | Dual deck, pitch bend, slip, brake/spinback, beat jump | Specified | P1/P2 |
| Mixer | Gain, 3-band EQ, filters, crossfader, meters, master limiter | Specified; audio engine documented | P0/P1 |
| Waveforms | Overview, multicolor detailed view, beatgrid, markers, zoom | Specified; UX polish pending | P0/P1 |
| Library | Local files, metadata, playlists/crates, smart crates, search, history | Specified; minimal browser MVP | P0/P1 |
| Analysis | BPM, key, beatgrid, waveform analysis/editing | Specified | P1 |
| Hot cues | Cue store/jump/trigger | Integrated in docs/FEATURE_INTEGRATION_SUMMARY.md | P0 |
| Loops | Auto/manual, loop roll, beat-length selection | Specified | P1 |
| FX | Delay, echo, reverb, phaser/flanger, filter, roll, trans, FX chains | Specified; preset apply integrated | P1/P2 |
| VST3 | Plugin hosting, discovery and parameter control | Partial: explicit hosting stub | P3 |
| Sampler | Pads, banks, trigger/gate/loop, sample recording | Partial: uses deck playback; separate bus pending | P1/P2 |
| Stems | Vocals/drums/bass/other separation | Partial: frequency-band heuristic, not neural separation | P2 |
| Slicer | Beat slices, trigger, reorder and roll | Specified | P2 |
| DVS | Vinyl/CD timecode, absolute/relative, needle-drop | Deferred; simple decoder exists, real formats pending | P3 |
| Hardware | MIDI/HID mappings, LEDs, jog wheels, displays | Specified; device enumeration/refinement pending | P1 |
| Hardware | Denon DN-MC4000 as first physical test device | Project decision; validate protocol and test hardware | P1 |
| Recording | Session WAV/AIFF, monitoring and master output | Specified | P1 |
| Streaming | TIDAL/SoundCloud/Beatsource and broadcast | Fake provider exists; real integration deferred | P3 |
| Video | Video decks/mixing | Fake provider/mixer exists | P3 |
| Reliability | Logging, crash recovery, session restore, hot-swap | Specified; hot-swap pending | P1/P2 |
| Platforms | Windows/macOS, Linux later, older Intel Macs where feasible | Specified; compatibility must be tested | P0/P2 |

## Implementation evidence to audit next
- Source modules under src/DrDj.Audio.* and src/DrDj.Desktop
- tests/DrDj.Tests and tests/DrDj.E2E.Tests
- docs/phase3, docs/phase4, docs/next.md
- Controller mappings and asset documentation

## Migration policy
Every item needs source file(s), target crate(s), acceptance criteria, reference test(s), GUI control(s) and verified status before marking it complete. No silent feature drops.

## Extended workspace and session library acceptance (2026-10-24)

| ID | Implemented behavior | Automated evidence / remaining boundary |
| --- | --- | --- |
| R-UI-EXT-01 | Compact heads, shared stacked detail waveforms, library and separate status (`ui/app.slint`) | Headless geometry at 850x740 and 1280x860; visual/framework acceptance open |
| R-LIB-01 | Worker import of existing MP3/WAV paths, duplicates/invalid paths counted, cancel preserved (`library.rs`, `controller.rs`) | Real generated WAV and bundled MP3 imports; decode occurs at load; persistence/folder scans open |
| R-LIB-02 | Case-insensitive search, deterministic name/type/path sort and stable selected IDs | Domain and accessible UI tests, hidden selections cleared; tags/crates/history open |
| R-LIB-03 | Selected path loads into A/B, selection independent of sorting | Actual worker/engine headless test; native picker and physical output acceptance open |
| R-LOAD-03 | Playing decks cannot be replaced, even when started during decode | UI guard plus controller completion guard tested; old track survives decode failure |
| R-LIB-04 | Versioned durable IDs/paths and missing-file retention (`library.rs`, `library_store.rs`) | Unicode/ID/missing-file restart and real restored-track headless load; tags/crates/relocation open |
| R-LIB-05 | Atomic catalog replacement, previous-good backup, unknown/corrupt data preservation | Interrupted temp, forced write failure, backup recovery and invalid/future schema tests; power-loss qualification open |
| R-LIB-06 | Background storage, OS writer lock, visible errors/retry, latest-snapshot shutdown flush | Lock exclusion/release, pending-save close and GUI failure/retry tests; maximum-scale performance and cross-platform acceptance open |
| R-LIB-07 | Versioned SQLite schema and one-time JSON primary/backup migration (`library_store.rs`, `legacy_catalog.rs`) | Stable-ID/Unicode/missing-path migration, source byte preservation, transactional schema rollback/retry and headless engine/controller restart; metadata/crates deferred |
| R-LIB-08 | Transactional changed-row saves and SQLite online backups | Partial insert/delete rollback, external-writer contention/retry, backup recovery, foreign/newer schema protection and forced-process hot-journal recovery with integrity check; physical power-loss and scale qualification open |
| R-LIB-09 | Cached MP3/WAV title/artist/album/duration and table search/sort (`track_metadata.rs`, library/controller/Slint) | Generated UTF-8 ID3/WAV INFO, missing/error/bounded metadata, numeric sort, restarted stable-ID A/B workflow; real-music corpus and native visuals open |
| R-LIB-10 | Transactional metadata schema/backfill preserving track IDs (introduced in schema 2) | Successful upgrade/cache/missing-file retention and failed-upgrade row/version preservation; crates tracked in R-LIB-11/12, tag editing deferred |
| R-LIB-11 | Flat crate create/rename/confirmed delete and stable set membership (`crates.rs`, library/controller/Slint) | Name/reference validation, duplicate-add isolation, scoped search/selection, active-import capture, stale-confirm cancellation, file/global-track preservation and playing-deck headless workflow; nested/smart crates/manual ordering open |
| R-LIB-12 | Schema 1/2-to-3 crate persistence and durable ID high-water mark | Restart/ID non-reuse, relationship rollback/retry and successful/failed metadata-preserving schema upgrade; scale and platform qualification open |

## Local Rust loading/mixer slice (2026-10-24)

These entries describe Rust evidence only; .NET implementation parity has not been independently established.

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-LOAD-01 | MP3/WAV selection and paused load in A/B (`controller.rs`, `playback.rs`, `app.slint`) | Bundled MP3 + generated WAV, injected selector, actual worker and accessible Load actions | Native dialog appearance and physical output not tested |
| R-LOAD-02 | Reload, cancel, decode failure preserve deck isolation | Headless controller and actual sink tests; invalid/missing/unsupported/size limit cases | Strict detection of partially corrupt streams deferred |
| R-MIX-01 | Channel A/B and linear crossfader affect Rodio | Accessible slider actions plus exact offline mixed sample assertions for endpoints, midpoint, attenuation and mute | No EQ, limiter, equal-power curve or headphone bus |
| R-POS-01 | Engine-derived elapsed/total/percent; pause, Cue, EOF and restart | Offline mixer advance/freeze and controller snapshot assertions | Device latency uncompensated; detailed waveform/beat analysis open |
| R-BUILD-01 | Local checks independent of Slint CI debug variable | Compiler config enables accessibility metadata; ordinary Cargo checks | Windows host only; no cross-platform or hard real-time acceptance claim |

## Seeking follow-up slice (2026-10-24)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-SEEK-01 | Per-deck position slider seeks prepared PCM (`playback.rs`, `controller.rs`, `app.slint`) | Accessible GUI slider tests for A/B; paused/playing state, EOF/restart, Cue, offset reset on reload; direct MP3 seek | Abrupt source replacement, no click suppression or physical output acceptance |
| R-SEEK-02 | Preserve gain, other-deck state and stereo channel identity after seek | Exact stereo output samples on both sides of a step signal, advance/freeze after offsets, error/bounds tests without a running audio consumer | Rodio internal locks remain; device latency and sample-accurate synchronization unverified |

The stereo regression test exposed Rodio queue startup metadata describing mono/44.1 kHz before the actual source is active. Replacement sinks now report the fixed track format through a small output adapter. This also applies to initial load and Cue; there is still one fixed-format track per sink.

## Full-track peak overview slice (2026-10-24)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-WAVE-01 | At most 256 equal-time peak bins from prepared PCM (`waveform.rs`, `playback.rs`) | Deterministic mono/stereo, silence, short-track, uneven boundaries and last-frame tests; WAV/MP3 preparation checks | Absolute peak envelope only, no frequency colors, normalization, BPM or beatgrid |
| R-WAVE-02 | Separate per-deck overview and engine playhead (`controller.rs`, `app.slint`) | Model identity remains stable through timer/seek/Cue; load/reload/error/cancel preserve deck isolation | Slint rendering and visual design not accepted |
| R-WAVE-03 | Click full-track overview to seek | Real pointer events and accessible value actions in the headless backend; time mapping and marker geometry at two window widths including 0/100% | No physical interaction or visual/usability claim; detailed zoom/scroll deferred |

## Detail waveform and zoom slice (2026-10-24)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-DETAIL-01 | Prepared high-resolution peak envelope and bounded local re-binning | Stereo/transient/final-bin tests, short-track and start/middle/end viewport tests, clamped zoom | 256-frame envelope precision; no signed oscillogram, frequency colors or beatgrid |
| R-DETAIL-02 | Per-deck following viewport with 2–32s zoom | Actual accessible zoom controls, independent deck state, paused cache identity, playing/EOF/Cue marker checks | Headless geometry and events only; rendering/visual acceptance open |
| R-DETAIL-03 | Detail clicks seek within visible time range | Real pointer events at two widths, local-time target assertion; error/cancel/reload model isolation | Existing abrupt seek transition; no physical listening acceptance |

## Frequency-color follow-up (2026-10-24)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-COLOR-01 | Approximate low/mid/high energy colors from PCM (`waveform_color.rs`) | 80 Hz/1 kHz/high-frequency tests at 8/44.1/48/96 kHz; opposite-phase stereo, silence, temporal transitions and partial final chunk | First-order display heuristic, not FFT/spectrum measurement, beat analysis or audio filtering |
| R-COLOR-02 | Full overview and following detail share a red/green/blue palette | Matching peak/color model lengths, local-content re-bin tests, real MP3/WAV GUI models and independent reload/error/cancel behavior | No visual/design approval or device listening acceptance |
| R-COLOR-03 | Prepare frequency data once; reuse unchanged view models | Paused color-model identity plus existing zoom/seek/marker tests; diagnostic peak/color re-bin timing | GUI control-plane allocations remain; rendering/frame pacing and hard real-time gates open |

## Manual beatgrid follow-up (2026-10-24)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-GRID-01 | Validated constant-BPM 4/4 grid anchored at current source position (`beatgrid.rs`, `playback.rs`) | Invalid/fractional BPM, anchored positive/negative indices, spacing/downbeats, bounded 32s view | Manually defined; no automatic musical correctness, variable tempo or meter detection |
| R-GRID-02 | Per-deck BPM/apply/clear and detail overlay (`controller.rs`, `app.slint`) | Actual accessible BPM/button actions, line count and geometry at two widths, zoom and independent grid state | Headless event/geometry checks only; visual approval open |
| R-GRID-03 | Track-owned grid lifetime | Engine tests preserve through seek/Cue/EOF restart and invalid/failed load; raw engine installation resets grid, controller restores durable library data (R-GRID-05) | No audio tempo/sync change |
| R-GRID-04 | Isolated decimal-BPM grid drafts with anchor/slip, numbered bars and explicit Save/Cancel | Domain range/phase tests and accessible preview/invalid/cancel/clear/zoom/save tests; engine source/position/playback/mixer preservation | Constant 4/4 only; variable tempo/warp/meter and native visuals open |
| R-GRID-05 | Schema-4 per-track grid persistence and application reload/restore | Fractional BPM/nanosecond anchor round trip, explicit deletion, schema-3 upgrade/failure preserving metadata/crates and failed grid-write transaction rollback | No audio-file tag writes or automatic musical alignment |
| R-GRID-06 | Same-track committed grid sharing and stale-draft protection | Two-deck optimistic-conflict test, failed-load draft retention and successful replacement discard/reload | Identity is canonical library path/prepared source; file content fingerprinting remains open |

## Experimental tempo suggestion follow-up (2026-10-24)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-TEMPO-01 | Bounded offline 40–240 BPM suggestion (`tempo.rs`, `playback.rs`) | 21 synthetic pulse fixtures at 8/44.1/48 kHz including fractional BPM; rejection of silence, steady tones/MP3, noise, short/invalid input; opposite-phase stereo and exact first-60s cap | Heuristic candidate, not confidence or real-music accuracy; octave/tempo-change ambiguity remains |
| R-TEMPO-02 | Optional explicit adoption into pending per-deck BPM field | Actual worker WAV decode/estimate and accessible load/use/apply actions; grid/playback unchanged until explicit manual action | Whole-BPM GUI rounding; no auto-anchor, downbeat or audio sync |
| R-TEMPO-03 | Suggestions follow track lifetime | Headless cancel/error retention and successful reload removal, other-deck isolation | No persistence or corpus qualification |

## Hot-cue data foundation (2026-10-25)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-CUE-01 | Eight fixed validated per-track slots, nanosecond positions, bounded Unicode names and RGB colors | Domain bounds/Unicode/color tests; sparse-slot clear preserves other identities | Data only; GUI/engine triggers, source-duration checking and quantization open |
| R-CUE-02 | Schema-5 persistence, library snapshots and migration from schemas 1–4 | Restart with missing media, exact name/color/position round trip, explicit clear/deletion, failed cue-write rollback of metadata/crates/grid and retry; schema-4 upgrade/failure; invalid/orphaned-row rejection | No tag writes, external library compatibility, max-scale benchmark or hardware acceptance |
| R-CUE-03 | Eight GUI pads per deck: empty set, occupied recall, explicit slot deletion | Headless accessible pad/select/delete actions, same-track sharing, different-track isolation, failed/successful replacement and two fresh-controller restoration checks | Name/color editor and waveform markers are covered by R-CUE-05/06; keyboard/hardware feedback and quantization remain open |
| R-CUE-04 | Absolute source-time recall with transport/grid/mixer preservation | Engine nanosecond-offset/paused/playing/bounds tests; headless EOF/stale-cue rejection and Sink-volume assertions | Source offset is preserved; sample-grid audible timing and physical latency are not qualified |
| R-CUE-05 | Per-deck cue name/RGB drafts, Save/Cancel and source/value conflict protection | Headless Unicode/invalid/cancel/save, simultaneous same-track drafts, deletion conflict, successful/failed source replacement, transport/volume preservation and restart | No keyboard shortcuts or physical/hardware acceptance |
| R-CUE-06 | Numbered/named/color overview and detail markers | Two-size line geometry; seek/zoom viewport visibility; invalid duration suppression and persisted RGB/name round trip | Display-only; overlapping labels/contrast/native rendering performance await acceptance |

## Manual loop engine (2026-10-25)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-LOOP-01 | Integer-frame [In, Out) repetition from shared immutable PCM | Stereo channel order over 100,000 samples, one-frame/EOF bounds, 44.1/48/96 kHz frame-time round trips | No smoothing/quantization or musical alignment |
| R-LOOP-02 | Manual per-deck In/Out/activate/exit lifecycle | Real Rodio offline sample assertions across many wraps, wrapped position, pause/resume/exit/EOF, validation, seek/Cue and failed/successful replacement, grid/sink-volume and other-deck isolation | GUI covered by R-LOOP-04; saved-loop persistence follows separately |
| R-LOOP-03 | Prepared PCM next() has no allocations or locks | Test allocator records zero allocations for one million calls; diagnostic timing; source inspection has only index/branch/read operations | Does not qualify Rodio/backend queues, source replacement, deallocation, hardware latency or hard real-time deadlines |
| R-LOOP-04 | Per-deck manual-loop GUI controls and actual engine state | Headless IN/OUT/LOOP/EXIT actions, disabled/unloaded/loading cases, invalid bounds, paused/playing/offline wrap/exit, seek/Cue, failed/successful load, sink-volume/other-deck isolation and two-size geometry | Transient per-deck loop only; saved slots, auto-loop/quantization, visual/hardware acceptance open |
| R-LOOP-05 | Eight durable named In/Out slots per track in schema 6 | Model boundary/Unicode/slot identity validation; missing-file restart with exact nanoseconds/name; clear/track deletion; schema-5 upgrade/failure retaining metadata/cues/grid/crates; invalid/orphaned/duplicate row rejection; injected loop-write rollback/retry | GUI controls and source validation now covered by R-LOOP-06 |
| R-LOOP-06 | Saved-loop GUI Save/Recall/Delete with atomic source validation | Actual accessible panel/slot/name/actions; shared/different tracks, occupied-target protection, paused/playing recall, stale-duration/subframe/invalid-slot/loading failures, restart and durable clear; engine EOF round trip; both-panel two-size geometry | Recall loads inactive bounds; LOOP activates explicitly. Rename/replace drafts, auto-loops, quantization, waveform overlays and native/hardware acceptance open |
| R-LOOP-07 | Committed-grid BPM auto-loops from playhead, 1/8–32 beats | 44.1/48/96 kHz and fractional BPM frame-length bounds, repeated stereo frame-region output, engine/headless non-snapped activation, pending-grid isolation and invalid-grid/length/EOF rejection | Constant manual grid only; no snapping, quantization or musical correctness claim |
| R-LOOP-08 | Half/double existing integer-frame span around fixed In | One-frame/end bounds, active phase wrapping and inactive no-seek behavior; preserved Play/Pause/grid/Sink levels/other deck and saved-slot reuse; three-panel geometry | Half floors odd frame counts; active control-plane source replacement has no physical latency guarantee |
| R-LOOP-09 | Actual deck loop area and In/Out overlays, active/inactive, overview/detail clipping | Headless two-size full/partial/outside/touching viewport cases, seek/zoom, incomplete/active/exit/reset behavior, one-frame EOF and long-track collapsed-float regression, other-deck/volume preservation | Display-only; overlapping labels/contrast/native rendering, loop dragging and physical timing remain unqualified |
| R-LOOP-10 | Optional per-deck Auto Loop start alignment to committed manual grid | Domain 44.1/48/96kHz anchor/later-tie/first-nonnegative/third-beat rounding, nonbinary-anchor zero and ±1 ns midpoint regressions, rate/range rejection; engine optional/safe-failure state and other-deck isolation; actual checkbox/default/draft/disable actions and two-size geometry | Immediate start alignment only, no scheduled cue/loop triggering, persisted toggle, global quantization, sync or musical grid accuracy claim |

## Coupled playback speed/pitch engine (2026-10-25)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-PITCH-01 | Validated requested/effective0.5–2.0 speed, whole-Hz source rate | 44.1/48/96k rounding/error tests, NaN/infinity/out-of-range/zero-rate/rate-overflow rejection | Engine only; GUI and persistent preferences not implemented |
| R-PITCH-02 | Existing Rodio conversion path renders coupled duration/pitch from fixed-rate PCM | Offline440Hz stereo tone at0.5/1/1.25/2x plus44.1kHz rounded1.333333: rising-crossing frequency, non-silent signal length and antisymmetry; finite/loop source duration/seek and zero-allocation next() checks at changed rate | Linear Rodio resampling path; anti-alias quality, transitions, native listening and physical latency unqualified |
| R-PITCH-03 | Original-source-time transport/loop ownership across speed changes | Effective-ratio elapsed mapping, changed-rate loop phase/pause/seek/Cue, invalid request atomicity, reset after new track and retention after failed load, grid/Sink-volume/other-deck tests and direct steady-state region samples through changed-rate repeated wraps/resizes | Position uses existing5ms Rodio publication; source replacement is a control-plane operation; keylock/Sync open |
| R-PITCH-04 | Per-deck pitch slider/reset and requested/effective/source/current-BPM display | Actual headless controls, burst coalescing/latest-value apply, invalid/unloaded/loading/stale-source requests, reset, loop/position/grid/Sink-volume/other-deck preservation, manual/estimated/unavailable BPM display and failed-load retention; four-panel two-size geometry | Selectable/fine ranges covered by R-PITCH-05; native visual/listening and transition quality, keylock/Sync open |
| R-PITCH-05 | Per-deck selectable pitch ranges and integer0.01pp fine gestures | Bounds/quantization/invalid-value tests,100-step stability; actual fine buttons and slider limits; out-of-range display and no-audio-change range switch, pending-edge retention/cancellation, reset/new-source/deck independence and two-size geometry | Session-only preference; no temporary nudge, native drag/focus/listening or transition qualification, keylock/Sync open |
| R-PITCH-06 | Separate base pitch and temporary0.98/1/1.02 nudge engine state | Composition/rate/limit tests, current-base release after a pitch edit, phase/loop/grid/gain/deck preservation, pause/seek/Cue/new-source and EOF release, steady stereo-tone frequency direction/base checks | Engine-only; native momentary GUI cancellation, live transition/physical timing, keylock/Sync open |

- R-PITCH-07: Momentary nudge GUI/control-plane: fifth deck panel, pointer/Space/Return hold/release, direct focus and panel cancellation, source-bound held flags, busy/transport/EOF/shutdown neutralization. Headless input/geometry/state evidence; native visual/listening/live-transition/latency qualification remains open. Manual-grid tempo matching is next; phase Sync/keylock remain separate.

## Selectable cue quantization follow-up (2026-10-29)

| ID | Capability | Evidence | Remaining scope |
| --- | --- | --- | --- |
| R-QUANT-01 | Per-deck 1/8, 1/4, 1/2, 1, 2, 4 beat cue placement/trigger lattice | Exact tie and anchor tests at 44.1/48/96 kHz, fractional BPM, strict-future source-frame rounding, one-Hz edge, invalid inputs; both-deck actual controller/PCM workflows for all six steps, exact saved cue recall and EOF rejection | Manual quarter-beat grids only; global Quantize mode, persisted preferences, loop quantization and musical/native acceptance remain open |
| R-QUANT-02 | Queued trigger cancellation on bound step change; independent deck choices | Generated UI property change, Slint change-handler processing and rendered source timeline; enabled-control and two-size geometry checks | Native dropdown/keyboard/focus and physical hardware acceptance not established |

## Global Quantize follow-up (2026-10-29)

| ID | Capability | Evidence | Remaining scope |
| --- | --- | --- | --- |
| R-QUANT-03 | Toolbar master gating opted-in cue snap, scheduled recall and Auto Loop snap on A/B, preserving preferences | Actual accessible toolbar action, immediate dual-trigger cancellation verified by rendered source progression; programmatic property cancellation; retained flags/steps, exact unsnapped cues, immediate paused recall, playhead auto-loops, restored snap behavior | Default-on session master; global/deck preference persistence, full reference parity and native/hardware acceptance remain open |
| R-QUANT-04 | Master toggle preserves unrelated performance state | Unchanged source position, playing state, grid, base tempo and gains; preserved active engine Sync correction and nudge; existing loop bounds retained when re-enabling; toolbar geometry at 850x740 and 1280x860 | Headless checks do not establish visual, keyboard/focus, musical or physical audio acceptance |

## Persistent Quantize preferences (2026-10-29)

| ID | Capability | Evidence | Remaining scope |
| --- | --- | --- | --- |
| R-QUANT-05 | Versioned global/deck Quantize settings in library SQLite, restored before editing | Restart with distinct deck flags/steps; newer edits before acknowledgement; final close flush and inflight revert; closing during restore; worker-only atomic delta/preference rollback, injected failure/Retry and invalid input rejection | No portable JSON preference export or unrelated transport/device settings; full reference/native acceptance open |
| R-QUANT-06 | Malformed preferences use defaults and warn without losing the catalog or silently rewriting the original | Invalid JSON/version/unknown fields/step and oversize/embedded-NUL payloads; bounded production SQL read, worker load and unchanged original row after close | Existing SQLite backup/recovery applies to the database; subjective usability and native accessibility acceptance remain open |
