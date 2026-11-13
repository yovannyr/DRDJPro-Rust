use crate::{Import, Playlist, Track};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};
/// URI serialization preserves percent-encoded/non-UTF8 local filenames.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    pub path_url: String,
    pub available: bool,
    pub size: Option<u64>,
    pub modified_ms: Option<u64>,
}
impl FileInfo {
    pub fn path(&self) -> Result<PathBuf, String> {
        let path = url::Url::parse(&self.path_url)
            .map_err(|e| e.to_string())?
            .to_file_path()
            .map_err(|_| "Invalid platform file path".to_owned())?;
        // Ordinary app imports use canonical Windows paths. Preserve that form
        // even for a currently missing file, without another filesystem call.
        #[cfg(windows)]
        {
            use std::path::Prefix;
            let prefix = path.components().next();
            Ok(match prefix {
                Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_)) => {
                    let mut value = std::ffi::OsString::from(r"\\?\");
                    value.push(path.as_os_str());
                    PathBuf::from(value)
                }
                Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::UNC(_, _)) => {
                    PathBuf::from(format!(
                        r"\\?\UNC\{}",
                        path.to_string_lossy().trim_start_matches('\\')
                    ))
                }
                _ => path,
            })
        }
        #[cfg(not(windows))]
        Ok(path)
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PreparedTrack {
    pub track: Track,
    pub file: FileInfo,
}
#[derive(Clone, Debug, Default)]
pub struct Prepared {
    pub tracks: Vec<PreparedTrack>,
    pub playlists: Vec<Playlist>,
    pub duplicates: usize,
    pub skipped: usize,
    pub missing: usize,
    pub unresolved_links: usize,
    pub warnings: Vec<String>,
}
/// Conventional candidates; none is opened without an explicit import action.
pub fn default_xml_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Music/iTunes/iTunes Music Library.xml"),
        home.join("Music/iTunes/iTunes Library.xml"),
        home.join("Music/Music/Library.xml"),
    ]
}
fn resolve(
    location: &str,
    music_folder: Option<&str>,
    media_root: Option<&Path>,
) -> Result<PathBuf, String> {
    if location.chars().any(char::is_control) {
        return Err("Control character in track Location".into());
    }
    let url = url::Url::parse(location).map_err(|_| "Invalid track Location URI")?;
    if url.scheme() != "file"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Only file Location URIs can be imported".into());
    }
    if let (Some(root), Some(folder)) = (media_root, music_folder) {
        if !root.is_absolute() {
            return Err("Media root must be absolute".into());
        }
        let base = url::Url::parse(folder).map_err(|_| "Invalid Music Folder URI")?;
        let prefix = format!("{}/", base.path().trim_end_matches('/'));
        if base.scheme() == "file" && base.host_str() == url.host_str() {
            if let Some(relative) = url.path().strip_prefix(&prefix) {
                let decoded = percent_encoding::percent_decode_str(relative)
                    .decode_utf8()
                    .map_err(|_| "Media remap path is not UTF8")?;
                let path = Path::new(decoded.as_ref());
                if path
                    .components()
                    .any(|p| !matches!(p, Component::Normal(_)))
                {
                    return Err("Invalid relative media path".into());
                }
                return Ok(root.join(path));
            }
        }
        return Err("Track is outside the exported Music Folder; cannot remap safely".into());
    }
    url.to_file_path().map_err(|_| {
        "Location cannot be represented on this platform; configure the media root".into()
    })
}
fn key(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.to_path_buf()
    }
}
enum InspectError {
    Cancelled,
    Invalid(String),
}
fn file(path: PathBuf, cancel: &dyn Fn() -> bool) -> Result<FileInfo, InspectError> {
    if cancel() {
        return Err(InspectError::Cancelled);
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ext == "m4p" {
        return Err(InspectError::Invalid("DRM .m4p track excluded".into()));
    }
    if ![
        "mp3", "wav", "flac", "aif", "aiff", "ogg", "opus", "aac", "m4a", "mp4", "alac",
    ]
    .contains(&ext.as_str())
    {
        return Err(InspectError::Invalid(
            "Unsupported local audio file type".into(),
        ));
    }
    let metadata = std::fs::metadata(&path).ok().filter(|m| m.is_file());
    if cancel() {
        return Err(InspectError::Cancelled);
    }
    let canonical = if metadata.is_some() {
        std::fs::canonicalize(&path).unwrap_or(path)
    } else {
        canonical_parent(path, cancel)?
    };
    let path_url = url::Url::from_file_path(canonical)
        .map_err(|_| InspectError::Invalid("Local Location must be absolute".into()))?
        .to_string();
    Ok(FileInfo {
        path_url,
        available: metadata.is_some(),
        size: metadata.as_ref().map(|m| m.len()),
        modified_ms: metadata
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| d.as_millis().try_into().ok()),
    })
}
fn canonical_parent(path: PathBuf, cancel: &dyn Fn() -> bool) -> Result<PathBuf, InspectError> {
    let mut parent = path.clone();
    let mut suffix = Vec::new();
    for _ in 0..64 {
        if cancel() {
            return Err(InspectError::Cancelled);
        }
        let Some(name) = parent.file_name().map(|s| s.to_os_string()) else {
            break;
        };
        suffix.push(name);
        if !parent.pop() {
            break;
        }
        if let Ok(mut base) = std::fs::canonicalize(&parent) {
            for part in suffix.into_iter().rev() {
                base.push(part);
            }
            return Ok(base);
        }
    }
    Ok(path)
}
/// File inspection is worker-only. OS/network filesystem calls may block that
/// worker; cancellation is checked between calls, never in an audio callback.
pub fn inspect(
    import: Import,
    media_root: Option<&Path>,
    cancel: &dyn Fn() -> bool,
) -> Result<Prepared, String> {
    if media_root.is_some()
        && import
            .music_folder
            .as_deref()
            .and_then(|s| url::Url::parse(s).ok())
            .is_none_or(|u| u.scheme() != "file")
    {
        return Err("Media remapping requires an exported file Music Folder URI".into());
    }
    let mut result = Prepared {
        skipped: import.skipped,
        warnings: import.warnings,
        ..Default::default()
    };
    let mut paths = HashMap::new();
    let mut aliases = HashMap::new();
    for track in import.tracks {
        if cancel() {
            return Err("iTunes import cancelled".into());
        }
        let info = resolve(&track.location, import.music_folder.as_deref(), media_root)
            .map_err(InspectError::Invalid)
            .and_then(|path| file(path, cancel));
        if cancel() {
            return Err("iTunes import cancelled".into());
        }
        match info {
            Ok(file) => {
                let path = file.path()?;
                let identity = key(&path);
                if let Some(id) = paths.get(&identity) {
                    aliases.insert(track.id, *id);
                    result.duplicates += 1;
                } else {
                    paths.insert(identity, track.id);
                    aliases.insert(track.id, track.id);
                    if !file.available {
                        result.missing += 1;
                    }
                    result.tracks.push(PreparedTrack { track, file });
                }
            }
            Err(InspectError::Cancelled) => return Err("iTunes import cancelled".into()),
            Err(InspectError::Invalid(e)) => {
                result.skipped += 1;
                if result.warnings.len() < 50 {
                    result.warnings.push(format!("Track {}: {e}", track.id));
                }
            }
        }
    }
    for mut playlist in import.playlists {
        if cancel() {
            return Err("iTunes import cancelled".into());
        }
        let before = playlist.tracks.len();
        let mut tracks = Vec::with_capacity(before);
        for (index, id) in playlist.tracks.into_iter().enumerate() {
            if index.is_multiple_of(1024) && cancel() {
                return Err("iTunes import cancelled".into());
            }
            if let Some(id) = aliases.get(&id) {
                tracks.push(*id);
            }
        }
        result.unresolved_links += before - tracks.len();
        playlist.tracks = tracks;
        result.playlists.push(playlist);
    }
    if cancel() {
        return Err("iTunes import cancelled".into());
    }
    Ok(result)
}
