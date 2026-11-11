/// Crash handler for DRDJPro-Rust.
///
/// Generates minidumps on panic, captures stack traces, and optionally uploads
/// crash reports to a configured server endpoint.
///
/// Design goals:
/// - Non-blocking in normal operation
/// - Minimal footprint in audio-critical paths
/// - Platform-agnostic API with platform-specific handlers

mod config;
mod reporter;
mod handler;
mod uploader;
mod queue;

#[cfg(test)]
mod tests;

pub use config::CrashHandlerConfig;
pub use handler::init_crash_handler;

/// Initialize the crash handler subsystem.
///
/// This registers panic hooks and signal handlers that delegate to the
/// [`CrashReporter`].  The handler runs asynchronously and never blocks
/// the audio processing thread.
pub fn init(config: &CrashHandlerConfig) -> Result<(), CrashError> {
    reporter::init(config)?;
    handler::init_crash_handler(config);
    queue::load(config)?;
    Ok(())
}

pub use reporter::CrashError;
