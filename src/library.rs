//! Session library state. File inspection runs on the control worker, not the UI/audio thread.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub const MAX_IMPORT: usize = 5000;
const MAX_TRACKS: usize = 100_000;
pub struct ImportBatch {
    pub paths: Vec<PathBuf>,
    pub rejected: usize,
}
pub fn inspect_paths(paths: Vec<PathBuf>) -> ImportBatch {
    let mut result = ImportBatch {
        paths: Vec::new(),
        rejected: paths.len().saturating_sub(MAX_IMPORT),
    };
    for path in paths.into_iter().take(MAX_IMPORT) {
        let supported = path
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("mp3") || v.eq_ignore_ascii_case("wav"));
        match std::fs::canonicalize(&path) {
            Ok(path) if supported && path.is_file() => result.paths.push(path),
            _ => result.rejected += 1,
        }
    }
    result
}

pub struct Track {
    pub id: i32,
    pub path: PathBuf,
    pub name: String,
    pub format: String,
}
#[derive(Default)]
pub struct Library {
    tracks: Vec<Track>,
    keys: HashSet<PathBuf>,
    query: String,
    selected: Option<i32>,
    sort_field: i32,
    descending: bool,
}
fn identity(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.to_owned()
    }
}
impl Library {
    /// Paths must already be inspected/canonicalized by the worker.
    pub fn insert(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let before = self.tracks.len();
        for path in paths {
            if self.tracks.len() == MAX_TRACKS {
                break;
            }
            if !self.keys.insert(identity(&path)) {
                continue;
            }
            self.tracks.push(Track {
                id: self.tracks.len() as i32 + 1,
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                format: path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_uppercase(),
                path,
            });
        }
        self.tracks.len() - before
    }
    pub fn len(&self) -> usize {
        self.tracks.len()
    }
    pub fn set_query(&mut self, query: String) {
        self.query = query.trim().to_lowercase();
        if self
            .selected
            .is_some_and(|id| !self.visible().iter().any(|track| track.id == id))
        {
            self.selected = None;
        }
    }
    pub fn sort(&mut self, field: i32) {
        let field = field.clamp(0, 2);
        if field == self.sort_field {
            self.descending = !self.descending;
        } else {
            self.sort_field = field;
            self.descending = false;
        }
    }
    pub fn visible(&self) -> Vec<&Track> {
        let mut tracks: Vec<_> = self
            .tracks
            .iter()
            .filter(|track| {
                self.query.is_empty()
                    || track.name.to_lowercase().contains(&self.query)
                    || track
                        .path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&self.query)
            })
            .collect();
        tracks.sort_by_cached_key(|track| {
            (
                match self.sort_field {
                    1 => track.format.clone(),
                    2 => track.path.to_string_lossy().to_lowercase(),
                    _ => track.name.to_lowercase(),
                },
                track.id,
            )
        });
        if self.descending {
            tracks.reverse();
        }
        tracks
    }
    pub fn select(&mut self, id: i32) {
        self.selected = self
            .visible()
            .iter()
            .find(|track| track.id == id)
            .map(|track| track.id);
    }
    pub fn selected_id(&self) -> i32 {
        self.selected.unwrap_or(-1)
    }
    pub fn move_selection(&mut self, delta: i32) {
        let id = {
            let tracks = self.visible();
            if tracks.is_empty() {
                None
            } else {
                let current = tracks
                    .iter()
                    .position(|track| Some(track.id) == self.selected);
                let index = current.map_or(0, |i| {
                    (i as i32 + delta).clamp(0, tracks.len() as i32 - 1) as usize
                });
                Some(tracks[index].id)
            }
        };
        self.selected = id;
    }
    pub fn selected_path(&self) -> Option<PathBuf> {
        self.tracks
            .iter()
            .find(|track| Some(track.id) == self.selected)
            .map(|track| track.path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_identity_survives_sort_and_filter_never_loads_a_hidden_selection() {
        let mut library = Library::default();
        assert_eq!(
            library.insert([
                PathBuf::from("/music/Beta.wav"),
                PathBuf::from("/music/Alpha.mp3")
            ]),
            2
        );
        assert_eq!(library.insert([PathBuf::from("/music/Beta.wav")]), 0);
        library.select(1);
        library.sort(0);
        assert_eq!(
            library.selected_path(),
            Some(PathBuf::from("/music/Beta.wav"))
        );
        library.set_query("ALPHA".into());
        assert_eq!(library.visible().len(), 1);
        assert_eq!(library.selected_id(), -1);
        library.select(1);
        assert!(library.selected_path().is_none());
        library.select(2);
        assert!(library.selected_path().unwrap().ends_with("Alpha.mp3"));
        library.set_query("not found".into());
        assert!(library.visible().is_empty());
    }
    #[test]
    fn import_rejects_missing_unsupported_and_directory_entries() {
        let wav = crate::test_support::wav_file(1, 100);
        let other = crate::test_support::TempFile::new("txt", b"not music");
        let batch = inspect_paths(vec![
            wav.path().into(),
            other.path().into(),
            PathBuf::from("missing.wav"),
            std::env::temp_dir(),
        ]);
        assert_eq!(batch.paths.len(), 1);
        assert_eq!(batch.rejected, 3);
        let mut library = Library::default();
        library.insert(batch.paths);
        assert_eq!(library.len(), 1);
    }
}
