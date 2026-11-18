# A1 inference service

The optional ORT feature provides an isolated worker for the A0 synthetic fixture.
The application can instantiate the service through A2. The worker uses sequential CPU
ORT with one requested intra-op thread and disabled spinning. On Windows it runs
below normal priority. Playback protection starts enabled and requests a 1 ms
pause after each chunk, including the last. A future controller can update this policy
through the service handle. The worker owns model loading, inference and cache I/O.

Register a trusted local model with its exact ID/version, SHA-256, license,
synthetic-test use and declared minimum RAM. The caller supplies the RAM budget.
This admission comparison does not measure free RAM or enforce a process memory
limit. On a cache miss the worker checks at most 64 KiB of model data and passes
those same verified bytes to ORT. The model metadata and cache directory are
trusted local inputs. No remote model discovery or download is provided.

Submit prepared chunks and an audio fingerprint. The service accepts at most
eight waiting jobs and one running job. Each job has at most 256 chunks of eight
floats. High priority goes first, with FIFO order at equal priority. Running
jobs are not preempted. Submission can return QueueBusy under short mutex
contention or QueueFull at capacity. Both are immediate backpressure.
Cancellation is cooperative with the same between-work checkpoints as the
application's Cancellation token. The service uses its own atomic job phase
because cancellation must race atomically with final cache publication.
A native model call already in progress can finish. Ticket progress counts
completed chunks and reaches the terminal count only after successful publication.

The cache uses a dedicated local directory and 32 replaceable slots. The key
covers source audio SHA-256, model ID/version/hash, exact chunk values and the
pipeline revision. Fixed-size entries contain the full key, payload checksum
and finite output tensors. A valid hit requires registry metadata but does not
open the model file or initialize ORT. The cache stores only complete results.
Slots occupy at most 395,264 bytes of managed logical data, plus one in-flight
temporary file per service. Hard process termination can leave staging files
and filesystem allocations are separate. Use a dedicated directory per service.
On Windows, atomic replacement retries access-denied and sharing/lock violations
after 5, 15, 40 and 80 ms. Persistent errors leave the old slot intact.

## Checks

Reuse the generated A0 fixture, local ORT DLL and shared Cargo target. From the
workspace root set DRDJ_ML_FIXTURE and DRDJ_ORT_LIBRARY to absolute paths.
Use a single Cargo job. Build and test with cached sources:

```powershell
cargo test -p drdj-ml --features ort --locked --offline
cargo clippy -p drdj-ml --features ort --all-targets --locked --offline -- -D warnings
cargo build -p drdj-ml --example a1_load --features ort --profile ml-spike --locked --offline
```

Set DRDJ_A1_LOAD_BIN to the resulting a1_load.exe. The following opt-in test
renders both offline decks for 600 seconds without ORT, then 600 seconds under
active ORT service load. DRDJ_A1_SOAK_SECONDS shortens both phases equally for
smoke tests. Use the interactive development optimization level for the app
package while reusing the test profile's existing dependency artifacts.

```powershell
cargo --config 'profile.test.package.drdjpro-ui-spike.opt-level=1' test --bin drdjpro-ui-spike --locked --offline a1_headless_render_with_inference_load -- --ignored --nocapture
```

On Windows each phase runs on a dedicated thread with CPAL's WASAPI render
priority. The harness paces 480-frame blocks at 48 kHz against a fixed clock.
Late wakeups count against the original deadline, including when one missed
block affects the next one. It reports render time and
dispatch lateness p99, render overruns against 10 ms, end-to-end deadline
misses, wrong/missing-output blocks and allocations on the measured render thread.
Native device underruns, driver latency, hearing quality and larger production
models require separate acceptance.

## Local evidence

On Windows, Rust 1.99.1 passed all 14 ORT tests offline, including an actual
cache-miss inference over the fixture followed by a cache hit with the model
file removed. The deterministic worker tests cover cancellation in a chunk,
priority and FIFO ordering, contention, RAM admission and cache integrity.
Strict ORT Clippy with Rust 1.100.0 and the three Python tests passed.
The default workspace passed strict all-target Clippy and 2,363 library and
binary unit tests. Seven opt-in tests were excluded from that invocation,
including the separately executed A1 soak. Formatting and diff checks passed.

Both long measurements used the unoptimized Windows application test profile.
The 600-second run before playback pacing completed 60,000 render blocks with
zero incorrect-output blocks and zero measured allocations. It failed the
10 ms timing gate with 62 render overruns and 250 end-to-end misses. The
loaded render p99 was 4,754 µs and dispatch p99 was 655 µs. The 20-second
baseline had zero misses. The unequal durations limit the comparison.

The paced 600-second run also rendered all 60,000 blocks with zero
incorrect-output blocks and zero measured allocations. It processed 1,477
cache-miss jobs with 256 chunks each. Render p99 was 4,207 µs and dispatch p99
was 730 µs. There were 34 render overruns and 125 end-to-end deadline misses.
The 20-second baseline again had zero misses. The strict 10 ms timing gate
failed for that run. These observations do not establish the cause of each late block,
the baseline tail over an equal duration or playback behavior on a device.

Independent review by GPT-6.1 Sol at medium reasoning checked the code,
follow-up fixes and these measurements against the logs. The measured timing failure motivated the following repeat.


### Equal-duration deadline comparison on Windows

Both phases used the same Rust 1.99.1 executable, the app package at opt-level 1,
and the WASAPI thread priority. Each rendered 60,000 blocks over 600 seconds.
The absolute limit was 10 ms from each scheduled start, with zero misses allowed.

| Phase | Render p99 / max (us) | Dispatch p99 / max (us) | Misses | Wrong-output blocks | Render allocations |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline | 1510 / 2203 | 568 / 2505 | 0 | 0 | 0 |
| ORT load | 1402 / 2665 | 609 / 2028 | 0 | 0 | 0 |

The service completed 1,462 distinct cache-miss jobs of 256 chunks. Neither
phase recorded a render overrun or a start later than the block's deadline.
EQ now compares its 21 coefficients only when a command is pending, preserving
latest-command behavior through a fade. Optimization, render scheduling and EQ
changed together, so their individual timing effects are unknown. This pair
passes the headless timing gate for the synthetic model on this host. Constant
PCM verifies the expected level but cannot expose a duplicated identical sample.

## A2 library worker integration

Build the app with `--features ml-import`. Before launch, set
`DRDJ_ML_IMPORT_CACHE` to an absolute dedicated cache directory and
`DRDJ_ML_FIXTURE` and `DRDJ_ORT_LIBRARY` to the trusted absolute model and
runtime paths used in the checks above. Omit the cache setting to disable this
path. Configuration is read once per process. No downloads occur.

Configured file and folder imports perform full classical analysis on the import
worker. This increases import time. Explicit track analysis uses the same
single decode. The worker hashes decoded PCM plus its format and pools the
waveform overview to at most 256 eight-value peak tensors, zero-padding the
last one. The synthetic model output is cached for diagnostics. Library BPM,
key and gain remain classical. Queue pressure, model and cache errors skip
the optional result. Cancellation abandons the ticket. Deck loading, playback,
relocation and passive tag reads use their existing paths.

To repeat the A2 integration check with the fixture and runtime paths set above:

```powershell
cargo test --bin drdjpro-ui-spike --features ml-import --locked --offline import_and_analysis_use_optional_inference -- --ignored
```

The test isolates runtime configuration in child processes and uses a temporary
cache. The A1 timing evidence above covers its synthetic inference load. It
does not measure the end-to-end cost of decoding and hashing during A2 imports.
