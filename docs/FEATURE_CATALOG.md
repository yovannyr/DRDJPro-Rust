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
| R-GRID-03 | Track-owned grid lifetime | Engine/GUI tests preserve through seek/Cue/EOF restart and invalid/failed load; reload resets only its track | No persistence or audio tempo/sync change |

## Experimental tempo suggestion follow-up (2026-10-24)

| ID | Capability / target | Acceptance evidence | Remaining boundary |
| --- | --- | --- | --- |
| R-TEMPO-01 | Bounded offline 40–240 BPM suggestion (`tempo.rs`, `playback.rs`) | 21 synthetic pulse fixtures at 8/44.1/48 kHz including fractional BPM; rejection of silence, steady tones/MP3, noise, short/invalid input; opposite-phase stereo and exact first-60s cap | Heuristic candidate, not confidence or real-music accuracy; octave/tempo-change ambiguity remains |
| R-TEMPO-02 | Optional explicit adoption into pending per-deck BPM field | Actual worker WAV decode/estimate and accessible load/use/apply actions; grid/playback unchanged until explicit manual action | Whole-BPM GUI rounding; no auto-anchor, downbeat or audio sync |
| R-TEMPO-03 | Suggestions follow track lifetime | Headless cancel/error retention and successful reload removal, other-deck isolation | No persistence or corpus qualification |
