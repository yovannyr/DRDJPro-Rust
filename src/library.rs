//! Session library state. File inspection runs on the control worker, not the UI/audio thread.
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

pub const MAX_IMPORT: usize = 5000;
pub(crate) const MAX_TRACKS: usize = 100_000;
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u32,
    pub tracks: Vec<SavedTrack>,
    #[serde(skip)]
    pub crates: crate::crates::CrateCatalog,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedTrack {
    pub id: i32,
    pub path: PathBuf,
    #[serde(skip)]
    pub metadata: crate::track_metadata::TrackMetadata,
    #[serde(skip)]
    pub grid: Option<crate::beatgrid::BeatGrid>,
}
impl Catalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("Unsupported library version {}", self.version));
        }
        if self.tracks.len() > MAX_TRACKS {
            return Err("Library track limit exceeded".into());
        }
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        for track in &self.tracks {
            if track.id <= 0
                || !track.metadata.validate()
                || track.id == i32::MAX
                || !ids.insert(track.id)
                || !track.path.is_absolute()
                || !paths.insert(identity(&track.path))
                || !track
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("mp3") || e.eq_ignore_ascii_case("wav"))
            {
                return Err("Invalid or duplicate library identity/path".into());
            }
        }
        self.crates.validate(&ids)
    }
}
pub struct ImportBatch {
    pub paths: Vec<PathBuf>,
    pub metadata: Vec<(PathBuf, crate::track_metadata::TrackMetadata)>,
    pub rejected: usize,
}
pub fn inspect_paths(paths: Vec<PathBuf>) -> ImportBatch {
    let mut result = ImportBatch {
        paths: Vec::new(),
        metadata: Vec::new(),
        rejected: paths.len().saturating_sub(MAX_IMPORT),
    };
    for path in paths.into_iter().take(MAX_IMPORT) {
        let supported = path
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("mp3") || v.eq_ignore_ascii_case("wav"));
        match std::fs::canonicalize(&path) {
            Ok(path) if supported && path.is_file() => {
                result
                    .metadata
                    .push((path.clone(), crate::track_metadata::read(&path)));
                result.paths.push(path);
            }
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
    pub available: bool,
    pub metadata: crate::track_metadata::TrackMetadata,
    pub grid: Option<crate::beatgrid::BeatGrid>,
}
impl Track {
    pub fn display_title(&self) -> &str {
        if self.metadata.title.is_empty() {
            &self.name
        } else {
            &self.metadata.title
        }
    }
    fn new(id: i32, path: PathBuf, available: bool) -> Self {
        Self {
            id,
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
            available,
            metadata: Default::default(),
            grid: None,
        }
    }
}
#[derive(Default)]
pub struct Library {
    tracks: Vec<Track>,
    last_id: i32,
    keys: HashMap<PathBuf, usize>,
    query: String,
    selected: Option<i32>,
    sort_field: i32,
    descending: bool,
    crates: crate::crates::CrateCatalog,
    active_crate: i32,
}
pub(crate) fn identity(path: &Path) -> PathBuf {
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
    pub fn grid(&self, path: &Path) -> Option<crate::beatgrid::BeatGrid> {
        self.keys
            .get(&identity(path))
            .and_then(|index| self.tracks[*index].grid)
    }
    pub fn set_grid(
        &mut self,
        path: &Path,
        grid: Option<crate::beatgrid::BeatGrid>,
    ) -> Result<(), String> {
        let index = self
            .keys
            .get(&identity(path))
            .ok_or("Track is not in the library")?;
        self.tracks[*index].grid = grid;
        Ok(())
    }
    pub fn crates(&self) -> &crate::crates::CrateCatalog {
        &self.crates
    }
    pub fn active_crate(&self) -> i32 {
        self.active_crate
    }
    pub fn select_crate(&mut self, id: i32) -> Result<(), String> {
        if id != 0 && !self.crates.entries.iter().any(|c| c.id == id) {
            return Err("Crate no longer exists".into());
        }
        self.active_crate = id;
        self.set_query(self.query.clone());
        Ok(())
    }
    pub fn create_crate(&mut self, name: &str) -> Result<i32, String> {
        self.crates.create(name)
    }
    pub fn rename_crate(&mut self, id: i32, name: &str) -> Result<(), String> {
        self.crates.rename(id, name)
    }
    pub fn delete_crate(&mut self, id: i32) -> Result<(), String> {
        self.crates.delete(id)?;
        if self.active_crate == id {
            self.select_crate(0)?;
        }
        Ok(())
    }
    pub fn add_selected_to_crate(&mut self, id: i32) -> Result<bool, String> {
        let track = self.selected.ok_or("Select a track first")?;
        self.crates.add(id, track)
    }
    pub fn add_paths_to_crate(&mut self, id: i32, paths: &[PathBuf]) -> Result<usize, String> {
        if id == 0 {
            return Ok(0);
        }
        let tracks: HashSet<_> = paths
            .iter()
            .filter_map(|p| self.keys.get(&identity(p)).map(|i| self.tracks[*i].id))
            .collect();
        self.crates.add_many(id, &tracks)
    }
    pub fn remove_selected_from_crate(&mut self) -> Result<bool, String> {
        let track = self.selected.ok_or("Select a track first")?;
        let removed = self.crates.remove(self.active_crate, track)?;
        self.set_query(self.query.clone());
        Ok(removed)
    }
    pub fn set_metadata(&mut self, path: &Path, metadata: crate::track_metadata::TrackMetadata) {
        if let Some(index) = self.keys.get(&identity(path)) {
            self.tracks[*index].metadata = metadata;
        }
    }
    /// Called by the storage worker: availability checks never run in the UI tick.
    pub fn restore(catalog: Catalog) -> Result<Self, String> {
        catalog.validate()?;
        let mut library = Self {
            crates: catalog.crates,
            ..Default::default()
        };
        for saved in catalog.tracks {
            let available = saved.path.is_file();
            library
                .keys
                .insert(identity(&saved.path), library.tracks.len());
            let metadata = if available && !saved.metadata.scanned {
                crate::track_metadata::read(&saved.path)
            } else {
                saved.metadata
            };
            let mut track = Track::new(saved.id, saved.path, available);
            track.metadata = metadata;
            track.grid = saved.grid;
            library.tracks.push(track);
            library.last_id = library.last_id.max(saved.id);
        }
        Ok(library)
    }
    pub fn catalog(&self) -> Catalog {
        Catalog {
            version: 1,
            crates: self.crates.clone(),
            tracks: self
                .tracks
                .iter()
                .map(|t| SavedTrack {
                    id: t.id,
                    path: t.path.clone(),
                    metadata: t.metadata.clone(),
                    grid: t.grid,
                })
                .collect(),
        }
    }
    pub fn missing_count(&self) -> usize {
        let members = self.scope_members();
        self.tracks
            .iter()
            .filter(|t| {
                !t.available
                    && (self.active_crate == 0 || members.is_some_and(|m| m.contains(&t.id)))
            })
            .count()
    }
    fn scope_members(&self) -> Option<&HashSet<i32>> {
        self.crates
            .entries
            .iter()
            .find(|c| c.id == self.active_crate)
            .map(|c| &c.tracks)
    }
    pub fn scope_len(&self) -> usize {
        if self.active_crate == 0 {
            self.len()
        } else {
            self.scope_members().map_or(0, HashSet::len)
        }
    }
    /// Paths must already be inspected/canonicalized by the worker.
    pub fn insert(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let before = self.tracks.len();
        for path in paths {
            if let Some(index) = self.keys.get(&identity(&path)) {
                self.tracks[*index].available = true;
                continue;
            }
            if self.tracks.len() == MAX_TRACKS {
                continue;
            }
            let Some(id) = self.last_id.checked_add(1).filter(|id| *id < i32::MAX) else {
                break;
            };
            self.last_id = id;
            self.keys.insert(identity(&path), self.tracks.len());
            self.tracks.push(Track::new(id, path, true));
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
        let field = field.clamp(0, 5);
        if field == self.sort_field {
            self.descending = !self.descending;
        } else {
            self.sort_field = field;
            self.descending = false;
        }
    }
    pub fn visible(&self) -> Vec<&Track> {
        let members = self.scope_members();
        let mut tracks: Vec<_> = self
            .tracks
            .iter()
            .filter(|track| {
                (self.active_crate == 0 || members.is_some_and(|c| c.contains(&track.id)))
                    && (self.query.is_empty()
                        || track.metadata.title.to_lowercase().contains(&self.query)
                        || track.metadata.artist.to_lowercase().contains(&self.query)
                        || track.metadata.album.to_lowercase().contains(&self.query)
                        || track.name.to_lowercase().contains(&self.query)
                        || track
                            .path
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&self.query))
            })
            .collect();
        tracks.sort_by_cached_key(|track| {
            (
                match self.sort_field {
                    1 => track.format.clone(),
                    2 => track.path.to_string_lossy().to_lowercase(),
                    3 => track.metadata.artist.to_lowercase(),
                    4 => track.metadata.album.to_lowercase(),
                    5 => String::new(),
                    _ => track.display_title().to_lowercase(),
                },
                if self.sort_field == 5 {
                    track.metadata.duration_ms
                } else {
                    None
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
    #[test]
    fn crate_filter_composes_with_search_and_removal_retains_global_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let mut library = Library::default();
        library.insert([dir.path().join("Alpha.wav"), dir.path().join("Beta.mp3")]);
        let crate_id = library.create_crate("Set").unwrap();
        library.select(1);
        library.add_selected_to_crate(crate_id).unwrap();
        library.select(2);
        library.select_crate(crate_id).unwrap();
        assert_eq!(library.selected_id(), -1);
        assert_eq!(library.visible().len(), 1);
        library.set_query("beta".into());
        assert!(library.visible().is_empty());
        library.set_query(String::new());
        library.select(1);
        library.remove_selected_from_crate().unwrap();
        assert_eq!(library.selected_id(), -1);
        assert!(library.visible().is_empty());
        assert_eq!(library.len(), 2);
        library.delete_crate(crate_id).unwrap();
        assert_eq!(library.active_crate(), 0);
        assert_eq!(library.visible().len(), 2);
        assert!(library.catalog().validate().is_ok());
    }
}
