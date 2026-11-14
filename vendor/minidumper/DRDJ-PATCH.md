# Bounded IPC patch

Upstream: EmbarkStudios/crash-handling, crates.io `minidumper` 0.11.0.
MIT/Apache-2.0 licenses and source are retained. Only the library source, manifest,
README and licenses are vendored; this is not an implementation of other specs.

Local functional change in `src/ipc/client.rs`: set five-second receive/send
timeouts before the macOS handshake. Upstream hides its socket and otherwise
permits an indefinite receive if a monitor becomes unresponsive. This is needed
for Spec 26's bounded failure path, not a lint workaround. macOS crash-context
Mach IPC already specifies its own two/five-second deadlines upstream.

A `socket2` dependency sets these options on the private native socket wrappers.
The Windows connect error carries its operation name for startup diagnostics.
On Linux/Android, `uds`0.4's existing safe timeout setters are used directly:
its socket does not implement the `AsFd` trait required by `socket2`0.6.
This build-compatibility adjustment retains both five-second deadlines and socket
ownership; it adds no new crash-reporting functionality or unsafe descriptor code.

`src/ipc/server.rs` retains/rearms the connection after capture on all supported
platforms rather than consuming it. This is needed for caught/worker Rust panics:
the app may continue, and a later native fault still needs the same monitor.
There is exactly one connection per process/PID, including on macOS. All PONG and
CRASH_ACK receives are serialized inside one native callback; other paths only
send messages. The wire format and minidump writer remain unchanged.

The application helper watchdog starts before the dump request and expires
before the client receive timeout. Timeouts are failure, never receipt/capture
success. Remove this patch when upstream provides equivalent deadlines and
persistent capture connections. Preserve/re-run the process regression corpus.
