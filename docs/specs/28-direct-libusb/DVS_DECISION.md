# DVS: xwax integration assessment (2026-11-12)

Status: approved and implemented as the optional `dvs-xwax` build (2026-11-12).
The user explicitly authorized GPLv3-compatible integration. The pinned vendored
decoder, tests, native input worker and A/B app integration are documented in
[IMPLEMENTATION.md](IMPLEMENTATION.md); the optimized app build passes.
Physical DVS acceptance and a complete binary-distribution license audit remain
open. The findings below record the rationale for this decision.

## Findings

The repository's own `LICENSE` is MIT. Upstream xwax licenses `timecoder.c`,
`lut.c` and their headers under GPL version 3. The decoder's source header
explicitly directs incompatible/proprietary integrations to obtain a separate
license from its author. MIT notices can remain on our own code, but they do not
make a distributed executable containing xwax an MIT-only binary.

For a linked build, the working integration requirement is GPLv3-compatible
distribution of the combined program, preserved copyright/license notices and
corresponding source/build instructions when distributing binaries. Audit the
complete dependency/license set before release. This is an engineering assessment
of the upstream terms, not a legal clearance. An optional Cargo feature would
separate build choices, not remove obligations from binaries built with xwax.
A dedicated IPC process is not assumed to be a license exemption.

Sources inspected:

- [xwax license](https://github.com/xwax/xwax/blob/master/COPYING)
- [decoder implementation and license notice](https://github.com/xwax/xwax/blob/master/timecoder.c)
- [decoder API](https://github.com/xwax/xwax/blob/master/timecoder.h)
- [build dependencies](https://github.com/xwax/xwax/blob/master/Makefile)
- [lookup table implementation](https://github.com/xwax/xwax/blob/master/lut.c)

## Proposed bounded implementation, subject to distribution decision

1. Pin an upstream revision and vendor only the decoder dependency subset with
   original notices, GPL text, provenance and documented portability changes.
   A C wrapper owns an opaque decoder; Rust never duplicates C struct layouts.
   Confirm the Windows/MSVC build separately: upstream includes POSIX headers
   and the complete desktop build targets ALSA/JACK/OSS, not our CPAL runtime.
2. Expose the upstream-supported named record editions explicitly. A `Serato`
   label alone must not imply all NoiseMap/Ink editions or physical validation.
   The native API receives stereo signed-16-bit PCM and exposes absolute
   position, pitch, resolution, validity and time since a valid position.
3. Construct lookup tables and buffers before capture, serialize upstream global
   definition/table initialization, then keep decoder ownership on one worker.
   Do not free shared lookup tables while any decoder uses them. No lookup
   allocation, logging, driver I/O or UI calls in the audio output callback.
4. Receive actual stereo USB-audio input through an OS audio backend such as
   CPAL. Generic USB bulk/interrupt transfers are not a substitute for an
   isochronous USB Audio stream. Use bounded PCM handoff and explicit loss
   counters. Overflow, capture errors and stale/invalid timecode must stop DVS
   control safely, without automatically restarting paused decks.
5. Replace the existing simulated path only after generated/native fixtures
   establish forward/reverse tracking, -5% speed, absolute jumps, silence,
   channel swaps and invalid input. Add playback ownership and UI tests, then
   independently review FFI safety and timing. Physical ±10ms and device
   compatibility acceptance remain deferred to joint hardware tests.

## Decision and remaining acceptance

The user explicitly authorized a GPLv3-compatible DVS-enabled combined executable
after reviewing these distribution consequences. Preserve the MIT notices for
our own code and document the combined build's GPL obligations. FR-007 has software
implementation and automated evidence; SC-003 physical position accuracy remains
unmeasured. See [ACCEPTANCE.md](ACCEPTANCE.md) for the current acceptance ledger.
