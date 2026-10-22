# Rust target architecture (proposal, not yet finalized)

## Design intent
Native Rust desktop DJ application, optimized for real-time audio and highly custom UI. Domain boundaries are inherited conceptually from the .NET application, not its framework structure.

## Initial Cargo workspace candidate
```text
apps/drdj-desktop/       # binary, composition root and lifecycle
crates/drdj-domain/      # deck, mixer, library domain state and commands
crates/drdj-audio/       # engine, DSP, scheduling, buffering
crates/drdj-audio-io/    # platform audio device adapters
crates/drdj-library/     # local library, analysis, persistence
crates/drdj-hardware/    # MIDI/HID and device mapping
crates/drdj-ui/          # Slint spike, views and reusable controls
crates/drdj-testkit/     # fakes, fixtures, performance harness
```

These are candidate boundaries; only create crates when implementation work warrants them. Optional streaming/video/DVS/FX integrations may be separate later.

## Runtime separation
- Audio callback: deterministic bounded processing, preallocated buffers, no blocking I/O, no logging or UI work.
- Control plane: domain commands and state updates with explicit ownership and bounded communication.
- UI: observes snapshots and issues commands; never owns the audio clock.
- Workers: file decoding, waveform analysis, persistence, metadata and library scans.

## Dependency injection
Use explicit constructors and a composition root; trait objects or generics where substitution adds value. No global service locator or mandatory DI framework.

## GUI evaluation gate
Validate Slint with scrolling multicolor waveform, two deck panels, mixer meters, pads and knobs; benchmark sustained frame time, memory, interaction latency and cross-platform behavior before locking framework.

## Acceptance gates
- Compiles and runs on supported targets
- Unit and integration tests for every slice
- Real-time performance measurement for audio paths
- Visual comparison for custom controls
- Feature traceability to original .NET reference

## Open decisions
Slint/custom rendering integration, audio backend libraries, decoding and time-stretch algorithms, storage model, MIDI/HID stack, deployment/minimum OS support and licensed codecs.
