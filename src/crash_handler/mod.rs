//! Crash evidence storage. Filesystem operations belong on control/worker threads,
//! never in an audio callback or a native crash signal handler.
mod config;
mod privacy;
mod store;

pub use config::{default_database_dir, CrashSettings};
pub use store::{CrashReport, CrashStore, ReportStatus};

#[cfg(test)]
mod tests;
