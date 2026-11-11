/// Install panic hooks and signal handlers for crash detection.
///
/// This module installs handlers during application startup and never
/// blocks — all heavy work (minidump writing, queue management) runs
/// synchronously only during an actual panic so the panic handler
/// returns quickly.

use super::config::CrashHandlerConfig;
use std::sync::atomic::AtomicBool;

static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install the crash handler panic hooks.
///
/// This is called once during application startup.  It is idempotent —
/// calling it multiple times is safe.
pub fn init_crash_handler(config: &CrashHandlerConfig) {
    if HANDLER_INSTALLED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    if !config.enabled {
        return;
    }

    // The reporter module already installs the panic hook during init(),
    // so this function exists for future signal-handler integration on
    // Unix (SIGSEGV, SIGABRT via libc) and Windows (SEH via
    // `windows-sys`).

    // TODO (Phase 1, follow-up):
    // - On Unix: register a signal handler for SIGSEGV, SIGABRT, SIGILL
    //   using `libc::signal` that calls the reporter.
    // - On Windows: register a structured-exception handler via
    //   `SetUnhandledExceptionFilter` or the `windows-sys` crate.
}

/// Check whether the crash handler has been installed.
pub fn is_installed() -> bool {
    HANDLER_INSTALLED.load(std::sync::atomic::Ordering::SeqCst)
}
