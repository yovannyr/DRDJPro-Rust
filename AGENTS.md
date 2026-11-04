# AGENTS.md

## Review workflow override (user instruction, 2026-11-03)

For the remaining MC4000 slices S8-S15, the user explicitly removes the
independent sub-agent review requirement. Complete appropriate automated tests
and final checks before committing and pushing each bounded slice. The existing
publication authorization and requirement to preserve unrelated changes remain.
Do not claim independent review for these slices. Native hardware/listening
acceptance remains separate from automated verification.

## Project Context

Implementation override (user instruction, 2026-11-04): after another QWEN
timeout, the user explicitly authorizes GPT-6.1 Sol for S13-S15. Codex may
implement these slices directly with Sol; the S8-S15 independent-review waiver
and green-check/publication workflow above remain in force.

DRDJPro-Rust is a professional DJ application focused on:

- audio playback
- beat detection
- BPM analysis
- waveform rendering
- synchronization
- playlist management
- low-latency performance
- cross-platform compatibility

When making decisions, prioritize:

1. Audio accuracy
2. Playback stability
3. Real-time performance
4. Maintainability
5. User experience

---

## Coding Standards

### General

- Prefer clear and maintainable code.
- Avoid premature optimization.
- Keep functions small and focused.
- Prefer composition over inheritance.
- Follow SOLID principles where applicable.
- Minimize technical debt.
- Eliminate dead code when discovered.

### Rust Guidelines

- Prefer idiomatic Rust.
- Use strong typing.
- Avoid unnecessary cloning.
- Prefer Result<T, E> over panics.
- Propagate errors using `?`.
- Document public APIs.
- Keep lifetimes simple whenever possible.
- Prefer enums over string-based state handling.

### Async

- Use async only when it provides measurable value.
- Avoid unnecessary task spawning.
- Minimize lock contention.
- Prefer message passing where appropriate.

---

## Performance Rules

Performance is important.

When modifying code:

- Avoid unnecessary allocations.
- Avoid unnecessary copies.
- Minimize heap usage.
- Profile before introducing complex optimizations.
- Consider cache friendliness.
- Measure performance-sensitive changes.

For audio processing:

- Avoid blocking operations.
- Avoid large allocations in real-time paths.
- Prefer deterministic execution.
- Minimize latency.

---

## Audio/DSP Rules

Treat DSP and audio-engine code as critical.

When modifying:

- beat detection
- BPM calculation
- synchronization
- waveform generation
- effects processing
- audio playback

Always:

- explain assumptions
- explain performance impact
- identify potential timing issues
- identify thread-safety concerns

Never sacrifice audio correctness for code elegance.

---

## Architecture Rules

Preserve separation between:

- UI
- Application Logic
- Domain Logic
- Infrastructure

Avoid tight coupling between modules.

Prefer:

- interfaces/traits
- dependency injection
- inversion of control

Do not introduce framework dependencies without justification.

---

## Pull Request Reviews

Review for:

- correctness
- maintainability
- thread safety
- audio stability
- resource usage
- security concerns

Highlight:

- bugs
- race conditions
- performance regressions
- unnecessary complexity

Do not approve changes simply because they compile.

---

## Testing Requirements

Prefer automated tests.

Create:

- unit tests
- integration tests
- regression tests

For audio features:

- verify BPM accuracy
- verify beat-grid stability
- verify synchronization logic
- verify playback correctness

Important audio functionality should not rely solely on manual testing.

---

## Documentation

Document:

- public APIs
- architecture decisions
- performance-sensitive code
- DSP algorithms

Keep documentation concise and practical.

---

# AI MODEL ROUTING POLICY

## Primary Engineering Model

Use qwen-3.6-35b-sovereign for:

- architecture decisions
- implementation planning
- debugging
- root-cause analysis
- refactoring
- code reviews
- Rust development
- C# development
- Azure discussions
- technical documentation
- multi-file changes
- repository-wide analysis

This is the default model.

---

## Fast Execution Model

Use deepseek-v4-flash-sovereign for:

