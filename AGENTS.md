# Agent instructions – DrDJ Pro Rust

1. Work in small, explicitly scoped slices. Do not implement the whole application in one task.
2. Read docs/MIGRATION_PLAN.md and docs/GUI_COMPONENTS.md before proposing changes.
3. Treat https://github.com/yovannyr/DRDJ as a read-only functional reference. Never modify that repository as part of this project.
4. Use idiomatic Rust: Cargo workspace, focused crates, traits at meaningful boundaries, explicit ownership and constructor injection/composition root by default. Do not recreate Prism or a .NET DI container.
5. Keep real-time audio callbacks free of blocking operations, heap allocations and locks wherever practical; validate these constraints with tests and measurements.
6. GUI framework is provisional: evaluate Slint with a representative deck/waveform prototype before final selection.
7. Before code: state scope, files to change, acceptance criteria and test plan. After code: summarize changes, commands run, results and remaining risks.
8. Preserve feature parity through a traceable inventory and reference tests; do not assume README claims are verified.
9. Stop after the agreed slice and request review before expanding scope. Avoid unnecessary dependencies and verbose generated output.
