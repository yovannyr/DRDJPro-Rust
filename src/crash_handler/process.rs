use super::{privacy::sanitize_stack, store::Capture, CrashStore};
use minidumper::{Client, LoopAction, Server, ServerHandler, SocketName};
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

type PanicHook = dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static;
const STACK: u32 = 1;
const HEARTBEAT: u32 = 2;
const SHUTDOWN: u32 = 3;
const BEGIN_CAPTURE: u32 = 4;
static ACTIVE: AtomicBool = AtomicBool::new(false);

struct PanicState {
    handler: Arc<crash_handler::CrashHandler>,
    client: Arc<Client>,
    busy: AtomicBool,
}

/// Owns the dedicated monitor, native handler and heartbeat. Startup is bounded;
/// normal shutdown waits at most 500 ms before terminating an unresponsive helper.
/// No work is added to audio callbacks. A crashing thread must wait for its memory
/// snapshot; the helper has a five-second watchdog. Rust panic symbolization uses
/// the panic thread (not the native signal callback) and may allocate on failure.
pub struct MonitorGuard {
    child: Child,
    stop: mpsc::Sender<()>,
    heartbeat: Option<thread::JoinHandle<()>>,
    previous: Arc<PanicHook>,
    state: Option<Arc<PanicState>>,
    _socket_dir: SocketDirectory,
}

impl MonitorGuard {
    /// Start this executable in monitor mode before creating UI/audio resources.
    pub fn start(executable: &Path, store: &CrashStore) -> io::Result<Self> {
        if ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Crash monitor already active",
            ));
        }
        let result = Self::start_inner(executable, store);
        if result.is_err() {
            ACTIVE.store(false, Ordering::Release);
        }
        result
    }

    fn start_inner(executable: &Path, store: &CrashStore) -> io::Result<Self> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Diagnostics::Debug::{SetErrorMode, SEM_NOGPFAULTERRORBOX};
            // SAFETY: startup-only process setting suppresses WER's blocking modal
            // dialog; capture and next-startup consent are handled by this subsystem.
            unsafe {
                SetErrorMode(SEM_NOGPFAULTERRORBOX);
            }
        }
        let socket_dir = SocketDirectory::new()?;
        let socket = socket_dir.path.join("ipc");
        let mut command = Command::new(executable);
        command
            .arg("--crash-monitor")
            .arg(store.root())
            .arg(&socket)
            .arg(std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: internal helper.
        }
        let mut child = command.spawn()?;
        let started = Instant::now();
        let client = loop {
            let error = match Client::with_name(SocketName::Path(&socket)) {
                Ok(client) => break Arc::new(client),
                Err(error) => error,
            };
            if child.try_wait()?.is_some() || started.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("Crash monitor did not become ready: {error}"),
                ));
            }
            thread::sleep(Duration::from_millis(10));
        };
        // Exactly one receive owner: all PONG/CRASH_ACK reads happen together
        // inside this serialized callback. Panic/heartbeat paths only send.
        let dump_client = Arc::clone(&client);
        let busy = AtomicBool::new(false);
        // SAFETY: the native callback only uses preinitialized IPC and an atomic
        // reentrancy guard. No allocation, filesystem/log calls, symbols or mutexes.
        let event = unsafe {
            crash_handler::make_crash_event(move |context: &crash_handler::CrashContext| {
                if busy
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    let _ = dump_client.send_message(BEGIN_CAPTURE, []);
                    if dump_client.ping().is_ok() {
                        let _ = dump_client.request_dump(context);
                    }
                    busy.store(false, Ordering::Release);
                }
                // Preserve OS fatal exception semantics; do not resume faulting code.
                crash_handler::CrashEventResult::Handled(false)
            })
        };
        let handler = match crash_handler::CrashHandler::attach(event) {
            Ok(handler) => Arc::new(handler),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::other(error.to_string()));
            }
        };
        #[cfg(target_os = "linux")]
        handler.set_ptracer(Some(child.id()));
        let previous: Arc<PanicHook> = Arc::from(std::panic::take_hook());
        let old_hook = Arc::clone(&previous);
        let state = Arc::new(PanicState {
            handler,
            client: Arc::clone(&client),
            busy: AtomicBool::new(false),
        });
        let weak = Arc::downgrade(&state);
        std::panic::set_hook(Box::new(move |info| {
            if let Some(state) = weak.upgrade() {
                if state
                    .busy
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    // Do not store the arbitrary panic payload (may contain credentials).
                    let mut stack = sanitize_stack(&format!("{:?}", backtrace::Backtrace::new()));
                    if stack.len() > 48_000 {
                        let mut end = 48_000;
                        while !stack.is_char_boundary(end) {
                            end -= 1;
                        }
                        stack.truncate(end);
                    }
                    let _ = state.client.send_message(STACK, stack.as_bytes());
                    // The native callback flushes delivery and owns every receive.
                    {
                        #[cfg(windows)]
                        let _ = state.handler.simulate_exception(None);
                        #[cfg(target_os = "linux")]
                        let _ = state.handler.simulate_signal(6);
                        #[cfg(target_os = "macos")]
                        let _ = state.handler.simulate_exception(None);
                    }
                    state.busy.store(false, Ordering::Release);
                }
            }
            old_hook(info); // Preserve unwinding/catch_unwind semantics.
        }));
        let (stop, stopped) = mpsc::channel();
        let heartbeat = thread::spawn(move || {
            while stopped.recv_timeout(Duration::from_secs(1)).is_err() {
                if client.send_message(HEARTBEAT, []).is_err() {
                    return;
                }
            }
            let _ = client.send_message(SHUTDOWN, []);
        });
        log::info!("Crash monitor started");
        Ok(Self {
            child,
            stop,
            heartbeat: Some(heartbeat),
            previous,
            state: Some(state),
            _socket_dir: socket_dir,
        })
    }

    /// Monitor process ID for lifecycle verification/diagnostics.
    pub fn process_id(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        if !thread::panicking() {
            let previous = Arc::clone(&self.previous);
            std::panic::set_hook(Box::new(move |info| previous(info)));
        }
        let _ = self.stop.send(());
        // Do not join a potentially blocked IPC send during app shutdown.
        if let Some(heartbeat) = self.heartbeat.take() {
            if heartbeat.is_finished() {
                let _ = heartbeat.join();
            }
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                _ if Instant::now() >= deadline => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
                _ => thread::sleep(Duration::from_millis(5)),
            }
        }
        drop(self.state.take()); // Detach even when hook restoration is forbidden during unwinding.
        ACTIVE.store(false, Ordering::Release);
    }
}

