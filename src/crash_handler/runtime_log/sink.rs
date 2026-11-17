//! All filesystem work occurs here under one cross-process log lock. Paths are
//! fixed/private, never taken from event payloads. Unknown files are preserved.
use super::{Entry, MAX_LINE};
use crate::crash_handler::store::{private_file, reject_link};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const NAMES: [&str; 3] = ["runtime.jsonl", "runtime.1.jsonl", "runtime.2.jsonl"];
const PREFIX: &str = "log-write-";

pub(super) struct Sink {
    pub recovered: u64,
    dir: PathBuf,
    max: usize,
    _lock: File,
}
impl Sink {
    pub fn open(root: &Path, max: usize) -> io::Result<Self> {
        reject_link(root)?;
        if !fs::metadata(root)?.is_dir() {
            return Err(io::Error::other("Diagnostic root is not a directory"));
        }
        let dir = root.join("runtime-logs");
        reject_link(&dir)?;
        match fs::create_dir(&dir) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
        if !fs::metadata(&dir)?.is_dir() {
            return Err(io::Error::other("Diagnostic directory is not regular"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        let path = dir.join("runtime.lock");
        retry("diagnostic path validation failed", || reject_link(&path))?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        match lock.try_lock() {
            Ok(()) => (),
            Err(TryLockError::WouldBlock) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(e)) => return Err(e),
        }
        let mut sink = Self {
            recovered: 0,
            dir,
            max,
            _lock: lock,
        };
        // Lock precedes *all* staging, recovery and rotation, even across PIDs.
        sink.recovered = sink.recover()?;
        Ok(sink)
    }
    fn path(&self, name: &str) -> io::Result<PathBuf> {
        let path = self.dir.join(name);
        reject_link(&path)?;
        Ok(path)
    }
    fn recover(&self) -> io::Result<u64> {
        let mut removed = 0;
        for name in NAMES {
            let path = self.path(name)?;
            match fs::metadata(&path) {
                Ok(meta) if !meta.is_file() => {
                    return Err(io::Error::other("Diagnostic file is not regular"))
                }
                Ok(meta) if meta.len() > self.max as u64 => {
                    retry("oversized log removal failed", || fs::remove_file(&path))?;
                    removed += 1;
                }
                Ok(_) => {
                    let mut bytes = Vec::new();
                    File::open(&path)?
                        .take(self.max as u64 + 1)
                        .read_to_end(&mut bytes)?;
                    if bytes.len() > self.max || !valid(&bytes) {
                        retry("invalid log removal failed", || fs::remove_file(&path))?;
                        removed += 1;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e),
            }
        }
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(random) = name
                .strip_prefix(PREFIX)
                .and_then(|s| s.strip_suffix(".tmp"))
            else {
                continue;
            };
            if random.len() != 16 || !random.bytes().all(|b| b.is_ascii_alphanumeric()) {
                continue;
            }
            let path = self.path(name)?;
            if !fs::metadata(&path)?.is_file() {
                return Err(io::Error::other("Diagnostic staging is not regular"));
            }
            retry("staging recovery failed", || fs::remove_file(&path))?;
            removed += 1;
        }
        Ok(removed)
    }
    pub fn append(&mut self, entries: &[Entry]) -> io::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let path = self.path(NAMES[0])?;
        let mut bytes = match retry("current log read failed", || File::open(&path)) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(self.max as u64 + 1).read_to_end(&mut bytes)?;
                if bytes.len() > self.max || !valid(&bytes) {
                    return Err(io::Error::other("Diagnostic file changed"));
                }
                bytes
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        let mut changed = false;
        for entry in entries {
            let mut line = serde_json::to_vec(entry).map_err(io::Error::other)?;
            line.push(b'\n');
            if line.len() > MAX_LINE || line.len() > self.max {
                return Err(io::Error::other("Diagnostic entry exceeds limit"));
            }
            if bytes.len() + line.len() > self.max {
                if changed {
                    self.commit(&bytes)?;
                }
                self.rotate()?;
                bytes.clear();
            }
            bytes.extend_from_slice(&line);
            changed = true;
        }
        if changed {
            self.commit(&bytes)?;
        }
        Ok(())
    }
    fn commit(&self, bytes: &[u8]) -> io::Result<()> {
        let path = self.path(NAMES[0])?;
        let mut temp = tempfile::Builder::new()
            .prefix(PREFIX)
            .suffix(".tmp")
            .rand_bytes(16)
            .tempfile_in(&self.dir)?;
        private_file(temp.as_file())?;
        temp.write_all(bytes)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        let mut attempt = 0;
        loop {
            match temp.persist(&path) {
                Ok(file) => {
                    drop(file);
                    break;
                }
                Err(error) => {
                    if pause(&error.error, attempt) {
                        temp = error.file;
                        attempt += 1;
                    } else {
                        return Err(io::Error::new(
                            error.error.kind(),
                            "atomic log replacement failed",
                        ));
                    }
                }
            }
        }
        #[cfg(unix)]
        File::open(&self.dir)?.sync_all()?;
        Ok(())
    }
    fn rotate(&self) -> io::Result<()> {
        let oldest = self.path(NAMES[2])?;
        match retry("archive removal failed", || fs::remove_file(&oldest)) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        for index in (0..2).rev() {
            let from = self.path(NAMES[index])?;
            let to = self.path(NAMES[index + 1])?;
            match retry("archive rotation failed", || fs::rename(&from, &to)) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

fn retry<T>(stage: &'static str, mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut attempt = 0;
    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(error) if pause(&error, attempt) => attempt += 1,
            Err(error) => return Err(io::Error::new(error.kind(), stage)),
        }
    }
}

fn pause(error: &io::Error, attempt: u32) -> bool {
    #[cfg(windows)]
    if attempt < 5
        && (error.kind() == io::ErrorKind::PermissionDenied
            || matches!(error.raw_os_error(), Some(32 | 33)))
    {
        // Windows replacement/deletion can briefly encounter open/deleting names.
        // Writer only: at most 155ms of retry sleeps, never a producer/UI wait.
        std::thread::sleep(std::time::Duration::from_millis(5 << attempt));
        return true;
    }
    let _ = (error, attempt);
    false
}
fn valid(bytes: &[u8]) -> bool {
    bytes.is_empty()
        || (bytes.ends_with(b"\n")
            && bytes.split_inclusive(|b| *b == b'\n').all(|line| {
                line.len() <= MAX_LINE
                    && serde_json::from_slice::<Entry>(line).is_ok_and(|entry| entry.schema == 1)
            }))
}
