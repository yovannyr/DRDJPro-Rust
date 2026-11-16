//! Synchronous pre-audio capture setup. Failure never prevents application startup.
use super::{
    default_database_dir,
    runtime_log::{self, emit, Event, Role},
    CrashStore, MonitorGuard,
};
use std::{io, path::Path};

/// Keep alive until UI, storage and audio have shut down. Uploads/UI are owned
/// separately, so closing the UI can cancel uploads before this guard is dropped.
pub struct CrashStartup {
    store: Option<CrashStore>,
    monitor: Option<MonitorGuard>,
    notice: String,
    // Declared after monitor: final capture/shutdown events precede log drain.
    runtime_log: Option<runtime_log::Guard>,
}

impl CrashStartup {
    /// Start capture for a normal GUI launch, before creating audio/UI resources.
    pub fn start() -> Self {
        match (std::env::current_exe(), default_database_dir()) {
            (Ok(executable), Ok(database)) => Self::at(&executable, &database),
            _ => Self::unavailable(),
        }
    }

    /// Explicit composition root, also used by isolated subprocess tests.
    pub fn at(executable: &Path, database: &Path) -> Self {
        let mut logger = None;
        let mut failed = false;
        let mut startup = Self::with_monitor(database, |store| {
            match runtime_log::install(store.root(), Role::App) {
                Ok(guard) => logger = Some(guard),
                Err(_) => failed = true,
            }
            MonitorGuard::start(executable, store)
        });
        startup.runtime_log = logger;
        if failed {
            startup
                .notice
                .push_str(" Diagnostic logging could not start.");
        }
        startup
    }

    fn with_monitor(
        database: &Path,
        start: impl FnOnce(&CrashStore) -> io::Result<MonitorGuard>,
    ) -> Self {
        let store = match CrashStore::open(database.into()) {
            Ok(store) => store,
            Err(_) => return Self::unavailable(),
        };
        match start(&store) {
            Ok(monitor) => Self {
                store: Some(store),
                monitor: Some(monitor),
                notice: String::new(),
                runtime_log: None,
            },
            Err(error) => {
                emit(Event::CaptureUnavailable, None, Some(error.kind()));
                Self {
                    store: Some(store),
                    monitor: None,
                    notice: "Crash capture unavailable; saved reports remain manageable".into(),
                    runtime_log: None,
                }
            }
        }
    }

    fn unavailable() -> Self {
        emit(Event::StoreUnavailable, None, None);
        Self {
            store: None,
            monitor: None,
            notice: "Crash storage unavailable; capture and sharing disabled".into(),
            runtime_log: None,
        }
    }

    /// Canonical store handle; no filesystem work is performed by this getter.
    pub fn store(&self) -> Option<CrashStore> {
        self.store.clone()
    }
    /// Non-sensitive startup diagnostic suitable for the UI.
    pub fn notice(&self) -> &str {
        &self.notice
    }
    /// PID for isolated lifecycle diagnostics.
    pub fn monitor_pid(&self) -> Option<u32> {
        self.monitor.as_ref().map(MonitorGuard::process_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_store_fails_open_without_starting_monitor() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("file");
        std::fs::write(&file, b"not a directory").unwrap();
        let startup = CrashStartup::with_monitor(&file, |_| panic!("must not start"));
        assert!(startup.store().is_none());
        assert!(startup.monitor_pid().is_none());
        assert!(startup.notice().contains("disabled"));
    }

    #[test]
    fn monitor_failure_retains_store_and_does_not_enable_sharing() {
        let temp = tempfile::tempdir().unwrap();
        let startup = CrashStartup::with_monitor(temp.path(), |_| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private diagnostic path",
            ))
        });
        let store = startup.store().unwrap();
        assert!(!store.settings().unwrap().auto_submit);
        assert!(store.settings().unwrap().server_url.is_empty());
        assert!(startup.monitor_pid().is_none());
        assert!(!startup.notice().contains("private diagnostic"));
    }
}
