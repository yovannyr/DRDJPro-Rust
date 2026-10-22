# Architecture baseline from DRDJ (.NET)

Source: https://github.com/yovannyr/DRDJ
Reviewed: docs/arc42/arc42.md, docs/mvp.md, docs/next.md, docs/FEATURE_INTEGRATION_SUMMARY.md, ADR-0001 through ADR-0003, README.md and AGENTS.md.

## Product goals
Professional DJ desktop software for Windows and macOS (Linux later), low-latency playback, reliable long DJ sets, advanced hardware controller support and customizable high-quality visual UI.

## Original quality targets (aspirational, not verified)
- Audio latency <10 ms; no audible dropouts
- 90+ hardware controllers in long-term roadmap
- 8+ hour sets, 99.9% uptime target
- Library search <100 ms at 100k tracks; UI frame time <16 ms
- Sample rates 44.1/48/88.2/96 kHz

## Architectural principles to preserve
- Clear domain/application/infrastructure/presentation responsibilities
- Audio backend abstraction; Windows WASAPI/ASIO and macOS CoreAudio adapters
- DSP/FX shared across platforms; device I/O isolated
- No direct cross-feature UI-model dependencies
- Explicit events, commands or stable interfaces between features
- Incremental delivery with testable and runnable slices
- Audio callbacks isolated from UI and blocking work

## .NET-specific decisions not to copy
- Avalonia, PrismApplication, named Prism regions, ReactiveUI and .NET DI registration
- NAudio, EF Core, NativeAOT, C# interfaces and ViewModel structures
- The old package/deployment specifics

## Rust target principles (proposed)
Cargo workspace, idiomatic ownership, small domain-oriented crates, traits at external boundaries, composition root, bounded message channels for non-real-time coordination, explicit UI state and a dedicated real-time audio path. UI framework remains provisional pending a representative Slint spike.

## Important source discrepancy
The arc42 is a March 2026 draft with ambitious scope; the later MVP and October roadmap narrow current implementation. A specification is not evidence of completion. Verify code and tests before declaring feature parity.

## Source ADR mapping
- ADR-0001: retain platform-specific audio backends as a concept.
- ADR-0002: VST3 is a stub; plan separate licensing/SDK/host validation.
- ADR-0003: preserve modularity, event boundaries and incremental approach; do not reproduce Prism regions in Rust.

## Remaining audit
Inspect code-level interfaces, controller mappings, tests, detailed phase docs, visual assets, user manuals and current branch history. Record exact source paths for every verified capability.