struct Handler {
    store: CrashStore,
    capture: Mutex<Option<Capture>>,
    stack: Mutex<String>,
    connected: Arc<AtomicBool>,
    dumping: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl ServerHandler for Handler {
    fn create_minidump_file(&self) -> io::Result<(File, PathBuf)> {
        self.dumping.store(true, Ordering::Release);
        let stack = self
            .stack
            .lock()
            .map_err(|_| io::Error::other("Stack state poisoned"))?
            .clone();
        let capture = self.store.begin_capture(&stack)?;
        let writer = capture.writer()?;
        *self
            .capture
            .lock()
            .map_err(|_| io::Error::other("Capture state poisoned"))? = Some(capture);
        Ok(writer)
    }

    fn on_minidump_created(
        &self,
        result: Result<minidumper::MinidumpBinary, minidumper::Error>,
    ) -> LoopAction {
        let capture = self
            .capture
            .lock()
            .ok()
            .and_then(|mut capture| capture.take());
        let result = result
            .map_err(|e| io::Error::other(e.to_string()))
            .and_then(|dump| {
                dump.file.sync_all()?;
                drop(dump);
                capture
                    .ok_or_else(|| io::Error::other("Missing capture transaction"))?
                    .commit()
            });
        if let Err(error) = result {
            log::error!("Minidump capture failed: {error}");
        }
        self.dumping.store(false, Ordering::Release);
        if let Ok(mut stack) = self.stack.lock() {
            stack.clear();
        }
        LoopAction::Continue
    }

    fn on_message(&self, kind: u32, buffer: Vec<u8>) {
        if kind == STACK && buffer.len() <= 131_328 {
            if let Ok(mut stack) = self.stack.lock() {
                *stack = sanitize_stack(&String::from_utf8_lossy(&buffer));
            }
        } else if kind == SHUTDOWN {
            self.stop.store(true, Ordering::Release);
        } else if kind == BEGIN_CAPTURE {
            self.dumping.store(true, Ordering::Release);
        }
    }

    fn on_client_connected(&self, _count: usize) -> LoopAction {
        self.connected.store(true, Ordering::Release);
        LoopAction::Continue
    }

    fn on_client_disconnected(&self, count: usize) -> LoopAction {
        if count == 0 {
            LoopAction::Exit
        } else {
            LoopAction::Continue
        }
    }
}

/// Internal executable mode; runs outside the audio/UI process. Missing initial
/// connection, stale clients, and a hung native writer have bounded lifetimes.
pub fn run_monitor(database: &Path, socket: &Path, parent: Option<u32>) -> io::Result<()> {
    let store = CrashStore::open(database.to_owned())?;
    store.enforce_limits()?;
    let mut server = Server::with_name(SocketName::Path(socket)).map_err(io::Error::other)?;
    let stop = Arc::new(AtomicBool::new(false));
    let connected = Arc::new(AtomicBool::new(false));
    let dumping = Arc::new(AtomicBool::new(false));
    let watcher = (
        Arc::clone(&stop),
        Arc::clone(&connected),
        Arc::clone(&dumping),
    );
    let watchdog = thread::spawn(move || {
        let started = Instant::now();
        let mut capture_started = None;
        while !watcher.0.load(Ordering::Acquire) {
            // Independent of the event loop: even a stuck IPC reader cannot keep
            // a monitor alive after its owning process has disappeared.
            if parent.is_some_and(|pid| !process_alive(pid)) {
                std::process::exit(0);
            }
            if watcher.2.load(Ordering::Acquire) {
                let start = capture_started.get_or_insert_with(Instant::now);
                // Kill before the client's five-second receive deadline, so no
                // writer can commit using an expired crash-context pointer.
                if start.elapsed() > Duration::from_secs(4) {
                    std::process::exit(74);
                }
            } else {
                capture_started = None;
            }
            if !watcher.1.load(Ordering::Acquire) && started.elapsed() > Duration::from_secs(5) {
                watcher.0.store(true, Ordering::Release);
            }
            thread::sleep(Duration::from_millis(20));
        }
    });
    let result = server
        .run(
            Box::new(Handler {
                store,
                capture: Mutex::new(None),
                stack: Mutex::new(String::new()),
                connected,
                dumping,
                stop: Arc::clone(&stop),
            }),
            &stop,
            Some(Duration::from_secs(10)),
        )
        .map_err(io::Error::other);
    stop.store(true, Ordering::Release);
    let _ = watchdog.join();
    result
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use windows::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_ACCESS_RIGHTS},
    };
    // SAFETY: synchronize-only query; never reads/writes the other process memory.
    unsafe {
        match OpenProcess(PROCESS_ACCESS_RIGHTS(0x0010_0000), false, pid) {
            Ok(handle) => {
                let alive = WaitForSingleObject(handle, 0) != WAIT_OBJECT_0;
                let _ = CloseHandle(handle);
                alive
            }
            // Only INVALID_PARAMETER proves the PID is absent. Access/query
            // failures conservatively preserve its directory and monitor.
            Err(error) => error.code() != windows::core::HRESULT::from_win32(87),
        }
    }
}

