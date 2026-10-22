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
