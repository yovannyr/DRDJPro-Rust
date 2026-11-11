//! Crash evidence storage. Filesystem operations belong on control/worker threads,
//! never in an audio callback or a native crash signal handler.
mod config;
mod privacy;
mod process;
mod store;

pub use config::{default_database_dir, CrashSettings};
pub use process::{run_monitor, MonitorGuard};
pub use store::{CrashReport, CrashStore, ReportStatus};

/// Dispatch internal subprocess modes before the application initializes audio.
/// Probe modes require an explicit directory and never run in ordinary startup.
pub fn internal_mode() -> Option<std::io::Result<()>> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next()?;
    if mode != "--crash-monitor" && mode != "--crash-probe" {
        return None;
    }
    Some((|| {
        let database = args
            .next()
            .ok_or_else(|| std::io::Error::other("Missing crash database"))?;
        let argument = args
            .next()
            .ok_or_else(|| std::io::Error::other("Missing crash mode argument"))?;
        if mode == "--crash-monitor" {
            let parent = args
                .next()
                .map(|pid| {
                    pid.to_string_lossy()
                        .parse::<u32>()
                        .map_err(std::io::Error::other)
                })
                .transpose()?;
            return run_monitor(
                std::path::Path::new(&database),
                std::path::Path::new(&argument),
                parent,
            );
        }
        let store = CrashStore::open(database.into())?;
        let guard = MonitorGuard::start(&std::env::current_exe()?, &store)?;
        std::fs::write(
            store.root().join("probe-monitor.pid"),
            guard.process_id().to_string(),
        )?;
        match argument.to_str() {
            Some("panic") => panic_probe(),
            Some("thread") => {
                let _ = std::thread::spawn(panic_probe).join();
            }
            Some("sequential") => {
                for _ in 0..2 {
                    let _ = std::thread::spawn(panic_probe).join();
                }
                native_probe();
            }
            Some("native") => {
                native_probe();
            }
            Some("concurrent") => {
                let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
                let worker = std::sync::Arc::clone(&barrier);
                let _ = std::thread::spawn(move || {
                    worker.wait();
                    panic_probe();
                });
                barrier.wait();
                std::thread::sleep(std::time::Duration::from_millis(5));
                native_probe();
            }
            Some("normal") => {}
            Some("restart") => {
                let root = store.root().to_owned();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _guard = guard;
                    panic_probe();
                }));
                assert!(result.is_err());
                let _restarted =
                    MonitorGuard::start(&std::env::current_exe()?, &CrashStore::open(root)?)?;
                return Ok(());
            }
            Some("killed") => std::process::exit(23), // Simulate abrupt termination without Drop.
            _ => return Err(std::io::Error::other("Unknown crash probe")),
        }
        drop(guard);
        Ok(())
    })())
}

#[inline(never)]
fn panic_probe() {
    panic!("Controlled crash probe (payload deliberately omitted from reports)");
}

#[inline(never)]
fn native_probe() {
    // SAFETY: intentional OS access violation in an explicitly selected disposable
    // probe process; never executed by ordinary application startup.
    unsafe {
        std::ptr::write_volatile(std::hint::black_box(1usize) as *mut u8, 0);
    }
}

#[cfg(test)]
mod tests;
