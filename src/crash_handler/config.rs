use serde::{Deserialize, Serialize};
use std::{io, path::PathBuf};

/// Sharing preferences. Local capture does not authorize transmission.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CrashSettings {
    /// Explicit opt-in for future textual reports and retries.
    pub auto_submit: bool,
    /// Configured HTTPS endpoint; empty means uploads are unavailable.
    pub server_url: String,
}

/// Spec 26's database location; never falls back to the working directory.
pub fn default_database_dir() -> io::Result<PathBuf> {
    #[cfg(windows)]
    let base = dirs::config_dir();
    #[cfg(not(windows))]
    let base = dirs::home_dir().map(|home| home.join(".local/share"));
    base.map(|base| base.join("DRDJPro-Rust/crashes"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No user data directory"))
}
