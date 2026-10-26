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

## Standing review/publication workflow — user override (2026-10-25)

For subsequent slices, the user explicitly authorizes independent sub-agent review with `gpt-6-astra`, reasoning effort `medium` (GPT-6 Astra, Mittel), followed by fixing findings and appropriate final checks. After acceptable review and green checks, commit/push and continue the next planned bounded slice autonomously, with meaningful progress reports. This replaces rule 9's human-review stop for routine slices; keep each slice bounded and do not bypass unresolved blocking findings or failed checks. If the reviewer is unavailable, resolve a substitute with the user. Preserve unrelated work and report actual review/check/publication results. Do not claim native visual/listening acceptance from headless tests.


## Implementation/reviewer selection override (user instruction, 2026-10-26)

For subsequent implementation work, use the QWEN model configured through QWEN_API_KEY, QWEN_BASE_URL and QWEN_MODEL. Codex coordinates context, applies/inspects returned changes and runs real local checks; do not claim the orchestrator itself has changed model. Never print credentials or commit them, and do not silently replace an unavailable/unauthenticated QWEN implementation model. Report the blocker and obtain the user's direction.

The user now selects independent review with gpt-6.1-sol, reasoning effort medium (GPT-6.1 Sol, Mittel), replacing the earlier Astra-medium reviewer selection for this repository. Do not automatically downgrade to low merely to reduce cost; any different reviewer/effort needs user direction. Keep the bounded-slice, fix-findings, final-green-checks, commit/push and progress-reporting workflow above. The user's low-versus-medium price question is not authorization to claim a cheaper guaranteed bill or to weaken audio/Sync review.
