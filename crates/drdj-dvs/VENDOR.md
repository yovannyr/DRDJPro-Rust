# xwax decoder provenance and local changes

Upstream: https://github.com/xwax/xwax
Pinned revision: `be863572137929f70aa3ff0cef3b6b76f4973005`.
Author/copyright: Mark Hills, 2026 (original headers retained).
License: **GPL-3.0-only**, full text in `vendor/xwax/COPYING`.
This crate and its C adapter are distributed under GPL-3.0-only.

Vendored subset: `timecoder.c`, `timecoder.h`, `lut.c`, `lut.h`, `pitch.h`,
`debug.h`, `COPYING`. The complete xwax desktop UI/audio engine is not linked.
The synthetic test generator adapts the pinned upstream `mktimecode.c` method;
its notice and GPL terms are retained in the test source. This is not a
recorded or physically validated vinyl fixture and not a new protocol claim.

Local modifications (DRDJPro, 2026-11-12):

- `timecoder.c`: removed unused POSIX `unistd.h` for Windows/MSVC; replaced
  shifting negative signed PCM with defined signed multiplication; widened
  zero-crossing threshold comparisons and subtraction to avoid signed overflow
  for full-scale/DC input. No report-format definitions changed.
- `lut.c`: free the first allocation if the second allocation fails.
- `src/bridge.c`: opaque allocation/free, readonly PCM adapter, scalar getters
  and full reset; decoder memory is zero-initialized, including upstream fields
  not individually initialized by `timecoder_init`.

Rust serializes lookup construction. At most the three exposed edition tables
remain for process lifetime (about 21 MiB total), so no decoder can retain freed
tables. Instances are intentionally neither Send nor Sync and are created on
their decoding thread. Native submit does not retain the input slice.
The C decoder is compiled at optimization level 2 in all profiles.

Rebuild/check: `cargo test -p drdj-dvs --locked` and
`cargo clippy -p drdj-dvs --all-targets --locked -- -D warnings`.
Decoder timing probe:
`cargo test -p drdj-dvs --release --locked decoder_processing -- --nocapture`.
These checks do not exercise audio input hardware, UI dispatch or output latency.
