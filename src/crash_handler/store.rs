use super::{privacy::sanitize_stack, CrashSettings};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

const MAX_JSON: u64 = 1_048_576;

/// Durable consent/delivery state. Declining preserves local evidence.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ReportStatus {
    /// Not yet authorized for transmission.
    Pending,
    /// User explicitly chose local-only storage.
    Declined,
    /// User authorized textual submission, including retry after failure.
    Queued,
    /// Server acknowledged receipt.
    Submitted,
}

/// Path-redacted metadata. Attachment paths are never read from JSON.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CrashReport {
    pub id: String,
    pub created_ms: i64,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub stack: String,
    pub status: ReportStatus,
}

/// Owned report directory. Operations serialize across processes with a file lock;
/// atomic metadata replacement recovers after interrupted writes. The root must
/// belong to the current user. Link checks do not defend against a malicious
/// same-account process racing filesystem operations. Never use on a realtime thread.
#[derive(Clone, Debug)]
pub struct CrashStore {
    root: PathBuf,
    pub(super) max_reports: usize,
    pub(super) max_bytes: u64,
}

impl CrashStore {
    /// Create/open a private directory. Empty paths never select the working directory.
    pub fn open(root: PathBuf) -> io::Result<Self> {
        if root.as_os_str().is_empty() {
            return Err(invalid("Empty crash directory"));
        }
        reject_link(&root)?;
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self {
            root: root.canonicalize()?,
            max_reports: 50,
            max_bytes: 500_000_000,
        })
    }

    /// Canonical directory for evidence and the monitor's IPC endpoint.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn lock(&self) -> io::Result<File> {
        reject_link(&self.root)?;
        let path = self.root.join(".lock");
        reject_link(&path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        file.lock()?;
        self.clean_interrupted_writes()?;
        Ok(file) // Closing releases the lock, including on process death.
    }

    fn path(&self, id: &str, extension: &str) -> io::Result<PathBuf> {
        validate_id(id)?;
        let path = self.root.join(format!("{id}.{extension}"));
        reject_link(&path)?;
        Ok(path)
    }

    /// Persist textual evidence. No fake minidump is created.
    pub fn insert(&self, stack: &str) -> io::Result<CrashReport> {
        let _lock = self.lock()?;
        let report = CrashReport {
            id: random_id()?,
            created_ms: chrono::Utc::now().timestamp_millis(),
            version: env!("CARGO_PKG_VERSION").into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            stack: sanitize_stack(stack),
            status: ReportStatus::Pending,
        };
        self.write(&self.path(&report.id, "json")?, &report)?;
        self.enforce_unlocked()?;
        if !self.path(&report.id, "json")?.exists() {
            return Err(invalid("Report exceeds store capacity"));
        }
        log::info!("Crash evidence stored: {}", report.id);
        Ok(report)
    }

    /// Read validated records. Malformed entries remain locally available and count
    /// towards retention; settings/queue files never become phantom reports.
    pub fn list(&self) -> io::Result<Vec<CrashReport>> {
        let _lock = self.lock()?;
        let mut reports = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if validate_id(id).is_err() {
                continue;
            }
            reject_link(&path)?;
            match read_json::<CrashReport>(&path) {
                Ok(mut report) if report.id == id => {
                    report.stack = sanitize_stack(&report.stack);
                    reports.push(report);
                }
                _ => log::warn!("Invalid crash metadata: {id}"),
            }
        }
        reports.sort_by(|a, b| (a.created_ms, &a.id).cmp(&(b.created_ms, &b.id)));
        Ok(reports)
    }

    /// Persist consent/delivery status atomically. Only a real acknowledged upload
    /// should be marked Submitted; this method performs no network operation.
    pub fn set_status(&self, id: &str, status: ReportStatus) -> io::Result<()> {
        let _lock = self.lock()?;
        let path = self.path(id, "json")?;
        let mut report: CrashReport = read_json(&path)?;
        if report.id != id {
            return Err(invalid("Mismatched report ID"));
        }
        report.status = status;
        self.write(&path, &report)
    }

    /// Delete only the ID's owned files. Failed dump removal preserves metadata.
    pub fn delete(&self, id: &str) -> io::Result<()> {
        let _lock = self.lock()?;
        self.delete_unlocked(id)
    }

    fn delete_unlocked(&self, id: &str) -> io::Result<()> {
        let dump = self.path(id, "dmp")?;
        let metadata = self.path(id, "json")?;
        remove_if_present(&dump)?;
        remove_if_present(&metadata)?;
        log::info!("Crash evidence deleted: {id}");
        Ok(())
    }

    /// Enforce actual managed count/bytes, including corrupt records/orphan dumps.
    /// Unknown files are never deleted.
    pub fn enforce_limits(&self) -> io::Result<()> {
        let _lock = self.lock()?;
        self.enforce_unlocked()
    }

    fn enforce_unlocked(&self) -> io::Result<()> {
        let mut inventory: BTreeMap<String, (u64, SystemTime)> = BTreeMap::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("json" | "dmp")
            ) {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if validate_id(id).is_err() {
                continue;
            }
            reject_link(&path)?;
            let metadata = fs::metadata(&path)?;
            if !metadata.is_file() {
                return Err(invalid("Report attachment is not a file"));
            }
            let group = inventory
                .entry(id.to_owned())
                .or_insert((0, SystemTime::UNIX_EPOCH));
            group.0 += metadata.len();
            group.1 = group.1.max(metadata.modified()?);
        }
        let mut bytes: u64 = inventory.values().map(|(bytes, _)| bytes).sum();
        let mut count = inventory.len();
        let mut oldest: Vec<_> = inventory.into_iter().collect();
        oldest.sort_by(|(a, (_, at)), (b, (_, bt))| (at, a).cmp(&(bt, b)));
        for (id, (size, _)) in oldest {
            if count <= self.max_reports && bytes <= self.max_bytes {
                break;
            }
            self.delete_unlocked(&id)?;
            bytes -= size;
            count -= 1;
        }
        Ok(())
    }

    /// Corrupt sharing settings fail closed; absence means no automatic upload.
    pub fn settings(&self) -> io::Result<CrashSettings> {
        let _lock = self.lock()?;
        let path = self.root.join("settings.json");
        reject_link(&path)?;
        match read_json(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(CrashSettings::default()),
            result => result,
        }
    }

    /// Save explicit sharing preferences separately from report metadata.
    pub fn save_settings(&self, settings: &CrashSettings) -> io::Result<()> {
        let _lock = self.lock()?;
        let path = self.root.join("settings.json");
        reject_link(&path)?;
        self.write(&path, settings)
    }

    fn write(&self, path: &Path, value: &impl Serialize) -> io::Result<()> {
        let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
        if bytes.len() as u64 > MAX_JSON {
            return Err(invalid("Crash metadata exceeds limit"));
        }
        let mut temp = tempfile::Builder::new()
            .prefix("crash-write-")
            .suffix(".tmp")
            .rand_bytes(16)
            .tempfile_in(&self.root)?;
        private_file(temp.as_file())?;
        temp.write_all(&bytes)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
        #[cfg(unix)]
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }

    // The cross-process lock excludes live writers. Only this reserved namespace
    // belongs to us; unknown temporary files are preserved. No paths come from JSON.
    fn clean_interrupted_writes(&self) -> io::Result<()> {
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(random) = name
                .strip_prefix("crash-write-")
                .and_then(|s| s.strip_suffix(".tmp"))
            else {
                continue;
            };
            if random.len() != 16 || !random.bytes().all(|b| b.is_ascii_alphanumeric()) {
                continue;
            }
            reject_link(&path)?;
            if !fs::metadata(&path)?.is_file() {
                return Err(invalid("Invalid temporary report"));
            }
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

fn validate_id(id: &str) -> io::Result<()> {
    if id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(invalid("Invalid crash ID"))
    }
}

fn random_id() -> io::Result<String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_JSON {
        return Err(invalid("Crash metadata exceeds limit"));
    }
    serde_json::from_reader(file.take(MAX_JSON + 1)).map_err(io::Error::other)
}

fn reject_link(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(invalid("Reparse point in crash store"));
                }
            }
            if metadata.file_type().is_symlink() {
                return Err(invalid("Symlink in crash store"));
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn private_file(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
