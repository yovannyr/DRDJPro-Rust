# Spec 30 implementation and acceptance

## Scope and external dependency

The user confirmed on2026-11-13 that no Beatport partner/SDK access is available.
This slice implements the native browser/help shell. Website login is distinct
from an authorized catalog/audio integration. OAuth provider registration, token
exchange, Beatport deck loading and the two-second streaming scenario remain
blocked, not simulated or marked complete. Browser audio is not routed into decks.

## Architecture

Windows uses Wry0.57.2 / OS WebView2 as a child of a dedicated Slint container
`ui/web_view.slint`. WEB and HELP have independent native views/history and separate
application-specific browser data directories. Incognito sessions keep cookies
out of the main application; no token extraction, IPC bridge, host objects,
certificate-error bypass or browser-sandbox disabling is installed. No runtime
starts until an explicit action. Missing runtime/init failures leave the app
running with a status message. Other OS backends are explicitly unavailable.

Back/forward/reload use fixed native-executed scripts; address input accepts HTTPS
and our exact help origin only. Credentials, control characters and unsafe schemes
are rejected. Query/fragment values are omitted from host status/address events;
raw popup targets remain transiently in the bounded queue to navigate them. Popups
are routed into the same view after validation; popup/opener-dependent OAuth may
need a provider-specific supported flow. Downloads and device permissions are
blocked. The native engine handles page CSP and default GPU/software fallback;
actual acceleration/fallback/resource consumption is not yet measured.

Bundled HTML/CSS/SVG is served from an explicit resource table, not file paths.
Local CSP disables scripts, forms, external assets and framing. Help works without
network. Main-thread native browser lifecycle and a bounded32-event queue are
separate from the audio callback; no audio-engine access is added. A30s host timeout
leaves retry available; native WebView2 handles HTTP/network/certificate error
pages. The host reports 'Navigation ended; inspect the page for errors', never HTTP success.

## Requirement ledger

| Requirement | Implemented software | Remaining |
|---|---|---|
| FR-001/002/009 | Wry child browser in Slint container, HTTPS load | Native render acceptance, other OS backends |
| FR-003/004 | Per-view native history and back/forward/reload/address | Native50-page history qualification |
| FR-005 | Standard engine JavaScript; fixed navigation commands | Provider-site compatibility |
| FR-006 | Website login possible in isolated native view | Provider OAuth/SDK registration and Beatport access absent |
| FR-007/008 | WebView2 default GPU/software fallback policy | Actual GPU/software/load qualification |
| FR-010 | Dedicated data paths, incognito, no host bridge | Native storage/cookie isolation qualification |
| FR-011 | Embedded offline help and assets with restrictive CSP | Native visual acceptance |
| FR-012 | Init errors, rejected URL status,30s timeout, native error pages | Native HTTP/network/certificate cases |
| FR-013 | Custom DRDJPro-Rust/0.1.0 User-Agent | Provider compatibility; custom UA may require approved adjustment |
| FR-014 | Third-party CSP retained; help CSP scoped to own responses | Native site compatibility |

SC-001/002/003/005/006/007/008 require network/provider/native performance or visual
qualification and remain open. SC-004 has50-address headless dispatch coverage;
this does not establish actual engine history. No physical/browser/runtime/manual
session has been performed. Spec26 remains stopped.

References inspected:
- https://docs.rs/wry/0.57.2/wry/
- https://docs.slint.dev/latest/docs/rust/slint/struct.Window
- https://support.beatport.com/hc/en-us/articles/4414997837716-Terms-and-Conditions
- https://support.beatport.com/hc/en-us/articles/4412639493012-What-are-the-terms-of-use-for-Beatport-Streaming

Review corrections: Wry defaults disable SmartScreen; explicit browser arguments
replace that default while retaining user-gesture autoplay. Unsupported public
custom-scheme URLs are rejected; the help resource callback still accepts the
engine's internal protocol representation. Runtime102+ is required before native
creation, preventing the older-runtime InPrivate fallback. Boundary tests cover
invalid/old/current runtime versions. Non-Windows app actions return an explicit
unsupported status; no native Windows types leak into that adapter.

## Automated checkpoint

Five targeted web tests passed: unsafe URL rejection, explicit help resources/CSP,
credential-value redaction, runtime-version boundaries and lazy independent browser
UI actions (50address dispatches, navigation commands, close without playback
changes). No native web view opened. Strict root Clippy for all targets passed
without warning suppression. Focused formatting/diff checks passed. Independent
GPT-6.1-Sol review (medium) accepted the three fixes and runtime guard described
above. Slint app layout/waveform and wider library regressions follow below.

Final regression for this browser slice:1,688 library tests, eight modern-waveform
and five layout tests passed (1,701 total). Strict root all-target Clippy and
focused formatting/diff checks passed. Independent GPT-6.1-Sol medium final review
accepted fixes, native feature configuration and explicit unavailable-OS adapter.
No native browser/network/provider/visual/manual acceptance inferred.

After rebuilding test/check/app artifacts from the user-cleared cache, target
occupied5,127,205,263bytes (about4.8GiB); incremental held no files. This is an
observed checkpoint, not a hard quota or proof of a universal reduction ratio.
Release validation is deferred to the final combined Spec31 checkpoint to avoid
rebuilding that profile for every slice. App dev playback optimization is retained.
