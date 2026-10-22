# Slint two-deck UI spike

## Run
Install a supported Rust toolchain, then from repository root:

```bash
cargo run
```

This is a UI-only proof of concept. It has not yet been compiled or visually verified in CI.

## Included
- Two independent deck panels
- Procedural waveform bars and animated horizontal scrolling when playing
- Play/pause, cue reset and position slider per deck
- Mixer channel sliders and crossfader (visual only)
- Desktop layout and placeholder library footer

## Explicit non-goals
No audio, decoding, real BPM, scratching, waveform analysis, hardware mapping, MIDI, persistence or polished final design. Slider positions are demo state only.

## Acceptance checklist
- [ ] `cargo check` passes on Windows and macOS
- [ ] `cargo run` displays two decks and mixer
- [ ] Play animates only the selected deck
- [ ] Cue resets the selected deck position
- [ ] Window resizing remains usable
- [ ] Record CPU/GPU load and animation smoothness
- [ ] Screenshot and visual review against approved design reference

## Next slice
Compile and resolve Slint API issues, add proper beat grid and multicolor waveform visualization, then profile before committing to Slint as the final UI framework.
