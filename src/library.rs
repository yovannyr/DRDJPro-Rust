//! Session library state. File inspection runs on the control worker, not the UI/audio thread.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
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
    #[serde(skip)]
    pub hot_cues: crate::hot_cues::HotCues,
    #[serde(skip)]
    pub saved_loops: crate::saved_loops::SavedLoops,
}
pub struct CatalogDelta {
    pub revision: u64,
    pub tracks: Vec<SavedTrack>,
    pub crates: Option<CrateDelta>,
    pub memberships: Vec<(i32, i32, bool)>,
}
pub struct CrateDelta {
    pub last_id: i32,
    pub entries: Vec<(i32, Option<String>)>,
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
#[cfg(test)]
pub fn inspect_paths(paths: Vec<PathBuf>) -> ImportBatch {
    inspect_paths_cancellable(paths, &crate::cancellation::Cancellation::default()).unwrap()
}
pub(crate) fn inspect_paths_cancellable(
    paths: Vec<PathBuf>,
    cancel: &crate::cancellation::Cancellation,
) -> Result<ImportBatch, String> {
    cancel.check()?;
    let mut result = ImportBatch {
        paths: Vec::new(),
        metadata: Vec::new(),
        rejected: paths.len().saturating_sub(MAX_IMPORT),
    };
    for path in paths.into_iter().take(MAX_IMPORT) {
        cancel.check()?;
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
    cancel.check()?;
    Ok(result)
}

pub struct Track {
    pub id: i32,
    pub path: PathBuf,
    pub name: String,
    pub format: String,
    pub available: bool,
    pub metadata: crate::track_metadata::TrackMetadata,
    pub grid: Option<crate::beatgrid::BeatGrid>,
    pub hot_cues: crate::hot_cues::HotCues,
    pub saved_loops: crate::saved_loops::SavedLoops,
}
impl Track {
    fn saved(&self) -> SavedTrack {
        SavedTrack {
            id: self.id,
            path: self.path.clone(),
            metadata: self.metadata.clone(),
            grid: self.grid,
            hot_cues: self.hot_cues.clone(),
            saved_loops: self.saved_loops.clone(),
        }
    }

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
            hot_cues: Default::default(),
            saved_loops: Default::default(),
        }
    }
}
#[derive(Default)]
pub struct Library {
    revision: u64,
    changed_tracks: BTreeMap<usize, u64>,
    changed_crates: BTreeMap<i32, u64>,
    changed_memberships: BTreeMap<(i32, i32), (u64, bool)>,
    tracks: Vec<Track>,
    last_id: i32,
    keys: HashMap<PathBuf, usize>,
    id_indices: HashMap<i32, usize>,
    view_cache: std::cell::RefCell<Option<Vec<usize>>>,
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
    fn mark_track(&mut self, index: usize) {
        *self.view_cache.get_mut() = None;
        self.revision = self
            .revision
            .checked_add(1)
            .expect("Library revision overflow");
        self.changed_tracks.insert(index, self.revision);
    }
    fn mark_crate(&mut self, id: i32) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("Library revision overflow");
        self.changed_crates.insert(id, self.revision);
    }
    fn mark_memberships(
        &mut self,
        crate_id: i32,
        tracks: impl IntoIterator<Item = i32>,
        present: bool,
    ) {
        *self.view_cache.get_mut() = None;
        self.revision = self
            .revision
            .checked_add(1)
            .expect("Library revision overflow");
        for track_id in tracks {
            self.changed_memberships
                .insert((crate_id, track_id), (self.revision, present));
        }
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn has_changes(&self) -> bool {
        !self.changed_tracks.is_empty()
            || !self.changed_crates.is_empty()
            || !self.changed_memberships.is_empty()
    }
    pub fn acknowledge(&mut self, revision: u64) {
        self.changed_tracks.retain(|_, changed| *changed > revision);
        self.changed_memberships
            .retain(|_, (changed, _)| *changed > revision);
        self.changed_crates.retain(|_, changed| *changed > revision);
    }
    pub fn changes(&self) -> CatalogDelta {
        CatalogDelta {
            revision: self.revision,
            tracks: self
                .changed_tracks
                .keys()
                .map(|index| self.tracks[*index].saved())
                .collect(),
            crates: (!self.changed_crates.is_empty()).then(|| CrateDelta {
                last_id: self.crates.last_id,
                entries: self
                    .changed_crates
                    .keys()
                    .map(|&id| {
                        (
                            id,
                            self.crates
                                .entries
                                .iter()
                                .find(|c| c.id == id)
                                .map(|c| c.name.clone()),
                        )
                    })
                    .collect(),
            }),
            memberships: self
                .changed_memberships
                .iter()
                .map(|(&(crate_id, track_id), &(_, present))| (crate_id, track_id, present))
                .collect(),
        }
    }

    pub fn saved_loops(&self, path: &Path) -> crate::saved_loops::SavedLoops {
        self.keys
            .get(&identity(path))
            .map(|i| self.tracks[*i].saved_loops.clone())
            .unwrap_or_default()
    }
    pub fn set_saved_loop(
        &mut self,
        path: &Path,
        slot: usize,
        value: Option<crate::saved_loops::SavedLoop>,
    ) -> Result<(), String> {
        let index = self
            .keys
            .get(&identity(path))
            .ok_or("Track is not in the library")?;
        let index = *index;
        self.tracks[index].saved_loops.set(slot, value)?;
        self.mark_track(index);
        Ok(())
    }

    pub fn hot_cues(&self, path: &Path) -> crate::hot_cues::HotCues {
        self.keys
            .get(&identity(path))
            .map(|i| self.tracks[*i].hot_cues.clone())
            .unwrap_or_default()
    }
    pub fn set_hot_cue(
        &mut self,
        path: &Path,
        slot: usize,
        cue: Option<crate::hot_cues::HotCue>,
    ) -> Result<(), String> {
        let index = self
            .keys
            .get(&identity(path))
            .ok_or("Track is not in the library")?;
        let index = *index;
        self.tracks[index].hot_cues.set(slot, cue)?;
        self.mark_track(index);
        Ok(())
    }

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
        let index = *index;
        self.tracks[index].grid = grid;
        self.mark_track(index);
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
        if self.active_crate != id {
            self.active_crate = id;
            *self.view_cache.get_mut() = None;
        }
        self.set_query(self.query.clone());
        Ok(())
    }
    pub fn create_crate(&mut self, name: &str) -> Result<i32, String> {
        let id = self.crates.create(name)?;
        self.mark_crate(id);
        Ok(id)
    }
    pub fn rename_crate(&mut self, id: i32, name: &str) -> Result<(), String> {
        self.crates.rename(id, name)?;
        self.mark_crate(id);
        Ok(())
    }
    pub fn delete_crate(&mut self, id: i32) -> Result<(), String> {
        self.crates.delete(id)?;
        self.changed_memberships
            .retain(|(crate_id, _), _| *crate_id != id);
        self.mark_crate(id);
        if self.active_crate == id {
            self.select_crate(0)?;
        }
        Ok(())
    }
    pub fn add_selected_to_crate(&mut self, id: i32) -> Result<bool, String> {
        let track = self.selected.ok_or("Select a track first")?;
        let changed = self.crates.add(id, track)?;
        if changed {
            self.mark_memberships(id, [track], true);
        }
        Ok(changed)
    }
    pub fn add_paths_to_crate(&mut self, id: i32, paths: &[PathBuf]) -> Result<usize, String> {
        if id == 0 {
            return Ok(0);
        }
        let tracks: HashSet<_> = paths
            .iter()
            .filter_map(|p| self.keys.get(&identity(p)).map(|i| self.tracks[*i].id))
            .collect();
        let entry = self
            .crates
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or("Crate no longer exists")?;
        let added: Vec<_> = tracks.difference(&entry.tracks).copied().collect();
        let changed = self.crates.add_many(id, &tracks)?;
        if changed > 0 {
            self.mark_memberships(id, added, true);
        }
        Ok(changed)
    }
    pub fn remove_selected_from_crate(&mut self) -> Result<bool, String> {
        let track = self.selected.ok_or("Select a track first")?;
        let removed = self.crates.remove(self.active_crate, track)?;
        if removed {
            self.mark_memberships(self.active_crate, [track], false);
        }
        self.set_query(self.query.clone());
        Ok(removed)
    }
    pub fn set_metadata(&mut self, path: &Path, metadata: crate::track_metadata::TrackMetadata) {
        if let Some(index) = self.keys.get(&identity(path)) {
            let index = *index;
            if self.tracks[index].metadata != metadata {
                self.tracks[index].metadata = metadata;
                self.mark_track(index);
            }
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
            let enriched = available && !saved.metadata.scanned;
            let metadata = if enriched {
                crate::track_metadata::read(&saved.path)
            } else {
                saved.metadata
            };
            let mut track = Track::new(saved.id, saved.path, available);
            let changed = enriched && metadata.scanned;
            track.metadata = metadata;
            track.grid = saved.grid;
            track.hot_cues = saved.hot_cues;
            track.saved_loops = saved.saved_loops;
            library.id_indices.insert(track.id, library.tracks.len());
            library.tracks.push(track);
            if changed {
                library.mark_track(library.tracks.len() - 1);
            }
            library.last_id = library.last_id.max(saved.id);
        }
        Ok(library)
    }
    pub fn catalog(&self) -> Catalog {
        Catalog {
            version: 1,
            crates: self.crates.clone(),
            tracks: self.tracks.iter().map(Track::saved).collect(),
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
            self.id_indices.insert(id, self.tracks.len());
            self.tracks.push(Track::new(id, path, true));
            self.mark_track(self.tracks.len() - 1);
        }
        self.tracks.len() - before
    }
    pub fn len(&self) -> usize {
        self.tracks.len()
    }
    pub fn set_query(&mut self, query: String) {
        let normalized = query.trim().to_lowercase();
        if self.query != normalized {
            self.query = normalized;
            *self.view_cache.get_mut() = None;
        }
        if self
            .selected
            .is_some_and(|id| !self.visible().iter().any(|track| track.id == id))
        {
            self.selected = None;
        }
    }
    pub fn sort(&mut self, field: i32) {
        *self.view_cache.get_mut() = None;
        let field = field.clamp(0, 5);
        if field == self.sort_field {
            self.descending = !self.descending;
        } else {
            self.sort_field = field;
            self.descending = false;
        }
    }
    pub fn track_by_id(&self, id: i32) -> Option<&Track> {
        self.id_indices
            .get(&id)
            .and_then(|index| self.tracks.get(*index))
    }
    pub fn visible(&self) -> Vec<&Track> {
        let missing = self.view_cache.borrow().is_none();
        if missing {
            let id_indices = self
                .build_visible()
                .iter()
                .map(|track| self.id_indices[&track.id])
                .collect();
            *self.view_cache.borrow_mut() = Some(id_indices);
        }
        self.view_cache
            .borrow()
            .as_ref()
            .unwrap()
            .iter()
            .map(|&i| &self.tracks[i])
            .collect()
    }
    fn build_visible(&self) -> Vec<&Track> {
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
    fn cancelled_import_does_not_inspect_or_return_partial_paths() {
        let cancel = crate::cancellation::Cancellation::default();
        cancel.request();
        assert!(
            inspect_paths_cancellable(vec![std::path::PathBuf::from("missing.wav")], &cancel)
                .err()
                .unwrap()
                .contains("cancelled")
        );
    }
    #[test]
    fn delta_snapshot_at_100000_tracks_only_clones_changed_records_and_acks_exact_revision() {
        let profile = tempfile::tempdir().unwrap();
        let mut library = Library::default();
        library.insert((0..MAX_TRACKS).map(|i| profile.path().join(format!("{i}.wav"))));
        library.acknowledge(library.revision());
        assert!(!library.has_changes());
        let path = profile.path().join("0.wav");
        library
            .set_hot_cue(
                &path,
                0,
                Some(
                    crate::hot_cues::HotCue::new(
                        std::time::Duration::from_secs(1),
                        "Cue".into(),
                        0,
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
        let mut patch = None;
        let start = std::time::Instant::now();
        let delta_alloc = crate::looping::counted_allocations(|| patch = Some(library.changes()));
        let delta_time = start.elapsed();
        let mut full = None;
        let start = std::time::Instant::now();
        let full_alloc = crate::looping::counted_allocations(|| full = Some(library.catalog()));
        let full_time = start.elapsed();
        let patch = patch.unwrap();
        assert_eq!(patch.tracks.len(), 1);
        assert!(patch.crates.is_none());
        assert!(full_alloc > 10000 && delta_alloc < 20);
        assert_eq!(full.unwrap().tracks.len(), MAX_TRACKS);
        let grid = crate::beatgrid::BeatGrid::new(120., std::time::Duration::ZERO).unwrap();
        library.set_grid(&path, Some(grid)).unwrap();
        library.acknowledge(patch.revision);
        let latest = library.changes();
        assert_eq!(latest.tracks.len(), 1);
        assert_eq!(latest.tracks[0].grid, Some(grid));
        assert!(latest.tracks[0].hot_cues.slots()[0].is_some());
        let crate_id = library.create_crate("New crate").unwrap();
        library.acknowledge(latest.revision);
        let remaining = library.changes();
        assert!(remaining.tracks.is_empty());
        assert_eq!(remaining.crates.unwrap().entries[0].0, crate_id);
        library.acknowledge(library.revision());
        assert!(!library.has_changes());
        println!("100000-track snapshots: delta {delta_alloc} allocations {delta_time:?}; full {full_alloc} allocations {full_time:?}");
    }
    #[test]
    fn cached_library_view_reuses_order_and_invalidates_on_query_sort_metadata_and_crates() {
        let dir = tempfile::tempdir().unwrap();
        let mut library = Library::default();
        let size = 10_000;
        library.insert((0..size).map(|i| dir.path().join(format!("track-{i:05}.wav"))));
        let cold = crate::looping::counted_allocations(|| {
            std::hint::black_box(library.visible());
        });
        let cached = crate::looping::counted_allocations(|| {
            std::hint::black_box(library.visible());
        });
        assert!(cold > size);
        assert!(cached <= 2);
        println!("10,000-track view: cold {cold} allocations; cached {cached}");
        assert_eq!(library.visible()[0].id, 1);
        library.set_query(" TRACK-00001 ".into());
        assert_eq!(
            library.visible().iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![2]
        );
        library.set_query(String::new());
        library.sort(0);
        assert_eq!(library.visible()[0].id, size as i32);
        library.sort(0);
        let metadata = crate::track_metadata::TrackMetadata {
            title: "AAA first".into(),
            ..Default::default()
        };
        library.set_metadata(&dir.path().join("track-09999.wav"), metadata);
        assert_eq!(library.visible()[0].id, size as i32);
        library.set_query("AAA".into());
        assert_eq!(
            library.visible().iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![size as i32]
        );
        library.set_query(String::new());
        let crate_id = library.create_crate("Set").unwrap();
        let first = dir.path().join("track-00000.wav");
        let second = dir.path().join("track-00001.wav");
        library
            .add_paths_to_crate(crate_id, std::slice::from_ref(&first))
            .unwrap();
        library.select_crate(crate_id).unwrap();
        assert_eq!(
            library.visible().iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![1]
        );
        library.add_paths_to_crate(crate_id, &[second]).unwrap();
        assert_eq!(
            library.visible().iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        library.select(1);
        assert!(library.remove_selected_from_crate().unwrap());
        assert_eq!(
            library.visible().iter().map(|t| t.id).collect::<Vec<_>>(),
            vec![2]
        );
        library.delete_crate(crate_id).unwrap();
        assert_eq!(library.active_crate(), 0);
        assert_eq!(library.visible().len(), size);
        library.insert([dir.path().join("new.wav")]);
        assert_eq!(library.visible().len(), size + 1);
    }

    #[test]
    fn membership_deltas_survive_inflight_ack_and_coalesce_without_catalog_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let paths = [dir.path().join("a.wav"), dir.path().join("b.wav")];
        let mut library = Library::default();
        assert_eq!(library.insert(paths.clone()), 2);
        let id = library.create_crate("Set").unwrap();
        library.acknowledge(library.revision());
        assert_eq!(library.add_paths_to_crate(id, &paths[..1]).unwrap(), 1);
        let inflight = library.changes();
        assert!(inflight.crates.is_none());
        assert_eq!(inflight.memberships, vec![(id, 1, true)]);
        library.select_crate(id).unwrap();
        library.select(1);
        assert!(library.remove_selected_from_crate().unwrap());
        assert_eq!(library.add_paths_to_crate(id, &paths[1..]).unwrap(), 1);
        library.acknowledge(inflight.revision);
        let latest = library.changes();
        assert!(latest.crates.is_none());
        assert_eq!(latest.memberships, vec![(id, 1, false), (id, 2, true)]);
        library.acknowledge(latest.revision);
        assert!(!library.has_changes());
        assert_eq!(library.add_paths_to_crate(id, &paths[1..]).unwrap(), 0);
        assert!(!library.has_changes());
        library.delete_crate(id).unwrap();
        let deleted = library.changes();
        assert!(deleted.crates.is_some());
        assert!(deleted.memberships.is_empty());
    }

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
