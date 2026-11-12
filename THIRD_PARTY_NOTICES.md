# Additional build-specific third-party notices

This file supplements, rather than replaces, dependency and asset notices.
The repository's own MIT license remains in `LICENSE`.

## Optional `dvs-xwax` build

The `drdj-dvs` crate includes the xwax decoder by Mark Hills, pinned to
`be863572137929f70aa3ff0cef3b6b76f4973005`, under **GPL-3.0-only**.
Original copyright notices and the complete GPL text accompany the source in
`crates/drdj-dvs/vendor/xwax`; local changes are listed in its `VENDOR.md`.

A combined executable containing this decoder must be distributed on
GPLv3-compatible terms. Retain upstream notices and provide corresponding source,
including modifications and build instructions, with distributed binaries under
the applicable GPLv3 source-conveyance requirements. Do not label the combined
DVS build as MIT-only. This opt-in is not an exemption from those requirements.
Review the complete dependency/license inventory before any binary release.

Build from this repository revision and its Cargo.lock with
`cargo build --release --locked --features dvs-xwax`.
Rust 1.92 or newer, the platform C/C++ toolchain/SDK and the normal application's
system dependencies are required; the decoder subset is vendored and built by
Cargo, not downloaded at runtime. Manual/hardware acceptance is separate from
buildability and license notices. No claim of a completed legal release audit.
