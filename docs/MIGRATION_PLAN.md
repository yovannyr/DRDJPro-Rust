# Migration plan (draft)

## Reference
.NET repository: https://github.com/yovannyr/DRDJ

Known documented modules: Desktop; Audio.Abstractions; Audio.Engine; Audio.Windows; Audio.Mac; Hardware; Streaming.Abstractions/Fake; Video.Abstractions/Fake. Reference docs include docs/arc42/arc42.md, docs/adrs/, docs/mvp.md, docs/next.md and phase documentation. Verify details before mapping.

## Phases
1. Inventory every requirement, feature, ADR, UI control, audio capability, controller mapping and test. Record source paths and implementation status.
2. Establish target architecture and evaluate Rust libraries and platform requirements; use a Cargo workspace with crates selected by bounded responsibility, not a mechanical project-to-crate conversion.
3. Build a single visual DJ deck spike with waveform, transport, cue and responsive controls; benchmark rendering and inspect visuals on Windows/macOS.
4. Port audio functionality incrementally using behavior and tests from .NET as the reference; validate real-time performance.
5. Integrate hardware, library, persistence and optional streaming/video modules in independently testable slices.
6. Run cross-platform performance, stability and usability tests before feature parity claims.

## Proposed architecture (subject to review)
- app: composition root and application lifecycle
- domain: deck/mixer/library state and commands
- audio: engine, decoding, timing and backend abstractions
- ui: GUI composition and custom controls
- hardware: MIDI/HID/controller adapters
- integrations: optional streaming/video providers

## Decisions to validate
- Slint versus other rendering/UI options, especially custom waveform performance.
- Audio I/O backends and supported sample formats.
- Controller protocol, starting with the Denon controller.
- macOS minimum supported version and older Intel Mac feasibility.
- Feature-by-feature traceability from .NET.

## Budget discipline
One bounded task at a time; reserve capacity for tests, visual review and rework. Do not use a single broad agent prompt for migration.