- unit tests
- boilerplate code
- helper functions
- JSON generation
- YAML generation
- commit messages
- summaries
- code explanations
- small code modifications

Do not use as the sole reviewer for architecture decisions.

Escalate to qwen-3.6-35b-sovereign whenever deeper reasoning is required.

---

## Embedding Model

Use qwen-3-vl-embedding-2b-sovereign exclusively for:

- embedding generation
- vector indexing
- semantic search
- RAG pipelines
- similarity search
- documentation retrieval
- screenshot retrieval
- image search

Never use embedding models for:

- code generation
- code reviews
- debugging
- architecture decisions
- user-facing responses

---

## Model Escalation Rules

Always escalate to qwen-3.6-35b-sovereign if:

- more than one module changes
- architecture is discussed
- DSP code is modified
- synchronization logic changes
- security impacts exist
- performance-critical code changes
- root cause analysis is needed

---

## Retrieval-Augmented Generation (RAG)

When RAG is available:

Generate embeddings using:

- qwen-3-vl-embedding-2b-sovereign

Index:

- Markdown files
- Architecture documents
- ADRs
- Wiki pages
- API specifications
- Technical documentation

Retrieve context before generating large implementations.

---

## Preferred Output Style

Responses should be:

- precise
- implementation focused
- technically accurate
- concise when possible

Prefer:

- code examples
- concrete recommendations
- actionable next steps

Avoid:

- excessive theory
- vague statements
- generic best practices without context

---

## Decision Priority

1. Correctness
2. Stability
3. Performance
4. Maintainability
5. Developer Productivity
6. Convenience

## Repository Review Strategy

Before making changes:

1. Read relevant files.
2. Understand existing architecture.
3. Follow existing patterns.
4. Prefer consistency over novelty.
5. Minimize file modifications.
6. Explain tradeoffs before large changes.

## Model Preference
Prefer Models from adesso-sovereign.If there are not available, then use the Copilot included Models

Prefer qwen-3.6-35b-sovereign for:

- architecture
- repository analysis
- debugging
- refactoring
- code review
- DSP
- audio engine changes

Prefer deepseek-v4-flash-sovereign for:

- tests
- documentation
- code generation
- simple modifications

When uncertain, choose qwen-3.6-35b-sovereign.
## Rust Guidance

Prefer qwen-3.6-35b-sovereign for:

- ownership analysis
- lifetime management
- async architecture
- tokio concurrency
- trait design
- performance optimization
- unsafe code review
- DSP algorithms
- audio engine changes
- cross-crate refactoring

Prefer deepseek-v4-flash-sovereign for:

- unit tests
- documentation
- serde models
- boilerplate
- cargo configuration
- small localized fixes

When uncertain, choose qwen-3.6-35b-sovereign.

Never introduce unnecessary cloning.
Prefer borrowing over ownership transfer.
Minimize allocations in audio processing paths.
Avoid blocking operations in realtime threads.

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

For subsequent implementation work, use the QWEN model (qwen-3.6-35b-sovereign) configured through QWEN_API_KEY, QWEN_BASE_URL and QWEN_MODEL. And For DeepSeek use DEEPSEEK_API_KEY,DEEPSEEK_BASE_URL, DEEPSEEK_MODEL. Use the adesso-Advance Models only if the Task is really complex. Codex coordinates context, applies/inspects returned changes and runs real local checks; do not claim the orchestrator itself has changed model. Never print credentials or commit them, and do not silently replace an unavailable/unauthenticated QWEN or DeepSeek implementation model. Report the blocker and obtain the user's direction.

The user now selects independent review with gpt-6.1-sol, reasoning effort medium (GPT-6.1 Sol, Mittel), replacing the earlier Astra-medium reviewer selection for this repository. Do not automatically downgrade to low merely to reduce cost; any different reviewer/effort needs user direction. Keep the bounded-slice, fix-findings, final-green-checks, commit/push and progress-reporting workflow above. The user's low-versus-medium price question is not authorization to claim a cheaper guaranteed bill or to weaken audio/Sync review.
