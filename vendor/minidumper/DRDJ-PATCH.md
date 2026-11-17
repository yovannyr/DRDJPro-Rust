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

## Hard temporary-file limit (Spec 26)

`ServerHandler::minidump_size_limit` optionally bounds logical file length while
writing. The app sets 10,000,000 bytes. Linux/Android and macOS use a checked
Write/Seek adapter; the Linux writer also receives its upstream size hint. The
hint alone is not a hard bound. Rewrites do not consume cumulative quota; writes
past the end fail before touching disk. Failure is sticky and checked even when
upstream tolerates a section failure. Constructor/write errors still reach the
handler completion callback, so partial evidence and its store lock are released.

Windows bounded captures use a small external-process-only MiniDumpNormal path
with the existing windows 0.54 bindings, preserving ClientPointers=TRUE and owning
the process handle with RAII. IoStart/IoWriteAll/IoFinish direct all writes through
the same checked adapter. DbgHelp receives only a read-only fallback handle: even
missing alternate-I/O support cannot write past the bound. The parent's native
exception callback and IPC deadlines are unchanged. This additional Windows path
must be compared against upstream changes when updating minidump-writer.

This library is now a workspace member so its limiter/callback unit tests and
Clippy run in the normal CI. Removed example/integration-test manifest entries
pointed at files not present in the vendored subset; unused upstream devdeps were
replaced by the workspace's existing tempfile version. No new crate versions.
The tiny ipc/client.rs formatting change follows workspace rustfmt.
