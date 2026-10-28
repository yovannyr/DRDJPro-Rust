# drdj-keylock

Worker-only whole-clip tempo processing preserving pitch. Not yet connected to the desktop decks. Supports mono/stereo, 8–192 kHz and tempo 0.5–2.0. Input and output are independently capped at 16,777,216 interleaved samples; DSP internal allocations are additional. Never invoke from the realtime callback. The adapter copies input, catches C++ exceptions, checks finite output and returns explicit errors.

Vendored implementation: Signalsmith Stretch and Signalsmith Linear as packaged in signalsmith-stretch 0.1.3, plus its interleaved C wrapper. Upstream licenses are retained in vendor; WRAPPER-LICENSE.md covers the Rust-package wrapper. Our bridge adds an exception boundary and hand-written minimal FFI, avoiding build-time libclang. cc compiles C++17; /Zc:__cplusplus is requested when supported on MSVC.

The synthetic stereo test checks duration, central-window zero-crossing frequency and channel polarity at 0.8 and 1.25 tempo. It does not qualify transients, arbitrary music, sound quality, runtime rate changes or hardware. Renderer buffering, source-coordinate mapping, latency compensation and transport integration remain required.
