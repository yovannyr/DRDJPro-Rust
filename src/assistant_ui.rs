//! Run the local library assistant outside the UI/audio threads.
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) fn suggest(
    script: &Path,
    db: &Path,
    id: i32,
    transition: bool,
) -> Result<String, String> {
    let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
    command.arg("-B").arg(script).arg("--library").arg(db);
    if let Some(dictionary) = std::env::var_os("DRDJ_EMBEDDING_DICTIONARY") {
        command.arg("--dictionary").arg(dictionary);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .arg(if transition { "transition" } else { "similar" })
        .arg(id.to_string())
        .args(["--limit", "1", "--summary"])
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Library assistant unavailable: {e}"))?;
    let until = Instant::now() + Duration::from_secs(90);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => {
                return Err(
                    "Search failed. Check the saved library and descriptor dictionary.".into(),
                )
            }
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Library search interrupted".into());
            }
        }
    }
    let mut summary = String::new();
    child
        .stdout
        .take()
        .expect("piped stdout")
        .take(8193)
        .read_to_string(&mut summary)
        .map_err(|e| e.to_string())?;
    if summary.len() > 8192 || summary.trim().is_empty() {
        return Err("Invalid library assistant response".into());
    }
    Ok(summary.trim().to_owned())
}
