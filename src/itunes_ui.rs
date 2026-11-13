//! Ordered source browsing and bounded shared-library application. Filesystem,
//! XML, SQLite and native pickers run on workers, not the audio/UI callback.
use crate::{controller::AudioController, AppWindow, ItunesRow, ItunesWindow};
use drdj_itunes::{PreparedTrack, Snapshot, Source, Store, SyncReport};
use slint::{ComponentHandle, Model, ModelNotify, ModelRc, Timer, TimerMode, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::BufReader,
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};
trait Bridge {
    fn ready(&self) -> bool;
    fn apply(&self, row: &PreparedTrack, changed: bool) -> Result<bool, String>;
    fn publish(&self);
    fn load(&self, deck: usize, path: PathBuf) -> Result<(), String>;
}
impl Bridge for AudioController {
    fn ready(&self) -> bool {
        self.itunes_ready()
    }
    fn apply(&self, row: &PreparedTrack, changed: bool) -> Result<bool, String> {
        self.apply_itunes_record(row, changed)
    }
    fn publish(&self) {
        self.publish_itunes_changes();
    }
    fn load(&self, deck: usize, path: PathBuf) -> Result<(), String> {
        self.load_itunes_track(deck, path)
    }
}
enum Work {
    Sources,
    Read(i64),
    Import(PathBuf, Option<PathBuf>),
    ChooseXml,
    ChooseRoot,
}
struct Data {
    snapshot: Snapshot,
    views: Vec<Arc<Vec<usize>>>,
}
impl Data {
    fn new(snapshot: Snapshot) -> Self {
        let ids: HashMap<_, _> = snapshot
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.track.id, i))
            .collect();
        let mut views = vec![Arc::new((0..snapshot.tracks.len()).collect())];
        for p in &snapshot.playlists {
            views.push(Arc::new(
                p.tracks
                    .iter()
                    .filter_map(|id| ids.get(id).copied())
                    .collect(),
            ));
        }
        Self { snapshot, views }
    }
}
enum Output {
    Sources(Vec<Source>),
    Loaded {
        sources: Vec<Source>,
        data: Arc<Data>,
        report: Option<SyncReport>,
        note: String,
    },
    Chosen {
        path: Option<PathBuf>,
        root: bool,
    },
}
fn run(work: Work, db: PathBuf, cancel: Arc<AtomicBool>) -> Result<Output, String> {
    let cancelled = || cancel.load(Ordering::Acquire);
    match work {
        Work::ChooseXml => {
            let mut dialog = rfd::FileDialog::new().add_filter("iTunes/Music XML", &["xml"]);
            if let Some(home) = dirs::home_dir() {
                for candidate in drdj_itunes::default_xml_paths(&home) {
                    if cancelled() {
                        return Err("iTunes operation cancelled".into());
                    }
                    if candidate.is_file() {
                        if let Some(parent) = candidate.parent() {
                            dialog = dialog.set_directory(parent);
                        }
                        if let Some(name) = candidate.file_name() {
                            dialog = dialog.set_file_name(name.to_string_lossy());
                        }
                        break;
                    }
                }
            }
            return Ok(Output::Chosen {
                path: dialog.pick_file(),
                root: false,
            });
        }
        Work::ChooseRoot => {
            return Ok(Output::Chosen {
                path: rfd::FileDialog::new().pick_folder(),
                root: true,
            })
        }
        _ => {}
    }
    if cancelled() {
        return Err("iTunes operation cancelled".into());
    }
    std::fs::create_dir_all(db.parent().ok_or("Source catalog has no parent")?)
        .map_err(|e| e.to_string())?;
    let mut store = Store::open(&db)?;
    match work {
        Work::Sources => Ok(Output::Sources(store.sources()?)),
        Work::Read(id) => {
            let snapshot = store.snapshot(id)?;
            let prepared =
                drdj_itunes::refresh_files(snapshot.tracks, snapshot.playlists, &cancelled)?;
            // A cached read must not write an older metadata generation over a
            // concurrent XML sync. Only availability in this owned view refreshes.
            let snapshot = Snapshot {
                source: snapshot.source,
                tracks: prepared.tracks,
                playlists: prepared.playlists,
            };
            if cancelled() {
                return Err("iTunes operation cancelled".into());
            }
            Ok(Output::Loaded {
                sources: store.sources()?,
                data: Arc::new(Data::new(snapshot)),
                report: None,
                note: "Cached source loaded; reconciling the shared library".into(),
            })
        }
        Work::Import(path, root) => {
            let root = root
                .map(|path| -> Result<PathBuf, String> {
                    let path =
                        std::fs::canonicalize(path).map_err(|e| format!("Media folder: {e}"))?;
                    if !path.is_dir() {
                        return Err("Media override must be a directory".into());
                    }
                    Ok(path)
                })
                .transpose()?;
            let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let parsed = drdj_itunes::parse(BufReader::new(file), &cancelled)?;
            let prepared = drdj_itunes::inspect(parsed, root.as_deref(), &cancelled)?;
            let xml = url::Url::from_file_path(&path)
                .map_err(|_| "Invalid XML path")?
                .to_string();
            let root = root
                .map(|r| {
                    url::Url::from_directory_path(r)
                        .map(|u| u.to_string())
                        .map_err(|_| "Invalid media folder")
                })
                .transpose()?;
            let report = store.sync(&xml, root.as_deref(), &prepared, &cancelled)?;
            let note=format!("Source: {} added, {} updated, {} unchanged; {} missing, {} duplicate, {} skipped entries, {} unresolved links{}",report.added,report.updated,report.unchanged,prepared.missing,prepared.duplicates,prepared.skipped,prepared.unresolved_links,prepared.warnings.first().map_or(String::new(),|s|format!(". {s}")));
            let snapshot = store.snapshot(report.source_id)?;
            if cancelled() {
                return Err("Operation cancelled after source commit; reopen the cached source to reconcile".into());
            }
            Ok(Output::Loaded {
                sources: store.sources()?,
                data: Arc::new(Data::new(snapshot)),
                report: Some(report),
                note,
            })
        }
        _ => unreachable!("picker work returned earlier"),
    }
}
type Executor = Rc<dyn Fn(Work, PathBuf, Arc<AtomicBool>) -> Receiver<Result<Output, String>>>;
fn executor() -> Executor {
    Rc::new(|work, db, cancel| {
        let (tx, rx) = mpsc::sync_channel(1);
        let fail = tx.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("itunes-source".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(work, db, cancel)
                }))
                .unwrap_or_else(|_| Err("iTunes worker panicked".into()));
                let _ = tx.send(result);
            })
        {
            let _ = fail.try_send(Err(format!("Cannot start iTunes worker: {e}")));
        }
        rx
    })
}
struct Rows {
    data: Arc<Data>,
    order: Arc<Vec<usize>>,
    tracker: ModelNotify,
}
impl Model for Rows {
    type Data = ItunesRow;
    fn row_count(&self) -> usize {
        self.order.len()
    }
    fn row_data(&self, row: usize) -> Option<ItunesRow> {
        let t = self.data.snapshot.tracks.get(*self.order.get(row)?)?;
        Some(ItunesRow {
            title: t.track.title.as_str().into(),
            artist: t.track.artist.as_str().into(),
            album: t.track.album.as_str().into(),
            state: if t.file.available {
                "Available"
            } else {
                "Missing"
            }
            .into(),
        })
    }
    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &self.tracker
    }
}
struct Applying {
    data: Arc<Data>,
    index: usize,
    changed: HashSet<u64>,
    force: bool,
    modified: usize,
    note: String,
}
struct Session {
    view: ItunesWindow,
    db: PathBuf,
    executor: Executor,
    bridge: Rc<dyn Bridge>,
    job: Option<Receiver<Result<Output, String>>>,
    cancel: Arc<AtomicBool>,
    applying: Option<Applying>,
    sources: Vec<Source>,
    data: Option<Arc<Data>>,
    closed: bool,
    xml_choice: Option<PathBuf>,
    root_choice: Option<PathBuf>,
}
impl Session {
    fn start(&mut self, work: Work) {
        if self.job.is_some() || self.applying.is_some() {
            return;
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        self.job = Some((self.executor)(
            work,
            self.db.clone(),
            Arc::clone(&self.cancel),
        ));
        self.view.set_busy(true);
        self.view
            .set_status("Reading iTunes source on a worker…".into());
    }
    fn selected(&self) -> Option<&PreparedTrack> {
        let data = self.data.as_ref()?;
        let playlist = usize::try_from(self.view.get_playlist_index()).ok()?;
        let row = usize::try_from(self.view.get_selected_row()).ok()?;
        data.snapshot
            .tracks
            .get(*data.views.get(playlist)?.get(row)?)
    }
    fn playlist(&self) {
        let Some(data) = &self.data else {
            return;
        };
        let index = usize::try_from(self.view.get_playlist_index()).unwrap_or(0);
        if let Some(order) = data.views.get(index) {
            self.view.set_rows(ModelRc::new(Rows {
                data: Arc::clone(data),
                order: Arc::clone(order),
                tracker: ModelNotify::default(),
            }));
            self.view.set_selected_row(-1);
            self.view
                .set_details("Select a track to inspect its source metadata".into());
        }
    }
    fn details(&self) {
        if let Some(t) = self.selected() {
            let bpm = t
                .track
                .bpm
                .map_or_else(|| "—".into(), |n| format!("{n:.2}"));
            let rating = t.track.rating.map_or_else(|| "—".into(), |n| n.to_string());
            let plays = t
                .track
                .play_count
                .map_or_else(|| "—".into(), |n| n.to_string());
            let duration = t
                .track
                .duration_ms
                .map_or_else(|| "—".into(), |n| format!("{:.3} s", n as f64 / 1000.));
            let size = t
                .file
                .size
                .map_or_else(|| "—".into(), |n| format!("{n} bytes"));
            let modified = t
                .file
                .modified_ms
                .and_then(|n| i64::try_from(n).ok())
                .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
                .map_or_else(|| "—".into(), |t| t.to_rfc3339());
            let path = url::Url::parse(&t.file.path_url)
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .map_or_else(
                    || "Unavailable path".into(),
                    |p| p.to_string_lossy().into_owned(),
                );
            self.view.set_details(format!("{} — {}\nAlbum: {} | Album artist: {} | Genre: {}\nSource BPM: {bpm} | Rating: {rating}/100 | iTunes plays: {plays}\nDuration: {duration} | Added: {}\n{path}\nSize: {size} | Modified: {modified}",t.track.title,t.track.artist,t.track.album,t.track.album_artist,t.track.genre,t.track.date_added.as_deref().unwrap_or("—")).into());
        }
    }
    fn install(
        &mut self,
        sources: Vec<Source>,
        data: Arc<Data>,
        report: Option<SyncReport>,
        note: String,
    ) {
        let source_id = data.snapshot.source.id;
        self.sources = sources;
        self.view.set_sources(ModelRc::new(VecModel::from(
            self.sources
                .iter()
                .map(|s| s.xml_url.as_str().into())
                .collect::<Vec<_>>(),
        )));
        self.view.set_source_index(
            self.sources
                .iter()
                .position(|s| s.id == source_id)
                .map_or(-1, |i| i as i32),
        );
        self.view.set_xml_path(
            url::Url::parse(&data.snapshot.source.xml_url)
                .ok()
                .and_then(|u| u.to_file_path().ok())
                .map_or(String::new(), |p| p.to_string_lossy().into_owned())
                .into(),
        );
        self.view.set_media_root(
            data.snapshot
                .source
                .media_root_url
                .as_deref()
                .and_then(|s| url::Url::parse(s).ok())
                .and_then(|u| u.to_file_path().ok())
                .map_or(String::new(), |p| p.to_string_lossy().into_owned())
                .into(),
        );
        let names = std::iter::once("All source tracks".into())
            .chain(
                data.snapshot
                    .playlists
                    .iter()
                    .map(|p| p.name.as_str().into()),
            )
            .collect::<Vec<_>>();
        self.view.set_playlists(ModelRc::new(VecModel::from(names)));
        self.view.set_playlist_index(0);
        self.data = Some(Arc::clone(&data));
        self.playlist();
        self.applying = Some(Applying {
            data,
            index: 0,
            force: report.is_none(),
            changed: report.map_or_else(HashSet::new, |r| r.changed_ids),
            modified: 0,
            note,
        });
    }
    fn poll(&mut self) {
        if let Some(job) = &self.job {
            let result = match job.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("iTunes worker ended without a result".into()))
                }
            };
            if let Some(result) = result {
                self.job = None;
                if self.cancel.load(Ordering::Acquire) {
                    self.view.set_busy(false);
                    self.view.set_status("Cancelled. Reopen this window to reload any already committed source data.".into());
                    return;
                }
                match result {
                    Ok(Output::Chosen { path, root }) => {
                        if let Some(path) = path {
                            if root {
                                self.view
                                    .set_media_root(path.to_string_lossy().into_owned().into());
                                self.root_choice = Some(path);
                            } else {
                                self.view
                                    .set_xml_path(path.to_string_lossy().into_owned().into());
                                self.xml_choice = Some(path);
                            }
                        }
                        self.view.set_busy(false);
                        self.view
                            .set_status("Selection ready; choose IMPORT / SYNC".into());
                    }
                    Ok(Output::Sources(sources)) => {
                        self.sources = sources;
                        self.view.set_sources(ModelRc::new(VecModel::from(
                            self.sources
                                .iter()
                                .map(|s| s.xml_url.as_str().into())
                                .collect::<Vec<_>>(),
                        )));
                        self.view.set_busy(false);
                        if let Some(id) = self.sources.last().map(|s| s.id) {
                            self.start(Work::Read(id));
                        } else {
                            self.view.set_status(
                                "Choose an exported XML file; no source has been imported yet"
                                    .into(),
                            );
                        }
                    }
                    Ok(Output::Loaded {
                        sources,
                        data,
                        report,
                        note,
                    }) => self.install(sources, data, report, note),
                    Err(e) => {
                        self.view.set_busy(false);
                        self.view.set_status(e.into());
                    }
                }
            }
        }
        if self.cancel.load(Ordering::Acquire) {
            if let Some(applying) = self.applying.take() {
                self.bridge.publish();
                self.view.set_status(format!("Stopped after {} shared tracks changed; cached source can be reopened to reconcile",applying.modified).into());
                self.view.set_busy(false);
            }
            return;
        }
        let Some(applying) = self.applying.as_mut() else {
            return;
        };
        if !self.bridge.ready() {
            self.view
                .set_status("Source cached; waiting for the shared library to be ready".into());
            return;
        }
        let deadline = Instant::now() + Duration::from_millis(2);
        let start = applying.index;
        let mut changed = false;
        let mut error = None;
        while applying.index < applying.data.snapshot.tracks.len()
            && applying.index - start < 1024
            && Instant::now() < deadline
        {
            let row = &applying.data.snapshot.tracks[applying.index];
            match self.bridge.apply(
                row,
                applying.force || applying.changed.contains(&row.track.id),
            ) {
                Ok(modified) => {
                    if modified {
                        applying.modified += 1;
                        changed = true;
                    }
                    applying.index += 1;
                }
                Err(e) => {
                    error = Some(e);
                    break;
                }
            }
        }
        if changed {
            self.bridge.publish();
        }
        if let Some(e) = error {
            let modified = applying.modified;
            self.applying = None;
            self.view.set_busy(false);
            self.view.set_status(
                format!(
                    "Shared import stopped after {modified} changes: {e}. Source remains cached."
                )
                .into(),
            );
            return;
        }
        if applying.index == applying.data.snapshot.tracks.len() {
            let note = format!(
                "{}. Shared library: {} changed; save status below.",
                applying.note, applying.modified
            );
            self.applying = None;
            self.view.set_busy(false);
            self.view.set_status(note.into());
        } else {
            self.view.set_status(
                format!(
                    "Applying shared library: {}/{}",
                    applying.index,
                    applying.data.snapshot.tracks.len()
                )
                .into(),
            );
        }
    }
}
type State = Rc<RefCell<Option<Session>>>;
/// Compose lazy source browsing with the existing audio/library controller.
pub fn wire(ui: &AppWindow, controller: Rc<AudioController>, db: PathBuf) -> Timer {
    wire_with(ui, controller, db, executor()).0
}
fn wire_with(
    ui: &AppWindow,
    bridge: Rc<dyn Bridge>,
    db: PathBuf,
    executor: Executor,
) -> (Timer, State) {
    let state: State = Rc::new(RefCell::new(None));
    let opening = Rc::clone(&state);
    let weak = ui.as_weak();
    ui.on_show_itunes(move || {
        if let Some(s) = opening.borrow().as_ref() {
            let _ = s.view.show();
            return;
        }
        let result = (|| -> Result<Session, String> {
            let view = ItunesWindow::new().map_err(|e| e.to_string())?;
            let callback = |f: fn(&mut Session)| {
                let state = Rc::downgrade(&opening);
                move || {
                    if let Some(state) = state.upgrade() {
                        if let Some(s) = state.borrow_mut().as_mut() {
                            f(s);
                        }
                    }
                }
            };
            view.on_choose_xml(callback(|s| s.start(Work::ChooseXml)));
            view.on_choose_root(callback(|s| s.start(Work::ChooseRoot)));
            view.on_import_source(callback(|s| {
                let xml = s.view.get_xml_path().to_string();
                let path = s
                    .xml_choice
                    .as_ref()
                    .filter(|p| p.to_string_lossy() == xml)
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from(xml));
                let root = s.view.get_media_root().to_string();
                let root = if root.trim().is_empty() {
                    None
                } else {
                    Some(
                        s.root_choice
                            .as_ref()
                            .filter(|p| p.to_string_lossy() == root)
                            .cloned()
                            .unwrap_or_else(|| PathBuf::from(root)),
                    )
                };
                s.start(Work::Import(path, root));
            }));
            view.on_source_selected(callback(|s| {
                if let Some(id) = usize::try_from(s.view.get_source_index())
                    .ok()
                    .and_then(|i| s.sources.get(i))
                    .map(|v| v.id)
                {
                    s.start(Work::Read(id));
                }
            }));
            view.on_playlist_selected(callback(|s| s.playlist()));
            view.on_row_selected(callback(|s| s.details()));
            let loading = Rc::downgrade(&opening);
            view.on_load_track(move |deck| {
                if let Some(state) = loading.upgrade() {
                    if let Some(s) = state.borrow().as_ref() {
                        if !s.view.get_busy() {
                            if let Some(row) = s.selected() {
                                let result = row
                                    .file
                                    .path()
                                    .and_then(|p| s.bridge.load(deck as usize, p));
                                if let Err(e) = result {
                                    s.view.set_status(e.into());
                                }
                            }
                        }
                    }
                }
            });
            view.on_enrich(callback(|s| {
                s.view
                    .set_status("Apple Music metadata adapter is not yet configured".into())
            }));
            view.on_cancel(callback(|s| {
                s.cancel.store(true, Ordering::Release);
            }));
            view.on_dismiss(callback(|s| {
                s.cancel.store(true, Ordering::Release);
                s.closed = true;
            }));
            let closing = Rc::downgrade(&opening);
            view.window().on_close_requested(move || {
                if let Some(state) = closing.upgrade() {
                    if let Some(s) = state.borrow_mut().as_mut() {
                        s.cancel.store(true, Ordering::Release);
                        s.closed = true;
                    }
                }
                slint::CloseRequestResponse::HideWindow
            });
            view.show().map_err(|e| e.to_string())?;
            let mut s = Session {
                view,
                db: db.clone(),
                executor: Rc::clone(&executor),
                bridge: Rc::clone(&bridge),
                job: None,
                cancel: Arc::new(AtomicBool::new(false)),
                applying: None,
                sources: Vec::new(),
                data: None,
                closed: false,
                xml_choice: None,
                root_choice: None,
            };
            s.start(Work::Sources);
            Ok(s)
        })();
        match result {
            Ok(s) => *opening.borrow_mut() = Some(s),
            Err(e) => {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status(e.into());
                }
            }
        }
    });
    let closing = Rc::clone(&state);
    ui.on_close_itunes(move || {
        if let Some(s) = closing.borrow_mut().take() {
            s.cancel.store(true, Ordering::Release);
            let _ = s.view.hide();
        }
    });
    let ticking = Rc::clone(&state);
    let weak = ui.as_weak();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(10), move || {
        let mut slot = ticking.borrow_mut();
        if slot.as_ref().is_some_and(|s| s.closed) {
            if let Some(s) = slot.take() {
                s.cancel.store(true, Ordering::Release);
                let _ = s.view.hide();
            }
            return;
        }
        if let Some(s) = slot.as_mut() {
            s.poll();
            if let Some(ui) = weak.upgrade() {
                s.view.set_shared_storage(ui.get_library_storage());
            }
        }
    });
    (timer, state)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[derive(Default)]
    struct FakeBridge {
        ready: Cell<bool>,
        records: RefCell<HashMap<PathBuf, bool>>,
        loads: RefCell<Vec<(usize, PathBuf)>>,
    }
    impl Bridge for FakeBridge {
        fn ready(&self) -> bool {
            self.ready.get()
        }
        fn apply(&self, row: &PreparedTrack, _: bool) -> Result<bool, String> {
            let path = row.file.path()?;
            Ok(self.records.borrow_mut().insert(path, row.file.available)
                != Some(row.file.available))
        }
        fn publish(&self) {}
        fn load(&self, deck: usize, path: PathBuf) -> Result<(), String> {
            if self.records.borrow().get(&path) != Some(&true) {
                return Err("Track unavailable".into());
            }
            self.loads.borrow_mut().push((deck, path));
            Ok(())
        }
    }
    fn pump(mut ready: impl FnMut() -> bool) {
        let until = Instant::now() + Duration::from_secs(15);
        while !ready() {
            assert!(Instant::now() < until, "headless source workflow timed out");
            i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(11));
            slint::platform::update_timers_and_animations();
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn fixture(dir: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
        let present = dir.join("one.mp3");
        let missing = dir.join("two.mp3");
        std::fs::write(&present, b"fixture").unwrap();
        let xml = dir.join("Library.xml");
        let uri = |p: &std::path::Path| xml_escape(url::Url::from_file_path(p).unwrap().as_str());
        let record = |id, title: &str, path: &std::path::Path| {
            format!("<key>{id}</key><dict><key>Track ID</key><integer>{id}</integer><key>Name</key><string>{title}</string><key>Artist</key><string>Artist</string><key>Album</key><string>Album</string><key>Album Artist</key><string>Group</string><key>Genre</key><string>House</string><key>Rating</key><integer>80</integer><key>Play Count</key><integer>12</integer><key>Location</key><string>{}</string></dict>",uri(path))
        };
        let content=format!("<plist><dict><key>Tracks</key><dict>{}{}</dict><key>Playlists</key><array><dict><key>Playlist ID</key><integer>1</integer><key>Name</key><string>Ordered</string><key>Playlist Items</key><array><dict><key>Track ID</key><integer>2</integer></dict><dict><key>Track ID</key><integer>1</integer></dict><dict><key>Track ID</key><integer>2</integer></dict></array></dict></array></dict></plist>",record(1,"First",&present),record(2,"Second",&missing));
        std::fs::write(&xml, content).unwrap();
        (xml, present, missing)
    }
    fn xml_escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }
    #[test]
    fn itunes_window_runs_real_import_keeps_playlist_order_and_refreshes_cached_availability() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        ui.set_left_playing(true);
        ui.set_left_position(42.);
        let dir = tempfile::tempdir().unwrap();
        let (xml, present, missing) = fixture(dir.path());
        let bridge = Rc::new(FakeBridge::default());
        bridge.ready.set(true);
        let (_timer, state) = wire_with(
            &ui,
            bridge.clone(),
            dir.path().join("source.sqlite"),
            executor(),
        );
        assert!(state.borrow().is_none());
        ui.invoke_show_itunes();
        let view = state.borrow().as_ref().unwrap().view.clone_strong();
        pump(|| !view.get_busy());
        view.set_xml_path(xml.to_string_lossy().into_owned().into());
        view.invoke_import_source();
        pump(|| !view.get_busy());
        assert_eq!(view.get_rows().row_count(), 2);
        assert_eq!(bridge.records.borrow().len(), 2);
        view.set_playlist_index(1);
        view.invoke_playlist_selected();
        assert_eq!(view.get_rows().row_count(), 3);
        assert_eq!(view.get_rows().row_data(0).unwrap().title, "Second");
        assert_eq!(view.get_rows().row_data(1).unwrap().title, "First");
        assert_eq!(view.get_rows().row_data(2).unwrap().title, "Second");
        view.set_selected_row(0);
        view.invoke_row_selected();
        assert!(view.get_details().contains("Rating: 80/100"));
        assert!(!view.get_details().contains("Some("));
        view.invoke_load_track(0);
        assert!(bridge.loads.borrow().is_empty());
        view.set_selected_row(1);
        view.invoke_load_track(0);
        assert_eq!(bridge.loads.borrow().len(), 1);
        ui.invoke_close_itunes();
        assert!(state.borrow().is_none());
        std::fs::remove_file(present).unwrap();
        std::fs::write(missing, b"now present").unwrap();
        ui.invoke_show_itunes();
        let view = state.borrow().as_ref().unwrap().view.clone_strong();
        pump(|| !view.get_busy());
        assert_eq!(view.get_rows().row_data(0).unwrap().state, "Missing");
        assert_eq!(view.get_rows().row_data(1).unwrap().state, "Available");
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 42.);
        ui.invoke_close_itunes();
    }
    #[test]
    fn cancel_ignores_late_source_result_and_does_not_chain_another_worker() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let bridge = Rc::new(FakeBridge::default());
        bridge.ready.set(true);
        let sender = Rc::new(RefCell::new(None));
        let tx = Rc::clone(&sender);
        let calls = Rc::new(Cell::new(0));
        let count = Rc::clone(&calls);
        let fake: Executor = Rc::new(move |_, _, _| {
            count.set(count.get() + 1);
            let (sender, rx) = mpsc::sync_channel(1);
            *tx.borrow_mut() = Some(sender);
            rx
        });
        let (_timer, state) =
            wire_with(&ui, bridge.clone(), dir.path().join("source.sqlite"), fake);
        ui.invoke_show_itunes();
        let view = state.borrow().as_ref().unwrap().view.clone_strong();
        view.invoke_cancel();
        assert!(sender
            .borrow_mut()
            .take()
            .unwrap()
            .send(Ok(Output::Sources(vec![Source {
                id: 1,
                xml_url: "file:///C:/Library.xml".into(),
                media_root_url: None
            }])))
            .is_ok());
        pump(|| !view.get_busy());
        assert_eq!(calls.get(), 1);
        assert!(bridge.records.borrow().is_empty());
        assert!(state.borrow().as_ref().unwrap().applying.is_none());
        assert!(view.get_status().contains("Cancelled"));
        ui.invoke_close_itunes();
    }
    #[test]
    fn invalid_media_override_does_not_replace_an_existing_source() {
        let dir = tempfile::tempdir().unwrap();
        let (xml, _, _) = fixture(dir.path());
        let db = dir.path().join("source.sqlite");
        let output = run(
            Work::Import(xml.clone(), None),
            db.clone(),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        let Output::Loaded { data, .. } = output else {
            panic!("source not loaded");
        };
        let id = data.snapshot.source.id;
        assert!(run(
            Work::Import(xml, Some(dir.path().join("not-a-folder"))),
            db.clone(),
            Arc::new(AtomicBool::new(false))
        )
        .is_err());
        let store = Store::open(&db).unwrap();
        assert_eq!(store.snapshot(id).unwrap().tracks.len(), 2);
    }
}