struct SocketDirectory {
    path: PathBuf,
    #[cfg(not(windows))]
    _temp: tempfile::TempDir,
}

impl SocketDirectory {
    #[cfg(not(windows))]
    fn new() -> io::Result<Self> {
        let temp = tempfile::Builder::new().prefix("drdj-").tempdir()?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            path: temp.path().to_owned(),
            _temp: temp,
        })
    }

    #[cfg(windows)]
    fn new() -> io::Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::{LocalFree, HLOCAL},
                Security::{
                    Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
                    PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
                },
                Storage::FileSystem::CreateDirectoryW,
            },
        };
        // Some Windows AF_UNIX providers reject user-Temp endpoints (WSAEINVAL).
        // CommonData is only an IPC location: a protected owner-only DACL is
        // applied AT directory creation, before a socket/secret can be placed here.
        let base = std::env::var_os("PROGRAMDATA")
            .map(PathBuf::from)
            .filter(|base| base.is_absolute())
            .ok_or_else(|| io::Error::other("ProgramData unavailable for private IPC"))?;
        cleanup_orphaned_socket_dirs(&base)?;
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).map_err(io::Error::other)?;
        let nonce: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let path = base.join(format!("drdj-crash-{nonce}"));
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let descriptor_text: Vec<u16> = "D:P(A;OICI;GA;;;OW)"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: valid terminated UTF-16 strings; descriptor lives through the
        // synchronous CreateDirectory call and is released exactly once afterward.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(descriptor_text.as_ptr()),
                1,
                &mut descriptor,
                None,
            )
            .map_err(io::Error::other)?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: false.into(),
            };
            let result = CreateDirectoryW(PCWSTR(wide.as_ptr()), Some(&attributes));
            let _ = LocalFree(HLOCAL(descriptor.0));
            result.map_err(io::Error::other)?;
        }
        let directory = Self { path };
        std::fs::write(
            directory.path.join("owner.pid"),
            std::process::id().to_string(),
        )?;
        Ok(directory)
    }
}

#[cfg(windows)]
impl Drop for SocketDirectory {
    fn drop(&mut self) {
        if !self.path.is_absolute() {
            return;
        }
        // Only the fixed endpoint and this newly created empty directory; never
        // recursive deletion or any path supplied by persisted report metadata.
        let _ = std::fs::remove_file(self.path.join("ipc"));
        let _ = std::fs::remove_file(self.path.join("owner.pid"));
        let _ = std::fs::remove_dir(&self.path);
    }
}

#[cfg(windows)]
fn cleanup_orphaned_socket_dirs(base: &Path) -> io::Result<()> {
    use std::{io::Read, os::windows::fs::MetadataExt};
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(nonce) = name
            .to_str()
            .and_then(|name| name.strip_prefix("drdj-crash-"))
        else {
            continue;
        };
        if nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            continue;
        }
        let path = entry.path();
        let marker = path.join("owner.pid");
        let Ok(metadata) = std::fs::symlink_metadata(&marker) else {
            continue;
        };
        if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 || metadata.len() > 10 {
            continue;
        }
        let Ok(file) = File::open(&marker) else {
            continue;
        }; // Other users' private roots stay private.
        let mut text = String::new();
        if file.take(11).read_to_string(&mut text).is_err() {
            continue;
        }
        let Ok(pid) = text.parse::<u32>() else {
            continue;
        };
        if process_alive(pid) {
            continue;
        } // Conservative if a PID has been reused.
          // Fixed names in our reserved namespace only; unknown files are preserved.
        let _ = std::fs::remove_file(path.join("ipc"));
        let _ = std::fs::remove_file(marker);
        let _ = std::fs::remove_dir(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_ipc_peer_has_a_receive_deadline() {
        let temp = SocketDirectory::new().unwrap();
        let socket = temp.path.join("ipc");
        let _server = Server::with_name(SocketName::Path(&socket)).unwrap();
        let started = Instant::now();
        // macOS constructor handshake itself may already hit the deadline.
        if let Ok(client) = Client::with_name(SocketName::Path(&socket)) {
            assert!(client.ping().is_err());
        }
        assert!(started.elapsed() >= Duration::from_secs(4));
        assert!(started.elapsed() < Duration::from_secs(12));
    }

    #[cfg(windows)]
    #[test]
    fn windows_ipc_directory_has_only_protected_owner_rights() {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            core::{PCWSTR, PWSTR},
            Win32::{
                Foundation::{LocalFree, ERROR_SUCCESS, HLOCAL},
                Security::{
                    Authorization::{
                        ConvertSecurityDescriptorToStringSecurityDescriptorW,
                        GetNamedSecurityInfoW, SE_FILE_OBJECT,
                    },
                    DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
                },
            },
        };
        let temp = SocketDirectory::new().unwrap();
        let path: Vec<_> = temp.path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let mut text = PWSTR::null();
        // SAFETY: Windows allocates the descriptor/SDDL; buffers are valid for
        // conversion and both allocations are released before making assertions.
        let sddl = unsafe {
            assert_eq!(
                GetNamedSecurityInfoW(
                    PCWSTR(path.as_ptr()),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    None,
                    None,
                    &mut descriptor
                ),
                ERROR_SUCCESS
            );
            let result = ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                None,
            );
            let _ = LocalFree(HLOCAL(descriptor.0));
            result.unwrap();
            let sddl = text.to_string().unwrap();
            let _ = LocalFree(HLOCAL(text.0.cast()));
            sddl
        };
        assert!(sddl.starts_with("D:P"), "DACL must be protected: {sddl}");
        assert_eq!(
            sddl.matches('(').count(),
            1,
            "Unexpected access entries: {sddl}"
        );
        assert!(
            sddl.contains(";;;OW)"),
            "Only owner rights are permitted: {sddl}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn stale_ipc_directory_is_recovered_without_touching_live_owner() {
        let mut stale = SocketDirectory::new().unwrap();
        let path = stale.path.clone();
        std::fs::write(path.join("owner.pid"), u32::MAX.to_string()).unwrap();
        // Simulate the missing destructor of an abruptly terminated process.
        stale.path = PathBuf::new();
        drop(stale);
        let live = SocketDirectory::new().unwrap();
        assert!(!path.exists());
        let another = SocketDirectory::new().unwrap();
        assert!(live.path.exists());
        drop(another);
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: signal zero is an existence check, no signal is delivered.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
