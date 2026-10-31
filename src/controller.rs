//! GUI control plane with injectable file selection and one bounded load worker.
use crate::{
    playback::{DeckId, DualDeckEngine, MixerLevels, PreparedTrack},
    AppWindow,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
};

type LoadResult =
    Option<Result<(PathBuf, PreparedTrack, crate::track_metadata::TrackMetadata), String>>;
type ImportResult = Option<Result<crate::library::ImportBatch, String>>;
type DeviceScanResult = Result<Vec<crate::audio_devices::DeviceChoice>, String>;
type DeviceCatalog = Arc<dyn Fn() -> DeviceScanResult + Send + Sync>;
type DeviceOpener =
    Rc<dyn Fn(Option<&crate::audio_devices::DeviceSelector>) -> Result<DualDeckEngine, String>>;
type KeylockJob = (Arc<crate::waveform::DetailPeaks>, drdj_keylock::Job, f64);
struct PitchRequest {
    source: Arc<crate::waveform::DetailPeaks>,
    factor: f64,
}
struct SyncSession {
    target: DeckId,
    sources: [Arc<crate::waveform::DetailPeaks>; 2],
    grids: [crate::beatgrid::BeatGrid; 2],
    versions: [u64; 2],
    factor: f64,
    last_frame: u64,
    last_good: std::time::Instant,
    alignment: crate::phase_sync::Alignment,
}
struct PendingSync {
    session: SyncSession,
    tempo: f64,
    prepared: bool,
}
struct NudgeSession {
    source: Arc<crate::waveform::DetailPeaks>,
    held: [bool; 2],
}
struct CueSession {
    path: Arc<std::path::Path>,
    source: Arc<crate::waveform::DetailPeaks>,
    slot: usize,
    original: crate::hot_cues::HotCue,
}
struct HotCueHoldSession {
    source: Arc<crate::waveform::DetailPeaks>,
    slot: usize,
    position: std::time::Duration,
    inputs: u16,
    preview: bool,
    promoted: bool,
}
struct GridSession {
    path: Arc<std::path::Path>,
    source: Arc<crate::waveform::DetailPeaks>,
    stored_base: Option<crate::beatgrid::BeatGrid>,
    draft: crate::beatgrid::GridDraft,
}
#[derive(Clone, Copy)]
enum GridAction {
    Tempo,
    Anchor,
    Nudge(i32),
    MeterSet,
    MeterRemove,
    TempoSet,
    TempoRemove,
    WarpSet,
    WarpRemove,
    Clear,
}
struct DetailCache {
    source: Arc<crate::waveform::DetailPeaks>,
    range: crate::waveform::DetailRange,
    grid: Option<crate::beatgrid::BeatGrid>,
}
pub type TrackPicker = Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>;
pub type LibraryPicker = Arc<dyn Fn() -> Option<Vec<PathBuf>> + Send + Sync>;
pub type LibraryFolderPicker = Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>;
struct LibraryRows {
    library: std::rc::Weak<std::cell::RefCell<crate::library::Library>>,
    generation: std::rc::Weak<std::cell::Cell<u64>>,
    expected_generation: u64,
    rows: Vec<i32>,
    #[cfg(test)]
    row_data_calls: Cell<usize>,
}
impl slint::Model for LibraryRows {
    type Data = crate::LibraryRow;

    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        #[cfg(test)]
        self.row_data_calls.set(self.row_data_calls.get() + 1);
        let id = *self.rows.get(row)?;
        let generation = self.generation.upgrade()?;
        if generation.get() != self.expected_generation {
            return None;
        }
        let library = self.library.upgrade()?;
        let library = library.try_borrow().ok()?;
        let track = library.track_by_id(id)?;
        Some(library_row(track))
    }

    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        &()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
fn library_row(track: &crate::library::Track) -> crate::LibraryRow {
    crate::LibraryRow {
        id: track.id,
        name: track.display_title().into(),
        artist: track.metadata.artist.clone().into(),
        album: track.metadata.album.clone().into(),
        duration: track.metadata.duration_text().into(),
        metadata_error: track.metadata.error.clone().unwrap_or_default().into(),
        format: track.format.clone().into(),
        path: track.path.to_string_lossy().into_owned().into(),
        available: track.available,
    }
}

type SearchRequest = (
    Arc<crate::library::SearchProjection>,
    crate::library::SearchSpec,
);
const LIBRARY_PAGE_SIZE: usize = 200;

struct SearchPage {
    ids: Vec<i32>,
    total: usize,
    page_index: usize,
}
type SearchResponse = (crate::library::SearchSpec, SearchPage);

struct SearchWorker {
    sender: mpsc::SyncSender<SearchRequest>,
    receiver: Receiver<SearchResponse>,
    pending: Option<SearchRequest>,
    wanted: Option<crate::library::SearchSpec>,
    projection: Option<(u64, u64, i32, Arc<crate::library::SearchProjection>)>,
    published: Option<crate::library::SearchSpec>,
    published_total: usize,
}

impl SearchWorker {
    fn start() -> std::io::Result<Self> {
        let (sender, requests) = mpsc::sync_channel::<SearchRequest>(1);
        let (responses, receiver) = mpsc::sync_channel::<SearchResponse>(1);
        std::thread::Builder::new()
            .name("library-search".into())
            .spawn(move || {
                let mut cached: Option<(crate::library::SearchSpec, Vec<i32>)> = None;
                while let Ok((projection, spec)) = requests.recv() {
                    let mut key = spec.clone();
                    key.page_index = 0;
                    if cached.as_ref().is_none_or(|(previous, _)| *previous != key) {
                        cached = Some((key, projection.search(&spec)));
                    }
                    let ids = &cached.as_ref().unwrap().1;
                    let total = ids.len();
                    let page_index = spec
                        .page_index
                        .min(total.saturating_sub(1) / LIBRARY_PAGE_SIZE);
                    let start = page_index * LIBRARY_PAGE_SIZE;
                    let end = start.saturating_add(LIBRARY_PAGE_SIZE).min(total);
                    let page = SearchPage {
                        ids: ids[start..end].to_vec(),
                        total,
                        page_index,
                    };
                    if responses.send((spec, page)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender,
            receiver,
            pending: None,
            wanted: None,
            projection: None,
            published: None,
            published_total: 0,
        })
    }

    fn send_pending(&mut self) -> Result<(), &'static str> {
        if let Some(request) = self.pending.take() {
            match self.sender.try_send(request) {
                Ok(()) => (),
                Err(mpsc::TrySendError::Full(request)) => self.pending = Some(request),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err("Library search worker stopped")
                }
            }
        }
        Ok(())
    }
}

pub struct AudioController {
    device_opener: DeviceOpener,
    pending_device_switch: RefCell<Option<crate::audio_devices::StartupSelection>>,
    recovery_key: RefCell<Option<crate::output_preferences::OutputDeviceKey>>,
    output_failure_handled: Cell<bool>,
    transport_accept_after: Cell<std::time::Instant>,
    output_profiles_cache: RefCell<Result<crate::output_preferences::OutputRoutes, String>>,
    device_catalog: DeviceCatalog,
    device_scan: RefCell<Option<Receiver<DeviceScanResult>>>,
    device_choices: RefCell<Vec<crate::audio_devices::DeviceChoice>>,
    device_list_ready: Cell<bool>,
    saved_startup_audio: RefCell<crate::audio_devices::StartupSelection>,
    startup_audio_saving: Cell<bool>,
    ui: slint::Weak<AppWindow>,
    engine: Rc<RefCell<Rc<DualDeckEngine>>>,
    picker: TrackPicker,
    library_picker: LibraryPicker,
    library_folder_picker: RefCell<Option<LibraryFolderPicker>>,
    library: Rc<RefCell<crate::library::Library>>,
    library_generation: Rc<Cell<u64>>,
    library_page: Cell<usize>,
    pending_page_selection: Cell<i8>,
    search_worker: RefCell<Option<SearchWorker>>,
    library_perf: RefCell<Option<Rc<RefCell<crate::library_perf::Recorder>>>>,
    perf_status_updated: Cell<std::time::Instant>,
    persistence: RefCell<Option<crate::library_store::Worker>>,
    storage_ready: Cell<bool>,
    dirty: Cell<bool>,
    saving: Cell<bool>,
    inflight_revision: Cell<Option<u64>>,
    saved_quantize: Cell<crate::preferences::QuantizePreferences>,
    observed_quantize: Cell<crate::preferences::QuantizePreferences>,
    inflight_quantize: Cell<Option<crate::preferences::QuantizePreferences>>,
    saved_output: RefCell<Option<crate::output_preferences::OutputRoute>>,
    observed_output: RefCell<Option<crate::output_preferences::OutputRoute>>,
    inflight_output: RefCell<Option<crate::output_preferences::OutputRoute>>,
    output_profiles_writable: Cell<bool>,
    fx_state: RefCell<crate::delay_fx::FxPreferences>,
    saved_fx: RefCell<crate::delay_fx::FxPreferences>,
    observed_fx: RefCell<crate::delay_fx::FxPreferences>,
    inflight_fx: RefCell<Option<crate::delay_fx::FxPreferences>>,
    fx_writable: Cell<bool>,
    save_failed: Cell<bool>,
    initial_paths: RefCell<Vec<PathBuf>>,
    crate_editor: Cell<Option<(i32, Option<i32>, bool)>>,
    crate_delete: Cell<Option<i32>>,
    crate_target: Cell<i32>,
    import_crate: Cell<i32>,
    pitch_ranges: Cell<[crate::pitch_control::PitchRange; 2]>,
    alignments: Cell<[crate::phase_sync::Alignment; 2]>,
    pending_pitch: RefCell<[Option<PitchRequest>; 2]>,
    keylock_next: Cell<[Option<f64>; 2]>,
    keylock_jobs: RefCell<[Option<KeylockJob>; 2]>,
    nudges: RefCell<[Option<NudgeSession>; 2]>,
    sync_session: RefCell<Option<SyncSession>>,
    pending_sync: RefCell<Option<PendingSync>>,
    cue_editors: RefCell<[Option<CueSession>; 2]>,
    hot_cue_holds: RefCell<[Option<HotCueHoldSession>; 2]>,
    grid_editors: RefCell<[Option<GridSession>; 2]>,
    pending_import: RefCell<Option<Receiver<ImportResult>>>,
    pending_relocation_id: Cell<Option<i32>>,
    pending_cancel: RefCell<Option<crate::cancellation::Cancellation>>,
    pending: RefCell<Option<(DeckId, Receiver<LoadResult>)>>,
    displayed_loops: RefCell<[Option<crate::saved_loops::SavedLoops>; 2]>,
    displayed_cues: RefCell<[Option<(crate::hot_cues::HotCues, std::time::Duration)>; 2]>,
    displayed_overviews: RefCell<[Option<Arc<[f32]>>; 2]>,
    displayed_details: RefCell<[Option<DetailCache>; 2]>,
}
impl Drop for AudioController {
    fn drop(&mut self) {
        for id in [DeckId::A, DeckId::B] {
            if let Some(session) = self.hot_cue_holds.get_mut()[Self::grid_index(id)].take() {
                let engine = self.engine.borrow();
                if session.preview
                    && !session.promoted
                    && engine
                        .snapshot(id)
                        .detail
                        .as_ref()
                        .is_some_and(|source| Arc::ptr_eq(source, &session.source))
                {
                    engine.pause(id);
                    let _ = engine.seek_position(id, session.position);
                }
            }
        }
        if let Some(session) = self.sync_session.get_mut().take() {
            let _ = self.engine.borrow().apply_sync_factor(session.target, 1.);
        }
        if let Some(cancel) = self.pending_cancel.get_mut().as_ref() {
            cancel.request();
        }
    }
}
impl AudioController {
    pub fn wire(
        ui: &AppWindow,
        engine: Rc<DualDeckEngine>,
        picker: TrackPicker,
        library_picker: LibraryPicker,
    ) -> Rc<Self> {
        Self::wire_with_device_catalog(
            ui,
            engine,
            picker,
            library_picker,
            Arc::new(crate::audio_devices::enumerate),
        )
    }
    fn wire_with_device_catalog(
        ui: &AppWindow,
        engine: Rc<DualDeckEngine>,
        picker: TrackPicker,
        library_picker: LibraryPicker,
        device_catalog: DeviceCatalog,
    ) -> Rc<Self> {
        Self::wire_with_audio_services(
            ui,
            engine,
            picker,
            library_picker,
            device_catalog,
            Rc::new(|selection| {
                DualDeckEngine::open_selected(selection).map_err(|e| e.to_string())
            }),
        )
    }
    fn wire_with_audio_services(
        ui: &AppWindow,
        engine: Rc<DualDeckEngine>,
        picker: TrackPicker,
        library_picker: LibraryPicker,
        device_catalog: DeviceCatalog,
        device_opener: DeviceOpener,
    ) -> Rc<Self> {
        let controller = Rc::new(Self {
            device_opener,
            pending_device_switch: RefCell::new(None),
            recovery_key: RefCell::new(None),
            output_failure_handled: Cell::new(false),
            transport_accept_after: Cell::new(std::time::Instant::now()),
            output_profiles_cache: RefCell::new(Ok(Default::default())),
            device_catalog,
            device_scan: RefCell::new(None),
            device_choices: RefCell::new(Vec::new()),
            device_list_ready: Cell::new(false),
            saved_startup_audio: RefCell::new(Default::default()),
            startup_audio_saving: Cell::new(false),
            ui: ui.as_weak(),
            engine: Rc::new(RefCell::new(engine)),
            picker,
            library_picker,
            library_folder_picker: RefCell::new(None),
            library: Rc::new(RefCell::new(crate::library::Library::default())),
            library_generation: Rc::new(Cell::new(0)),
            library_page: Cell::new(0),
            pending_page_selection: Cell::new(0),
            search_worker: RefCell::new(None),
            library_perf: RefCell::new(None),
            perf_status_updated: Cell::new(std::time::Instant::now()),
            persistence: RefCell::new(None),
            storage_ready: Cell::new(true),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            inflight_revision: Cell::new(None),
            saved_quantize: Cell::new(Default::default()),
            observed_quantize: Cell::new(Default::default()),
            inflight_quantize: Cell::new(None),
            saved_output: RefCell::new(None),
            observed_output: RefCell::new(None),
            inflight_output: RefCell::new(None),
            output_profiles_writable: Cell::new(false),
            fx_state: RefCell::new(Default::default()),
            saved_fx: RefCell::new(Default::default()),
            observed_fx: RefCell::new(Default::default()),
            inflight_fx: RefCell::new(None),
            fx_writable: Cell::new(false),
            save_failed: Cell::new(false),
            initial_paths: RefCell::new(Vec::new()),
            crate_editor: Cell::new(None),
            crate_delete: Cell::new(None),
            crate_target: Cell::new(0),
            import_crate: Cell::new(0),
            pitch_ranges: Cell::new([crate::pitch_control::PitchRange::default(); 2]),
            alignments: Cell::new([crate::phase_sync::Alignment::Beat; 2]),
            pending_pitch: RefCell::new([None, None]),
            keylock_jobs: RefCell::new([None, None]),
            keylock_next: Cell::new([None, None]),
            nudges: RefCell::new([None, None]),
            sync_session: RefCell::new(None),
            pending_sync: RefCell::new(None),
            cue_editors: RefCell::new([None, None]),
            hot_cue_holds: RefCell::new([None, None]),
            grid_editors: RefCell::new([None, None]),
            pending_import: RefCell::new(None),
            pending_relocation_id: Cell::new(None),
            pending_cancel: RefCell::new(None),
            pending: RefCell::new(None),
            displayed_loops: RefCell::new([None, None]),
            displayed_cues: RefCell::new([None, None]),
            displayed_overviews: RefCell::new([None, None]),
            displayed_details: RefCell::new([None, None]),
        });
        ui.set_audio_enabled(true);
        let count = (controller.engine.borrow().output_channels() / 2).max(1);
        ui.set_output_pair_options(slint::ModelRc::new(slint::VecModel::from(
            (0..count)
                .map(|pair| {
                    slint::SharedString::from(
                        if controller.engine.borrow().output_channels() == 1 {
                            "OUT 1 (mono)".into()
                        } else {
                            format!("OUT {}/{}", pair * 2 + 1, pair * 2 + 2)
                        },
                    )
                })
                .collect::<Vec<_>>(),
        )));
        ui.set_audio_device_current(
            format!(
                "{} ({} channels)",
                controller.engine.borrow().output_device_label(),
                controller.engine.borrow().output_channels()
            )
            .into(),
        );
        ui.on_track_transfer(|path| {
            let mut transfer = slint::DataTransfer::default();
            transfer.set_file_paths([std::path::PathBuf::from(path.as_str())]);
            transfer
        });
        let weak = Rc::downgrade(&controller);
        ui.on_accepts_track_drop(move |deck, data| {
            weak.upgrade()
                .is_some_and(|c| c.track_drop_path(deck, &data).is_ok())
        });
        let weak = Rc::downgrade(&controller);
        ui.on_drop_track(move |deck, data| {
            let Some(c) = weak.upgrade() else {
                return false;
            };
            match c.track_drop_path(deck, &data) {
                Ok((id, path)) => {
                    c.start_load(id, Arc::new(move || Some(path.clone())));
                    c.pending.borrow().is_some()
                }
                Err(error) => {
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_status(error.into());
                    }
                    false
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_audio_device_scan(move || {
            if let Some(c) = weak.upgrade() {
                c.start_device_scan();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_audio_device_save(move || {
            if let Some(c) = weak.upgrade() {
                c.save_startup_audio();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_audio_device_apply(move || {
            if let Some(c) = weak.upgrade() {
                c.request_device_switch();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_audio_device_reopen(move || {
            if let Some(c) = weak.upgrade() {
                c.request_output_reopen();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_output_pair_changed(move || {
            if let Some(c) = weak.upgrade() {
                if let Some(ui) = c.ui.upgrade() {
                    let result = u16::try_from(ui.get_output_pair_index())
                        .map_err(|_| "Invalid output pair".to_string())
                        .and_then(|pair| c.engine.borrow().set_output_pair(pair));
                    if let Err(error) = result {
                        ui.set_output_pair_index(i32::from(c.engine.borrow().output_pair()));
                        ui.set_status(error.into());
                    }
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_cancel_background_job(move || {
            if let Some(c) = weak.upgrade() {
                if let Some(cancel) = c.pending_cancel.borrow().as_ref() {
                    cancel.request();
                }
                if let Some(ui) = c.ui.upgrade() {
                    ui.set_status("Cancellation requested".into());
                }
                c.refresh();
            }
        });
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(&controller);
            let toggle = move || {
                if let Some(c) = weak.upgrade() {
                    if c.engine.borrow().output_failed() {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(
                                "Audio output failed; reopen the output in AUDIO before playing"
                                    .into(),
                            );
                        }
                        return;
                    }
                    if c.promote_hot_cue_hold(id) {
                        c.refresh();
                        return;
                    }
                    if c.engine.borrow().snapshot(id).playing {
                        c.cancel_nudge(id);
                        c.engine.borrow().pause(id);
                    } else {
                        c.engine.borrow().play(id);
                        c.record_history(id);
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_toggle_left(toggle),
                DeckId::B => ui.on_toggle_right(toggle),
            }
            let weak = Rc::downgrade(&controller);
            let cue = move || {
                if let Some(c) = weak.upgrade() {
                    c.cancel_nudge(id);
                    c.engine.borrow().cue(id);
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_reset_left(cue),
                DeckId::B => ui.on_reset_right(cue),
            }
            let weak = Rc::downgrade(&controller);
            let load = move || {
                if let Some(c) = weak.upgrade() {
                    c.request_load(id);
                }
            };
            match id {
                DeckId::A => ui.on_load_left(load),
                DeckId::B => ui.on_load_right(load),
            }
            let weak = Rc::downgrade(&controller);
            let seek = move |percent| {
                if let Some(c) = weak.upgrade() {
                    if let Err(err) = c.engine.borrow().seek_percent(id, percent) {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(format!("Deck {id:?}: {err}").into());
                        }
                    } else {
                        c.cancel_nudge(id);
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_seek_left(seek),
                DeckId::B => ui.on_seek_right(seek),
            }
            let weak = Rc::downgrade(&controller);
            let zoom = move || {
                if let Some(c) = weak.upgrade() {
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_zoom_left(zoom),
                DeckId::B => ui.on_zoom_right(zoom),
            }
        }
        let weak = Rc::downgrade(&controller);
        ui.on_mixer_changed(move || {
            if let Some(c) = weak.upgrade() {
                c.apply_levels();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_eq_changed(move |deck| {
            if let Some(c) = weak.upgrade() {
                match deck {
                    0 => c.apply_eq(DeckId::A),
                    1 => c.apply_eq(DeckId::B),
                    _ => (),
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_clear_channel_clip(move |deck| {
            if let Some(c) = weak.upgrade() {
                match deck {
                    0 => c.engine.borrow().clear_channel_clip(DeckId::A),
                    1 => c.engine.borrow().clear_channel_clip(DeckId::B),
                    _ => (),
                }
                c.refresh();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.set_monitor_status(
            if controller.engine.borrow().output_channels() >= 4 {
                "Select a master pair other than OUT 3/4; cue uses OUT 3/4 (session only)"
            } else {
                "Independent cue requires an output device with at least four channels"
            }
            .into(),
        );
        ui.on_monitor_changed(move || {
            if let Some(c) = weak.upgrade() {
                if !c.storage_ready.get() {
                    c.refresh();
                    return;
                }
                if let Some(ui) = c.ui.upgrade() {
                    let result = c.engine.borrow().set_monitor_settings(
                        crate::output_routing::MonitorSettings {
                            gain: ui.get_monitor_gain() / 100.,
                            mix: ui.get_monitor_mix() / 100.,
                        },
                    );
                    ui.set_monitor_status(match result {
                        Ok(()) => if c.output_profiles_writable.get() {
                            "Monitor changed"
                        } else {
                            "Monitor updated (session only)"
                        }
                        .into(),
                        Err(error) => error.into(),
                    });
                    c.refresh();
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_monitor_cue_changed(move |deck| {
            if let Some(c) = weak.upgrade() {
                if !c.storage_ready.get() {
                    c.refresh();
                    return;
                }
                if let Some(ui) = c.ui.upgrade() {
                    let request = match deck {
                        0 => Some((DeckId::A, ui.get_monitor_cue_a())),
                        1 => Some((DeckId::B, ui.get_monitor_cue_b())),
                        _ => None,
                    };
                    if let Some((id, enabled)) = request {
                        let status = match c.engine.borrow().set_headphone_cue(id, enabled) {
                            Ok(()) => if c.output_profiles_writable.get() {
                                "Headphone cue changed"
                            } else {
                                "Headphone cue updated (session only)"
                            }
                            .to_string(),
                            Err(error) => error,
                        };
                        ui.set_monitor_status(status.into());
                    }
                    c.refresh();
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_fx_changed(move |deck| {
            if let Some(c) = weak.upgrade() {
                match deck {
                    0 => c.apply_fx(DeckId::A),
                    1 => c.apply_fx(DeckId::B),
                    _ => (),
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_fx_preset(move |deck, slot, save| {
            if let Some(c) = weak.upgrade() {
                match deck {
                    0 => c.fx_preset(DeckId::A, slot, save),
                    1 => c.fx_preset(DeckId::B, slot, save),
                    _ => (),
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_import_library(move || {
            if let Some(c) = weak.upgrade() {
                c.start_import(Arc::clone(&c.library_picker));
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_import_library_folder(move || {
            if let Some(c) = weak.upgrade() {
                c.start_folder_import();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_relocate_library_track(move || {
            if let Some(c) = weak.upgrade() {
                c.start_relocation();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_library_search(move || {
            if let Some(c) = weak.upgrade() {
                if let Some(ui) = c.ui.upgrade() {
                    c.measure_library_action(crate::library_perf::Operation::Search, || {
                        c.library
                            .borrow_mut()
                            .set_query_background(ui.get_library_query().to_string());
                        c.publish_library();
                    });
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_select_library(move |id| {
            let Some(c) = weak.upgrade() else {
                return;
            };
            if c.search_in_progress() {
                return;
            }
            let Some(ui) = c.ui.upgrade() else {
                return;
            };
            let rows = ui.get_library_rows();
            let valid = id < 0
                || rows
                    .as_any()
                    .downcast_ref::<LibraryRows>()
                    .is_some_and(|model| model.rows.contains(&id));
            if !valid {
                return;
            }
            c.library.borrow_mut().select(id);
            c.publish_selected_library_track(&ui);
        });
        let weak = Rc::downgrade(&controller);
        ui.on_move_library_selection(move |delta| {
            let Some(c) = weak.upgrade() else {
                return;
            };
            if c.search_in_progress() {
                return;
            }
            let Some(ui) = c.ui.upgrade() else {
                return;
            };
            let rows = ui.get_library_rows();
            let Some(model) = rows.as_any().downcast_ref::<LibraryRows>() else {
                return;
            };
            let current = c.library.borrow().selected_id();
            let index = model.rows.iter().position(|id| *id == current);
            if let Some(index) = index {
                let past_end = delta > 0 && index + 1 == model.rows.len();
                let before_start = delta < 0 && index == 0;
                if (past_end && ui.get_library_can_next_page())
                    || (before_start && ui.get_library_can_previous_page())
                {
                    c.change_library_page(delta.signum(), delta.signum() as i8);
                    return;
                }
            }
            let target = if model.rows.is_empty() {
                -1
            } else {
                let index = index.map_or(0, |i| {
                    (i as i32 + delta).clamp(0, model.rows.len() as i32 - 1) as usize
                });
                model.rows[index]
            };
            c.library.borrow_mut().select(target);
            c.publish_selected_library_track(&ui);
        });
        let weak = Rc::downgrade(&controller);
        ui.on_sort_library(move |field| {
            if let Some(c) = weak.upgrade() {
                c.measure_library_action(crate::library_perf::Operation::Sort, || {
                    c.library.borrow_mut().sort(field);
                    c.publish_library();
                });
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_library_change_page(move |delta| {
            if let Some(c) = weak.upgrade() {
                c.change_library_page(delta, 0);
            }
        });
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(&controller);
            let load = move || {
                if let Some(c) = weak.upgrade() {
                    if c.search_in_progress() {
                        return;
                    }
                    let selected = c.library.borrow().selected_id();
                    if selected < 0
                        || c.ui
                            .upgrade()
                            .is_none_or(|ui| ui.get_selected_track() != selected)
                    {
                        return;
                    }
                    let path = c.library.borrow().selected_path();
                    if let Some(path) = path {
                        c.start_load(id, Arc::new(move || Some(path.clone())));
                    }
                }
            };
            match id {
                DeckId::A => ui.on_load_library_left(load),
                DeckId::B => ui.on_load_library_right(load),
            }
        }
        controller.publish_library();
        let weak = Rc::downgrade(&controller);
        ui.on_retry_library_save(move || {
            if let Some(c) = weak.upgrade() {
                c.save_failed.set(false);
                c.save_library();
            }
        });
        controller.apply_levels();
        controller.wire_crates(ui);
        controller.wire_grid_editor(ui);
        controller.wire_hot_cues(ui);
        controller.wire_loops(ui);
        controller.wire_pitch(ui);
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(&controller);
            let change = move || {
                if let Some(c) = weak.upgrade() {
                    c.poll_keylock();
                }
            };
            match id {
                DeckId::A => ui.on_keylock_left(change),
                DeckId::B => ui.on_keylock_right(change),
            }
        }
        controller.wire_nudge(ui);
        controller.wire_saved_loops(ui);
        controller.refresh();
        controller
    }
    fn can_control_deck(&self, id: DeckId) -> bool {
        // Imports merge into the catalog; restore still owns it exclusively.
        self.storage_ready.get()
            && self
                .pending
                .borrow()
                .as_ref()
                .is_none_or(|(loading, _)| *loading != id)
    }
    fn can_change_crates(&self) -> bool {
        self.storage_ready.get()
            && self.pending.borrow().is_none()
            && self.pending_import.borrow().is_none()
    }
    fn cancel_crate_dialog(&self) {
        self.crate_editor.set(None);
        self.crate_delete.set(None);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_crate_editor_open(false);
            ui.set_crate_delete_pending(false);
        }
    }
    fn crate_changed(&self, result: Result<String, String>) {
        if let Some(ui) = self.ui.upgrade() {
            match result {
                Ok(message) => {
                    self.library_changed();
                    self.cancel_crate_dialog();
                    self.publish_library();
                    ui.set_status(message.into());
                }
                Err(error) => ui.set_status(error.into()),
            }
        }
    }
    fn wire_crates(self: &Rc<Self>, ui: &AppWindow) {
        let weak = Rc::downgrade(self);
        ui.on_select_crate(move |id| {
            if let Some(c) = weak.upgrade() {
                if !c.storage_ready.get() {
                    return;
                }
                let result = c.library.borrow_mut().select_crate(id);
                c.cancel_crate_dialog();
                if result.is_ok() {
                    c.publish_library();
                }
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_target_crate(move |index| {
            if let Some(c) = weak.upgrade() {
                if let Some(entry) = c
                    .library
                    .borrow()
                    .crates()
                    .ordered()
                    .into_iter()
                    .filter(|entry| entry.smart_query.is_none())
                    .nth(index.max(0) as usize)
                {
                    c.crate_target.set(entry.id);
                }
            }
        });
        for rename in [false, true] {
            let weak = Rc::downgrade(self);
            let callback = move || {
                if let Some(c) = weak.upgrade() {
                    if !c.can_change_crates() {
                        return;
                    }
                    c.cancel_crate_dialog();
                    let library = c.library.borrow();
                    let id = if rename { library.active_crate() } else { 0 };
                    let (name, smart_query) = if rename {
                        let Some(entry) = library.crates().entries.iter().find(|e| e.id == id)
                        else {
                            return;
                        };
                        (entry.name.clone(), entry.smart_query.clone())
                    } else {
                        (String::new(), None)
                    };
                    let smart = smart_query.is_some();
                    c.crate_editor.set(Some((id, None, smart)));
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_crate_edit_name(name.into());
                        ui.set_crate_edit_smart_query(smart_query.unwrap_or_default().into());
                        ui.set_crate_editor_smart(smart);
                        ui.set_crate_dialog_title(
                            if rename {
                                if smart {
                                    "Edit smart crate"
                                } else {
                                    "Rename crate"
                                }
                            } else {
                                "New crate"
                            }
                            .into(),
                        );
                        ui.set_crate_editor_open(true);
                    }
                }
            };
            if rename {
                ui.on_rename_crate(callback);
            } else {
                ui.on_new_crate(callback);
            }
        }
        let weak = Rc::downgrade(self);
        ui.on_new_smart_crate(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                c.cancel_crate_dialog();
                c.crate_editor.set(Some((0, None, true)));
                if let Some(ui) = c.ui.upgrade() {
                    ui.set_crate_edit_name("".into());
                    ui.set_crate_edit_smart_query("".into());
                    ui.set_crate_editor_smart(true);
                    ui.set_crate_dialog_title("New smart crate".into());
                    ui.set_crate_editor_open(true);
                }
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_new_subcrate(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                let library = c.library.borrow();
                let parent_id = library.active_crate();
                if parent_id == 0 {
                    return;
                }
                let Ok(parent_name) = library.crates().hierarchy_name(parent_id) else {
                    return;
                };
                c.crate_editor.set(Some((0, Some(parent_id), false)));
                if let Some(ui) = c.ui.upgrade() {
                    ui.set_crate_edit_name("".into());
                    ui.set_crate_edit_smart_query("".into());
                    ui.set_crate_editor_smart(false);
                    ui.set_crate_dialog_title(format!("New subcrate in {parent_name}").into());
                    ui.set_crate_editor_open(true);
                }
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_save_crate_name(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                let Some((id, parent_id, smart)) = c.crate_editor.get() else {
                    return;
                };
                let Some(ui) = c.ui.upgrade() else {
                    return;
                };
                let name = ui.get_crate_edit_name().to_string();
                let smart_query = smart.then(|| ui.get_crate_edit_smart_query().to_string());
                let result = if id == 0 {
                    let mut library = c.library.borrow_mut();
                    let created = if let Some(query) = smart_query.as_deref() {
                        library.create_smart_crate(&name, query)
                    } else {
                        match parent_id {
                            Some(parent_id) => library.create_crate_under(Some(parent_id), &name),
                            None => library.create_crate(&name),
                        }
                    };
                    created.and_then(|id| {
                        c.crate_target.set(id);
                        library.select_crate(id)
                    })
                } else {
                    let mut library = c.library.borrow_mut();
                    if smart {
                        library.edit_crate(id, &name, smart_query.as_deref())
                    } else {
                        library.rename_crate(id, &name)
                    }
                };
                c.crate_changed(result.map(|()| "Crate saved".into()));
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_delete_crate(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                let library = c.library.borrow();
                let id = library.active_crate();
                let Some(entry) = library.crates().entries.iter().find(|e| e.id == id) else {
                    return;
                };
                c.cancel_crate_dialog();
                c.crate_delete.set(Some(id));
                if let Some(ui) = c.ui.upgrade() {
                    ui.set_crate_dialog_title(format!("Delete {}?", entry.name).into());
                    ui.set_crate_delete_pending(true);
                }
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_confirm_delete_crate(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                let Some(id) = c.crate_delete.get() else {
                    return;
                };
                let result = c.library.borrow_mut().delete_crate(id);
                c.crate_changed(result.map(|()| "Crate deleted; tracks remain in library".into()));
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_cancel_crate_dialog(move || {
            if let Some(c) = weak.upgrade() {
                c.cancel_crate_dialog();
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_add_to_crate(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                let result = c
                    .library
                    .borrow_mut()
                    .add_selected_to_crate(c.crate_target.get());
                c.crate_changed(result.map(|added| {
                    if added {
                        "Track added to crate"
                    } else {
                        "Track already in crate"
                    }
                    .into()
                }));
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_remove_from_crate(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                let result = c.library.borrow_mut().remove_selected_from_crate();
                c.crate_changed(result.map(|_| "Track removed from crate; file retained".into()));
            }
        });
    }
    fn grid_index(id: DeckId) -> usize {
        if id == DeckId::A {
            0
        } else {
            1
        }
    }
    fn set_grid_editing(&self, id: DeckId, editing: bool) {
        if let Some(ui) = self.ui.upgrade() {
            match id {
                DeckId::A => ui.set_left_grid_editing(editing),
                DeckId::B => ui.set_right_grid_editing(editing),
            }
        }
    }
    fn cancel_grid_editor(&self, id: DeckId) {
        self.grid_editors.borrow_mut()[Self::grid_index(id)].take();
        self.set_grid_editing(id, false);
    }
    fn grid_bpm_text(ui: &AppWindow, id: DeckId) -> String {
        match id {
            DeckId::A => ui.get_left_grid_draft_bpm(),
            DeckId::B => ui.get_right_grid_draft_bpm(),
        }
        .to_string()
    }
    fn meter_denominator_from_index(index: i32) -> Result<u8, String> {
        match index {
            0 => Ok(2),
            1 => Ok(4),
            2 => Ok(8),
            3 => Ok(16),
            _ => Err("Invalid meter denominator".into()),
        }
    }
    fn meter_denominator_index(denominator: u8) -> i32 {
        match denominator {
            2 => 0,
            4 => 1,
            8 => 2,
            16 => 3,
            _ => 1,
        }
    }
    fn change_saved_loop(&self, id: DeckId, command: usize) -> Result<String, String> {
        if !self.can_control_deck(id) {
            return Err("Finish loading before using saved loops".into());
        }
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let (slot, name) = match id {
            DeckId::A => (ui.get_left_saved_loop_slot(), ui.get_left_saved_loop_name()),
            DeckId::B => (
                ui.get_right_saved_loop_slot(),
                ui.get_right_saved_loop_name(),
            ),
        };
        let slot = slot
            .checked_sub(1)
            .and_then(|slot| usize::try_from(slot).ok())
            .filter(|slot| *slot < crate::saved_loops::SLOT_COUNT)
            .ok_or("Loop slot must be 1 through 8")?;
        let snapshot = self.engine.borrow().snapshot(id);
        let path = snapshot.path.ok_or("Load a track first")?;
        let saved = self.library.borrow().saved_loops(&path);
        let value = match command {
            0 => {
                if saved.slots()[slot].is_some() {
                    return Err(
                        "Slot occupied; delete it explicitly before saving a replacement".into(),
                    );
                }
                let start = snapshot.loop_in.ok_or("Set Loop In first")?;
                let end = snapshot.loop_out.ok_or("Set Loop Out first")?;
                let name = if name.is_empty() {
                    format!("Loop {}", slot + 1)
                } else {
                    name.to_string()
                };
                Some(crate::saved_loops::SavedLoop::new(start, end, name)?)
            }
            1 => {
                let value = saved.slots()[slot]
                    .as_ref()
                    .ok_or("Saved loop slot is empty")?;
                self.engine
                    .borrow()
                    .restore_loop(id, value.start(), value.end())?;
                self.cancel_nudge(id);
                return Ok("Saved boundaries loaded; press LOOP to activate".into());
            }
            _ => {
                if saved.slots()[slot].is_none() {
                    return Err("Saved loop slot is empty".into());
                }
                None
            }
        };
        let inserted = self.library.borrow_mut().insert([path.to_path_buf()]);
        self.library
            .borrow_mut()
            .set_saved_loop(&path, slot, value)?;
        self.library_changed();
        if inserted > 0 {
            self.publish_library();
        }
        Ok(if self.persistence.borrow().is_some() {
            "Saved loop updated; save queued"
        } else {
            "Saved loop updated for this session only"
        }
        .into())
    }
    fn wire_saved_loops(self: &Rc<Self>, ui: &AppWindow) {
        for id in [DeckId::A, DeckId::B] {
            for command in 0..3 {
                let weak = Rc::downgrade(self);
                let action = move || {
                    if let Some(c) = weak.upgrade() {
                        let result = c.change_saved_loop(id, command);
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(
                                format!("Deck {id:?}: {}", result.unwrap_or_else(|e| e)).into(),
                            );
                        }
                        c.refresh();
                    }
                };
                match (id, command) {
                    (DeckId::A, 0) => ui.on_save_loop_left(action),
                    (DeckId::B, 0) => ui.on_save_loop_right(action),
                    (DeckId::A, 1) => ui.on_recall_loop_left(action),
                    (DeckId::B, 1) => ui.on_recall_loop_right(action),
                    (DeckId::A, _) => ui.on_delete_loop_left(action),
                    (DeckId::B, _) => ui.on_delete_loop_right(action),
                }
            }
        }
    }
    fn tempo_match_target(&self, id: DeckId) -> Result<f64, String> {
        if !self.can_control_deck(DeckId::A) || !self.can_control_deck(DeckId::B) {
            return Err("Finish loading both decks before matching tempo".into());
        }
        if self.grid_editors.borrow().iter().any(Option::is_some) {
            return Err("Save or cancel both grid edits before matching tempo".into());
        }
        if self.pending_pitch.borrow().iter().any(Option::is_some) {
            return Err("Wait for pending pitch changes before matching tempo".into());
        }
        if self.keylock_jobs.borrow().iter().any(Option::is_some) {
            return Err("Wait for Keylock preparation before matching tempo".into());
        }
        if self.nudges.borrow().iter().any(Option::is_some) {
            return Err("Release nudge on both decks before matching tempo".into());
        }
        let factor = self.engine.borrow().tempo_match_factor(id)?;
        if !self.pitch_ranges.get()[Self::grid_index(id)].contains(factor) {
            return Err("Widen the target pitch range before matching tempo".into());
        }
        Ok(factor)
    }
    fn change_alignment(&self, id: DeckId) -> Result<(), String> {
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let index = match id {
            DeckId::A => ui.get_left_alignment_index(),
            DeckId::B => ui.get_right_alignment_index(),
        };
        let alignment = crate::phase_sync::Alignment::from_index(index)?;
        let mut choices = self.alignments.get();
        if choices[Self::grid_index(id)] == alignment {
            return Ok(());
        }
        let follower = self.sync_session.borrow().as_ref().map(|s| s.target);
        let preparing = self
            .pending_sync
            .borrow()
            .as_ref()
            .map(|s| s.session.target);
        if follower == Some(id) || preparing == Some(id) {
            self.stop_sync();
        }
        choices[Self::grid_index(id)] = alignment;
        self.alignments.set(choices);
        Ok(())
    }
    fn stop_sync(&self) {
        if let Some(pending) = self.pending_sync.borrow_mut().take() {
            let index = Self::grid_index(pending.session.target);
            self.keylock_jobs.borrow_mut()[index].take();
            let mut next = self.keylock_next.get();
            next[index] = None;
            self.keylock_next.set(next);
        }
        if let Some(session) = self.sync_session.borrow_mut().take() {
            let _ = self.engine.borrow().apply_sync_factor(session.target, 1.);
        }
    }
    fn sync_start_ready(&self, id: DeckId) -> Result<f64, String> {
        if self.keylock_requested(id) && !self.keylock_active(id) {
            return Err("Wait for initial Keylock preparation".into());
        }
        let factor = self.tempo_match_target(id)?;
        self.engine
            .borrow()
            .phase_sync_target_frame(id, self.alignments.get()[Self::grid_index(id)])?;
        Ok(factor)
    }
    fn capture_sync(&self, id: DeckId, frame: u64) -> SyncSession {
        let snapshots = [
            self.engine.borrow().snapshot(DeckId::A),
            self.engine.borrow().snapshot(DeckId::B),
        ];
        SyncSession {
            target: id,
            sources: [
                snapshots[0].detail.clone().unwrap(),
                snapshots[1].detail.clone().unwrap(),
            ],
            grids: [snapshots[0].grid.unwrap(), snapshots[1].grid.unwrap()],
            versions: [
                self.engine.borrow().command_version(DeckId::A).unwrap(),
                self.engine.borrow().command_version(DeckId::B).unwrap(),
            ],
            factor: 1.,
            last_frame: frame,
            last_good: self.engine.borrow().control_now(),
            alignment: self.alignments.get()[Self::grid_index(id)],
        }
    }
    fn begin_sync_now(&self, id: DeckId) -> Result<(), String> {
        self.phase_sync_ready(id)?;
        let alignment = self.alignments.get()[Self::grid_index(id)];
        let (_, frame, _) = self
            .engine
            .borrow()
            .sync_observation(id, alignment)
            .ok_or("Wait for fresh playing deck frames before Sync")?;
        self.engine.borrow().phase_sync_once(id, alignment)?;
        *self.sync_session.borrow_mut() = Some(self.capture_sync(id, frame));
        Ok(())
    }
    fn toggle_sync(&self, id: DeckId) -> Result<(), String> {
        let preparing = self
            .pending_sync
            .borrow()
            .as_ref()
            .map(|s| s.session.target);
        if let Some(target) = preparing {
            if target != id {
                return Err("Cancel the pending Sync follower first".into());
            }
            self.stop_sync();
            return Ok(());
        }
        let existing = self.sync_session.borrow().as_ref().map(|s| s.target);
        if let Some(target) = existing {
            if target != id {
                return Err("Stop the existing Sync follower first".into());
            }
            self.stop_sync();
            return Ok(());
        }
        let factor = self.sync_start_ready(id)?;
        let (_, frame, _) = self
            .engine
            .borrow()
            .sync_observation(id, self.alignments.get()[Self::grid_index(id)])
            .ok_or("Wait for fresh playing deck frames before Sync")?;
        if self
            .engine
            .borrow()
            .keylock_factor(id)
            .is_some_and(|prepared| (prepared - factor).abs() > 1e-10)
        {
            self.start_keylock(id, factor)?;
            *self.pending_sync.borrow_mut() = Some(PendingSync {
                session: self.capture_sync(id, frame),
                tempo: factor,
                prepared: false,
            });
            return Ok(());
        }
        self.begin_sync_now(id)
    }
    fn pending_sync_valid(&self, pending: &PendingSync) -> bool {
        let session = &pending.session;
        let snapshots = [
            self.engine.borrow().snapshot(DeckId::A),
            self.engine.borrow().snapshot(DeckId::B),
        ];
        [DeckId::A, DeckId::B]
            .into_iter()
            .enumerate()
            .all(|(i, id)| {
                self.can_control_deck(id)
                    && snapshots[i].playing
                    && !snapshots[i].loop_active
                    && snapshots[i].nudge_direction == 0
                    && snapshots[i].grid == Some(session.grids[i])
                    && snapshots[i]
                        .detail
                        .as_ref()
                        .is_some_and(|s| Arc::ptr_eq(s, &session.sources[i]))
                    && self.engine.borrow().command_version(id) == Some(session.versions[i])
            })
            && self.grid_editors.borrow().iter().all(Option::is_none)
            && self.nudges.borrow().iter().all(Option::is_none)
            && self.pending_pitch.borrow().iter().all(Option::is_none)
            && self.keylock_requested(session.target)
            && self
                .engine
                .borrow()
                .tempo_match_factor(session.target)
                .is_ok_and(|tempo| (tempo - pending.tempo).abs() < 1e-10)
            && self.pitch_ranges.get()[Self::grid_index(session.target)].contains(pending.tempo)
            && self.alignments.get()[Self::grid_index(session.target)] == session.alignment
            && self
                .engine
                .borrow()
                .control_now()
                .checked_duration_since(session.last_good)
                .is_some_and(|age| age <= std::time::Duration::from_secs(30))
    }
    fn cancel_invalid_pending_sync(&self) {
        let invalid = self
            .pending_sync
            .borrow()
            .as_ref()
            .is_some_and(|s| !self.pending_sync_valid(s));
        if invalid {
            self.stop_sync();
            if let Some(ui) = self.ui.upgrade() {
                ui.set_status(
                    "Pending Sync cancelled after input, transport or timeout change".into(),
                );
            }
        }
    }
    fn poll_pending_sync(&self) {
        self.cancel_invalid_pending_sync();
        let Some(pending) = self.pending_sync.borrow_mut().take() else {
            return;
        };
        if !pending.prepared
            || self
                .engine
                .borrow()
                .sync_observation(pending.session.target, pending.session.alignment)
                .is_none()
        {
            *self.pending_sync.borrow_mut() = Some(pending);
            return;
        }
        let result = if self
            .engine
            .borrow()
            .keylock_factor(pending.session.target)
            .is_some_and(|factor| (factor - pending.tempo).abs() < 1e-10)
        {
            self.begin_sync_now(pending.session.target)
        } else {
            Err("Pending Sync cancelled after Keylock tempo change".into())
        };
        if let Some(ui) = self.ui.upgrade() {
            ui.set_status(
                result
                    .map(|()| "Sync follower active".to_string())
                    .unwrap_or_else(|e| e)
                    .into(),
            );
        }
    }
    fn maintain_sync(&self, now: std::time::Instant) {
        let Some(mut session) = self.sync_session.borrow_mut().take() else {
            return;
        };
        let snapshots = [
            self.engine.borrow().snapshot(DeckId::A),
            self.engine.borrow().snapshot(DeckId::B),
        ];
        let valid = [DeckId::A, DeckId::B]
            .into_iter()
            .enumerate()
            .all(|(i, id)| {
                self.can_control_deck(id)
                    && snapshots[i].playing
                    && !snapshots[i].loop_active
                    && snapshots[i].nudge_direction == 0
                    && snapshots[i].grid == Some(session.grids[i])
                    && snapshots[i]
                        .detail
                        .as_ref()
                        .is_some_and(|s| Arc::ptr_eq(s, &session.sources[i]))
                    && self.engine.borrow().command_version(id) == Some(session.versions[i])
            })
            && self.grid_editors.borrow().iter().all(Option::is_none)
            && self.nudges.borrow().iter().all(Option::is_none)
            && self.pending_pitch.borrow().iter().all(Option::is_none)
            && self.keylock_jobs.borrow().iter().all(Option::is_none)
            && (!self.keylock_requested(session.target) || self.keylock_active(session.target))
            && now
                .checked_duration_since(session.last_good)
                .is_some_and(|age| age <= std::time::Duration::from_millis(100));
        let result = if !valid {
            Err("Sync stopped after input, transport or freshness change".to_string())
        } else if let Some((error, frame, nominal)) = self
            .engine
            .borrow()
            .sync_observation(session.target, session.alignment)
        {
            let base = snapshots[Self::grid_index(session.target)].requested_speed;
            frame
                .checked_sub(session.last_frame)
                .ok_or("Sync clock reset".to_string())
                .and_then(|elapsed| {
                    crate::sync_control::step(
                        session.factor,
                        error,
                        nominal,
                        base,
                        elapsed,
                        self.engine.borrow().output_rate(),
                        session.alignment,
                    )
                })
                .and_then(|factor| {
                    self.engine
                        .borrow()
                        .apply_sync_factor(session.target, factor)?;
                    session.factor = factor;
                    session.last_frame = frame;
                    session.last_good = now;
                    Ok(())
                })
        } else {
            session.factor = 1.;
            session.last_frame = self.engine.borrow().rendered_frames();
            self.engine.borrow().apply_sync_factor(session.target, 1.)
        };
        match result {
            Ok(()) => {
                session.versions = [
                    self.engine.borrow().command_version(DeckId::A).unwrap(),
                    self.engine.borrow().command_version(DeckId::B).unwrap(),
                ];
                *self.sync_session.borrow_mut() = Some(session);
            }
            Err(error) => {
                let _ = self.engine.borrow().apply_sync_factor(session.target, 1.);
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_status(error.into());
                }
            }
        }
    }
    fn phase_sync_ready(&self, id: DeckId) -> Result<(), String> {
        if self.keylock_requested(id) && !self.keylock_active(id) {
            return Err("Wait for initial Keylock preparation".into());
        }
        self.tempo_match_target(id)?;
        self.engine
            .borrow()
            .phase_sync_frame(id, self.alignments.get()[Self::grid_index(id)])?;
        Ok(())
    }
    fn match_tempo(&self, id: DeckId) -> Result<(), String> {
        let factor = self.tempo_match_target(id)?;
        if self.keylock_requested(id) {
            // Preserve the current prepared rate until the worker result is adopted.
            self.start_keylock(id, factor)
        } else {
            self.engine.borrow().set_speed(id, factor)
        }
    }
    fn queue_pitch(&self, id: DeckId, percent: f32) -> Result<(), String> {
        self.pending_pitch.borrow_mut()[Self::grid_index(id)].take();
        if !self.can_control_deck(id) {
            return Err("Finish loading before changing pitch".into());
        }
        let source = self
            .engine
            .borrow()
            .snapshot(id)
            .detail
            .ok_or("Load a track first")?;
        let factor = self.pitch_ranges.get()[Self::grid_index(id)].factor(f64::from(percent))?;
        crate::playback_speed::PlaybackSpeed::new(factor, source.sample_rate())?;
        self.pending_pitch.borrow_mut()[Self::grid_index(id)] =
            Some(PitchRequest { source, factor });
        Ok(())
    }
    fn change_pitch_range(&self, id: DeckId) -> Result<(), String> {
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let index = match id {
            DeckId::A => ui.get_left_pitch_range_index(),
            DeckId::B => ui.get_right_pitch_range_index(),
        };
        let range = crate::pitch_control::PitchRange::from_index(index)?;
        self.pending_pitch.borrow_mut()[Self::grid_index(id)].take();
        let mut ranges = self.pitch_ranges.get();
        ranges[Self::grid_index(id)] = range;
        self.pitch_ranges.set(ranges);
        Ok(())
    }
    fn desired_pitch(
        &self,
        id: DeckId,
        source: &Arc<crate::waveform::DetailPeaks>,
        fallback: f64,
    ) -> f64 {
        let index = Self::grid_index(id);
        if let Some(request) = self.pending_pitch.borrow()[index]
            .as_ref()
            .filter(|r| Arc::ptr_eq(source, &r.source))
        {
            return request.factor;
        }
        if self.keylock_requested(id) {
            if let Some((_, _, tempo)) = self.keylock_jobs.borrow()[index]
                .as_ref()
                .filter(|(s, _, _)| Arc::ptr_eq(source, s))
            {
                return self.keylock_next.get()[index].unwrap_or(*tempo);
            }
        }
        fallback
    }
    fn fine_pitch(&self, id: DeckId, up: bool) -> Result<(), String> {
        if !self.can_control_deck(id) {
            return Err("Finish loading before changing pitch".into());
        }
        let snapshot = self.engine.borrow().snapshot(id);
        let source = snapshot.detail.ok_or("Load a track first")?;
        let factor = self.desired_pitch(id, &source, snapshot.requested_speed);
        let target = self.pitch_ranges.get()[Self::grid_index(id)].step(factor, up)?;
        self.queue_pitch(id, ((target - 1.) * 100.) as f32)
    }
    fn cancel_nudge(&self, id: DeckId) {
        self.nudges.borrow_mut()[Self::grid_index(id)] = None;
        // Release bypasses all busy guards and never starts a deck.
        let _ = self.engine.borrow().set_nudge(id, 0);
    }
    fn cancel_stale_nudges(&self) {
        for id in [DeckId::A, DeckId::B] {
            let snapshot = self.engine.borrow().snapshot(id);
            let stale = self.nudges.borrow()[Self::grid_index(id)]
                .as_ref()
                .is_some_and(|session| {
                    !self.can_control_deck(id)
                        || !snapshot.playing
                        || snapshot
                            .detail
                            .as_ref()
                            .is_none_or(|source| !Arc::ptr_eq(source, &session.source))
                        || (session.held[0] != session.held[1] && snapshot.nudge_direction == 0)
                });
            if stale {
                self.cancel_nudge(id);
            }
        }
    }
    fn hold_nudge(&self, id: DeckId, direction: i32, held: bool) -> Result<(), String> {
        if direction != -1 && direction != 1 {
            return Err("Invalid nudge direction".into());
        }
        if held
            && self.keylock_requested(id)
            && (self.keylock_jobs.borrow()[Self::grid_index(id)].is_some()
                || !self.keylock_active(id))
        {
            return Err("Wait for Keylock preparation before nudging".into());
        }
        self.cancel_stale_nudges();
        let index = Self::grid_index(id);
        if held && !self.can_control_deck(id) {
            return Err("Nudge unavailable during loading".into());
        }
        let snapshot = self.engine.borrow().snapshot(id);
        let mut sessions = self.nudges.borrow_mut();
        if held && sessions[index].is_none() {
            if !snapshot.playing {
                return Err("Play the deck before nudging".into());
            }
            sessions[index] = Some(NudgeSession {
                source: snapshot.detail.ok_or("Load a track first")?,
                held: [false; 2],
            });
        }
        let Some(session) = sessions[index].as_mut() else {
            return Ok(());
        };
        let button = usize::from(direction > 0);
        if session.held[button] == held {
            return Ok(());
        }
        let mut next = session.held;
        next[button] = held;
        let target = i32::from(next[1]) - i32::from(next[0]);
        if let Err(error) = self.engine.borrow().set_nudge(id, target) {
            if !held || session.held == [false; 2] {
                // A failed remaining direction must not retain already released inputs.
                drop(sessions);
                self.cancel_nudge(id);
            }
            return Err(error);
        }
        session.held = next;
        if next == [false; 2] {
            sessions[index] = None;
        }
        Ok(())
    }
    fn wire_nudge(self: &Rc<Self>, ui: &AppWindow) {
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(self);
            let hold = move |direction, held| {
                if let Some(c) = weak.upgrade() {
                    if let Err(error) = c.hold_nudge(id, direction, held) {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(error.into());
                        }
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_nudge_left(hold),
                DeckId::B => ui.on_nudge_right(hold),
            }
            let weak = Rc::downgrade(self);
            let cancel = move || {
                if let Some(c) = weak.upgrade() {
                    c.cancel_nudge(id);
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_cancel_nudge_left(cancel),
                DeckId::B => ui.on_cancel_nudge_right(cancel),
            }
        }
    }
    fn wire_pitch(self: &Rc<Self>, ui: &AppWindow) {
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(self);
            let alignment = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.change_alignment(id);
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_status(
                            result
                                .map(|()| {
                                    "Alignment mode selected; start PHASE or SYNC explicitly"
                                        .to_string()
                                })
                                .unwrap_or_else(|e| e)
                                .into(),
                        );
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_alignment_mode_left(alignment),
                DeckId::B => ui.on_alignment_mode_right(alignment),
            }

            let weak = Rc::downgrade(self);
            let follow = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.toggle_sync(id);
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_status(
                            result
                                .map(|()| {
                                    if c.pending_sync.borrow().is_some() {
                                        "Preparing Keylock Sync; press again to cancel".to_string()
                                    } else {
                                        "Sync follower updated".to_string()
                                    }
                                })
                                .unwrap_or_else(|e| e)
                                .into(),
                        );
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_toggle_sync_left(follow),
                DeckId::B => ui.on_toggle_sync_right(follow),
            }

            let weak = Rc::downgrade(self);
            let sync = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.phase_sync_ready(id).and_then(|()| {
                        c.engine
                            .borrow()
                            .phase_sync_once(id, c.alignments.get()[Self::grid_index(id)])
                    });
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_status(
                            result
                                .map(|()| format!("Deck {id:?}: one-shot beat phase sync"))
                                .unwrap_or_else(|error| error)
                                .into(),
                        );
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_phase_sync_left(sync),
                DeckId::B => ui.on_phase_sync_right(sync),
            }
            let weak = Rc::downgrade(self);
            let match_tempo = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.match_tempo(id);
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_status(
                            result
                                .map(|()| if c.keylock_requested(id) {
                                    format!("Deck {id:?}: preparing Keylock tempo match; phase unchanged")
                                } else {
                                    format!("Deck {id:?}: tempo matched; phase unchanged")
                                })
                                .unwrap_or_else(|error| error)
                                .into(),
                        );
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_match_tempo_left(match_tempo),
                DeckId::B => ui.on_match_tempo_right(match_tempo),
            }
            let weak = Rc::downgrade(self);
            let range = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.change_pitch_range(id);
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_status(
                            result
                                .map(|()| "Pitch range changed; playback retained".to_string())
                                .unwrap_or_else(|error| error)
                                .into(),
                        );
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_pitch_range_left(range),
                DeckId::B => ui.on_pitch_range_right(range),
            }
            for up in [false, true] {
                let weak = Rc::downgrade(self);
                let action = move || {
                    if let Some(c) = weak.upgrade() {
                        let result = c.fine_pitch(id, up);
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(
                                result
                                    .map(|()| "Fine pitch queued".to_string())
                                    .unwrap_or_else(|error| error)
                                    .into(),
                            );
                        }
                    }
                };
                match (id, up) {
                    (DeckId::A, false) => ui.on_pitch_down_left(action),
                    (DeckId::B, false) => ui.on_pitch_down_right(action),
                    (DeckId::A, true) => ui.on_pitch_up_left(action),
                    (DeckId::B, true) => ui.on_pitch_up_right(action),
                }
            }
            let weak = Rc::downgrade(self);
            let change = move |value| {
                if let Some(c) = weak.upgrade() {
                    let result = c.queue_pitch(id, value);
                    if let Some(ui) = c.ui.upgrade() {
                        match result {
                            Ok(()) => ui.set_status(format!("Deck {id:?}: pitch queued").into()),
                            Err(error) => {
                                ui.set_status(error.into());
                                c.refresh();
                            }
                        }
                    }
                }
            };
            match id {
                DeckId::A => ui.on_pitch_left(change),
                DeckId::B => ui.on_pitch_right(change),
            }
            let weak = Rc::downgrade(self);
            let reset = move || {
                if let Some(c) = weak.upgrade() {
                    if let Err(error) = c.queue_pitch(id, 0.) {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(error.into());
                        }
                    }
                }
            };
            match id {
                DeckId::A => ui.on_reset_pitch_left(reset),
                DeckId::B => ui.on_reset_pitch_right(reset),
            }
        }
    }
    pub fn keylock_active(&self, id: DeckId) -> bool {
        self.engine.borrow().keylock_factor(id).is_some()
    }
    fn keylock_requested(&self, id: DeckId) -> bool {
        self.ui.upgrade().is_some_and(|ui| match id {
            DeckId::A => ui.get_left_keylock(),
            DeckId::B => ui.get_right_keylock(),
        })
    }
    fn start_keylock(&self, id: DeckId, tempo: f64) -> Result<(), String> {
        if self.pending_sync.borrow().is_some() {
            self.stop_sync();
        }
        if self.keylock_jobs.borrow()[Self::grid_index(id)].is_some() {
            let mut next = self.keylock_next.get();
            next[Self::grid_index(id)] = Some(tempo);
            self.keylock_next.set(next);
            return Ok(());
        }
        let source = self
            .engine
            .borrow()
            .snapshot(id)
            .detail
            .ok_or("Load a track first")?;
        self.stop_sync();
        self.cancel_nudge(id);
        let job = self.engine.borrow().prepare_keylock(id, 0, tempo)?;
        self.keylock_jobs.borrow_mut()[Self::grid_index(id)] = Some((source, job, tempo));
        if let Some(ui) = self.ui.upgrade() {
            ui.set_status(format!("Deck {id:?}: preparing Keylock tempo").into());
        }
        Ok(())
    }
    fn poll_keylock(&self) {
        self.cancel_invalid_pending_sync();
        for id in [DeckId::A, DeckId::B] {
            let index = Self::grid_index(id);
            let wanted = self.keylock_requested(id);
            if !wanted {
                self.keylock_jobs.borrow_mut()[index].take();
                let mut next = self.keylock_next.get();
                next[index] = None;
                self.keylock_next.set(next);
                if self.engine.borrow().keylock_factor(id).is_some() {
                    // Mode changes cancel following explicitly: replacement render versions restart at zero.
                    self.stop_sync();
                    self.cancel_nudge(id);
                    let _ = self.engine.borrow().adopt_keylock(id, None);
                }
                continue;
            }
            let ready = {
                let jobs = self.keylock_jobs.borrow();
                jobs[index]
                    .as_ref()
                    .and_then(|(_, job, _)| match job.poll() {
                        Ok(Some(result)) => Some(result.map_err(str::to_owned)),
                        Err(e) => Some(Err(e.into())),
                        Ok(None) => None,
                    })
            };
            let result = if let Some(result) = ready {
                let (source, _, _) = self.keylock_jobs.borrow_mut()[index].take().unwrap();
                if self
                    .engine
                    .borrow()
                    .snapshot(id)
                    .detail
                    .as_ref()
                    .is_none_or(|current| !Arc::ptr_eq(current, &source))
                {
                    let mut next = self.keylock_next.get();
                    next[index] = None;
                    self.keylock_next.set(next);
                    continue;
                }
                let mut next = self.keylock_next.get();
                let queued = next[index].take();
                self.keylock_next.set(next);
                if let Some(tempo) = queued {
                    if let Err(error) = self.start_keylock(id, tempo) {
                        if let Some(ui) = self.ui.upgrade() {
                            ui.set_status(error.into());
                        }
                    }
                    continue;
                }
                result.and_then(|result| {
                    self.engine
                        .borrow()
                        .adopt_keylock(id, Some((result.tempo, result.clip)))
                        .inspect(|()| {
                            if let Some(pending) = self
                                .pending_sync
                                .borrow_mut()
                                .as_mut()
                                .filter(|s| s.session.target == id)
                            {
                                pending.prepared = true;
                                pending.session.versions[Self::grid_index(id)] =
                                    self.engine.borrow().command_version(id).unwrap();
                            }
                            if let Some(ui) = self.ui.upgrade() {
                                ui.set_status(format!("Deck {id:?}: Keylock tempo ready").into());
                            }
                        })
                })
            } else if self.keylock_jobs.borrow()[index].is_none()
                && self.engine.borrow().keylock_factor(id).is_none()
            {
                self.start_keylock(id, self.engine.borrow().snapshot(id).requested_speed)
            } else {
                continue;
            };
            if let Some(ui) = self.ui.upgrade() {
                if let Err(error) = result {
                    if self
                        .pending_sync
                        .borrow()
                        .as_ref()
                        .is_some_and(|s| s.session.target == id)
                    {
                        self.stop_sync();
                    }
                    let enabled = self.engine.borrow().keylock_factor(id).is_some();
                    match id {
                        DeckId::A => ui.set_left_keylock(enabled),
                        DeckId::B => ui.set_right_keylock(enabled),
                    }
                    ui.set_status(format!("Keylock: {error}").into());
                }
            }
        }
    }
    fn apply_pending_pitch(&self) {
        for id in [DeckId::A, DeckId::B] {
            let Some(request) = self.pending_pitch.borrow_mut()[Self::grid_index(id)].take() else {
                continue;
            };
            let result = if !self.can_control_deck(id) {
                Err("Pending pitch discarded during loading".into())
            } else if self
                .engine
                .borrow()
                .snapshot(id)
                .detail
                .as_ref()
                .is_none_or(|source| !Arc::ptr_eq(source, &request.source))
            {
                Err("Pending pitch discarded after track replacement".into())
            } else {
                if self.keylock_requested(id) {
                    self.start_keylock(id, request.factor)
                } else {
                    self.engine.borrow().set_speed(id, request.factor)
                }
            };
            if let Some(ui) = self.ui.upgrade() {
                ui.set_status(
                    match result {
                        Ok(()) => format!("Deck {id:?}: pitch applied"),
                        Err(error) => error,
                    }
                    .into(),
                );
            }
        }
    }
    fn wire_loops(self: &Rc<Self>, ui: &AppWindow) {
        for id in [DeckId::A, DeckId::B] {
            for command in 0..8 {
                let weak = Rc::downgrade(self);
                let action = move || {
                    if let Some(c) = weak.upgrade() {
                        let result = if !c.can_control_deck(id) {
                            Err("Finish loading before changing a loop".into())
                        } else {
                            let position = c.engine.borrow().snapshot(id).position;
                            match command {
                                0 => c.engine.borrow().set_loop_in(id, position),
                                1 => c.engine.borrow().set_loop_out(id, position),
                                2 => {
                                    if c.engine.borrow().snapshot(id).loop_active {
                                        c.engine.borrow().exit_loop(id)
                                    } else {
                                        c.engine.borrow().activate_loop(id)
                                    }
                                }
                                3 => c.engine.borrow().exit_loop(id),
                                4 => {
                                    let index =
                                        c.ui.upgrade()
                                            .map(|ui| match id {
                                                DeckId::A => ui.get_left_auto_loop_index(),
                                                DeckId::B => ui.get_right_auto_loop_index(),
                                            })
                                            .unwrap_or(-1);
                                    let snap = c.ui.upgrade().is_some_and(|ui| {
                                        ui.get_quantize_enabled()
                                            && match id {
                                                DeckId::A => ui.get_left_auto_loop_snap(),
                                                DeckId::B => ui.get_right_auto_loop_snap(),
                                            }
                                    });
                                    usize::try_from(index)
                                        .ok()
                                        .and_then(|i| {
                                            [1, 2, 4, 8, 16, 32, 64, 128, 256].get(i).copied()
                                        })
                                        .ok_or_else(|| "Choose a supported beat length".to_string())
                                        .and_then(|eighths| {
                                            c.engine.borrow().auto_loop(id, eighths, snap)
                                        })
                                }
                                5 => c.engine.borrow().resize_loop(id, false),
                                6 => c.engine.borrow().resize_loop(id, true),
                                _ => c.engine.borrow().reloop(id),
                            }
                        };
                        if result.is_ok() {
                            c.cancel_nudge(id);
                        }
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(
                                match result {
                                    Ok(()) => format!(
                                        "Deck {id:?}: {}",
                                        [
                                            "Loop In set",
                                            "Loop Out set",
                                            "Loop toggled",
                                            "Loop exited",
                                            "Auto loop active",
                                            "Loop halved",
                                            "Loop doubled",
                                            "Reloop from In"
                                        ][command]
                                    ),
                                    Err(e) => format!("Deck {id:?}: {e}"),
                                }
                                .into(),
                            );
                        }
                        c.refresh();
                    }
                };
                match (id, command) {
                    (DeckId::A, 0) => ui.on_loop_in_left(action),
                    (DeckId::B, 0) => ui.on_loop_in_right(action),
                    (DeckId::A, 1) => ui.on_loop_out_left(action),
                    (DeckId::B, 1) => ui.on_loop_out_right(action),
                    (DeckId::A, 2) => ui.on_loop_activate_left(action),
                    (DeckId::B, 2) => ui.on_loop_activate_right(action),
                    (DeckId::A, 3) => ui.on_loop_exit_left(action),
                    (DeckId::B, 3) => ui.on_loop_exit_right(action),
                    (DeckId::A, 4) => ui.on_auto_loop_left(action),
                    (DeckId::B, 4) => ui.on_auto_loop_right(action),
                    (DeckId::A, 5) => ui.on_half_loop_left(action),
                    (DeckId::B, 5) => ui.on_half_loop_right(action),
                    (DeckId::A, 6) => ui.on_double_loop_left(action),
                    (DeckId::B, 6) => ui.on_double_loop_right(action),
                    (DeckId::A, _) => ui.on_reloop_left(action),
                    (DeckId::B, _) => ui.on_reloop_right(action),
                }
            }
        }
    }
    fn cancel_cue_editor(&self, id: DeckId) {
        self.cue_editors.borrow_mut()[Self::grid_index(id)].take();
        if let Some(ui) = self.ui.upgrade() {
            match id {
                DeckId::A => ui.set_left_cue_editing(false),
                DeckId::B => ui.set_right_cue_editing(false),
            }
        }
    }
    fn begin_cue_editor(&self, id: DeckId, slot: i32) -> Result<(), String> {
        if !self.can_control_deck(id) {
            return Err("Finish loading before editing a cue".into());
        }
        let index = Self::grid_index(id);
        if self.cue_editors.borrow()[index].is_some() {
            return Err("Save or cancel the existing cue draft".into());
        }
        let slot = usize::try_from(slot)
            .ok()
            .filter(|s| *s < crate::hot_cues::SLOT_COUNT)
            .ok_or("Invalid cue slot")?;
        let snapshot = self.engine.borrow().snapshot(id);
        let path = snapshot.path.ok_or("Load a track first")?;
        let source = snapshot.detail.ok_or("Track source unavailable")?;
        let cue = self.library.borrow().hot_cues(&path).slots()[slot]
            .clone()
            .ok_or("Set a cue before editing it")?;
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        match id {
            DeckId::A => {
                ui.set_left_cue_name(cue.name().into());
                ui.set_left_cue_rgb(format!("{:06X}", cue.rgb()).into());
                ui.set_left_cue_editing(true);
            }
            DeckId::B => {
                ui.set_right_cue_name(cue.name().into());
                ui.set_right_cue_rgb(format!("{:06X}", cue.rgb()).into());
                ui.set_right_cue_editing(true);
            }
        }
        self.cue_editors.borrow_mut()[index] = Some(CueSession {
            path,
            source,
            slot,
            original: cue,
        });
        Ok(())
    }
    fn save_cue_editor(&self, id: DeckId) -> Result<(), String> {
        if !self.can_control_deck(id) {
            return Err("Finish loading before saving a cue".into());
        }
        let snapshot = self.engine.borrow().snapshot(id);
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let (name, rgb) = match id {
            DeckId::A => (ui.get_left_cue_name(), ui.get_left_cue_rgb()),
            DeckId::B => (ui.get_right_cue_name(), ui.get_right_cue_rgb()),
        };
        let rgb = rgb.strip_prefix('#').unwrap_or(&rgb);
        if rgb.len() != 6 || !rgb.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Color must be six RGB hex digits, optionally prefixed by #".into());
        }
        let rgb = u32::from_str_radix(rgb, 16).map_err(|e| e.to_string())?;
        {
            let editors = self.cue_editors.borrow();
            let edit = editors[Self::grid_index(id)]
                .as_ref()
                .ok_or("Open a cue editor first")?;
            if snapshot
                .detail
                .as_ref()
                .is_none_or(|source| !Arc::ptr_eq(source, &edit.source))
                || self.library.borrow().hot_cues(&edit.path).slots()[edit.slot].as_ref()
                    != Some(&edit.original)
            {
                return Err("Cue or source changed; cancel and reopen the editor".into());
            }
            let value =
                crate::hot_cues::HotCue::new(edit.original.position(), name.to_string(), rgb)?;
            self.library
                .borrow_mut()
                .set_hot_cue(&edit.path, edit.slot, Some(value))?;
        }
        self.library_changed();
        self.cancel_cue_editor(id);
        Ok(())
    }
    fn change_hot_cue(&self, id: DeckId, slot: i32, delete: bool) -> Result<(), String> {
        if !self.can_control_deck(id) {
            return Err("Finish loading before using hot cues".into());
        }
        let slot = usize::try_from(slot)
            .ok()
            .filter(|s| *s < crate::hot_cues::SLOT_COUNT)
            .ok_or("Cue slot must be 1 through 8")?;
        let source = self.engine.borrow().snapshot(id);
        let path = source.path.ok_or("Load a track first")?;
        let cues = self.library.borrow().hot_cues(&path);
        if !delete {
            if let Some(cue) = &cues.slots()[slot] {
                if cue.position() >= source.duration {
                    return Err("Saved cue is outside the loaded track; delete and reset it".into());
                }
                let ui = self.ui.upgrade().ok_or("Window closed")?;
                let scheduled = ui.get_quantize_enabled()
                    && match id {
                        DeckId::A => ui.get_left_quantize_triggers(),
                        DeckId::B => ui.get_right_quantize_triggers(),
                    };
                if scheduled {
                    if self.sync_session.borrow().is_some()
                        || self.pending_pitch.borrow()[Self::grid_index(id)].is_some()
                    {
                        return Err(
                            "Stop Sync and finish pitch changes before scheduling a cue".into()
                        );
                    }
                    if self.grid_editors.borrow()[Self::grid_index(id)].is_some() {
                        return Err("Save or cancel the grid draft first".into());
                    }
                    let step = crate::beatgrid::QuantizeStep::try_from(match id {
                        DeckId::A => ui.get_left_quantize_step(),
                        DeckId::B => ui.get_right_quantize_step(),
                    })?;
                    self.engine
                        .borrow()
                        .schedule_cue(id, cue.position(), step)?;
                } else {
                    self.engine.borrow().seek_position(id, cue.position())?;
                }
                if !scheduled {
                    self.cancel_nudge(id);
                }
                return Ok(());
            }
        } else if cues.slots()[slot].is_none() {
            return Err("Cue slot is empty".into());
        }
        let value = if delete {
            None
        } else {
            if source.position >= source.duration {
                return Err("A cue needs a playable position before the end of the track".into());
            }
            let ui = self.ui.upgrade().ok_or("Window closed")?;
            let quantize = ui.get_quantize_enabled()
                && match id {
                    DeckId::A => ui.get_left_quantize_cues(),
                    DeckId::B => ui.get_right_quantize_cues(),
                };
            let position = if quantize {
                if self.grid_editors.borrow()[Self::grid_index(id)].is_some() {
                    return Err("Save or cancel the grid draft first".into());
                }
                let grid = source
                    .grid
                    .ok_or("Save a manual grid before quantizing cues")?;
                let rate = source
                    .detail
                    .as_ref()
                    .ok_or("Missing source details")?
                    .sample_rate();
                let step = crate::beatgrid::QuantizeStep::try_from(match id {
                    DeckId::A => ui.get_left_quantize_step(),
                    DeckId::B => ui.get_right_quantize_step(),
                })?;
                let frame = grid.nearest_quantized_frame(source.position, rate, step)?;
                let time = crate::looping::frame_time(frame, rate);
                if time >= source.duration {
                    return Err("Nearest grid step is outside the playable track".into());
                }
                time
            } else {
                source.position
            };
            const COLORS: [u32; 8] = [
                0xe74c3c, 0xf39c12, 0xf1c40f, 0x2ecc71, 0x1abc9c, 0x3498db, 0x9b59b6, 0xe84393,
            ];
            Some(crate::hot_cues::HotCue::new(
                position,
                format!("Cue {}", slot + 1),
                COLORS[slot],
            )?)
        };
        let inserted = self.library.borrow_mut().insert([path.to_path_buf()]);
        self.library.borrow_mut().set_hot_cue(&path, slot, value)?;
        self.library_changed();
        if inserted > 0 {
            self.publish_library();
        }
        Ok(())
    }
    fn hold_hot_cue(&self, id: DeckId, slot: i32, held: bool, input: i32) -> Result<(), String> {
        let slot = usize::try_from(slot)
            .ok()
            .filter(|slot| *slot < crate::hot_cues::SLOT_COUNT)
            .ok_or("Cue slot must be 1 through 8")?;
        let input = u16::try_from(input)
            .ok()
            .filter(|input| (1..=16).contains(input))
            .ok_or("Invalid cue input")?;
        let input_mask = 1u16 << (input - 1);
        let index = Self::grid_index(id);
        if !held {
            let session = {
                let mut sessions = self.hot_cue_holds.borrow_mut();
                let Some(session) = sessions[index].as_mut() else {
                    return Ok(());
                };
                if session.slot != slot || session.inputs & input_mask == 0 {
                    return Ok(());
                }
                session.inputs &= !input_mask;
                (session.inputs == 0)
                    .then(|| sessions[index].take())
                    .flatten()
            };
            if let Some(session) = session {
                self.finish_hot_cue_session(id, session)?;
            }
            return Ok(());
        }
        if !self.can_control_deck(id) {
            return Err("Finish loading before using hot cues".into());
        }
        if let Some(session) = self.hot_cue_holds.borrow_mut()[index].as_mut() {
            if session.slot != slot {
                return Err("Release the current hot cue before triggering another".into());
            }
            session.inputs |= input_mask;
            return Ok(());
        }
        let snapshot = self.engine.borrow().snapshot(id);
        let path = snapshot.path.ok_or("Load a track first")?;
        let cue = self.library.borrow().hot_cues(&path).slots()[slot].clone();
        let Some(cue) = cue else {
            self.change_hot_cue(id, slot as i32, false)?;
            let position = self.library.borrow().hot_cues(&path).slots()[slot]
                .as_ref()
                .ok_or("Cue was not saved")?
                .position();
            self.hot_cue_holds.borrow_mut()[index] = Some(HotCueHoldSession {
                source: snapshot.detail.ok_or("Track source unavailable")?,
                slot,
                position,
                inputs: input_mask,
                preview: false,
                promoted: false,
            });
            return Ok(());
        };
        if snapshot.playing {
            self.change_hot_cue(id, slot as i32, false)?;
            self.hot_cue_holds.borrow_mut()[index] = Some(HotCueHoldSession {
                source: snapshot.detail.ok_or("Track source unavailable")?,
                slot,
                position: cue.position(),
                inputs: input_mask,
                preview: false,
                promoted: false,
            });
            return Ok(());
        }
        if cue.position() >= snapshot.duration {
            return Err("Saved cue is outside the loaded track; delete and reset it".into());
        }
        let source = snapshot.detail.ok_or("Track source unavailable")?;
        self.engine.borrow().seek_position(id, cue.position())?;
        self.engine.borrow().play(id);
        self.hot_cue_holds.borrow_mut()[index] = Some(HotCueHoldSession {
            source,
            slot,
            position: cue.position(),
            inputs: input_mask,
            preview: true,
            promoted: false,
        });
        Ok(())
    }
    fn finish_hot_cue_session(&self, id: DeckId, session: HotCueHoldSession) -> Result<(), String> {
        if session.preview
            && !session.promoted
            && self
                .engine
                .borrow()
                .snapshot(id)
                .detail
                .as_ref()
                .is_some_and(|source| Arc::ptr_eq(source, &session.source))
        {
            let engine = self.engine.borrow();
            engine.pause(id);
            engine.seek_position(id, session.position)?;
        }
        Ok(())
    }
    fn cancel_hot_cue_holds(&self) -> Result<(), String> {
        let mut first_error = None;
        for id in [DeckId::A, DeckId::B] {
            if let Some(session) = self.hot_cue_holds.borrow_mut()[Self::grid_index(id)].take() {
                if let Err(error) = self.finish_hot_cue_session(id, session) {
                    first_error.get_or_insert(error);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }
    pub(crate) fn handle_window_deactivated(&self) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_active_hot_cue_key(-1);
        }
        if let Err(error) = self.cancel_hot_cue_holds() {
            if let Some(ui) = self.ui.upgrade() {
                ui.set_status(error.into());
            }
        }
    }
    pub(crate) fn handle_global_hot_cue_key_released(&self, key: i32) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if ui.get_active_hot_cue_key() != key {
            return;
        }
        ui.set_active_hot_cue_key(-1);
        let (id, slot) = if key < 5 {
            (DeckId::A, key)
        } else {
            (DeckId::B, key - 5)
        };
        if let Err(error) = self.hold_hot_cue(id, slot, false, 5) {
            ui.set_status(error.into());
        }
    }
    fn promote_hot_cue_hold(&self, id: DeckId) -> bool {
        let index = Self::grid_index(id);
        let mut sessions = self.hot_cue_holds.borrow_mut();
        let Some(session) = sessions[index].as_mut() else {
            return false;
        };
        let snapshot = self.engine.borrow().snapshot(id);
        if snapshot
            .detail
            .as_ref()
            .is_none_or(|source| !Arc::ptr_eq(source, &session.source))
        {
            sessions[index] = None;
            return false;
        }
        if !session.preview || session.promoted {
            return false;
        }
        session.promoted = true;
        if !snapshot.playing {
            self.engine.borrow().play(id);
        }
        drop(sessions);
        self.record_history(id);
        true
    }
    fn cancel_stale_hot_cue_holds(&self) {
        let mut sessions = self.hot_cue_holds.borrow_mut();
        for id in [DeckId::A, DeckId::B] {
            let index = Self::grid_index(id);
            if sessions[index].as_ref().is_some_and(|session| {
                self.engine
                    .borrow()
                    .snapshot(id)
                    .detail
                    .as_ref()
                    .is_none_or(|source| !Arc::ptr_eq(source, &session.source))
            }) {
                sessions[index] = None;
            }
        }
    }
    fn wire_hot_cues(self: &Rc<Self>, ui: &AppWindow) {
        let engine = Rc::clone(&self.engine);
        let weak_ui = ui.as_weak();
        ui.on_quantize_mode_changed(move || {
            if weak_ui
                .upgrade()
                .is_some_and(|ui| !ui.get_quantize_enabled())
            {
                // Globales Bypass verwirft nur wartende Trigger, keine Deck-Präferenzen.
                engine.borrow().cancel_scheduled_cue(DeckId::A);
                engine.borrow().cancel_scheduled_cue(DeckId::B);
            }
        });
        for id in [DeckId::A, DeckId::B] {
            let engine = Rc::clone(&self.engine);
            let cancel = move || engine.borrow().cancel_scheduled_cue(id);
            match id {
                DeckId::A => ui.on_cancel_trigger_left(cancel),
                DeckId::B => ui.on_cancel_trigger_right(cancel),
            }
        }
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(self);
            let hold = move |slot, held, input| {
                if let Some(c) = weak.upgrade() {
                    let result = c.hold_hot_cue(id, slot, held, input);
                    if let Err(error) = result {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(format!("Deck {id:?}: {error}").into());
                        }
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_hold_hot_cue_left(hold),
                DeckId::B => ui.on_hold_hot_cue_right(hold),
            }
        }
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(self);
            let edit = move |slot| {
                if let Some(c) = weak.upgrade() {
                    if let Err(error) = c.begin_cue_editor(id, slot) {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(error.into());
                        }
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_edit_hot_cue_left(edit),
                DeckId::B => ui.on_edit_hot_cue_right(edit),
            }
            let weak = Rc::downgrade(self);
            let save = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.save_cue_editor(id);
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_status(match result {
                            Ok(()) => if c.persistence.borrow().is_some() {
                                "Cue updated; save queued"
                            } else {
                                "Cue updated for this session only"
                            }
                            .into(),
                            Err(e) => e.into(),
                        });
                    }
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_save_hot_cue_left(save),
                DeckId::B => ui.on_save_hot_cue_right(save),
            }
            let weak = Rc::downgrade(self);
            let cancel = move || {
                if let Some(c) = weak.upgrade() {
                    c.cancel_cue_editor(id);
                    c.refresh();
                }
            };
            match id {
                DeckId::A => ui.on_cancel_hot_cue_left(cancel),
                DeckId::B => ui.on_cancel_hot_cue_right(cancel),
            }
            for delete in [false, true] {
                let weak = Rc::downgrade(self);
                let action = move |slot| {
                    if let Some(c) = weak.upgrade() {
                        let result = c.change_hot_cue(id, slot, delete);
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(
                                match result {
                                    Ok(()) => format!(
                                        "Deck {id:?}: hot cue {} {}",
                                        slot + 1,
                                        if delete { "deleted" } else { "set / recalled" }
                                    ),
                                    Err(error) => format!("Deck {id:?}: {error}"),
                                }
                                .into(),
                            );
                        }
                        c.refresh();
                    }
                };
                match (id, delete) {
                    (DeckId::A, false) => ui.on_hot_cue_left(action),
                    (DeckId::B, false) => ui.on_hot_cue_right(action),
                    (DeckId::A, true) => ui.on_delete_hot_cue_left(action),
                    (DeckId::B, true) => ui.on_delete_hot_cue_right(action),
                }
            }
        }
    }
    fn begin_grid_editor(&self, id: DeckId) -> Result<(), String> {
        if !self.can_control_deck(id) {
            return Err("Finish library loading before editing its grid".into());
        }
        if self.grid_editors.borrow()[Self::grid_index(id)].is_some() {
            return Err("Save or cancel the existing grid draft first".into());
        }
        let snapshot = self.engine.borrow().snapshot(id);
        let path = snapshot
            .path
            .ok_or("Load a track before editing its grid")?;
        let source = snapshot.detail.ok_or("Track waveform is unavailable")?;
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let seed = match id {
            DeckId::A => ui.get_left_grid_bpm(),
            DeckId::B => ui.get_right_grid_bpm(),
        };
        let draft = crate::beatgrid::GridDraft::new(
            snapshot.grid,
            f64::from(seed),
            snapshot.position,
            snapshot.duration,
        )?;
        let text = draft.bpm().to_string().into();
        match id {
            DeckId::A => ui.set_left_grid_draft_bpm(text),
            DeckId::B => ui.set_right_grid_draft_bpm(text),
        }
        let stored_base = self.library.borrow().grid(&path);
        self.grid_editors.borrow_mut()[Self::grid_index(id)] = Some(GridSession {
            path,
            source,
            stored_base,
            draft,
        });
        let signature = self.grid_editors.borrow()[Self::grid_index(id)]
            .as_ref()
            .and_then(|e| e.draft.value())
            .map_or((4, 1), |grid| {
                (
                    i32::from(grid.beats_per_bar()),
                    Self::meter_denominator_index(grid.meter_denominator()),
                )
            });
        match id {
            DeckId::A => {
                ui.set_left_grid_draft_beats(signature.0);
                ui.set_left_grid_draft_denominator_index(signature.1);
            }
            DeckId::B => {
                ui.set_right_grid_draft_beats(signature.0);
                ui.set_right_grid_draft_denominator_index(signature.1);
            }
        }
        let tempo = self.grid_editors.borrow()[Self::grid_index(id)]
            .as_ref()
            .and_then(|editor| editor.draft.value())
            .map(|grid| {
                grid.tempo_changes()
                    .first()
                    .map_or((1, grid.bpm()), |change| (change.beat as i32, change.bpm))
            })
            .ok_or("Grid draft was cleared while opening the editor");
        let tempo = match tempo {
            Ok(tempo) => tempo,
            Err(error) => {
                self.grid_editors.borrow_mut()[Self::grid_index(id)].take();
                return Err(error.into());
            }
        };
        let tempo_bpm = tempo.1.to_string().into();
        match id {
            DeckId::A => {
                ui.set_left_grid_tempo_beat(tempo.0);
                ui.set_left_grid_tempo_bpm(tempo_bpm);
            }
            DeckId::B => {
                ui.set_right_grid_tempo_beat(tempo.0);
                ui.set_right_grid_tempo_bpm(tempo_bpm);
            }
        }
        let warp_beat = self.grid_editors.borrow()[Self::grid_index(id)]
            .as_ref()
            .and_then(|editor| editor.draft.value())
            .and_then(|grid| grid.warp_markers().first().map(|marker| marker.beat))
            .unwrap_or(1)
            .min(i32::MAX as u64) as i32;
        match id {
            DeckId::A => ui.set_left_grid_warp_beat(warp_beat),
            DeckId::B => ui.set_right_grid_warp_beat(warp_beat),
        }
        self.set_grid_editing(id, true);
        Ok(())
    }
    fn change_grid_draft(&self, id: DeckId, action: GridAction) -> Result<(), String> {
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let mut editors = self.grid_editors.borrow_mut();
        let editor = editors[Self::grid_index(id)]
            .as_mut()
            .ok_or("Open Edit Grid first")?;
        let mut draft = editor.draft;
        let result = (|| {
            if !matches!(action, GridAction::Clear) {
                let (beats, denominator_index) = match id {
                    DeckId::A => (
                        ui.get_left_grid_draft_beats(),
                        ui.get_left_grid_draft_denominator_index(),
                    ),
                    DeckId::B => (
                        ui.get_right_grid_draft_beats(),
                        ui.get_right_grid_draft_denominator_index(),
                    ),
                };
                draft.set_base_meter(
                    u8::try_from(beats).map_err(|_| "Invalid beats per bar")?,
                    Self::meter_denominator_from_index(denominator_index)?,
                )?;
                draft.set_bpm(&Self::grid_bpm_text(&ui, id))?;
            }
            match action {
                GridAction::Tempo => Ok(()),
                GridAction::Anchor => draft.anchor_here(self.engine.borrow().snapshot(id).position),
                GridAction::Nudge(ms) => draft.nudge(ms),
                GridAction::MeterSet | GridAction::MeterRemove => {
                    let (bar, beats, denominator_index) = match id {
                        DeckId::A => (
                            ui.get_left_grid_meter_bar(),
                            ui.get_left_grid_meter_beats(),
                            ui.get_left_grid_meter_denominator_index(),
                        ),
                        DeckId::B => (
                            ui.get_right_grid_meter_bar(),
                            ui.get_right_grid_meter_beats(),
                            ui.get_right_grid_meter_denominator_index(),
                        ),
                    };
                    let bar = u32::try_from(bar).map_err(|_| "Invalid meter bar")?;
                    if matches!(action, GridAction::MeterSet) {
                        draft.set_meter_signature_change(
                            bar,
                            u8::try_from(beats).map_err(|_| "Invalid meter")?,
                            Self::meter_denominator_from_index(denominator_index)?,
                        )
                    } else {
                        draft.remove_meter_change(bar)
                    }
                }
                GridAction::TempoSet | GridAction::TempoRemove => {
                    let (beat, bpm) = match id {
                        DeckId::A => (ui.get_left_grid_tempo_beat(), ui.get_left_grid_tempo_bpm()),
                        DeckId::B => (
                            ui.get_right_grid_tempo_beat(),
                            ui.get_right_grid_tempo_bpm(),
                        ),
                    };
                    let beat = u64::try_from(beat).map_err(|_| "Invalid tempo-change beat")?;
                    if matches!(action, GridAction::TempoSet) {
                        draft.set_tempo_change(beat, &bpm)
                    } else {
                        draft.remove_tempo_change(beat)
                    }
                }
                GridAction::WarpSet | GridAction::WarpRemove => {
                    let beat = match id {
                        DeckId::A => ui.get_left_grid_warp_beat(),
                        DeckId::B => ui.get_right_grid_warp_beat(),
                    };
                    let beat = u64::try_from(beat).map_err(|_| "Invalid warp-marker beat")?;
                    if matches!(action, GridAction::WarpSet) {
                        draft.set_warp_marker(beat, self.engine.borrow().snapshot(id).position)
                    } else {
                        draft.remove_warp_marker(beat)
                    }
                }
                GridAction::Clear => {
                    draft.clear();
                    Ok(())
                }
            }
        })();
        if result.is_ok() {
            editor.draft = draft;
        }
        result
    }
    fn apply_saved_grid(&self, id: DeckId) -> Result<(), String> {
        if let Some(path) = self.engine.borrow().snapshot(id).path {
            self.engine
                .borrow()
                .set_grid(id, self.library.borrow().grid(&path))?;
        }
        Ok(())
    }
    fn save_grid_editor(&self, id: DeckId) -> Result<(), String> {
        if !self.can_control_deck(id) {
            return Err("Finish library loading before saving its grid".into());
        }
        let snapshot = self.engine.borrow().snapshot(id);
        let index = Self::grid_index(id);
        let (path, value) = {
            let mut editors = self.grid_editors.borrow_mut();
            let editor = editors[index].as_mut().ok_or("Open Edit Grid first")?;
            if snapshot
                .detail
                .as_ref()
                .is_none_or(|source| !Arc::ptr_eq(source, &editor.source))
            {
                return Err("Track changed; cancel and reopen Edit Grid".into());
            }
            if self.library.borrow().grid(&editor.path) != editor.stored_base
                || snapshot.grid != editor.draft.original()
            {
                return Err("Saved grid changed; cancel and reopen Edit Grid".into());
            }
            let meter_ui = self.ui.upgrade().ok_or("Window closed")?;
            let (beats, denominator_index) = match id {
                DeckId::A => (
                    meter_ui.get_left_grid_draft_beats(),
                    meter_ui.get_left_grid_draft_denominator_index(),
                ),
                DeckId::B => (
                    meter_ui.get_right_grid_draft_beats(),
                    meter_ui.get_right_grid_draft_denominator_index(),
                ),
            };
            editor.draft.set_base_meter(
                u8::try_from(beats).map_err(|_| "Invalid beats per bar")?,
                Self::meter_denominator_from_index(denominator_index)?,
            )?;
            editor.draft.set_bpm(&Self::grid_bpm_text(
                &self.ui.upgrade().ok_or("Window closed")?,
                id,
            ))?;
            (Arc::clone(&editor.path), editor.draft.value())
        };
        let inserted = self.library.borrow_mut().insert([path.to_path_buf()]);
        self.library.borrow_mut().set_grid(&path, value)?;
        self.engine.borrow().set_grid(id, value)?;
        self.library_changed();
        self.cancel_grid_editor(id);
        if inserted > 0 {
            self.publish_library();
        }
        let other = if id == DeckId::A {
            DeckId::B
        } else {
            DeckId::A
        };
        if self
            .engine
            .borrow()
            .snapshot(other)
            .path
            .as_ref()
            .is_some_and(|p| crate::library::identity(p) == crate::library::identity(&path))
        {
            self.engine
                .borrow()
                .set_grid(other, value)
                .map_err(|e| format!("Grid saved; other deck: {e}"))?;
        }
        Ok(())
    }
    fn wire_grid_editor(self: &Rc<Self>, ui: &AppWindow) {
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(self);
            let open = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.begin_grid_editor(id);
                    c.grid_feedback(result, "Grid draft opened; Save or Cancel");
                }
            };
            match id {
                DeckId::A => ui.on_grid_left(open),
                DeckId::B => ui.on_grid_right(open),
            }
            for action in [
                GridAction::Tempo,
                GridAction::Anchor,
                GridAction::Nudge(-10),
                GridAction::Nudge(10),
                GridAction::MeterSet,
                GridAction::MeterRemove,
                GridAction::TempoSet,
                GridAction::TempoRemove,
                GridAction::WarpSet,
                GridAction::WarpRemove,
                GridAction::Clear,
            ] {
                let weak = Rc::downgrade(self);
                let change = move || {
                    if let Some(c) = weak.upgrade() {
                        let result = if matches!(action, GridAction::Clear)
                            && c.grid_editors.borrow()[Self::grid_index(id)].is_none()
                        {
                            c.begin_grid_editor(id)
                                .and_then(|()| c.change_grid_draft(id, action))
                        } else {
                            c.change_grid_draft(id, action)
                        };
                        c.grid_feedback(result, "Grid draft updated; Save or Cancel");
                    }
                };
                match (id, action) {
                    (DeckId::A, GridAction::Tempo) => ui.on_edit_grid_bpm_left(change),
                    (DeckId::B, GridAction::Tempo) => ui.on_edit_grid_bpm_right(change),
                    (DeckId::A, GridAction::Anchor) => ui.on_grid_anchor_left(change),
                    (DeckId::B, GridAction::Anchor) => ui.on_grid_anchor_right(change),
                    (DeckId::A, GridAction::Nudge(-10)) => ui.on_grid_earlier_left(change),
                    (DeckId::B, GridAction::Nudge(-10)) => ui.on_grid_earlier_right(change),
                    (DeckId::A, GridAction::Nudge(_)) => ui.on_grid_later_left(change),
                    (DeckId::B, GridAction::Nudge(_)) => ui.on_grid_later_right(change),
                    (DeckId::A, GridAction::MeterSet) => ui.on_grid_meter_set_left(change),
                    (DeckId::B, GridAction::MeterSet) => ui.on_grid_meter_set_right(change),
                    (DeckId::A, GridAction::MeterRemove) => ui.on_grid_meter_remove_left(change),
                    (DeckId::B, GridAction::MeterRemove) => ui.on_grid_meter_remove_right(change),
                    (DeckId::A, GridAction::TempoSet) => ui.on_grid_tempo_set_left(change),
                    (DeckId::B, GridAction::TempoSet) => ui.on_grid_tempo_set_right(change),
                    (DeckId::A, GridAction::TempoRemove) => ui.on_grid_tempo_remove_left(change),
                    (DeckId::B, GridAction::TempoRemove) => ui.on_grid_tempo_remove_right(change),
                    (DeckId::A, GridAction::WarpSet) => ui.on_grid_warp_set_left(change),
                    (DeckId::B, GridAction::WarpSet) => ui.on_grid_warp_set_right(change),
                    (DeckId::A, GridAction::WarpRemove) => ui.on_grid_warp_remove_left(change),
                    (DeckId::B, GridAction::WarpRemove) => ui.on_grid_warp_remove_right(change),
                    (DeckId::A, GridAction::Clear) => ui.on_clear_grid_left(change),
                    (DeckId::B, GridAction::Clear) => ui.on_clear_grid_right(change),
                }
            }
            let weak = Rc::downgrade(self);
            let save = move || {
                if let Some(c) = weak.upgrade() {
                    let result = c.save_grid_editor(id);
                    let message = if c.persistence.borrow().is_some() {
                        "Grid applied; catalog save queued"
                    } else {
                        "Grid applied for this session only"
                    };
                    c.grid_feedback(result, message);
                }
            };
            match id {
                DeckId::A => ui.on_save_grid_left(save),
                DeckId::B => ui.on_save_grid_right(save),
            }
            let weak = Rc::downgrade(self);
            let cancel = move || {
                if let Some(c) = weak.upgrade() {
                    c.cancel_grid_editor(id);
                    c.grid_feedback(Ok(()), "Grid draft discarded");
                }
            };
            match id {
                DeckId::A => ui.on_cancel_grid_left(cancel),
                DeckId::B => ui.on_cancel_grid_right(cancel),
            }
        }
    }
    fn grid_feedback(&self, result: Result<(), String>, message: &str) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_status(result.map_or_else(|e| e, |()| message.into()).into());
        }
        self.refresh();
    }
    pub fn restore_library(&self, path: Result<PathBuf, String>, initial_paths: Vec<PathBuf>) {
        self.fx_writable.set(false);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_fx_status("FX session only".into());
        }
        let result = path.and_then(crate::library_store::Worker::start);
        match result {
            Ok(worker) => {
                *self.persistence.borrow_mut() = Some(worker);
                self.storage_ready.set(false);
                *self.initial_paths.borrow_mut() = initial_paths;
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_busy(true);
                    ui.set_quantize_preferences_ready(false);
                    ui.set_library_storage("Restoring library…".into());
                    ui.set_fx_status("Restoring FX…".into());
                }
            }
            Err(error) => {
                self.storage_error(&error, false);
                if !initial_paths.is_empty() {
                    self.import_paths(initial_paths);
                }
            }
        }
    }
    fn storage_error(&self, error: &str, retry: bool) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_library_storage(format!("Library NOT saved: {error}").into());
            ui.set_library_save_failed(retry);
        }
    }
    fn library_changed(&self) {
        self.dirty.set(true);
        self.save_failed.set(false);
    }
    fn record_history(&self, id: DeckId) {
        let Some(path) = self.engine.borrow().snapshot(id).path else {
            return;
        };
        let played_at_ms = match std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        {
            Some(timestamp) => timestamp,
            None => {
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_status("Cannot record playback history: invalid system time".into());
                }
                return;
            }
        };
        let deck = u8::from(id == DeckId::B);
        let result = self
            .library
            .borrow_mut()
            .record_play_for_path(&path, played_at_ms, deck);
        match result {
            Ok(true) => {
                self.library_changed();
                if self.library.borrow().active_crate() == crate::library::HISTORY_SCOPE {
                    self.publish_library();
                }
            }
            Ok(false) => {}
            Err(error) => {
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_status(format!("Cannot record playback history: {error}").into());
                }
            }
        }
    }
    fn quantize_preferences(&self) -> Result<crate::preferences::QuantizePreferences, String> {
        use crate::preferences::{DeckQuantizePreferences, QuantizePreferences};
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let step = |value| u8::try_from(value).map_err(|_| "Invalid quantization step".to_string());
        let prefs = QuantizePreferences {
            version: 1,
            enabled: ui.get_quantize_enabled(),
            decks: [
                DeckQuantizePreferences {
                    cues: ui.get_left_quantize_cues(),
                    triggers: ui.get_left_quantize_triggers(),
                    auto_loop_snap: ui.get_left_auto_loop_snap(),
                    step: step(ui.get_left_quantize_step())?,
                },
                DeckQuantizePreferences {
                    cues: ui.get_right_quantize_cues(),
                    triggers: ui.get_right_quantize_triggers(),
                    auto_loop_snap: ui.get_right_auto_loop_snap(),
                    step: step(ui.get_right_quantize_step())?,
                },
            ],
        };
        prefs.validate()?;
        Ok(prefs)
    }
    fn restore_quantize_preferences(&self, prefs: crate::preferences::QuantizePreferences) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_left_quantize_cues(prefs.decks[0].cues);
            ui.set_right_quantize_cues(prefs.decks[1].cues);
            ui.set_left_quantize_triggers(prefs.decks[0].triggers);
            ui.set_right_quantize_triggers(prefs.decks[1].triggers);
            ui.set_left_auto_loop_snap(prefs.decks[0].auto_loop_snap);
            ui.set_right_auto_loop_snap(prefs.decks[1].auto_loop_snap);
            ui.set_left_quantize_step(i32::from(prefs.decks[0].step));
            ui.set_right_quantize_step(i32::from(prefs.decks[1].step));
            ui.set_quantize_enabled(prefs.enabled);
        }
        self.saved_quantize.set(prefs);
        self.observed_quantize.set(prefs);
    }
    fn observe_quantize_preferences(&self) {
        if !self.storage_ready.get() || self.persistence.borrow().is_none() {
            return;
        }
        if self.fx_writable.get() && *self.fx_state.borrow() != *self.observed_fx.borrow() {
            *self.observed_fx.borrow_mut() = self.fx_state.borrow().clone();
            self.library_changed();
            if let Some(ui) = self.ui.upgrade() {
                ui.set_fx_status("FX changed".into());
            }
        }
        let output = self.output_route();
        if self.output_profiles_writable.get() && output != *self.observed_output.borrow() {
            *self.observed_output.borrow_mut() = output;
            self.library_changed();
            if let Some(ui) = self.ui.upgrade() {
                ui.set_output_profile_status("Profile changed".into());
            }
        }
        match self.quantize_preferences() {
            Ok(prefs) if prefs != self.observed_quantize.get() => {
                self.observed_quantize.set(prefs);
                self.library_changed();
            }
            Err(error) => {
                self.save_failed.set(true);
                self.dirty.set(true);
                self.storage_error(&error, true);
            }
            _ => {}
        }
    }
    fn output_route(&self) -> Option<crate::output_preferences::OutputRoute> {
        self.engine
            .borrow()
            .output_device_key()
            .cloned()
            .map(|device| crate::output_preferences::OutputRoute {
                device,
                pair: self.engine.borrow().output_pair(),

                monitor: (self.engine.borrow().output_channels() >= 4).then(|| {
                    let settings = self.engine.borrow().monitor_settings();
                    crate::output_preferences::MonitorPreferences {
                        gain: settings.gain,
                        mix: settings.mix,
                        cues: [
                            self.engine.borrow().headphone_cue(DeckId::A),
                            self.engine.borrow().headphone_cue(DeckId::B),
                        ],
                    }
                }),
            })
    }
    fn pending_fx(&self, include_inflight: bool) -> Option<crate::delay_fx::FxPreferences> {
        if !self.fx_writable.get() {
            return None;
        }
        let current = self.fx_state.borrow();
        let changed = *current != *self.saved_fx.borrow()
            || (include_inflight
                && self
                    .inflight_fx
                    .borrow()
                    .as_ref()
                    .is_some_and(|inflight| inflight != &*current));
        changed.then(|| current.clone())
    }
    fn restore_fx(&self, state: Result<crate::delay_fx::FxPreferences, String>) -> Option<String> {
        let (value, warning) = match state {
            Ok(value) => {
                self.fx_writable.set(true);
                (value, None)
            }
            Err(error) => {
                self.fx_writable.set(false);
                (Default::default(), Some(error))
            }
        };
        for (id, setting) in [DeckId::A, DeckId::B].into_iter().zip(value.decks) {
            let _ = self.engine.borrow().set_fx(id, setting);
            self.put_fx_ui(id, setting);
        }
        *self.fx_state.borrow_mut() = value.clone();
        *self.saved_fx.borrow_mut() = value.clone();
        *self.observed_fx.borrow_mut() = value;
        if let Some(ui) = self.ui.upgrade() {
            ui.set_fx_status(
                if warning.is_some() {
                    "FX session only (saved data invalid)"
                } else {
                    "FX ready"
                }
                .into(),
            );
        }
        warning
    }
    fn pending_output(
        &self,
        include_inflight: bool,
    ) -> Option<crate::output_preferences::OutputRoute> {
        if !self.output_profiles_writable.get() {
            return None;
        }
        let current = self.output_route()?;
        let changed = self.saved_output.borrow().as_ref() != Some(&current);
        let inflight_changed = include_inflight
            && self
                .inflight_output
                .borrow()
                .as_ref()
                .is_some_and(|p| p != &current);
        (changed || inflight_changed).then_some(current)
    }
    fn restore_output_routes(
        &self,
        profiles: Result<crate::output_preferences::OutputRoutes, String>,
    ) -> Option<String> {
        *self.output_profiles_cache.borrow_mut() = profiles.clone();
        let mut warning = None;
        let mut monitor = None;
        let pair = match profiles {
            Ok(profiles) => match self.engine.borrow().output_device_key() {
                Some(key) => {
                    let saved = profiles.pair_for(key);
                    if saved.is_none()
                        && key.endpoint_id.is_some()
                        && profiles.routes.iter().any(|route| {
                            route.device.endpoint_id.is_none()
                                && route.device.backend == key.backend
                                && route.device.name == key.name
                                && route.device.channels == key.channels
                        })
                    {
                        warning = Some("Legacy name profile preserved; endpoint profile uses defaults. Set output/monitor settings for this endpoint".into());
                    }
                    monitor = profiles
                        .routes
                        .iter()
                        .find(|route| route.device.same_device(key))
                        .and_then(|route| route.monitor);
                    let writable = saved.is_some()
                        || profiles.routes.len() < crate::output_preferences::MAX_OUTPUT_PROFILES;
                    self.output_profiles_writable.set(writable);
                    if !writable {
                        warning=Some("Audio device profile limit reached; existing profiles preserved, routing is session only".into());
                    }
                    saved.unwrap_or(0)
                }
                None => {
                    self.output_profiles_writable.set(false);
                    0
                }
            },
            Err(error) => {
                self.output_profiles_writable.set(false);
                warning = Some(format!(
                    "{error}; saved output profiles preserved, routing is session only"
                ));
                0
            }
        };
        if self.engine.borrow().output_channels() >= 4 {
            let _ = self.engine.borrow().restore_monitor_state(
                crate::output_preferences::MonitorPreferences {
                    gain: 1.,
                    mix: 0.,
                    cues: [false; 2],
                },
            );
        }
        if let Err(error) = self.engine.borrow().set_output_pair(pair) {
            self.output_profiles_writable.set(false);
            warning = Some(error);
            let _ = self.engine.borrow().set_output_pair(0);
        }
        if let Some(preferences) = monitor {
            if let Err(error) = self.engine.borrow().restore_monitor_state(preferences) {
                self.output_profiles_writable.set(false);
                warning = Some(format!(
                    "{error}; saved monitor profile preserved, session only"
                ));
            }
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_monitor_status(
                if self.output_profiles_writable.get() {
                    if monitor.is_some() {
                        "Monitor profile restored"
                    } else {
                        "Monitor defaults; changes follow audio profile status"
                    }
                } else {
                    "Monitor settings are session only"
                }
                .into(),
            );
            ui.set_output_pair_index(i32::from(self.engine.borrow().output_pair()));
            ui.set_output_profile_status(
                if self.output_profiles_writable.get() {
                    "Profile ready"
                } else {
                    "Session only"
                }
                .into(),
            );
        }
        let current = self.output_route();
        *self.saved_output.borrow_mut() = current.clone();
        *self.observed_output.borrow_mut() = current;
        warning
    }
    fn poll_storage(&self) {
        let event = self
            .persistence
            .borrow()
            .as_ref()
            .map(|w| w.events.try_recv());
        match event {
            Some(Ok(crate::library_store::Event::StartupAudioSaved(result))) => {
                self.startup_audio_saving.set(false);
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_audio_device_saving(false);
                    match result {
                        Ok(selection) => {
                            *self.saved_startup_audio.borrow_mut() = selection;
                            ui.set_audio_device_status(
                                "Output selection saved; applies at next program start".into(),
                            );
                        }
                        Err(error) => ui.set_audio_device_status(
                            format!("Output selection not saved: {error}").into(),
                        ),
                    }
                }
            }
            Some(Ok(crate::library_store::Event::Loaded(result))) => {
                self.storage_ready.set(true);
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_busy(false);
                    ui.set_quantize_preferences_ready(true);
                }
                match result {
                    Ok(loaded) => {
                        let crate::library_store::LoadedLibrary {
                            startup_audio,
                            library,
                            note,
                            backfill_error,
                            quantize_preferences,
                            preference_warning,
                            output_routes,
                            fx_preferences,
                        } = *loaded;
                        match startup_audio {
                            Ok(selection) => {
                                *self.saved_startup_audio.borrow_mut() = selection;
                                self.sync_saved_device_choice();
                            }
                            Err(error) => {
                                if let Some(ui) = self.ui.upgrade() {
                                    ui.set_audio_device_status(format!("Saved device selection unavailable: {error}; preserved until explicit save").into());
                                }
                            }
                        }
                        self.restore_quantize_preferences(quantize_preferences);
                        let output_warning = self.restore_output_routes(output_routes);
                        let fx_warning = self.restore_fx(fx_preferences);
                        self.library_generation.set(
                            self.library_generation
                                .get()
                                .checked_add(1)
                                .expect("Library generation overflow"),
                        );
                        *self.library.borrow_mut() = library;
                        self.publish_library();
                        for id in [DeckId::A, DeckId::B] {
                            if let Err(error) = self.apply_saved_grid(id) {
                                self.grid_feedback(Err(error), "");
                            }
                        }
                        if let Some(error) = backfill_error {
                            self.dirty.set(true);
                            self.save_failed.set(true);
                            self.storage_error(
                                &format!("Metadata backfill: {error}; catalog remains available"),
                                true,
                            );
                        } else if let Some(ui) = self.ui.upgrade() {
                            let warning = [
                                preference_warning
                                    .map(|w| format!("{w}; default Quantize preferences active")),
                                output_warning,
                                fx_warning,
                            ]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join("; ");
                            ui.set_library_storage(
                                if warning.is_empty() {
                                    note
                                } else {
                                    format!("{note}; {warning}")
                                }
                                .into(),
                            );
                        }
                    }
                    Err(error) => {
                        self.persistence.borrow_mut().take();
                        self.fx_writable.set(false);
                        if let Some(ui) = self.ui.upgrade() {
                            ui.set_fx_status("FX session only (storage unavailable)".into());
                        }
                        self.storage_error(&format!("{error}; session only"), false);
                    }
                }
                let paths = self.initial_paths.take();
                if !paths.is_empty() {
                    self.import_paths(paths);
                }
            }
            Some(Ok(crate::library_store::Event::Saved(result))) => {
                self.saving.set(false);
                let revision = self.inflight_revision.take();
                let quantize = self.inflight_quantize.take();
                let output = self.inflight_output.borrow_mut().take();
                let fx = self.inflight_fx.borrow_mut().take();
                match result {
                    Ok(()) => {
                        if let Some(value) = fx {
                            *self.saved_fx.borrow_mut() = value;
                            if let Some(ui) = self.ui.upgrade() {
                                ui.set_fx_status(
                                    if self.pending_fx(false).is_some() {
                                        "FX changed"
                                    } else {
                                        "FX saved"
                                    }
                                    .into(),
                                );
                            }
                        }
                        if output.is_some() {
                            if let (Ok(profiles), Some(route)) =
                                (&mut *self.output_profiles_cache.borrow_mut(), &output)
                            {
                                let _ = profiles.update(route.clone());
                            }
                            *self.saved_output.borrow_mut() = output;
                            if let Some(ui) = self.ui.upgrade() {
                                ui.set_output_profile_status(
                                    if self.pending_output(false).is_some() {
                                        "Profile changed"
                                    } else {
                                        "Profile saved"
                                    }
                                    .into(),
                                );
                            }
                        }
                        if let Some(prefs) = quantize {
                            self.saved_quantize.set(prefs);
                        }
                        if let Some(revision) = revision {
                            self.library.borrow_mut().acknowledge(revision);
                        }
                        self.dirty.set(
                            self.library.borrow().has_changes()
                                || self.pending_output(false).is_some()
                                || self.pending_fx(false).is_some()
                                || self
                                    .quantize_preferences()
                                    .is_ok_and(|p| p != self.saved_quantize.get()),
                        );
                        self.save_failed.set(false);

                        if let Some(ui) = self.ui.upgrade() {
                            ui.set_library_storage("Library saved".into());
                            ui.set_library_save_failed(false);
                        }
                    }
                    Err(error) => {
                        if fx.is_some() {
                            if let Some(ui) = self.ui.upgrade() {
                                ui.set_fx_status("FX not saved; Retry library save".into());
                            }
                        }
                        self.dirty.set(true);
                        self.save_failed.set(true);
                        if output.is_some() {
                            if let Some(ui) = self.ui.upgrade() {
                                ui.set_output_profile_status("Route not saved".into());
                            }
                        }
                        self.storage_error(&error, true);
                    }
                }
            }
            Some(Ok(crate::library_store::Event::CheckpointFailed(error))) => {
                // A backup warning is not completion of any queued save command.
                self.dirty.set(true);
                self.save_failed.set(true);
                self.storage_error(&format!("Backup checkpoint: {error}"), true);
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.persistence.borrow_mut().take();
                self.startup_audio_saving.set(false);
                self.fx_writable.set(false);
                self.storage_ready.set(true);
                self.saving.set(false);
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_busy(false);
                    ui.set_quantize_preferences_ready(true);
                    ui.set_fx_status("FX session only (storage unavailable)".into());
                    ui.set_audio_device_saving(false);
                    ui.set_audio_device_status(
                        "Storage unavailable; next-start output is not saved".into(),
                    );
                }
                self.storage_error("Storage worker stopped; session only", false);
            }
            _ => {}
        }
    }
    fn save_library(&self) {
        self.observe_quantize_preferences();
        if !self.storage_ready.get()
            || !self.dirty.get()
            || self.saving.get()
            || self.save_failed.get()
        {
            return;
        }
        let storage = self.persistence.borrow();
        let Some(worker) = storage.as_ref() else {
            return;
        };
        let delta = self.library.borrow().changes();
        let revision = delta.revision;
        let preferences = match self.quantize_preferences() {
            Ok(prefs) => (prefs != self.saved_quantize.get()).then_some(prefs),
            Err(error) => {
                self.storage_error(&error, true);
                self.save_failed.set(true);
                return;
            }
        };
        let output = self.pending_output(false);
        let fx = self.pending_fx(false);
        match worker.save_session_state(delta, preferences, output.clone(), fx.clone()) {
            Ok(()) => {
                self.inflight_revision.set(Some(revision));
                self.inflight_quantize.set(preferences);
                *self.inflight_fx.borrow_mut() = fx;
                if output.is_some() {
                    if let Some(ui) = self.ui.upgrade() {
                        ui.set_output_profile_status("Saving route".into());
                    }
                }
                *self.inflight_output.borrow_mut() = output;
                self.dirty.set(false);
                self.saving.set(true);
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_storage("Saving library…".into());
                    ui.set_library_save_failed(false);
                }
            }
            Err(error) => {
                self.save_failed.set(true);
                self.storage_error(&error, true);
            }
        }
    }
    pub fn close_library(&self) -> Result<(), String> {
        for job in self.keylock_jobs.borrow_mut().iter_mut() {
            job.take();
        }
        self.stop_sync();
        if let Some(cancel) = self.pending_cancel.borrow().as_ref() {
            cancel.request();
        }
        for id in [DeckId::A, DeckId::B] {
            self.cancel_nudge(id);
        }
        let preferences = if self.storage_ready.get() && self.persistence.borrow().is_some() {
            let prefs = self.quantize_preferences()?;
            (prefs != self.saved_quantize.get()
                || self
                    .inflight_quantize
                    .get()
                    .is_some_and(|inflight| inflight != prefs))
            .then_some(prefs)
        } else {
            None
        };
        let output = if self.storage_ready.get() && self.persistence.borrow().is_some() {
            self.pending_output(true)
        } else {
            None
        };
        if let Some(worker) = self.persistence.borrow_mut().take() {
            let fx = if self.storage_ready.get() {
                self.pending_fx(true)
            } else {
                None
            };
            let final_delta = (self.storage_ready.get()
                && (preferences.is_some()
                    || output.is_some()
                    || fx.is_some()
                    || self.dirty.get()
                    || self.saving.get()
                    || self.library.borrow().has_changes()))
            .then(|| self.library.borrow().changes());
            let revision = final_delta.as_ref().map(|d| d.revision);
            worker.close_session_state(final_delta, preferences, output, fx)?;
            if let Some(revision) = revision {
                self.library.borrow_mut().acknowledge(revision);
            }
            self.inflight_revision.set(None);
            self.inflight_quantize.set(None);
            self.inflight_fx.borrow_mut().take();
            self.inflight_output.borrow_mut().take();
            self.saving.set(false);
            self.dirty.set(false);
        }
        Ok(())
    }
    fn apply_levels(&self) {
        if let Some(ui) = self.ui.upgrade() {
            self.engine.borrow().set_levels(MixerLevels {
                crossfader: ui.get_crossfader(),
                channel_a: ui.get_channel_a(),
                channel_b: ui.get_channel_b(),
            });
        }
    }
    fn apply_eq(&self, id: DeckId) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let settings = match id {
            DeckId::A => crate::mixer_dsp::EqSettings {
                trim: ui.get_eq_trim_a().into(),
                low: ui.get_eq_low_a().into(),
                mid: ui.get_eq_mid_a().into(),
                high: ui.get_eq_high_a().into(),
                filter: ui.get_eq_filter_a().into(),
            },
            DeckId::B => crate::mixer_dsp::EqSettings {
                trim: ui.get_eq_trim_b().into(),
                low: ui.get_eq_low_b().into(),
                mid: ui.get_eq_mid_b().into(),
                high: ui.get_eq_high_b().into(),
                filter: ui.get_eq_filter_b().into(),
            },
        };
        if let Err(error) = self.engine.borrow().set_eq(id, settings) {
            let old = self.engine.borrow().eq_settings(id);
            match id {
                DeckId::A => {
                    ui.set_eq_trim_a(old.trim as f32);
                    ui.set_eq_low_a(old.low as f32);
                    ui.set_eq_mid_a(old.mid as f32);
                    ui.set_eq_high_a(old.high as f32);
                    ui.set_eq_filter_a(old.filter as f32);
                }
                DeckId::B => {
                    ui.set_eq_trim_b(old.trim as f32);
                    ui.set_eq_low_b(old.low as f32);
                    ui.set_eq_mid_b(old.mid as f32);
                    ui.set_eq_high_b(old.high as f32);
                    ui.set_eq_filter_b(old.filter as f32);
                }
            }
            ui.set_status(error.into());
        }
    }
    fn put_fx_ui(&self, id: DeckId, value: crate::delay_fx::DelaySettings) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        match id {
            DeckId::A => {
                ui.set_fx_active_a(value.enabled);
                ui.set_fx_time_a(value.time_ms as f32);
                ui.set_fx_feedback_a(value.feedback * 100.);
                ui.set_fx_wet_a(value.wet * 100.);
            }
            DeckId::B => {
                ui.set_fx_active_b(value.enabled);
                ui.set_fx_time_b(value.time_ms as f32);
                ui.set_fx_feedback_b(value.feedback * 100.);
                ui.set_fx_wet_b(value.wet * 100.);
            }
        }
    }
    fn apply_fx(&self, id: DeckId) {
        if !self.storage_ready.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let (enabled, time, feedback, wet) = match id {
            DeckId::A => (
                ui.get_fx_active_a(),
                ui.get_fx_time_a(),
                ui.get_fx_feedback_a(),
                ui.get_fx_wet_a(),
            ),
            DeckId::B => (
                ui.get_fx_active_b(),
                ui.get_fx_time_b(),
                ui.get_fx_feedback_b(),
                ui.get_fx_wet_b(),
            ),
        };
        let result = if !time.is_finite() || !(20. ..=1000.).contains(&time) {
            Err("Invalid delay time".into())
        } else {
            self.engine.borrow().set_fx(
                id,
                crate::delay_fx::DelaySettings {
                    enabled,
                    time_ms: time.round() as u16,
                    feedback: feedback / 100.,
                    wet: wet / 100.,
                },
            )
        };
        if let Err(error) = result {
            self.put_fx_ui(id, self.engine.borrow().fx_settings(id));
            ui.set_status(error.into());
        } else {
            self.fx_state.borrow_mut().decks[Self::grid_index(id)] =
                self.engine.borrow().fx_settings(id);
            self.observe_quantize_preferences();
        }
    }
    fn fx_preset(&self, id: DeckId, slot: i32, save: bool) {
        if !self.storage_ready.get() {
            return;
        }
        let Ok(slot) = usize::try_from(slot) else {
            return;
        };
        if slot >= 8 {
            return;
        }
        if save {
            self.fx_state.borrow_mut().presets[slot] = Some(self.engine.borrow().fx_settings(id));
        } else {
            let value = self.fx_state.borrow().presets[slot];
            let Some(value) = value else {
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_status("Delay preset slot is empty".into());
                }
                return;
            };
            if self.engine.borrow().set_fx(id, value).is_ok() {
                self.put_fx_ui(id, value);
                self.fx_state.borrow_mut().decks[Self::grid_index(id)] = value;
            }
        }
        self.observe_quantize_preferences();
    }
    // Nur Pfade/Status auf dem UI-Thread prüfen; Dateizugriff und Decode bleiben im Worker.
    fn track_drop_path(
        &self,
        deck: i32,
        data: &slint::DataTransfer,
    ) -> Result<(DeckId, PathBuf), String> {
        let id = match deck {
            0 => DeckId::A,
            1 => DeckId::B,
            _ => return Err("Invalid drop deck".into()),
        };
        if !self.storage_ready.get()
            || self.pending.borrow().is_some()
            || self.pending_import.borrow().is_some()
            || self.pending_device_switch.borrow().is_some()
        {
            return Err("Wait for loading or audio operations before dropping a track".into());
        }
        if self.engine.borrow().snapshot(id).playing {
            return Err("Pause the target deck before dropping a replacement track".into());
        }
        let mut paths = data
            .file_paths()
            .map_err(|_| "Drop one audio file from the library or file manager")?;
        let path = paths.next().ok_or("Drop one audio file")?;
        if paths.next().is_some() {
            return Err("Drop exactly one track onto a deck".into());
        }
        if !path.is_absolute() || !crate::audio_formats::supported(path) {
            return Err("Drop one supported audio file using an absolute path".into());
        }
        Ok((id, path.to_owned()))
    }
    fn request_load(&self, id: DeckId) {
        self.start_load(id, Arc::clone(&self.picker));
    }
    fn start_load(&self, id: DeckId, picker: TrackPicker) {
        if !self.storage_ready.get()
            || self.pending.borrow().is_some()
            || self.pending_import.borrow().is_some()
        {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if self.engine.borrow().snapshot(id).playing {
            ui.set_status(format!("Pause deck {id:?} before replacing its track").into());
            return;
        }
        ui.set_loading(true);
        ui.set_status("Select an audio file; decoding runs in the background".into());
        let (tx, rx) = mpsc::sync_channel(1);
        let cancel = crate::cancellation::Cancellation::default();
        *self.pending_cancel.borrow_mut() = Some(cancel.clone());
        match std::thread::Builder::new()
            .name("track-loader".into())
            .spawn(move || {
                let result = picker().map(|path| {
                    cancel.check()?;
                    let path = std::fs::canonicalize(path).map_err(|err| err.to_string())?;
                    let track = PreparedTrack::open_cancellable(&path, &cancel)?;
                    let mut metadata = crate::track_metadata::read_cancellable(&path, &cancel)?;
                    metadata.duration_ms = Some(track.duration().as_millis() as u64);
                    metadata.duration_exact = true;
                    cancel.check()?;
                    Ok((path, track, metadata))
                });
                let _ = tx.send(result);
            }) {
            Ok(_) => {
                *self.pending.borrow_mut() = Some((id, rx));
            }
            Err(err) => {
                self.pending_cancel.borrow_mut().take();
                ui.set_loading(false);
                ui.set_status(format!("Cannot start loader: {err}").into());
            }
        }
        self.refresh();
    }
    fn complete_load(&self, id: DeckId, result: LoadResult) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        ui.set_loading(false);
        match result {
            Some(Ok((path, track, metadata))) => {
                self.library.borrow_mut().insert([path.clone()]);
                self.library.borrow_mut().set_metadata(&path, metadata);
                self.library_changed();
                self.publish_library();
                if self.engine.borrow().snapshot(id).playing {
                    ui.set_status(format!("Load discarded: deck {id:?} started playing").into());
                } else if let Err(error) = self.engine.borrow().try_install(id, track) {
                    ui.set_status(format!("Deck {id:?}: {error}").into());
                } else {
                    ui.set_status(format!("Deck {id:?} loaded and paused").into());
                    if let Err(error) = self.apply_saved_grid(id) {
                        ui.set_status(
                            format!("Deck loaded; saved grid unavailable: {error}").into(),
                        );
                    }
                }
            }
            Some(Err(err)) => ui.set_status(format!("Deck {id:?}: {err}").into()),
            None => ui.set_status("Selection cancelled".into()),
        }
        self.refresh();
    }
    pub fn audio_owner(&self) -> Rc<RefCell<Rc<DualDeckEngine>>> {
        self.engine.clone()
    }
    pub fn transport_accept_after(&self) -> std::time::Instant {
        self.transport_accept_after.get()
    }
    fn device_switch_allowed(&self) -> Result<(), String> {
        if !self.storage_ready.get() {
            return Err("Wait for library/profile loading before changing output".into());
        }
        if [DeckId::A, DeckId::B]
            .into_iter()
            .any(|id| self.engine.borrow().snapshot(id).playing)
        {
            return Err("Pause both decks before changing audio device".into());
        }
        if self.pending.borrow().is_some()
            || self.pending_import.borrow().is_some()
            || self.keylock_jobs.borrow().iter().any(Option::is_some)
            || self.pending_pitch.borrow().iter().any(Option::is_some)
            || self.pending_sync.borrow().is_some()
            || self.sync_session.borrow().is_some()
            || self.nudges.borrow().iter().any(Option::is_some)
        {
            return Err("Finish loading and tempo preparation before changing audio device".into());
        }
        if self.cue_editors.borrow().iter().any(Option::is_some)
            || self.grid_editors.borrow().iter().any(Option::is_some)
        {
            return Err("Close deck editors before changing audio device".into());
        }
        Ok(())
    }
    fn handle_output_failure(&self) {
        if !self.engine.borrow().output_failed() || self.output_failure_handled.replace(true) {
            return;
        }
        self.stop_sync();
        for id in [DeckId::A, DeckId::B] {
            self.cancel_nudge(id);
            self.engine.borrow().cancel_scheduled_cue(id);
            self.engine.borrow().pause(id);
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_audio_startup_warning("Audio output failed. Reconnect the device, then AUDIO → REOPEN CURRENT. Both decks are paused.".into());
        }
    }
    fn request_output_reopen(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let result =
            (|| {
                self.device_switch_allowed()?;
                if self.pending_device_switch.borrow().is_some()
                    || self.device_scan.borrow().is_some()
                    || self.startup_audio_saving.get()
                {
                    return Err("Wait for the pending audio operation".to_string());
                }
                let key =
                    self.engine.borrow().output_device_key().cloned().ok_or(
                        "Current output has no unique identity; select an output explicitly",
                    )?;
                let selection = crate::audio_devices::StartupSelection {
                    version: 1,
                    device: Some(crate::audio_devices::DeviceSelector {
                        backend: key.backend.clone(),
                        name: key.name.clone(),
                        endpoint_id: key.endpoint_id.clone(),
                    }),
                };
                *self.recovery_key.borrow_mut() = Some(key);
                *self.pending_device_switch.borrow_mut() = Some(selection);
                ui.set_audio_device_switching(true);
                ui.set_audio_device_status(
                    "Reopening current output after saving; decks stay paused".into(),
                );
                self.save_library();
                Ok::<_, String>(())
            })();
        if let Err(error) = result {
            ui.set_audio_device_status(error.into());
        }
    }
    fn request_device_switch(&self) {
        if self.pending_device_switch.borrow().is_none() {
            self.recovery_key.borrow_mut().take();
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let result = (|| {
            if self.pending_device_switch.borrow().is_some()
                || self.device_scan.borrow().is_some()
                || self.startup_audio_saving.get()
            {
                return Err("Wait for the pending audio operation".to_string());
            }
            self.device_switch_allowed()?;
            let index = usize::try_from(ui.get_audio_device_choice())
                .map_err(|_| "Invalid audio device selection")?;
            let choices = self.device_choices.borrow();
            let device = if index == 0 {
                None
            } else {
                let choice = choices
                    .get(index - 1)
                    .ok_or("Invalid audio device selection")?;
                crate::audio_devices::unique_choice(&choices, &choice.selector)?;
                crate::audio_devices::validate_new_startup_choice(&choice.selector)?;
                Some(choice.selector.clone())
            };
            Ok(crate::audio_devices::StartupSelection { version: 1, device })
        })();
        match result {
            Ok(selection) => {
                *self.pending_device_switch.borrow_mut() = Some(selection);
                ui.set_audio_device_switching(true);
                ui.set_audio_device_status(
                    "Preparing output change; saving pending settings first…".into(),
                );
                self.save_library();
            }
            Err(error) => ui.set_audio_device_status(error.into()),
        }
    }
    fn poll_device_switch(&self) {
        if self.pending_device_switch.borrow().is_none() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let error = self.device_switch_allowed().err().or_else(|| {
            self.save_failed
                .get()
                .then(|| "Save failed; retry saving before changing audio device".to_string())
        });
        if let Some(error) = error {
            self.pending_device_switch.borrow_mut().take();
            self.recovery_key.borrow_mut().take();
            ui.set_audio_device_switching(false);
            ui.set_audio_device_status(error.into());
            return;
        }
        if self.saving.get()
            || (self.dirty.get() && self.persistence.borrow().is_some())
            || self.startup_audio_saving.get()
        {
            self.save_library();
            return;
        }
        let selection = self.pending_device_switch.borrow_mut().take().unwrap();
        let old = self.engine.borrow().clone();
        let recovery_key = self.recovery_key.borrow_mut().take();
        let result = (self.device_opener)(selection.device.as_ref()).and_then(|engine| {
            if engine.output_failed() { return Err("New audio output already reported a stream error".into()); }
            if recovery_key.as_ref().is_some_and(|expected| !engine.output_device_key().is_some_and(|actual| expected.same_device(actual))) {
                return Err("Output identity or channel shape changed; select the output explicitly instead".into());
            }
            engine.inherit_paused_decks(&old)?;
            Ok(Rc::new(engine))
        });
        match result {
            Ok(engine) => {
                self.output_failure_handled.set(false);
                if let (Ok(profiles), Some(route)) = (
                    &mut *self.output_profiles_cache.borrow_mut(),
                    self.output_route(),
                ) {
                    let _ = profiles.update(route);
                }
                let previous = self.engine.replace(engine);
                let profiles = self.output_profiles_cache.borrow().clone();
                let warning = self.restore_output_routes(profiles);
                let count = (self.engine.borrow().output_channels() / 2).max(1);
                let names = (0..count)
                    .map(|pair| {
                        if self.engine.borrow().output_channels() == 1 {
                            "OUT 1 (mono)".into()
                        } else {
                            format!("OUT {}/{}", pair * 2 + 1, pair * 2 + 2).into()
                        }
                    })
                    .collect::<Vec<slint::SharedString>>();
                ui.set_output_pair_options(ModelRc::new(VecModel::from(names)));
                ui.set_audio_device_current(
                    format!(
                        "{} ({} channels)",
                        self.engine.borrow().output_device_label(),
                        self.engine.borrow().output_channels()
                    )
                    .into(),
                );
                ui.set_audio_startup_warning("".into());
                ui.set_audio_device_status(warning.map_or_else(||"Audio device changed; decks remain paused. Save separately for next start".to_string(),|warning|format!("Audio device changed; {warning}")).into());
                self.displayed_overviews.borrow_mut().fill(None);
                self.displayed_details.borrow_mut().fill_with(|| None);
                self.refresh();
                drop(previous);
            }
            Err(error) => ui.set_audio_device_status(
                format!("Output change failed; previous device retained: {error}").into(),
            ),
        }
        drop(old);
        self.transport_accept_after.set(std::time::Instant::now());
        ui.set_audio_device_switching(false);
    }
    fn start_device_scan(&self) {
        if self.device_scan.borrow().is_some() || self.startup_audio_saving.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let catalog = self.device_catalog.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("audio-device-list".into())
            .spawn(move || {
                let _ = tx.send(catalog());
            }) {
            Ok(_) => {
                *self.device_scan.borrow_mut() = Some(rx);
                ui.set_audio_device_scanning(true);
                ui.set_audio_device_status("Reading available outputs…".into());
            }
            Err(error) => ui
                .set_audio_device_status(format!("Cannot enumerate audio devices: {error}").into()),
        }
    }
    fn poll_device_scan(&self) {
        let result = self.device_scan.borrow().as_ref().map(|rx| rx.try_recv());
        let Some(result) = result else { return };
        let result = match result {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("Audio device enumeration stopped".into()),
        };
        self.device_scan.borrow_mut().take();
        let Some(ui) = self.ui.upgrade() else { return };
        ui.set_audio_device_scanning(false);
        match result {
            Ok(choices) => {
                let mut names = vec![slint::SharedString::from("System default")];
                names.extend(choices.iter().map(|choice| {
                    format!(
                        "{} — {} ({} ch){}",
                        choice.selector.backend,
                        choice.selector.name,
                        choice.channels,
                        if choice.selector.backend == "WASAPI"
                            && choice.selector.endpoint_id.is_none()
                        {
                            " — stable ID unavailable"
                        } else {
                            ""
                        }
                    )
                    .into()
                }));
                ui.set_audio_device_names(ModelRc::new(VecModel::from(names)));
                *self.device_choices.borrow_mut() = choices;
                self.device_list_ready.set(true);
                self.sync_saved_device_choice();
            }
            Err(error) => {
                self.device_list_ready.set(false);
                self.device_choices.borrow_mut().clear();
                ui.set_audio_device_names(ModelRc::new(VecModel::from(vec![
                    "System default".into()
                ])));
                ui.set_audio_device_choice(0);
                ui.set_audio_device_status(error.into());
            }
        }
    }
    fn sync_saved_device_choice(&self) {
        if !self.device_list_ready.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let saved = self.saved_startup_audio.borrow();
        let choices = self.device_choices.borrow();
        let index = saved
            .device
            .as_ref()
            .and_then(|selector| {
                let matches: Vec<_> = choices
                    .iter()
                    .enumerate()
                    .filter(|(_, choice)| {
                        choice.selector.backend == selector.backend
                            && match selector.endpoint_id.as_ref() {
                                Some(id) => choice.selector.endpoint_id.as_ref() == Some(id),
                                None => choice.selector.name == selector.name,
                            }
                    })
                    .map(|(index, _)| index)
                    .collect();
                if matches.len() == 1 {
                    matches.first().copied()
                } else {
                    None
                }
            })
            .map_or(0, |i| i + 1);
        ui.set_audio_device_choice(index as i32);
        let status=match &saved.device {
            Some(selector)=>match crate::audio_devices::unique_choice(&choices,selector) {
            Ok(()) if selector.endpoint_id.is_some()=>"ID-bound selection applies at next program start; current playback stays on the active output".to_string(),
            Ok(())=>"Legacy name-bound selection applies at next program start; save again to bind the available endpoint ID".to_string(),
            Err(error)=>format!("Saved output {}: {error}. Original selection is preserved until explicit save",selector.name),
            },
            None=>"System default selected for next program start; current playback stays on the active output".to_string(),
        };
        ui.set_audio_device_status(status.into());
    }
    fn save_startup_audio(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if !self.storage_ready.get()
            || self.device_scan.borrow().is_some()
            || self.startup_audio_saving.get()
        {
            return;
        }
        let result = (|| {
            let index = usize::try_from(ui.get_audio_device_choice())
                .map_err(|_| "Invalid audio device selection".to_string())?;
            let choices = self.device_choices.borrow();
            let device = if index == 0 {
                None
            } else {
                let choice = choices
                    .get(index - 1)
                    .ok_or("Invalid audio device selection")?;
                crate::audio_devices::unique_choice(&choices, &choice.selector)?;
                crate::audio_devices::validate_new_startup_choice(&choice.selector)?;
                Some(choice.selector.clone())
            };
            self.persistence
                .borrow()
                .as_ref()
                .ok_or("Library storage unavailable; selection not saved")?
                .save_startup_audio(crate::audio_devices::StartupSelection { version: 1, device })
        })();
        match result {
            Ok(()) => {
                self.startup_audio_saving.set(true);
                ui.set_audio_device_saving(true);
                ui.set_audio_device_status("Saving next-start output selection…".into());
            }
            Err(error) => ui.set_audio_device_status(error.into()),
        }
    }
    pub fn tick(&self) {
        if let Some(perf) = self.library_perf.borrow().as_ref() {
            let (tracks, both_playing) = self.library_perf_context();
            perf.borrow_mut().tick(
                std::time::Instant::now(),
                tracks,
                both_playing,
                self.engine.borrow().rendered_frames(),
            );
        }
        self.handle_output_failure();
        self.poll_device_scan();
        self.engine.borrow().collect_retired();
        self.cancel_stale_nudges();
        self.cancel_stale_hot_cue_holds();
        self.apply_pending_pitch();
        self.poll_keylock();
        self.poll_pending_sync();
        self.poll_storage();
        self.poll_search();
        let import = self
            .pending_import
            .borrow()
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err("Library worker stopped unexpectedly")),
            });
        if let Some(result) = import {
            let relocation_id = self.pending_relocation_id.take();
            let cancelled = self
                .pending_cancel
                .borrow_mut()
                .take()
                .is_some_and(|c| c.is_cancelled());
            let result = if cancelled {
                Ok(Some(Err("Operation cancelled".to_string())))
            } else {
                result
            };
            self.pending_import.borrow_mut().take();
            if let Some(ui) = self.ui.upgrade() {
                ui.set_library_busy(false);
                match result {
                    Ok(Some(Ok(batch))) => {
                        if let Some(track_id) = relocation_id {
                            self.complete_relocation(track_id, batch);
                        } else {
                            let total = batch.paths.len();
                            let added = self
                                .library
                                .borrow_mut()
                                .insert(batch.paths.iter().cloned());
                            let assigned = self
                                .library
                                .borrow_mut()
                                .add_paths_to_crate(self.import_crate.get(), &batch.paths);
                            for (path, metadata) in batch.metadata {
                                self.library.borrow_mut().set_metadata(&path, metadata);
                            }
                            self.library_changed();
                            ui.set_status(format!("Added {added} tracks; {} skipped (duplicate, unsupported, missing or limit)", total - added + batch.rejected).into());
                            if let Err(error) = assigned {
                                ui.set_status(format!("Added {added} library tracks; crate assignment failed: {error}").into());
                            }
                            self.publish_library();
                        }
                    }
                    Ok(None) => ui.set_status(if relocation_id.is_some() {
                        "Track relocation cancelled".into()
                    } else {
                        "Library import cancelled".into()
                    }),
                    Ok(Some(Err(error))) => ui.set_status(error.into()),
                    Err(error) => ui.set_status(error.into()),
                }
            }
        }
        let result = self
            .pending
            .borrow()
            .as_ref()
            .and_then(|(id, rx)| match rx.try_recv() {
                Ok(result) => Some((*id, result)),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some((*id, Some(Err("Track loader stopped unexpectedly".into()))))
                }
            });
        if let Some((id, result)) = result {
            let cancelled = self
                .pending_cancel
                .borrow_mut()
                .take()
                .is_some_and(|c| c.is_cancelled());
            self.pending.borrow_mut().take();
            self.complete_load(
                id,
                if cancelled {
                    Some(Err("Operation cancelled".into()))
                } else {
                    result
                },
            );
        }
        self.maintain_sync(self.engine.borrow().control_now());
        self.save_library();
        self.poll_device_switch();
        self.refresh();
        if self.perf_status_updated.get().elapsed() >= std::time::Duration::from_secs(1) {
            if let Some(perf) = self.library_perf.borrow().as_ref() {
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_performance(perf.borrow().progress().into());
                }
                self.perf_status_updated.set(std::time::Instant::now());
            }
        }
    }
    pub fn record_library_performance(&self, recorder: Rc<RefCell<crate::library_perf::Recorder>>) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_library_performance(recorder.borrow().progress().into());
        }
        *self.library_perf.borrow_mut() = Some(recorder);
    }

    fn library_perf_context(&self) -> (usize, bool) {
        let engine = self.engine.borrow();
        (
            self.library.borrow().scope_len(),
            engine.snapshot(DeckId::A).playing && engine.snapshot(DeckId::B).playing,
        )
    }

    fn measure_library_action(
        &self,
        operation: crate::library_perf::Operation,
        action: impl FnOnce(),
    ) {
        let perf = self.library_perf.borrow().clone();
        if let Some(perf) = &perf {
            let now = std::time::Instant::now();
            let (tracks, both_playing) = self.library_perf_context();
            let mut recorder = perf.borrow_mut();
            recorder.context(tracks, both_playing);
            recorder.begin(operation, now);
        }
        action();
        if let Some(perf) = perf {
            perf.borrow_mut().dispatched(std::time::Instant::now());
        }
    }

    fn library_perf_ready(&self, spec: &crate::library::SearchSpec) {
        if let Some(perf) = self.library_perf.borrow().as_ref() {
            let (tracks, both_playing) = self.library_perf_context();
            let mut recorder = perf.borrow_mut();
            recorder.context(tracks, both_playing);
            recorder.model_ready(spec, std::time::Instant::now());
            if let Some(ui) = self.ui.upgrade() {
                ui.window().request_redraw();
            }
        }
    }

    fn fail_library_performance(&self) {
        if let Some(perf) = self.library_perf.borrow().as_ref() {
            perf.borrow_mut().failed();
        }
    }

    pub fn import_paths(&self, paths: Vec<PathBuf>) {
        self.start_import(Arc::new(move || Some(paths.clone())));
    }
    fn start_import(&self, picker: LibraryPicker) {
        self.start_import_work(move |cancel| {
            picker().map(|paths| crate::library::inspect_paths_cancellable(paths, cancel))
        });
    }
    fn start_relocation(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if !self.storage_ready.get()
            || self.pending.borrow().is_some()
            || self.pending_import.borrow().is_some()
        {
            ui.set_status("Wait for the current library operation to finish".into());
            return;
        }
        let track_id = self.library.borrow().selected_id();
        let selected = self
            .library
            .borrow()
            .track_by_id(track_id)
            .map(|track| (track.path.clone(), track.available));
        let Some((track_path, available)) = selected else {
            ui.set_status("Select a missing library track to relocate".into());
            return;
        };
        if available {
            ui.set_status("Only missing library tracks can be relocated".into());
            return;
        }
        if self.deck_uses_library_path(&track_path) {
            ui.set_status("Unload this missing track from both decks before relocating it".into());
            return;
        }
        let picker = Arc::clone(&self.library_picker);
        self.pending_relocation_id.set(Some(track_id));
        self.start_import_work(move |cancel| {
            picker().map(|paths| crate::library::inspect_paths_cancellable(paths, cancel))
        });
        if self.pending_import.borrow().is_none() {
            self.pending_relocation_id.set(None);
        } else {
            ui.set_status("Choose one replacement audio file".into());
        }
    }

    fn deck_uses_library_path(&self, path: &Path) -> bool {
        let key = crate::library::identity(path);
        [DeckId::A, DeckId::B].into_iter().any(|id| {
            self.engine
                .borrow()
                .snapshot(id)
                .path
                .is_some_and(|loaded| crate::library::identity(&loaded) == key)
        })
    }

    fn complete_relocation(&self, track_id: i32, batch: crate::library::ImportBatch) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if batch.paths.len() != 1 || batch.metadata.len() != 1 || batch.rejected != 0 {
            ui.set_status("Choose exactly one supported replacement audio file".into());
            return;
        }
        let path = batch
            .paths
            .into_iter()
            .next()
            .expect("checked one replacement");
        let (metadata_path, metadata) = batch
            .metadata
            .into_iter()
            .next()
            .expect("checked one replacement metadata result");
        if crate::library::identity(&metadata_path) != crate::library::identity(&path) {
            ui.set_status("Replacement file metadata does not match the selected file".into());
            return;
        }
        let old_path = self
            .library
            .borrow()
            .track_by_id(track_id)
            .map(|track| track.path.clone());
        let Some(old_path) = old_path else {
            ui.set_status("The selected library track no longer exists".into());
            return;
        };
        if self.deck_uses_library_path(&old_path) {
            ui.set_status("Unload this missing track from both decks before relocating it".into());
            return;
        }
        let result = self
            .library
            .borrow_mut()
            .relocate_track(track_id, path, metadata);
        match result {
            Ok(true) => {
                self.library_changed();
                ui.set_status("Track relocated; library save queued".into());
                self.publish_library();
            }
            Ok(false) => ui.set_status("Track already points to this file".into()),
            Err(error) => ui.set_status(error.into()),
        }
    }
    pub fn set_library_folder_picker(&self, picker: LibraryFolderPicker) {
        *self.library_folder_picker.borrow_mut() = Some(picker);
    }
    fn start_folder_import(&self) {
        let Some(picker) = self.library_folder_picker.borrow().clone() else {
            if let Some(ui) = self.ui.upgrade() {
                ui.set_status("Folder picker is unavailable".into());
            }
            return;
        };
        self.start_import_work(move |cancel| {
            picker().map(|root| crate::library::inspect_directory_cancellable(&root, cancel))
        });
    }
    fn start_import_work(
        &self,
        work: impl FnOnce(
                &crate::cancellation::Cancellation,
            ) -> Option<Result<crate::library::ImportBatch, String>>
            + Send
            + 'static,
    ) {
        if !self.storage_ready.get()
            || self.pending.borrow().is_some()
            || self.pending_import.borrow().is_some()
        {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        ui.set_library_busy(true);
        let library = self.library.borrow();
        let active_crate = library.active_crate();
        let target_crate = if library
            .crates()
            .entries
            .iter()
            .any(|entry| entry.id == active_crate && entry.smart_query.is_none())
        {
            active_crate
        } else {
            0
        };
        self.import_crate.set(target_crate);
        ui.set_status("Importing local audio files…".into());
        let (tx, rx) = mpsc::sync_channel(1);
        let cancel = crate::cancellation::Cancellation::default();
        *self.pending_cancel.borrow_mut() = Some(cancel.clone());
        match std::thread::Builder::new()
            .name("library-import".into())
            .spawn(move || {
                let _ = tx.send(work(&cancel));
            }) {
            Ok(_) => *self.pending_import.borrow_mut() = Some(rx),
            Err(err) => {
                self.pending_cancel.borrow_mut().take();
                ui.set_library_busy(false);
                ui.set_status(format!("Cannot start library worker: {err}").into());
            }
        }
        self.refresh();
    }
    fn publish_library(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.library
            .borrow_mut()
            .set_query_background(ui.get_library_query().to_string());
        let library = self.library.borrow();
        let selected = library.selected_id();
        ui.set_selected_track(selected);
        ui.set_library_can_relocate_selected(
            library
                .track_by_id(selected)
                .is_some_and(|track| !track.available),
        );
        let crates = library.crates().ordered_with_depth();
        let target_crates: Vec<_> = crates
            .iter()
            .filter(|(entry, _)| entry.smart_query.is_none())
            .collect();
        let crate_names = match target_crates
            .iter()
            .map(|(entry, _)| library.crates().hierarchy_name(entry.id))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(names) => names,
            Err(error) => {
                ui.set_status(format!("Cannot display crate hierarchy: {error}").into());
                return;
            }
        };
        let target = target_crates
            .iter()
            .position(|(entry, _)| entry.id == self.crate_target.get())
            .or_else(|| (!target_crates.is_empty()).then_some(0));
        self.crate_target
            .set(target.map_or(0, |index| target_crates[index].0.id));
        ui.set_target_crate_index(target.map_or(-1, |index| index as i32));
        ui.set_crate_names(ModelRc::new(Rc::new(VecModel::from(
            crate_names
                .into_iter()
                .map(slint::SharedString::from)
                .collect::<Vec<_>>(),
        ))));
        ui.set_crate_rows(ModelRc::new(Rc::new(VecModel::from(
            crates
                .iter()
                .map(|(entry, depth)| crate::CrateRow {
                    id: entry.id,
                    name: entry.name.clone().into(),
                    count: entry.tracks.len() as i32,
                    depth: *depth as i32,
                    smart: entry.smart_query.is_some(),
                })
                .collect::<Vec<_>>(),
        ))));
        ui.set_selected_crate(library.active_crate());
        ui.set_selected_crate_smart(
            library
                .crates()
                .entries
                .iter()
                .find(|entry| entry.id == library.active_crate())
                .is_some_and(|entry| entry.smart_query.is_some()),
        );

        let mut spec = library.search_spec(self.library_generation.get());
        let mut worker_slot = self.search_worker.borrow_mut();
        if worker_slot.is_none() {
            match SearchWorker::start() {
                Ok(worker) => *worker_slot = Some(worker),
                Err(error) => {
                    self.fail_library_performance();
                    ui.set_status(format!("Cannot start library search: {error}").into());
                    ui.set_library_rows(ModelRc::new(Rc::new(
                        VecModel::<crate::LibraryRow>::default(),
                    )));
                    ui.set_library_can_previous_page(false);
                    ui.set_library_can_next_page(false);
                    ui.set_selected_track(-1);
                    ui.set_library_can_relocate_selected(false);
                    ui.set_library_count("Search unavailable".into());
                    ui.set_library_empty("Search unavailable. Change search to retry.".into());
                    return;
                }
            }
        }
        let worker = worker_slot.as_mut().unwrap();
        if let Some(previous) = worker.wanted.as_ref().or(worker.published.as_ref()) {
            if previous.generation != spec.generation
                || previous.scope != spec.scope
                || previous.query != spec.query
                || previous.smart_query != spec.smart_query
                || previous.field != spec.field
                || previous.descending != spec.descending
            {
                self.library_page.set(0);
                self.pending_page_selection.set(0);
            }
        }
        spec.page_index = self.library_page.get();
        if let Some(perf) = self.library_perf.borrow().as_ref() {
            perf.borrow_mut().bind(&spec);
        }
        if worker.published.as_ref() == Some(&spec) && worker.wanted.is_none() {
            if let Some(ids) = library.cached_visible_ids() {
                let total = worker.published_total;
                drop(worker_slot);
                drop(library);
                self.publish_search_rows(ids, total);
                self.library_perf_ready(&spec);
                return;
            }
            worker.published = None;
        }
        if worker.wanted.as_ref() == Some(&spec) {
            ui.set_selected_track(-1);
            return;
        }
        let key = (spec.generation, spec.revision, spec.scope);
        if !worker
            .projection
            .as_ref()
            .is_some_and(|(g, r, c, _)| (*g, *r, *c) == key)
        {
            let mut projection = library.search_projection();
            if let Some((generation, revision, _, previous)) = worker.projection.as_ref() {
                if (*generation, *revision) == (key.0, key.1) {
                    projection.share_index_from(previous);
                }
            }
            worker.projection = Some((key.0, key.1, key.2, Arc::new(projection)));
        }
        let projection = Arc::clone(&worker.projection.as_ref().unwrap().3);
        worker.wanted = Some(spec.clone());
        worker.pending = Some((projection, spec));
        if let Err(error) = worker.send_pending() {
            self.fail_library_performance();
            ui.set_status(error.into());
            *worker_slot = None;
            ui.set_library_rows(ModelRc::new(Rc::new(
                VecModel::<crate::LibraryRow>::default(),
            )));
            ui.set_library_can_previous_page(false);
            ui.set_library_can_next_page(false);
            ui.set_selected_track(-1);
            ui.set_library_can_relocate_selected(false);
            ui.set_library_count("Search unavailable".into());
            ui.set_library_empty("Search unavailable. Change search to retry.".into());
            return;
        }
        ui.set_library_can_previous_page(false);
        ui.set_library_can_next_page(false);
        ui.set_selected_track(-1);
        ui.set_library_can_relocate_selected(false);
        ui.set_library_count("Searching library…".into());
        ui.set_library_rows(ModelRc::new(Rc::new(
            VecModel::<crate::LibraryRow>::default(),
        )));
        ui.set_library_empty("Searching library…".into());
    }
    fn publish_selected_library_track(&self, ui: &AppWindow) {
        let library = self.library.borrow();
        let selected = library.selected_id();
        ui.set_selected_track(selected);
        ui.set_library_can_relocate_selected(
            library
                .track_by_id(selected)
                .is_some_and(|track| !track.available),
        );
    }
    fn change_library_page(&self, delta: i32, select_edge: i8) {
        if self.search_in_progress() {
            return;
        }
        let total = self
            .search_worker
            .borrow()
            .as_ref()
            .map_or(0, |w| w.published_total);
        let last = total.saturating_sub(1) / LIBRARY_PAGE_SIZE;
        let next =
            (self.library_page.get() as i64 + i64::from(delta)).clamp(0, last as i64) as usize;
        if next != self.library_page.get() {
            self.measure_library_action(crate::library_perf::Operation::Page, || {
                self.library_page.set(next);
                self.pending_page_selection.set(select_edge);
                self.library.borrow_mut().select(-1);
                self.publish_library();
            });
        }
    }

    fn search_in_progress(&self) -> bool {
        self.search_worker
            .borrow()
            .as_ref()
            .is_some_and(|worker| worker.wanted.is_some())
    }

    fn poll_search(&self) {
        let mut slot = self.search_worker.borrow_mut();
        let Some(worker) = slot.as_mut() else {
            return;
        };
        let mut accepted = None;
        let mut reschedule = false;
        let mut failed = false;
        loop {
            match worker.receiver.try_recv() {
                Ok((spec, page)) => {
                    let mut current = self
                        .library
                        .borrow()
                        .search_spec(self.library_generation.get());
                    current.page_index = self.library_page.get();
                    if worker.wanted.as_ref() == Some(&spec) {
                        if current == spec {
                            worker.wanted = None;
                            self.library_page.set(page.page_index);
                            let mut published = spec.clone();
                            published.page_index = page.page_index;
                            worker.published = Some(published);
                            worker.published_total = page.total;
                            accepted = Some((spec, page));
                        } else {
                            worker.wanted = None;
                            worker.pending = None;
                            worker.published = None;
                            reschedule = true;
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    failed = true;
                    break;
                }
            }
        }
        if !failed && worker.send_pending().is_err() {
            failed = true;
        }
        if failed {
            *slot = None;
        }
        drop(slot);
        if failed {
            self.fail_library_performance();
            if let Some(ui) = self.ui.upgrade() {
                ui.set_status("Library search worker stopped".into());
                ui.set_library_can_previous_page(false);
                ui.set_library_can_next_page(false);
                ui.set_selected_track(-1);
                ui.set_library_can_relocate_selected(false);
                ui.set_library_count("Search unavailable".into());
                ui.set_library_empty("Search unavailable. Change search to retry.".into());
            }
        } else if reschedule {
            self.publish_library();
        } else if let Some((spec, page)) = accepted {
            let edge = self.pending_page_selection.replace(0);
            let mut library = self.library.borrow_mut();
            library.install_search_result(&page.ids);
            if edge != 0 {
                let id = if edge > 0 {
                    page.ids.first()
                } else {
                    page.ids.last()
                }
                .copied()
                .unwrap_or(-1);
                library.select(id);
            } else if library.selected_id() != -1 && !page.ids.contains(&library.selected_id()) {
                library.select(-1);
            }
            drop(library);
            self.publish_search_rows(page.ids, page.total);
            if let Some(ui) = self.ui.upgrade() {
                ui.invoke_library_scroll_page(edge < 0);
            }
            self.library_perf_ready(&spec);
        }
    }

    fn publish_search_rows(&self, rows: Vec<i32>, total: usize) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let library = self.library.borrow();
        ui.set_library_count(
            format!(
                "{} / {} tracks ({} missing)",
                total,
                library.scope_len(),
                library.missing_count()
            )
            .into(),
        );
        let page_index = self.library_page.get();
        let pages = total.div_ceil(LIBRARY_PAGE_SIZE);
        ui.set_library_page_text(if pages == 0 {
            "0 / 0".into()
        } else {
            format!("{} / {}", page_index + 1, pages).into()
        });
        ui.set_library_can_previous_page(page_index > 0);
        ui.set_library_can_next_page(page_index + 1 < pages);
        ui.set_library_empty(
            if library.len() == 0 {
                "Add audio files to your library"
            } else if rows.is_empty()
                && library.active_crate() != 0
                && ui.get_library_query().is_empty()
            {
                "This crate is empty. Add tracks or import files."
            } else if rows.is_empty() {
                "No tracks match your search"
            } else {
                ""
            }
            .into(),
        );
        ui.set_library_rows(ModelRc::new(Rc::new(LibraryRows {
            rows,
            library: Rc::downgrade(&self.library),
            generation: Rc::downgrade(&self.library_generation),
            expected_generation: self.library_generation.get(),
            #[cfg(test)]
            row_data_calls: Cell::new(0),
        })));
        let selected = library.selected_id();
        ui.set_selected_track(selected);
        ui.set_library_can_relocate_selected(
            library
                .track_by_id(selected)
                .is_some_and(|track| !track.available),
        );
    }

    fn refresh(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        ui.set_audio_device_ready(self.storage_ready.get() && self.persistence.borrow().is_some());
        ui.set_monitor_available(
            self.engine.borrow().output_channels() >= 4 && self.storage_ready.get(),
        );
        ui.set_monitor_cue_a(self.engine.borrow().headphone_cue(DeckId::A));
        ui.set_monitor_cue_b(self.engine.borrow().headphone_cue(DeckId::B));
        let monitor = self.engine.borrow().monitor_settings();
        ui.set_monitor_gain(monitor.gain * 100.);
        ui.set_monitor_mix(monitor.mix * 100.);
        let (caption, level, warning) = peak_display(self.engine.borrow().output_peak());
        ui.set_output_peak_caption(caption.into());
        ui.set_output_peak_level(level);
        ui.set_output_peak_warning(warning);
        for (id, name) in [(DeckId::A, "A"), (DeckId::B, "B")] {
            let (caption, level, invalid) = peak_display(self.engine.borrow().channel_peak(id));
            let state = if self.engine.borrow().snapshot(id).playing {
                "PRE"
            } else {
                "PRE LAST"
            };
            let caption = caption.replacen("BUS", &format!("{name} {state}"), 1);
            let clipped = self.engine.borrow().channel_clipped(id);
            match id {
                DeckId::A => {
                    ui.set_channel_a_peak_caption(caption.into());
                    ui.set_channel_a_peak_level(level);
                    ui.set_channel_a_clipped(clipped);
                    ui.set_channel_a_peak_warning(invalid);
                }
                DeckId::B => {
                    ui.set_channel_b_peak_caption(caption.into());
                    ui.set_channel_b_peak_level(level);
                    ui.set_channel_b_clipped(clipped);
                    ui.set_channel_b_peak_warning(invalid);
                }
            }
        }
        ui.set_background_job_active(
            self.pending.borrow().is_some() || self.pending_import.borrow().is_some(),
        );
        ui.set_left_alignment_index(self.alignments.get()[0].index());
        ui.set_right_alignment_index(self.alignments.get()[1].index());
        let follower = self.sync_session.borrow().as_ref().map(|s| s.target);
        let preparing = self
            .pending_sync
            .borrow()
            .as_ref()
            .map(|s| s.session.target);
        ui.set_left_sync_preparing(preparing == Some(DeckId::A));
        ui.set_right_sync_preparing(preparing == Some(DeckId::B));
        ui.set_left_sync_active(follower == Some(DeckId::A));
        ui.set_right_sync_active(follower == Some(DeckId::B));
        ui.set_left_can_sync(
            follower == Some(DeckId::A)
                || preparing == Some(DeckId::A)
                || (follower.is_none()
                    && preparing.is_none()
                    && self.sync_start_ready(DeckId::A).is_ok()),
        );
        ui.set_right_can_sync(
            follower == Some(DeckId::B)
                || preparing == Some(DeckId::B)
                || (follower.is_none()
                    && preparing.is_none()
                    && self.sync_start_ready(DeckId::B).is_ok()),
        );
        ui.set_left_can_phase_sync(self.phase_sync_ready(DeckId::A).is_ok());
        ui.set_right_can_phase_sync(self.phase_sync_ready(DeckId::B).is_ok());
        ui.set_left_can_match_tempo(self.tempo_match_target(DeckId::A).is_ok());
        ui.set_right_can_match_tempo(self.tempo_match_target(DeckId::B).is_ok());
        ui.set_left_busy(!self.can_control_deck(DeckId::A));
        ui.set_right_busy(!self.can_control_deck(DeckId::B));
        for id in [DeckId::A, DeckId::B] {
            let s = self.engine.borrow().snapshot(id);
            let index = Self::grid_index(id);
            let desired = s.detail.as_ref().map_or(s.requested_speed, |source| {
                self.desired_pitch(id, source, s.requested_speed)
            });
            let pitch_range = self.pitch_ranges.get()[index];
            let (pitch_min, pitch_max) = pitch_range.bounds();
            let pitch_outside = !pitch_range.contains(desired);
            match id {
                DeckId::A => {
                    ui.set_left_pitch_range_index(pitch_range.index());
                    ui.set_left_pitch_min(pitch_min as f32);
                    ui.set_left_pitch_max(pitch_max as f32);
                    ui.set_left_pitch_outside(pitch_outside);
                    ui.set_left_pitch_can_down(pitch_range.step(desired, false).is_ok());
                    ui.set_left_pitch_can_up(pitch_range.step(desired, true).is_ok());
                }
                DeckId::B => {
                    ui.set_right_pitch_range_index(pitch_range.index());
                    ui.set_right_pitch_min(pitch_min as f32);
                    ui.set_right_pitch_max(pitch_max as f32);
                    ui.set_right_pitch_outside(pitch_outside);
                    ui.set_right_pitch_can_down(pitch_range.step(desired, false).is_ok());
                    ui.set_right_pitch_can_up(pitch_range.step(desired, true).is_ok());
                }
            }
            let pitch = format!(
                "Requested {:+.2}% · effective {:+.4}% · {}",
                (desired - 1.) * 100.,
                (s.effective_speed - 1.) * 100.,
                if self.engine.borrow().keylock_factor(id).is_some() {
                    "Keylock"
                } else {
                    "coupled"
                }
            );
            let pitch = if s.sync_factor != 1. {
                format!("Sync {:+.3}% · {pitch}", (s.sync_factor - 1.) * 100.)
            } else {
                pitch
            };
            let pitch = if s.nudge_direction != 0 {
                format!("Nudge {:+}% · {pitch}", s.nudge_direction * 2)
            } else {
                pitch
            };
            let pitch = if pitch_outside {
                format!("Outside range · {pitch}")
            } else {
                format!("0.01% steps · {pitch}")
            };
            let bpm = if let Some(grid) = s.grid {
                format!(
                    "Manual BPM: {:.2} → {:.2}",
                    grid.bpm(),
                    grid.bpm() * s.effective_speed
                )
            } else if let Some(tempo) = s.tempo {
                format!(
                    "Estimate BPM: ~{:.2} → ~{:.2}",
                    tempo.bpm,
                    tempo.bpm * s.effective_speed
                )
            } else {
                "Source BPM unavailable".into()
            };
            match id {
                DeckId::A => {
                    ui.set_left_pitch_percent(((desired - 1.) * 100.) as f32);
                    ui.set_left_pitch_caption(pitch.into());
                    ui.set_left_bpm_caption(bpm.into());
                }
                DeckId::B => {
                    ui.set_right_pitch_percent(((desired - 1.) * 100.) as f32);
                    ui.set_right_pitch_caption(pitch.into());
                    ui.set_right_bpm_caption(bpm.into());
                }
            }
            let bounds = match (s.loop_in, s.loop_out) {
                (Some(start), Some(end)) => {
                    format!("{:.3}–{:.3} s", start.as_secs_f64(), end.as_secs_f64())
                }
                (Some(start), None) => format!("IN {:.3} s", start.as_secs_f64()),
                _ => "No loop".into(),
            };
            let caption = format!("{}{}", if s.loop_active { "ACTIVE · " } else { "" }, bounds);
            let loop_range = loop_overlay(
                s.loop_in,
                s.loop_out,
                s.duration,
                std::time::Duration::ZERO,
                s.duration,
                s.loop_active,
            );
            if s.detail.is_none() {
                match id {
                    DeckId::A => ui.set_left_detail_loop_range(crate::LoopRange::default()),
                    DeckId::B => ui.set_right_detail_loop_range(crate::LoopRange::default()),
                }
            }
            match id {
                DeckId::A => {
                    ui.set_left_loop_range(loop_range);
                    ui.set_left_loop_caption(caption.into());
                    ui.set_left_loop_has_in(s.loop_in.is_some());
                    ui.set_left_loop_ready(s.loop_out.is_some());
                    ui.set_left_loop_active(s.loop_active);
                }
                DeckId::B => {
                    ui.set_right_loop_range(loop_range);
                    ui.set_right_loop_caption(caption.into());
                    ui.set_right_loop_has_in(s.loop_in.is_some());
                    ui.set_right_loop_ready(s.loop_out.is_some());
                    ui.set_right_loop_active(s.loop_active);
                }
            }
            let loops = s
                .path
                .as_ref()
                .map(|path| self.library.borrow().saved_loops(path))
                .unwrap_or_default();
            if self.displayed_loops.borrow()[index].as_ref() != Some(&loops) {
                let rows = loops
                    .slots()
                    .iter()
                    .map(|value| crate::SavedLoopRow {
                        occupied: value.is_some(),
                        description: value
                            .as_ref()
                            .map(|v| {
                                format!(
                                    "{} · {:.3}–{:.3} s",
                                    v.name(),
                                    v.start().as_secs_f64(),
                                    v.end().as_secs_f64()
                                )
                            })
                            .unwrap_or_else(|| "Empty slot".into())
                            .into(),
                    })
                    .collect::<Vec<_>>();
                let model = ModelRc::new(Rc::new(VecModel::from(rows)));
                match id {
                    DeckId::A => ui.set_left_saved_loops(model),
                    DeckId::B => ui.set_right_saved_loops(model),
                }
                self.displayed_loops.borrow_mut()[index] = Some(loops);
            }
            let cues = s
                .path
                .as_ref()
                .map(|p| self.library.borrow().hot_cues(p))
                .unwrap_or_default();
            if self.displayed_cues.borrow()[index]
                .as_ref()
                .is_none_or(|(previous, duration)| previous != &cues || *duration != s.duration)
            {
                let rows = cues
                    .slots()
                    .iter()
                    .enumerate()
                    .map(|(slot, cue)| crate::HotCueRow {
                        name: cue.as_ref().map_or("", |c| c.name()).into(),
                        valid: cue.as_ref().is_some_and(|c| c.position() < s.duration),
                        position: cue.as_ref().map_or(0., |c| {
                            if s.duration.is_zero() {
                                0.
                            } else {
                                (100. * c.position().as_secs_f64() / s.duration.as_secs_f64())
                                    as f32
                            }
                        }),
                        occupied: cue.is_some(),
                        description: cue
                            .as_ref()
                            .map(|c| format!("{} · {:.3} s", c.name(), c.position().as_secs_f64()))
                            .unwrap_or_else(|| "Set at current position".into())
                            .into(),
                        tint: slint::Color::from_rgb_u8(
                            cue.as_ref().map_or(0, |c| (c.rgb() >> 16) as u8),
                            cue.as_ref().map_or(0, |c| (c.rgb() >> 8) as u8),
                            cue.as_ref().map_or(0, |c| c.rgb() as u8),
                        ),
                        label_row: (slot % 4) as i32,
                        label_column: (slot / 4) as i32,
                    })
                    .collect::<Vec<_>>();
                let model = ModelRc::new(Rc::new(VecModel::from(rows)));
                match id {
                    DeckId::A => ui.set_left_hot_cues(model),
                    DeckId::B => ui.set_right_hot_cues(model),
                }
                self.displayed_cues.borrow_mut()[index] = Some((cues, s.duration));
            }
            let cue_replaced = self.cue_editors.borrow()[index]
                .as_ref()
                .is_some_and(|edit| {
                    s.detail
                        .as_ref()
                        .is_none_or(|source| !Arc::ptr_eq(source, &edit.source))
                });
            if cue_replaced {
                self.cancel_cue_editor(id);
                ui.set_status(
                    format!("Deck {id:?}: cue draft discarded after track replacement").into(),
                );
            }
            let changed_track = self.grid_editors.borrow()[index]
                .as_ref()
                .is_some_and(|editor| {
                    s.detail
                        .as_ref()
                        .is_none_or(|source| !Arc::ptr_eq(source, &editor.source))
                });
            if changed_track {
                self.cancel_grid_editor(id);
                ui.set_status(
                    format!("Deck {id:?}: grid draft discarded after track replacement").into(),
                );
            }
            let editing = self.grid_editors.borrow()[index].is_some();
            let preview = self.grid_editors.borrow()[index]
                .as_ref()
                .map_or(s.grid, |editor| editor.draft.value());

            let index = if id == DeckId::A { 0 } else { 1 };
            let mut displayed = self.displayed_overviews.borrow_mut();
            let changed = match (&displayed[index], &s.overview) {
                (Some(previous), Some(current)) => !Arc::ptr_eq(previous, current),
                (None, None) => false,
                _ => true,
            };
            if changed {
                match id {
                    DeckId::A => ui.set_left_saved_loop_name("".into()),
                    DeckId::B => ui.set_right_saved_loop_name("".into()),
                }
                let peaks = s.overview.as_ref().map_or_else(Vec::new, |v| v.to_vec());
                let model = ModelRc::new(Rc::new(VecModel::from(peaks)));
                let colors = s
                    .detail
                    .as_ref()
                    .map_or_else(ModelRc::default, |d| color_model(d.overview_colors()));
                match id {
                    DeckId::A => {
                        ui.set_left_waveform(model);
                        ui.set_left_waveform_colors(colors);
                    }
                    DeckId::B => {
                        ui.set_right_waveform(model);
                        ui.set_right_waveform_colors(colors);
                    }
                }
                displayed[index] = s.overview.clone();
            }
            drop(displayed);
            let percent = s.percent();
            if let Some(detail) = &s.detail {
                let seconds = match id {
                    DeckId::A => ui.get_left_detail_window(),
                    DeckId::B => ui.get_right_detail_window(),
                }
                .clamp(2, 32) as u32;
                let range = detail.range(s.position, seconds);
                let overlay = loop_overlay(
                    s.loop_in,
                    s.loop_out,
                    s.duration,
                    crate::looping::frame_time(range.start_frame, detail.sample_rate()),
                    crate::looping::frame_time(range.end_frame, detail.sample_rate())
                        .min(s.duration),
                    s.loop_active,
                );
                match id {
                    DeckId::A => ui.set_left_detail_loop_range(overlay),
                    DeckId::B => ui.set_right_detail_loop_range(overlay),
                }
                let start = detail.percent(range.start_frame);
                let end = detail.percent(range.end_frame);
                let marker = if end > start {
                    (100.0 * (percent - start) / (end - start)).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                let mut cache = self.displayed_details.borrow_mut();
                let changed = cache[index].as_ref().is_none_or(|previous| {
                    !Arc::ptr_eq(&previous.source, detail) || previous.range != range
                });
                let grid_changed = changed
                    || cache[index]
                        .as_ref()
                        .is_none_or(|previous| previous.grid != preview);
                if changed {
                    let model = ModelRc::new(Rc::new(VecModel::from(detail.render(range))));
                    let colors = color_model(&detail.render_colors(range));
                    match id {
                        DeckId::A => {
                            ui.set_left_detail(model);
                            ui.set_left_detail_colors(colors);
                        }
                        DeckId::B => {
                            ui.set_right_detail(model);
                            ui.set_right_detail_colors(colors);
                        }
                    }
                }
                if grid_changed {
                    let lines = preview.map_or_else(Vec::new, |grid| {
                        grid.lines(detail.time(range.start_frame), detail.time(range.end_frame))
                    });
                    let model = ModelRc::new(Rc::new(VecModel::from(
                        lines
                            .into_iter()
                            .map(|line| crate::BeatLine {
                                position: line.percent,
                                downbeat: line.downbeat,
                                first: line.first,
                                warp_marker: line.warp_marker,
                                bar: line.bar,
                            })
                            .collect::<Vec<_>>(),
                    )));
                    match id {
                        DeckId::A => ui.set_left_beats(model),
                        DeckId::B => ui.set_right_beats(model),
                    }
                }
                if grid_changed {
                    cache[index] = Some(DetailCache {
                        source: Arc::clone(detail),
                        range,
                        grid: preview,
                    });
                }
                match id {
                    DeckId::A => {
                        ui.set_left_detail_start(start);
                        ui.set_left_detail_end(end);
                        ui.set_left_detail_marker(marker);
                    }
                    DeckId::B => {
                        ui.set_right_detail_start(start);
                        ui.set_right_detail_end(end);
                        ui.set_right_detail_marker(marker);
                    }
                }
            }
            let time = deck_time(s.position, s.duration);
            let caption = preview.map_or_else(
                || "GRID OFF".into(),
                |grid| {
                    format!(
                        "{} BPM · MANUAL {}/{}",
                        grid.bpm(),
                        grid.beats_per_bar(),
                        grid.meter_denominator()
                    )
                },
            );
            let meter_summary = preview.map_or_else(
                || "No meter map".to_string(),
                |grid| {
                    let mut summary =
                        format!("1: {}/{}", grid.beats_per_bar(), grid.meter_denominator());
                    for change in grid.meter_changes() {
                        summary.push_str(&format!(
                            " · {}: {}/{}",
                            change.bar, change.beats, change.denominator
                        ));
                    }
                    summary
                },
            );
            let tempo_summary = preview.map_or_else(
                || "No tempo map".to_string(),
                |grid| {
                    let mut summary = format!("Base: {} BPM", grid.bpm());
                    for change in grid.tempo_changes() {
                        summary.push_str(&format!(" · Beat {}: {} BPM", change.beat, change.bpm));
                    }
                    summary
                },
            );
            let warp_summary = preview.map_or_else(
                || "No warp markers".to_string(),
                |grid| {
                    if grid.warp_markers().is_empty() {
                        "No warp markers".to_string()
                    } else {
                        let mut summary = String::new();
                        for marker in grid.warp_markers() {
                            if !summary.is_empty() {
                                summary.push_str(" · ");
                            }
                            summary.push_str(&format!(
                                "Beat {}: {:.3}s",
                                marker.beat,
                                marker.position_ns as f64 / 1_000_000_000.
                            ));
                        }
                        summary
                    }
                },
            );
            match id {
                DeckId::A => {
                    ui.set_left_grid_meter_summary(meter_summary.into());
                    ui.set_left_grid_tempo_summary(tempo_summary.into());
                    ui.set_left_grid_warp_summary(warp_summary.into());
                }
                DeckId::B => {
                    ui.set_right_grid_meter_summary(meter_summary.into());
                    ui.set_right_grid_tempo_summary(tempo_summary.into());
                    ui.set_right_grid_warp_summary(warp_summary.into());
                }
            }
            let caption = if editing {
                format!("DRAFT · {caption}")
            } else {
                caption
            };
            match id {
                DeckId::A => {
                    ui.set_left_title(s.title.into());
                    ui.set_left_loaded(s.loaded);
                    ui.set_left_playing(s.playing);
                    ui.set_left_position(percent);
                    ui.set_left_time(time.into());
                    ui.set_left_grid_active(s.grid.is_some());
                    ui.set_left_grid_caption(caption.into());
                    ui.set_left_suggested_bpm(
                        s.tempo.map_or(0, |estimate| estimate.bpm.round() as i32),
                    );
                }
                DeckId::B => {
                    ui.set_right_title(s.title.into());
                    ui.set_right_loaded(s.loaded);
                    ui.set_right_playing(s.playing);
                    ui.set_right_position(percent);
                    ui.set_right_time(time.into());
                    ui.set_right_grid_active(s.grid.is_some());
                    ui.set_right_grid_caption(caption.into());
                    ui.set_right_suggested_bpm(
                        s.tempo.map_or(0, |estimate| estimate.bpm.round() as i32),
                    );
                }
            }
        }
    }
}
fn color_model(colors: &[crate::waveform_color::Rgb]) -> ModelRc<slint::Color> {
    let colors: Vec<_> = colors
        .iter()
        .map(|c| slint::Color::from_rgb_u8(c.0, c.1, c.2))
        .collect();
    ModelRc::new(Rc::new(VecModel::from(colors)))
}
/// Decide visibility before reducing precise source times to viewport-local GUI coordinates.
fn loop_overlay(
    start: Option<std::time::Duration>,
    end: Option<std::time::Duration>,
    duration: std::time::Duration,
    view_start: std::time::Duration,
    view_end: std::time::Duration,
    active: bool,
) -> crate::LoopRange {
    let (Some(start), Some(end)) = (start, end) else {
        return crate::LoopRange::default();
    };
    if start >= end || end > duration || view_start >= view_end || view_end > duration {
        return crate::LoopRange::default();
    }
    let fraction = |time: std::time::Duration| {
        (100. * (time.clamp(view_start, view_end) - view_start).as_secs_f64()
            / (view_end - view_start).as_secs_f64()) as f32
    };
    crate::LoopRange {
        valid: true,
        active,
        visible: start < view_end && end > view_start,
        in_visible: start >= view_start && start < view_end,
        out_visible: end > view_start && end <= view_end,
        start: fraction(start),
        end: fraction(end),
        description: format!(
            "{} loop · In {:.6} s · Out {:.6} s",
            if active { "Active" } else { "Inactive" },
            start.as_secs_f64(),
            end.as_secs_f64()
        )
        .into(),
    }
}

fn peak_display(peak: Option<f32>) -> (String, f32, bool) {
    match peak {
        None => ("BUS PEAK —".into(), 0., false),
        Some(p) if !p.is_finite() || p < 0. => ("BUS INVALID".into(), 0., true),
        Some(0.) => ("BUS PEAK -∞ dBFS".into(), 0., false),
        Some(p) => (
            format!("BUS PEAK {:+.1} dBFS", 20. * p.log10()),
            p.clamp(0., 1.),
            p >= 1.,
        ),
    }
}

fn deck_time(position: std::time::Duration, duration: std::time::Duration) -> String {
    let elapsed = position.min(duration);
    let remaining = duration.saturating_sub(position);
    format!(
        "ELAPSED {}   LEFT {}\nTOTAL {}",
        precise_time(elapsed),
        precise_time(remaining),
        precise_time(duration)
    )
}

fn precise_time(time: std::time::Duration) -> String {
    let total_ms = time.as_millis();
    let secs = total_ms / 1000;
    let millis = total_ms % 1000;
    format!("{:02}:{:02}.{:03}", secs / 60, secs % 60, millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fixture, wav_file, TempFile};
    use i_slint_backend_testing::ElementHandle;
    use std::{
        collections::VecDeque,
        sync::Mutex,
        thread,
        time::{Duration, Instant},
    };
    #[test]
    fn keylock_gui_and_mc4000_prepare_update_disable_and_reject_replaced_sources() {
        i_slint_backend_testing::init_no_event_loop();
        fn finish_keylock(c: &AudioController, id: DeckId) {
            let deadline = Instant::now() + Duration::from_secs(15);
            while c.engine.borrow().keylock_factor(id).is_none()
                || c.keylock_jobs.borrow()[AudioController::grid_index(id)].is_some()
            {
                c.poll_keylock();
                assert!(Instant::now() < deadline, "Keylock preparation timed out");
                std::thread::yield_now();
            }
        }
        fn wait_for_discardable_sync_result(c: &AudioController, id: DeckId) {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !c.keylock_jobs.borrow()[AudioController::grid_index(id)]
                .as_ref()
                .unwrap()
                .1
                .is_finished()
            {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            // The result remains queued: cancellation must discard it before controller adoption.
        }
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let clock = Rc::new(Cell::new(Instant::now()));
        let source_clock = Rc::clone(&clock);
        let engine = Rc::new(DualDeckEngine::from_mixer_with_clock(
            mixer,
            1,
            48000,
            Box::new(move || source_clock.get()),
        ));
        let tone: Vec<_> = (0..96000)
            .map(|i| ((std::f32::consts::TAU * 440. * i as f32 / 48000.).sin() * 8000.) as i16)
            .collect();
        let first = crate::test_support::wav_samples(&tone, 48000, 1);
        let second = wav_file(8192, 48000);
        engine.load_a(first.path()).unwrap();
        engine.load_b(first.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        for id in [DeckId::A, DeckId::B] {
            engine.set_speed(id, 1.).unwrap();
            let channel = if id == DeckId::A { 0x90 } else { 0x91 };
            crate::mc4000::apply(&ui, crate::mc4000::decode(&[channel, 13, 127]).unwrap());
            assert!(c.hold_nudge(id, 1, true).is_err());
            c.fine_pitch(id, true).unwrap();
            c.apply_pending_pitch();
            c.fine_pitch(id, true).unwrap();
            c.apply_pending_pitch();
            finish_keylock(&c, id);
            assert!((engine.keylock_factor(id).unwrap() - 1.0002).abs() < 1e-8);
            c.start_keylock(id, 1.04).unwrap();
            c.start_keylock(id, 0.96).unwrap();
            finish_keylock(&c, id);
            assert_eq!(engine.keylock_factor(id), Some(0.96));
            let follower = if id == DeckId::A {
                DeckId::B
            } else {
                DeckId::A
            };
            for deck in [DeckId::A, DeckId::B] {
                engine
                    .set_grid(
                        deck,
                        Some(
                            crate::beatgrid::BeatGrid::new(
                                if deck == follower { 118. } else { 120. },
                                Duration::ZERO,
                            )
                            .unwrap(),
                        ),
                    )
                    .unwrap();
                engine.cue(deck);
                engine.play(deck);
            }
            assert!((c.tempo_match_target(follower).unwrap() - 0.96 * 120. / 118.).abs() < 1e-10);
            assert!(c.phase_sync_ready(id).is_err());
            let version = engine.command_version(id);
            assert!(engine.apply_sync_factor(id, 1.021).is_err());
            assert!(engine
                .phase_sync_once(id, crate::phase_sync::Alignment::Beat)
                .is_err());
            assert_eq!(engine.command_version(id), version);
            assert_eq!(engine.snapshot(id).requested_speed, 0.96);
            engine.apply_sync_factor(id, 1.01).unwrap();
            assert_eq!(engine.snapshot(id).requested_speed, 0.96);
            engine.apply_sync_factor(id, 1.).unwrap();
            c.match_tempo(follower).unwrap();
            c.refresh();
            output.by_ref().take(1000).for_each(drop);
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(follower).unwrap();
            // Measure only reference PCM while both decks render.
            engine.set_levels(crate::playback::MixerLevels {
                crossfader: if id == DeckId::A { 0. } else { 1. },
                channel_a: 1.,
                channel_b: 1.,
            });
            let before = engine.snapshot(id).position.as_secs_f64();
            let mut reference_pcm = Vec::new();
            for _ in 0..20 {
                reference_pcm.extend(output.by_ref().take(1584));
                clock.set(clock.get() + Duration::from_millis(33));
                let phase = engine
                    .sync_observation(follower, crate::phase_sync::Alignment::Beat)
                    .unwrap()
                    .0;
                assert!(phase.abs() < 0.001, "Sync phase error: {phase}");
                c.maintain_sync(clock.get());
                assert!(c.sync_session.borrow().is_some(), "{}", ui.get_status());
            }
            let seconds = reference_pcm.len() as f64 / 48000.;
            assert!(
                (engine.snapshot(id).position.as_secs_f64() - before - seconds * 0.96).abs()
                    < 0.001
            );
            {
                let hz = reference_pcm[6000..30000]
                    .windows(2)
                    .filter(|s| s[0] <= 0. && s[1] > 0.)
                    .count() as f64
                    * 2.;
                assert!((hz - 440.).abs() < 3., "Reference pitch changed: {hz}");
            }
            // Disabling Keylock on the reference must stop Sync even when render-version values repeat.
            crate::mc4000::apply(&ui, crate::mc4000::decode(&[channel, 13, 127]).unwrap());
            assert!(c.sync_session.borrow().is_none());
            assert_eq!(engine.snapshot(follower).sync_factor, 1.);
            assert_eq!(engine.keylock_factor(id), None);
            crate::mc4000::apply(&ui, crate::mc4000::decode(&[channel, 13, 127]).unwrap());
            finish_keylock(&c, id);
            assert_eq!(engine.keylock_factor(id), Some(0.96));
            c.refresh();
            output.by_ref().take(1000).for_each(drop);
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(follower).unwrap();
            match id {
                DeckId::A => ui.invoke_nudge_left(1, true),
                DeckId::B => ui.invoke_nudge_right(1, true),
            }
            assert_eq!(engine.snapshot(id).nudge_direction, 1);
            c.maintain_sync(clock.get());
            assert!(c.sync_session.borrow().is_none());
            match id {
                DeckId::A => ui.invoke_nudge_left(1, false),
                DeckId::B => ui.invoke_nudge_right(1, false),
            }
            assert_eq!(engine.snapshot(id).nudge_direction, 0);
            match id {
                DeckId::A => ui.invoke_nudge_left(-1, true),
                DeckId::B => ui.invoke_nudge_right(-1, true),
            }
            assert_eq!(engine.snapshot(id).nudge_direction, -1);
            c.cancel_nudge(id);
            assert_eq!(engine.snapshot(id).nudge_direction, 0);
            // Exercise automatic Keylock acquisition, late-result cancellation and fresh-frame gating.
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(119., Duration::ZERO).unwrap()),
                )
                .unwrap();
            for deck in [DeckId::A, DeckId::B] {
                engine.cue(deck);
                engine.play(deck);
            }
            c.refresh();
            output.by_ref().take(1000).for_each(drop);
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            for cancellation in 0..4 {
                match id {
                    DeckId::A => ui.invoke_toggle_sync_left(),
                    DeckId::B => ui.invoke_toggle_sync_right(),
                }
                assert!(c.pending_sync.borrow().is_some(), "{}", ui.get_status());
                assert!(c.sync_session.borrow().is_none());
                assert!(match id {
                    DeckId::A => ui.get_left_sync_preparing(),
                    DeckId::B => ui.get_right_sync_preparing(),
                });
                let before = engine.keylock_factor(id);
                // Complete the native job without adopting it, then invalidate/cancel the request.
                wait_for_discardable_sync_result(&c, id);
                if cancellation == 0 {
                    match id {
                        DeckId::A => ui.invoke_toggle_sync_left(),
                        DeckId::B => ui.invoke_toggle_sync_right(),
                    }
                } else {
                    match cancellation {
                        1 => engine.cue(follower),
                        2 => clock.set(clock.get() + Duration::from_secs(31)),
                        _ => match follower {
                            DeckId::A => engine.load_a(first.path()).unwrap(),
                            DeckId::B => engine.load_b(first.path()).unwrap(),
                        },
                    }
                    c.poll_keylock();
                    c.poll_pending_sync();
                }
                assert!(c.pending_sync.borrow().is_none());
                assert!(c.sync_session.borrow().is_none());
                assert_eq!(engine.keylock_factor(id), before);
                assert!(c.keylock_jobs.borrow()[AudioController::grid_index(id)].is_none());
                if cancellation == 3 {
                    engine
                        .set_grid(
                            follower,
                            Some(crate::beatgrid::BeatGrid::new(118., Duration::ZERO).unwrap()),
                        )
                        .unwrap();
                }
                engine.play(follower);
                c.refresh();
                output.by_ref().take(1000).for_each(drop);
                clock.set(clock.get() + Duration::from_millis(1));
                c.refresh();
            }
            match id {
                DeckId::A => ui.invoke_toggle_sync_left(),
                DeckId::B => ui.invoke_toggle_sync_right(),
            }
            assert!(c.pending_sync.borrow().is_some(), "{}", ui.get_status());
            finish_keylock(&c, id);
            c.poll_pending_sync();
            assert!(
                c.pending_sync.borrow().is_some(),
                "Wait for first replacement frame"
            );
            assert!(c.sync_session.borrow().is_none());
            output.by_ref().take(1000).for_each(drop);
            clock.set(clock.get() + Duration::from_millis(1));
            c.poll_pending_sync();
            assert!(c.pending_sync.borrow().is_none(), "{}", ui.get_status());
            assert_eq!(c.sync_session.borrow().as_ref().unwrap().target, id);
            let prepared = engine.keylock_factor(id).unwrap();
            let mut pcm = Vec::new();
            for _ in 0..20 {
                pcm.extend(output.by_ref().take(1584));
                clock.set(clock.get() + Duration::from_millis(33));
                let phase = engine
                    .sync_observation(id, crate::phase_sync::Alignment::Beat)
                    .unwrap()
                    .0;
                assert!(phase.abs() < 0.001, "Keylock follower phase: {phase}");
                c.maintain_sync(clock.get());
                assert!(c.sync_session.borrow().is_some(), "{}", ui.get_status());
                assert!((0.98..=1.02).contains(&engine.snapshot(id).sync_factor));
                assert_eq!(engine.keylock_factor(id), Some(prepared));
                assert_eq!(engine.snapshot(id).requested_speed, prepared);
            }
            let hz = pcm[6000..30000]
                .windows(2)
                .filter(|s| s[0] <= 0. && s[1] > 0.)
                .count() as f64
                * 2.;
            assert!((hz - 440.).abs() < 3., "Keylock Sync pitch: {hz}");
            c.refresh();
            assert!(match id {
                DeckId::A => ui.get_left_sync_active(),
                DeckId::B => ui.get_right_sync_active(),
            });
            assert!(!match id {
                DeckId::A => ui.get_left_sync_preparing(),
                DeckId::B => ui.get_right_sync_preparing(),
            });
            clock.set(clock.get() + Duration::from_millis(101));
            c.maintain_sync(clock.get());
            assert!(c.sync_session.borrow().is_none());
            assert_eq!(engine.snapshot(id).sync_factor, 1.);
            c.start_keylock(id, 1.04).unwrap();
            assert!(c.hold_nudge(id, 1, true).is_err());
            assert!(c.sync_session.borrow().is_none());
            assert_eq!(engine.snapshot(follower).sync_factor, 1.);
            assert!(c.tempo_match_target(follower).is_err());
            c.start_keylock(id, 1.02).unwrap();
            match id {
                DeckId::A => engine.load_a(second.path()).unwrap(),
                DeckId::B => engine.load_b(second.path()).unwrap(),
            }
            finish_keylock(&c, id);
            assert_eq!(engine.keylock_factor(id), Some(1.0));
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(125., Duration::ZERO).unwrap()),
                )
                .unwrap();
            let matched = c.tempo_match_target(id).unwrap();
            let position = engine.snapshot(id).position;
            c.refresh();
            assert!(match id {
                DeckId::A => ui.get_left_can_match_tempo(),
                DeckId::B => ui.get_right_can_match_tempo(),
            });
            match id {
                DeckId::A => ui.invoke_match_tempo_left(),
                DeckId::B => ui.invoke_match_tempo_right(),
            }
            assert!(ui.get_status().contains("preparing Keylock tempo match"));
            assert_eq!(engine.keylock_factor(id), Some(1.));
            assert!(c.phase_sync_ready(id).is_err());
            finish_keylock(&c, id);
            assert!((engine.keylock_factor(id).unwrap() - matched).abs() < 1e-10);
            assert_eq!(engine.snapshot(id).position, position);
            assert!(ui.get_status().contains("Keylock tempo ready"));
            assert!(engine
                .phase_sync_once(id, crate::phase_sync::Alignment::Beat)
                .is_err());
            crate::mc4000::apply(&ui, crate::mc4000::decode(&[channel, 13, 127]).unwrap());
            assert_eq!(engine.keylock_factor(id), None);
        }
    }

    #[test]
    fn bus_peak_caption_handles_unavailable_silence_invalid_and_full_scale() {
        assert_eq!(peak_display(None), ("BUS PEAK \u{2014}".into(), 0., false));
        assert_eq!(
            peak_display(Some(0.)),
            ("BUS PEAK -\u{221e} dBFS".into(), 0., false)
        );
        assert_eq!(
            peak_display(Some(1.)),
            ("BUS PEAK +0.0 dBFS".into(), 1., true)
        );
        assert_eq!(
            peak_display(Some(2.)),
            ("BUS PEAK +6.0 dBFS".into(), 1., true)
        );
        assert_eq!(
            peak_display(Some(0.5)),
            ("BUS PEAK -6.0 dBFS".into(), 0.5, false)
        );
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.] {
            assert_eq!(
                peak_display(Some(invalid)),
                ("BUS INVALID".into(), 0., true)
            );
        }
    }

    #[test]
    fn delay_preferences_presets_latest_ack_and_close_restore() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db.clone()), vec![]);
        finish_storage(&c);
        ui.set_fx_active_a(true);
        ui.set_fx_wet_a(40.);
        ui.invoke_fx_changed(0);
        ui.invoke_fx_preset(0, 7, true);
        c.save_library();
        assert!(c.inflight_fx.borrow().is_some());
        ui.set_fx_time_b(600.);
        ui.invoke_fx_changed(1);
        finish_storage(&c);
        ui.set_fx_wet_a(80.);
        ui.invoke_fx_changed(0);
        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output2) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db), vec![]);
        finish_storage(&c);
        assert_eq!(engine.fx_settings(DeckId::A).wet, 0.8);
        assert_eq!(engine.fx_settings(DeckId::B).time_ms, 600);
        ui.invoke_fx_preset(0, 7, false);
        assert_eq!(engine.fx_settings(DeckId::A).wet, 0.4);
        assert!(engine.fx_settings(DeckId::A).enabled);
        assert_eq!(engine.fx_settings(DeckId::B).time_ms, 600);
        c.close_library().unwrap();
    }
    #[test]
    fn malformed_fx_preferences_remain_session_only_without_blocking_quantize() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let worker = crate::library_store::Worker::start(db.clone()).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            crate::library_store::Event::Loaded(Ok(_))
        ));
        worker.close_delta(None).unwrap();
        let sql = rusqlite::Connection::open(&db).unwrap();
        let raw = "{\"version\":2}";
        sql.execute(
            "INSERT INTO library_meta(key,value) VALUES('delay_fx_v1',?1)",
            [raw],
        )
        .unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db), vec![]);
        finish_storage(&c);
        ui.set_fx_active_a(true);
        ui.invoke_fx_changed(0);
        ui.set_quantize_enabled(false);
        finish_storage(&c);
        assert!(engine.fx_settings(DeckId::A).enabled);
        assert!(ui.get_fx_status().contains("session only"));
        c.close_library().unwrap();
        let saved: String = sql
            .query_row(
                "SELECT value FROM library_meta WHERE key='delay_fx_v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(saved, raw);
        let quantize: String = sql
            .query_row(
                "SELECT value FROM library_meta WHERE key='quantize_preferences_v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !serde_json::from_str::<crate::preferences::QuantizePreferences>(&quantize)
                .unwrap()
                .enabled
        );
    }
    #[test]
    fn mc4000_headphone_buttons_drive_cue_audio_and_keep_transport_unchanged() {
        i_slint_backend_testing::init_no_event_loop();
        for channels in [2, 4] {
            let ui = AppWindow::new().unwrap();
            let (mixer, mut output) = rodio::mixer::mixer(channels, 48000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, channels, 48000));
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            let a = wav_file(8192, 48000 * 3);
            let b = wav_file(16384, 48000 * 3);
            engine.load_a(a.path()).unwrap();
            engine.load_b(b.path()).unwrap();
            engine.set_levels(MixerLevels {
                channel_a: 0.,
                channel_b: 0.,
                crossfader: 0.5,
            });
            engine.play(DeckId::A);
            engine.play(DeckId::B);
            let message = |status, note, value| {
                if let Some(command) = crate::mc4000::decode(&[status, note, value]) {
                    crate::mc4000::apply(&ui, command);
                }
            };
            message(0x90, 27, 127);
            message(0x90, 27, 0);
            message(0x80, 27, 0);
            assert_eq!(engine.headphone_cue(DeckId::A), channels == 4);
            assert_eq!(ui.get_monitor_cue_a(), channels == 4);
            assert!(!engine.headphone_cue(DeckId::B));
            message(0x91, 27, 127);
            message(0x91, 27, 0);
            assert_eq!(engine.headphone_cue(DeckId::B), channels == 4);
            output.by_ref().take(10000).for_each(drop);
            let actual: Vec<_> = output.by_ref().take(usize::from(channels)).collect();
            assert_eq!(
                actual,
                if channels == 4 {
                    vec![0., 0., 0.75, 0.75]
                } else {
                    vec![0., 0.]
                }
            );
            assert!(engine.snapshot(DeckId::A).playing && engine.snapshot(DeckId::B).playing);
            if channels == 4 {
                message(0x90, 27, 127);
                message(0x91, 27, 127);
                engine.set_output_pair(1).unwrap();
                message(0x91, 27, 127);
                assert!(!ui.get_monitor_cue_b());
                assert!(!engine.headphone_cue(DeckId::B));
                assert!(ui.get_monitor_status().contains("master pair"));
            } else {
                assert!(ui.get_monitor_status().contains("four output channels"));
            }
            c.tick();
        }
    }
    #[test]
    fn headphone_monitor_gui_drives_pre_fader_bus_and_restores_rejected_selection() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(4, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 4, 48000));
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        let track = wav_file(8192, 48000 * 3);
        engine.load_b(track.path()).unwrap();
        engine.set_levels(MixerLevels {
            channel_a: 0.,
            channel_b: 0.,
            crossfader: 0.,
        });
        engine.play(DeckId::B);
        ui.show().unwrap();
        control(&ui, "Show headphone monitor").invoke_accessible_default_action();
        assert!(ui.get_monitor_open());
        control(&ui, "Deck B headphone cue").invoke_accessible_default_action();
        assert!(engine.headphone_cue(DeckId::B));
        assert!(ui.get_monitor_available());
        assert!(!ui.get_monitor_cue_a());
        output.by_ref().take(10000).for_each(drop);
        assert_eq!(
            output.by_ref().take(4).collect::<Vec<_>>(),
            vec![0., 0., 0.25, 0.25]
        );
        ui.set_monitor_gain(50.);
        ui.set_monitor_mix(50.);
        ui.invoke_monitor_changed();
        output.by_ref().take(12000).for_each(drop);
        assert_eq!(
            output.by_ref().take(4).collect::<Vec<_>>(),
            vec![0., 0., 0.0625, 0.0625]
        );
        ui.set_monitor_gain(f32::NAN);
        ui.invoke_monitor_changed();
        assert_eq!(ui.get_monitor_gain(), 50.);
        for bytes in [[0xbf, 12, 64], [0xbf, 13, 127]] {
            crate::mc4000::apply(&ui, crate::mc4000::decode(&bytes).unwrap());
        }
        assert!((engine.monitor_settings().gain - 64. / 127.).abs() < 1e-6);
        assert_eq!(engine.monitor_settings().mix, 1.);
        for bytes in [[0xbf, 12, 127], [0xbf, 13, 0]] {
            crate::mc4000::apply(&ui, crate::mc4000::decode(&bytes).unwrap());
        }
        ui.set_monitor_cue_b(false);
        ui.invoke_monitor_cue_changed(1);
        engine.set_output_pair(1).unwrap();
        ui.set_monitor_cue_a(true);
        ui.invoke_monitor_cue_changed(0);
        assert!(!ui.get_monitor_cue_a());
        assert!(ui.get_monitor_status().contains("master pair"));
        assert!(!engine.headphone_cue(DeckId::A));
        c.tick();
        assert!(engine.snapshot(DeckId::B).playing);
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            for label in [
                "Deck A headphone cue",
                "Deck B headphone cue",
                "Close headphone monitor",
                "Headphone monitor gain",
                "Headphone cue master mix",
            ] {
                let item = control(&ui, label);
                assert!(item.size().width > 0. && item.size().height > 0.);
                let pos = item.absolute_position();
                assert!(pos.x >= 0. && pos.y >= 0.);
                assert!(pos.x + item.size().width <= width + 1.);
                assert!(pos.y + item.size().height <= height + 1.);
            }
        }
        ui.hide().unwrap();
    }
    #[test]
    fn channel_meter_gui_is_pre_fader_independent_and_clip_hold_is_explicit() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        let a = wav_file(16384, 48000 * 3);
        let b = wav_file(4096, 48000 * 3);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        engine.set_levels(MixerLevels {
            crossfader: 0.,
            channel_a: 0.,
            channel_b: 0.,
        });
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let mut pull = || {
            for _ in 0..10000 {
                output.next();
            }
            c.tick();
        };
        pull();
        assert_eq!(engine.output_peak(), Some(0.));
        assert_eq!(engine.channel_peak(DeckId::A), Some(0.5));
        assert_eq!(engine.channel_peak(DeckId::B), Some(0.125));
        assert_eq!(ui.get_channel_a_peak_caption(), "A PRE PEAK -6.0 dBFS");
        engine
            .set_eq(
                DeckId::A,
                crate::mixer_dsp::EqSettings {
                    trim: 12.,
                    ..Default::default()
                },
            )
            .unwrap();
        pull();
        assert!(engine.channel_peak(DeckId::A).unwrap() > 1.9);
        assert!(ui.get_channel_a_clipped());
        assert!(!ui.get_channel_b_clipped());
        assert_eq!(engine.output_peak(), Some(0.));
        engine
            .set_eq(DeckId::A, crate::mixer_dsp::EqSettings::default())
            .unwrap();
        pull();
        assert_eq!(engine.channel_peak(DeckId::A), Some(0.5));
        assert!(ui.get_channel_a_clipped());
        ui.invoke_clear_channel_clip(1);
        assert!(ui.get_channel_a_clipped());
        ui.show().unwrap();
        control(&ui, "Deck A clear clip hold").invoke_accessible_default_action();
        assert!(!ui.get_channel_a_clipped());
        engine
            .set_eq(
                DeckId::A,
                crate::mixer_dsp::EqSettings {
                    trim: 12.,
                    ..Default::default()
                },
            )
            .unwrap();
        pull();
        assert!(ui.get_channel_a_clipped());
        engine.pause(DeckId::A);
        pull();
        assert!(ui.get_channel_a_peak_caption().contains("LAST"));
        engine.load_a(a.path()).unwrap();
        c.tick();
        assert_eq!(engine.channel_peak(DeckId::A), None);
        assert!(
            ui.get_channel_a_clipped(),
            "clip history survives track replacement until clear"
        );
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            for deck in ["A", "B"] {
                let meter = control(&ui, &format!("Deck {deck} pre-fader sample peak"));
                assert!(meter.size().width > 100. && meter.size().height > 0.);
                assert!(meter.absolute_position().x >= 0.);
                assert!(meter.absolute_position().x + meter.size().width <= width + 1.);
                assert!(
                    meter.absolute_position().y + meter.size().height
                        <= control(&ui, "Shared waveform area").absolute_position().y + 1.
                );
            }
        }
        control(&ui, "Deck A clear clip hold").invoke_accessible_default_action();
        assert!(!engine.channel_clipped(DeckId::A));
        finish_search(&c);
        ui.hide().unwrap();
    }
    #[test]
    fn bus_peak_gui_measures_actual_post_fader_mix_pause_and_silence() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        assert_eq!(engine.output_peak(), None);
        assert_eq!(ui.get_output_peak_level(), 0.);
        assert!(ui.get_output_peak_caption().contains('\u{2014}'));
        let a = wav_file(8192, 48_000 * 3);
        let b = wav_file(4096, 48_000 * 3);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let mut render = || {
            output.by_ref().take(9600).for_each(drop);
            c.tick();
        };
        render();
        assert!((engine.output_peak().unwrap() - 0.1875).abs() < 0.00001);
        assert!((ui.get_output_peak_level() - 0.1875).abs() < 0.00001);
        assert_eq!(ui.get_output_peak_caption(), "BUS PEAK -14.5 dBFS");
        control(&ui, "Channel A fader").set_accessible_value("0.8");
        control(&ui, "Channel B fader").set_accessible_value("0.4");
        control(&ui, "Crossfader").set_accessible_value("0.25");
        render();
        assert!((engine.output_peak().unwrap() - 0.1625).abs() < 0.00001);
        control(&ui, "Crossfader").set_accessible_value("1");
        render();
        assert!((ui.get_output_peak_level() - 0.05).abs() < 0.00001);
        control(&ui, "Crossfader").set_accessible_value("0");
        render();
        assert!((ui.get_output_peak_level() - 0.2).abs() < 0.00001);
        let held = ui.get_output_peak_caption();
        c.tick(); // No output pulled: a last-window meter must retain its real prior reading.
        assert_eq!(ui.get_output_peak_caption(), held);
        engine.pause(DeckId::A);
        engine.pause(DeckId::B);
        render();
        assert_eq!(engine.output_peak(), Some(0.));
        assert_eq!(ui.get_output_peak_level(), 0.);
        assert!(!ui.get_output_peak_warning());
        assert_eq!(ui.get_output_peak_caption(), "BUS PEAK -\u{221e} dBFS");
        ui.show().unwrap();
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            let meter = control(&ui, "Post-fader bus sample peak");
            assert!(meter.size().width >= 170. && meter.size().height >= 22.);
            assert!(meter.absolute_position().x >= 0.);
            assert!(meter.absolute_position().x + meter.size().width <= width + 0.1);
            assert!(
                meter.absolute_position().y + meter.size().height
                    <= control(&ui, "Shared waveform area").absolute_position().y
            );
            for label in ["Channel A fader", "Crossfader", "Channel B fader"] {
                let slider = control(&ui, label);
                assert!(slider.size().width > 0.);
                assert!(
                    slider.absolute_position().x + slider.size().width
                        <= meter.absolute_position().x + 0.1
                );
            }
        }
    }

    #[test]
    fn lazy_library_model_only_materializes_viewport_rows_and_invalidates_old_catalogs() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(&ui, engine, Arc::new(|| None), Arc::new(|| None));
        let dir = tempfile::tempdir().unwrap();
        let count = 20_000;
        c.library
            .borrow_mut()
            .insert((0..count).map(|i| dir.path().join(format!("track-{i:05}.wav"))));
        let library = c.library.borrow();
        let visible = library.visible();
        let lazy_allocations = crate::looping::counted_allocations(|| {
            let ids: Vec<i32> = visible.iter().map(|track| track.id).collect();
            std::hint::black_box(ids);
        });
        let eager_allocations = crate::looping::counted_allocations(|| {
            let rows: Vec<_> = visible.iter().map(|track| library_row(track)).collect();
            std::hint::black_box(rows);
        });
        assert!(lazy_allocations < 5);
        assert!(eager_allocations > count * 3);
        println!("20,000-row preparation: lazy {lazy_allocations} allocations; eager {eager_allocations}");
        drop(visible);
        drop(library);
        ui.window().set_size(slint::LogicalSize::new(850., 740.));
        c.publish_library();
        finish_search(&c);
        ui.show().unwrap();
        let old_model = ui.get_library_rows();
        assert_eq!(old_model.row_count(), LIBRARY_PAGE_SIZE);
        let model = old_model.as_any().downcast_ref::<LibraryRows>().unwrap();
        control(&ui, "Library track 1");
        let requested = model.row_data_calls.get();
        assert!(
            requested > 0 && requested < 1000,
            "viewport requested {requested} rows"
        );
        assert_eq!(old_model.row_data(0).unwrap().id, 1);
        assert_eq!(
            old_model.row_data(LIBRARY_PAGE_SIZE - 1).unwrap().id,
            LIBRARY_PAGE_SIZE as i32
        );
        assert!(old_model.row_data(LIBRARY_PAGE_SIZE).is_none());
        let locked = c.library.borrow_mut();
        assert!(old_model.row_data(0).is_none());
        drop(locked);
        assert_eq!(old_model.row_data(0).unwrap().id, 1);
        control(&ui, "Sort library by name").invoke_accessible_default_action();
        finish_search(&c);
        let reverse = ui.get_library_rows();
        assert_eq!(reverse.row_data(0).unwrap().id, count as i32);
        control(&ui, "Library search").set_accessible_value("track-00001");
        finish_search(&c);
        let filtered = ui.get_library_rows();
        assert_eq!(filtered.row_count(), 1);
        assert_eq!(filtered.row_data(0).unwrap().id, 2);
        c.restore_library(Ok(dir.path().join("empty.sqlite3")), Vec::new());
        finish_storage(&c);
        assert!(old_model.row_data(0).is_none());
        assert!(reverse.row_data(0).is_none());
        assert!(filtered.row_data(0).is_none());
        assert_eq!(ui.get_library_rows().row_count(), 0);
        c.close_library().unwrap();
        drop(c);
        assert!(old_model.row_data(0).is_none());
        println!("20,000 rows: headless viewport requested {requested} model rows");
    }

    #[test]
    fn small_library_always_searches_in_background_and_clears_selection_on_failure() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        assert!(c.search_in_progress());
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 0);
        let dir = tempfile::tempdir().unwrap();
        c.library
            .borrow_mut()
            .insert([dir.path().join("one.wav"), dir.path().join("two.mp3")]);
        c.publish_library();
        assert!(c.search_in_progress());
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 2);
        ui.invoke_sort_library(0);
        assert!(c.search_in_progress());
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 2);
        ui.invoke_select_library(1);
        assert_eq!(ui.get_selected_track(), 1);
        let perf = Rc::new(RefCell::new(crate::library_perf::Recorder::new(
            Instant::now(),
        )));
        c.record_library_performance(Rc::clone(&perf));
        let (sender, input) = mpsc::sync_channel(1);
        drop(input);
        let (output, receiver) = mpsc::sync_channel(1);
        drop(output);
        *c.search_worker.borrow_mut() = Some(SearchWorker {
            sender,
            receiver,
            pending: None,
            wanted: None,
            projection: None,
            published: None,
            published_total: 0,
        });
        ui.set_library_query("no-match".into());
        ui.invoke_library_search();
        assert_eq!(c.library.borrow().selected_id(), -1);
        assert_eq!(ui.get_selected_track(), -1);
        assert_eq!(ui.get_library_rows().row_count(), 0);
        assert!(ui.get_library_empty().contains("unavailable"));
        assert_eq!(
            perf.borrow().report(Instant::now()).outcome,
            "measurement_failed"
        );
        ui.set_library_query("one".into());
        ui.invoke_library_search();
        assert!(c.search_in_progress());
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 1);
    }

    #[test]
    fn library_performance_records_handlers_and_installed_results_before_render() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(2, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48_000));
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        let dir = tempfile::tempdir().unwrap();
        c.library.borrow_mut().insert(
            (0..crate::library_perf::MIN_TRACKS)
                .map(|i| dir.path().join(format!("track-{i:06}.wav"))),
        );
        c.publish_library();
        finish_search(&c);
        let wav = wav_file(8192, 192000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(wav.path()).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let perf = Rc::new(RefCell::new(crate::library_perf::Recorder::new(
            Instant::now(),
        )));
        c.record_library_performance(Rc::clone(&perf));
        c.tick();
        output.by_ref().take(4800).for_each(drop);
        c.tick();
        let frames = engine.rendered_frames();
        ui.set_library_query("track".into());
        ui.invoke_library_search();
        finish_search(&c);
        let report = serde_json::to_value(perf.borrow().report(Instant::now())).unwrap();
        assert_eq!(report["samples"].as_array().unwrap().len(), 0);
        assert_eq!(report["unfinished_actions"], 1);
        // The testing backend has no native rendering; inject only the notifier boundary.
        perf.borrow_mut().rendered(Instant::now());
        ui.invoke_sort_library(0);
        finish_search(&c);
        perf.borrow_mut().rendered(Instant::now());
        ui.invoke_library_change_page(1);
        finish_search(&c);
        perf.borrow_mut().rendered(Instant::now());
        let report = serde_json::to_value(perf.borrow().report(Instant::now())).unwrap();
        let samples = report["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 3);
        for (sample, operation) in samples.iter().zip(["search", "sort", "page"]) {
            assert_eq!(sample["operation"], operation);
            assert!(
                sample["after_rendering_ms"].as_f64().unwrap()
                    >= sample["model_ms"].as_f64().unwrap()
            );
        }
        assert_eq!(report["outcome"], "insufficient_coverage");
        assert_eq!(engine.rendered_frames(), frames);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).playing);
        assert!(ui.get_library_performance().starts_with("PERF"));
        let small_crate = c
            .library
            .borrow_mut()
            .create_crate("Small measured scope")
            .unwrap();
        c.library.borrow_mut().select_crate(small_crate).unwrap();
        assert_eq!(c.library.borrow().len(), crate::library_perf::MIN_TRACKS);
        assert_eq!(c.library_perf_context().0, 0);
        ui.invoke_sort_library(0);
        finish_search(&c);
        perf.borrow_mut().rendered(Instant::now());
        let report = serde_json::to_value(perf.borrow().report(Instant::now())).unwrap();
        assert_eq!(
            report["samples"].as_array().unwrap().last().unwrap()["under_load"],
            false
        );
        engine.pause(DeckId::A);
        engine.pause(DeckId::B);
    }

    #[test]
    fn search_index_is_shared_across_scopes_but_not_revisions() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        finish_search(&c);
        let path = std::path::PathBuf::from("shared-index.wav");
        {
            let mut library = c.library.borrow_mut();
            library.insert([path.clone()]);
            let id = library.create_crate("scope").unwrap();
            library
                .add_paths_to_crate(id, std::slice::from_ref(&path))
                .unwrap();
            library.set_query("shared".into());
        }
        c.publish_library();
        finish_search(&c);
        let old = Arc::clone(
            &c.search_worker
                .borrow()
                .as_ref()
                .unwrap()
                .projection
                .as_ref()
                .unwrap()
                .3,
        );
        c.library.borrow_mut().select_crate(1).unwrap();
        c.publish_library();
        finish_search(&c);
        let scoped = Arc::clone(
            &c.search_worker
                .borrow()
                .as_ref()
                .unwrap()
                .projection
                .as_ref()
                .unwrap()
                .3,
        );
        assert!(old.shares_index_with(&scoped));
        assert_eq!(ui.get_library_rows().row_count(), 1);
        c.library.borrow_mut().set_metadata(
            &path,
            crate::track_metadata::TrackMetadata {
                title: "new unique title".into(),
                ..Default::default()
            },
        );
        c.library.borrow_mut().set_query("unique".into());
        c.publish_library();
        finish_search(&c);
        let updated = Arc::clone(
            &c.search_worker
                .borrow()
                .as_ref()
                .unwrap()
                .projection
                .as_ref()
                .unwrap()
                .3,
        );
        assert!(!scoped.shares_index_with(&updated));
        assert_eq!(ui.get_library_rows().row_count(), 1);
        c.library_generation.set(c.library_generation.get() + 1);
        c.publish_library();
        finish_search(&c);
        let next_generation = Arc::clone(
            &c.search_worker
                .borrow()
                .as_ref()
                .unwrap()
                .projection
                .as_ref()
                .unwrap()
                .3,
        );
        assert!(!updated.shares_index_with(&next_generation));
    }

    #[test]
    fn paged_worker_returns_correct_boundaries_and_global_order() {
        let dir = tempfile::tempdir().unwrap();
        let worker = SearchWorker::start().unwrap();
        for size in [0_usize, 1, 199, 200, 201, 400, 401] {
            let mut library = crate::library::Library::default();
            library.insert((0..size).map(|i| dir.path().join(format!("track-{i:04}.wav"))));
            let projection = Arc::new(library.search_projection());
            let mut spec = library.search_spec(17);
            for requested in [0, 1, 99] {
                spec.page_index = requested;
                worker
                    .sender
                    .send((Arc::clone(&projection), spec.clone()))
                    .unwrap();
                let (received, page) = worker
                    .receiver
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap();
                assert_eq!(received.page_index, requested);
                let expected_page = requested.min(size.saturating_sub(1) / LIBRARY_PAGE_SIZE);
                let start = expected_page * LIBRARY_PAGE_SIZE;
                let end = (start + LIBRARY_PAGE_SIZE).min(size);
                assert_eq!(page.total, size);
                assert_eq!(page.page_index, expected_page);
                assert_eq!(
                    page.ids,
                    (start..end).map(|i| i as i32 + 1).collect::<Vec<_>>()
                );
            }
            library.sort(0);
            let projection = Arc::new(library.search_projection());
            let spec = library.search_spec(17);
            worker.sender.send((projection, spec)).unwrap();
            let (_, page) = worker
                .receiver
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
            assert_eq!(page.total, size);
            assert_eq!(
                page.ids,
                (0..size)
                    .rev()
                    .take(LIBRARY_PAGE_SIZE)
                    .map(|i| i as i32 + 1)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn page_navigation_resets_filters_and_keeps_keyboard_edge_visible() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        let dir = tempfile::tempdir().unwrap();
        c.library
            .borrow_mut()
            .insert((0..401).map(|i| dir.path().join(format!("track-{i:04}.wav"))));
        ui.window().set_size(slint::LogicalSize::new(850., 740.));
        ui.show().unwrap();
        c.publish_library();
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "1 / 3");
        assert_eq!(ui.get_library_rows().row_count(), 200);
        ui.invoke_select_library(201);
        assert_eq!(ui.get_selected_track(), -1);
        ui.invoke_select_library(200);
        ui.invoke_move_library_selection(1);
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "2 / 3");
        assert_eq!(ui.get_selected_track(), 201);
        assert_eq!(ui.get_library_scroll_y(), 0.0);
        ui.invoke_move_library_selection(-1);
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "1 / 3");
        assert_eq!(ui.get_selected_track(), 200);
        assert!(ui.get_library_scroll_y() < 0.0);
        let selected_row = control(&ui, "Library track 200");
        let library_area = control(&ui, "Track library area");
        assert!(selected_row.absolute_position().y >= library_area.absolute_position().y);
        assert!(
            selected_row.absolute_position().y + selected_row.size().height
                <= library_area.absolute_position().y + library_area.size().height
        );
        ui.invoke_library_change_page(1);
        finish_search(&c);
        assert_eq!(ui.get_selected_track(), -1);
        assert_eq!(ui.get_library_scroll_y(), 0.0);
        ui.invoke_library_change_page(1);
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "3 / 3");
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 401);
        assert!(!ui.get_library_can_next_page());
        ui.invoke_library_change_page(-1);
        assert!(c.search_in_progress());
        ui.set_library_query("track-0001".into());
        ui.invoke_library_search();
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "1 / 1");
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 2);

        ui.set_library_query(String::new().into());
        ui.invoke_library_search();
        finish_search(&c);
        let crate_id = c.library.borrow_mut().create_crate("subset").unwrap();
        let members: Vec<_> = (0..201)
            .map(|i| dir.path().join(format!("track-{i:04}.wav")))
            .collect();
        c.library
            .borrow_mut()
            .add_paths_to_crate(crate_id, &members)
            .unwrap();
        c.library.borrow_mut().select_crate(crate_id).unwrap();
        c.publish_library();
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "1 / 2");
        ui.invoke_library_change_page(1);
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "2 / 2");
        ui.invoke_select_library(201);
        assert!(c.library.borrow_mut().remove_selected_from_crate().unwrap());
        c.publish_library();
        finish_search(&c);
        assert_eq!(ui.get_library_page_text(), "1 / 1");
        assert_eq!(ui.get_library_rows().row_count(), 200);
        assert_eq!(ui.get_selected_track(), -1);
    }

    // Explicitly opt in: reads the user's music collection and never writes to it.
    #[test]
    #[ignore = "requires DRDJ_MUSIC_ROOT and real media files"]
    fn real_music_collection_paged_search_and_sort_qualification() {
        use slint::Model;
        let root =
            std::path::PathBuf::from(std::env::var_os("DRDJ_MUSIC_ROOT").expect("DRDJ_MUSIC_ROOT"));
        let mut pending = vec![root];
        let mut paths = Vec::new();
        let mut inaccessible = 0_usize;
        let mut sidecars = 0_usize;
        while let Some(dir) = pending.pop() {
            let entries = match std::fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => {
                    inaccessible += 1;
                    continue;
                }
            };
            for entry in entries {
                let Ok(entry) = entry else {
                    inaccessible += 1;
                    continue;
                };
                let Ok(kind) = entry.file_type() else {
                    inaccessible += 1;
                    continue;
                };
                if kind.is_dir() {
                    pending.push(entry.path());
                } else if kind.is_file() && crate::audio_formats::supported(&entry.path()) {
                    if crate::audio_formats::is_apple_double(&entry.path()) {
                        sidecars += 1;
                    } else {
                        paths.push(entry.path());
                    }
                }
            }
        }
        paths.sort();
        assert!(
            paths.len() > LIBRARY_PAGE_SIZE,
            "collection must exceed one page"
        );
        assert!(paths.len() <= crate::library::MAX_TRACKS);
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        finish_search(&c);
        assert_eq!(c.library.borrow_mut().insert(paths.clone()), paths.len());
        let start = Instant::now();
        let mut errors = 0;
        let mut tag_warnings = 0;
        let mut container_warnings = 0;
        let mut failure_groups = std::collections::BTreeMap::<String, usize>::new();
        let mut failures = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let metadata = crate::track_metadata::read(path);
            let tag_warning = metadata
                .error
                .as_deref()
                .is_some_and(|error| error.starts_with(crate::media_input::TAG_WARNING));
            tag_warnings += usize::from(tag_warning);
            let container_warning =
                metadata.error.as_deref() == Some(crate::media_input::WAVE_WARNING);
            container_warnings += usize::from(container_warning);
            errors += usize::from(metadata.error.is_some() && !tag_warning && !container_warning);
            if let Some(error) = &metadata.error {
                use std::io::Read;
                *failure_groups.entry(error.clone()).or_default() += 1;
                let inspection = (|| -> std::io::Result<_> {
                    let mut file = std::fs::File::open(path)?;
                    let length = file.metadata()?.len();
                    let mut header = [0_u8; 10];
                    let count = file.read(&mut header)?;
                    Ok((length, header[..count].to_vec()))
                })();
                let (length, header, inspection_error) = match inspection {
                    Ok((length, header)) => (Some(length), Some(header), None),
                    Err(error) => (None, None, Some(error.to_string())),
                };
                failures.push(serde_json::json!({
                    "path": path,
                    "error": error,
                    "tag_warning": tag_warning,
                    "container_warning": container_warning,
                    "bytes": length,
                    "header": header,
                    "inspection_error": inspection_error,
                }));
            }
            c.library.borrow_mut().set_metadata(path, metadata);
            if (index + 1) % 1000 == 0 {
                eprintln!("metadata processed: {}", index + 1);
            }
        }
        let metadata_time = start.elapsed();
        assert_eq!(
            failure_groups.values().sum::<usize>(),
            errors + tag_warnings + container_warnings
        );
        if let Some(report) = std::env::var_os("DRDJ_METADATA_REPORT") {
            // Nur ein explizites Diagnoseziel schreiben; Musikdateien bleiben unverändert.
            let report = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(report)
                .expect("diagnostic report must use a new path");
            serde_json::to_writer_pretty(
                report,
                &serde_json::json!({
                    "files": paths.len(),
                    "inaccessible": inaccessible,
                    "excluded_apple_double": sidecars,
                    "audio_probe_errors": errors,
                    "skipped_tag_warnings": tag_warnings,
                    "container_warnings": container_warnings,
                    "error_groups": failure_groups,
                    "failures": failures,
                }),
            )
            .unwrap();
        }
        println!("metadata failure groups: {failure_groups:?}");
        ui.set_library_query(".mp3".into());
        let start = Instant::now();
        c.publish_library();
        let dispatch = start.elapsed();
        finish_search(&c);
        let completion = start.elapsed();
        let expected = c.library.borrow().reference_visible_ids();
        assert!(expected.len() > LIBRARY_PAGE_SIZE);
        for page in 0..expected.len().div_ceil(LIBRARY_PAGE_SIZE) {
            if page > 0 {
                ui.invoke_library_change_page(1);
                finish_search(&c);
            }
            let model = ui.get_library_rows();
            let first = page * LIBRARY_PAGE_SIZE;
            let last = (first + LIBRARY_PAGE_SIZE).min(expected.len());
            assert_eq!(model.row_count(), last - first);
            for (row, expected_id) in expected[first..last].iter().enumerate() {
                assert_eq!(model.row_data(row).unwrap().id, *expected_id);
            }
        }
        c.library.borrow_mut().set_metadata(
            &paths[0],
            crate::track_metadata::TrackMetadata {
                title: "qualification-unique-unsaved-marker".into(),
                ..Default::default()
            },
        );
        ui.set_library_query("qualification-unique-unsaved-marker".into());
        c.publish_library();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert!(c.library.borrow().has_changes());
        println!("real collection: files={} metadata_errors={} skipped_tag_warnings={} inaccessible={} metadata={:?} search_dispatch={:?} search_completion={:?} checked_rows={}", paths.len(), errors, tag_warnings, inaccessible, metadata_time, dispatch, completion, expected.len());
    }

    #[test]
    fn large_library_headless_view_search_sort_and_selection_at_track_limit() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(&ui, engine, Arc::new(|| None), Arc::new(|| None));
        let dir = tempfile::tempdir().unwrap();
        let count = crate::library::MAX_TRACKS;
        let start = Instant::now();
        assert_eq!(
            c.library
                .borrow_mut()
                .insert((0..count).map(|i| dir.path().join(format!("track-{i:06}.wav")))),
            count
        );
        let insert_time = start.elapsed();
        ui.window().set_size(slint::LogicalSize::new(850., 740.));
        ui.show().unwrap();
        let start = Instant::now();
        c.publish_library();
        let publish_time = start.elapsed();
        finish_search(&c);
        let initial_completion = start.elapsed();
        let model = ui.get_library_rows();
        assert_eq!(model.row_count(), LIBRARY_PAGE_SIZE);
        control(&ui, "Library track 1");
        let requested = model
            .as_any()
            .downcast_ref::<LibraryRows>()
            .unwrap()
            .row_data_calls
            .get();
        assert!(
            requested > 0 && requested < 1000,
            "viewport requested {requested} rows"
        );
        assert_eq!(model.row_data(0).unwrap().id, 1);
        assert_eq!(
            model.row_data(LIBRARY_PAGE_SIZE - 1).unwrap().id,
            LIBRARY_PAGE_SIZE as i32
        );
        let start = Instant::now();
        control(&ui, "Library search").set_accessible_value(format!("track-{:06}", count - 1));
        let search_time = start.elapsed();
        finish_search(&c);
        let search_completion = start.elapsed();
        let filtered = ui.get_library_rows();
        assert_eq!(filtered.row_count(), 1);
        assert_eq!(filtered.row_data(0).unwrap().id, count as i32);
        control(&ui, &format!("Library track {count}")).invoke_accessible_default_action();
        assert_eq!(ui.get_selected_track(), count as i32);
        let start = Instant::now();
        control(&ui, "Library search").set_accessible_value("track");
        let broad_time = start.elapsed();
        finish_search(&c);
        let broad_completion = start.elapsed();
        assert_eq!(ui.get_library_rows().row_count(), LIBRARY_PAGE_SIZE);
        control(&ui, "Sort library by name").invoke_accessible_default_action();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, count as i32);
        assert_eq!(ui.get_selected_track(), -1);

        // Changes made while a result is in flight must trigger a new search,
        // even if that change did not itself request a new library publication.
        control(&ui, "Library search").set_accessible_value("unique-live-title");
        assert!(c.search_in_progress());
        let changed_path = dir.path().join("track-000042.wav");
        c.library.borrow_mut().set_metadata(
            &changed_path,
            crate::track_metadata::TrackMetadata {
                title: "unique-live-title".into(),
                ..Default::default()
            },
        );
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 43);

        // Several requests can be outstanding, but only the latest is shown.
        control(&ui, "Library search").set_accessible_value("track-000001");
        control(&ui, "Library search").set_accessible_value("track-000002");
        control(&ui, "Library search").set_accessible_value("track-000003");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 4);

        control(&ui, "Library search").set_accessible_value("track-000004");
        c.library_generation.set(c.library_generation.get() + 1);
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 5);

        let (sender, receiver_end) = mpsc::sync_channel(1);
        drop(receiver_end);
        let (sender_end, receiver) = mpsc::sync_channel(1);
        drop(sender_end);
        *c.search_worker.borrow_mut() = Some(SearchWorker {
            sender,
            receiver,
            pending: None,
            wanted: None,
            projection: None,
            published: None,
            published_total: 0,
        });
        control(&ui, "Library search").set_accessible_value("track-000005");
        assert!(!c.search_in_progress());
        assert_eq!(ui.get_library_rows().row_count(), 0);
        assert!(ui.get_library_empty().contains("unavailable"));
        control(&ui, "Library search").set_accessible_value("track-000006");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 7);
        let crate_id = c.library.borrow_mut().create_crate("search scope").unwrap();
        c.library
            .borrow_mut()
            .add_paths_to_crate(crate_id, &[dir.path().join("track-000006.wav")])
            .unwrap();
        c.library.borrow_mut().select_crate(crate_id).unwrap();
        c.publish_library();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 7);
        control(&ui, "Library search").set_accessible_value("track");
        c.library
            .borrow_mut()
            .add_paths_to_crate(crate_id, &[dir.path().join("track-000007.wav")])
            .unwrap();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 2);

        control(&ui, "Library search").set_accessible_value("track-000007");
        *c.library.borrow_mut() = crate::library::Library::default();
        c.library_generation.set(c.library_generation.get() + 1);
        c.publish_library();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 0);

        // Exercise disconnect while polling, independently of a failed send.
        let (sender, _request_receiver) = mpsc::sync_channel(1);
        let (response_sender, receiver) = mpsc::sync_channel(1);
        drop(response_sender);
        *c.search_worker.borrow_mut() = Some(SearchWorker {
            sender,
            receiver,
            pending: None,
            wanted: None,
            projection: None,
            published: None,
            published_total: 0,
        });
        c.poll_search();
        assert!(c.search_worker.borrow().is_none());
        assert!(ui.get_library_empty().contains("unavailable"));
        println!("100,000 synthetic tracks: insert {insert_time:?}, initial dispatch {publish_time:?}, initial completion {initial_completion:?}, selective dispatch {search_time:?}, selective completion {search_completion:?}, broad dispatch {broad_time:?}, broad completion {broad_completion:?}; viewport requested {requested} rows");
    }

    #[test]
    fn source_time_formatting_handles_boundaries_long_tracks_and_overrun() {
        assert_eq!(precise_time(Duration::from_nanos(999_999)), "00:00.000");
        assert_eq!(precise_time(Duration::from_millis(59_999)), "00:59.999");
        assert_eq!(precise_time(Duration::from_secs(60)), "01:00.000");
        assert_eq!(precise_time(Duration::from_millis(3_600_123)), "60:00.123");
        assert_eq!(precise_time(Duration::MAX), "307445734561825860:15.999");
        assert_eq!(
            deck_time(Duration::ZERO, Duration::ZERO),
            "ELAPSED 00:00.000   LEFT 00:00.000\nTOTAL 00:00.000"
        );
        assert_eq!(
            deck_time(Duration::from_millis(250), Duration::from_millis(1_501)),
            "ELAPSED 00:00.250   LEFT 00:01.251\nTOTAL 00:01.501"
        );
        assert_eq!(
            deck_time(Duration::from_secs(3), Duration::from_secs(2)),
            "ELAPSED 00:02.000   LEFT 00:00.000\nTOTAL 00:02.000"
        );
    }

    #[test]
    fn source_time_display_tracks_both_decks_seek_pitch_pause_and_eof() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        assert_eq!(
            ui.get_left_time(),
            "ELAPSED 00:00.000   LEFT 00:00.000\nTOTAL 00:00.000"
        );
        let file = wav_file(8192, 96_000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_millis(1_250))
            .unwrap();
        engine
            .seek_position(DeckId::B, Duration::from_millis(500))
            .unwrap();
        engine.set_speed(DeckId::A, 2.).unwrap();
        engine.set_speed(DeckId::B, 0.5).unwrap();
        c.tick();
        let a = "ELAPSED 00:01.250   LEFT 00:00.750\nTOTAL 00:02.000";
        let b = "ELAPSED 00:00.500   LEFT 00:01.500\nTOTAL 00:02.000";
        assert_eq!(ui.get_left_time(), a);
        assert_eq!(ui.get_right_time(), b);
        engine.play(DeckId::A);
        c.tick();
        assert_eq!(ui.get_left_time(), a); // No produced frames: no invented time progression.
        engine.pause(DeckId::A);
        c.tick();
        assert_eq!(ui.get_left_time(), a);
        ui.show().unwrap();
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            let shared = control(&ui, "Shared waveform area");
            for deck in ["DECK A", "DECK B"] {
                let time = control(&ui, &format!("{deck} source time"));
                let seek = control(&ui, &format!("{deck} position"));
                assert!(time.size().height >= 22., "both time lines have height");
                assert!(time.size().width >= 275., "compact time row has width");
                assert!(time.absolute_position().x >= 0.);
                assert!(time.absolute_position().x + time.size().width <= width + 0.1);
                assert!(
                    time.absolute_position().y + time.size().height
                        <= seek.absolute_position().y + 0.1
                );
                assert!(seek.absolute_position().y < shared.absolute_position().y);
            }
        }
        control(&ui, "DECK A position").set_accessible_value("100");
        c.tick();
        assert_eq!(
            ui.get_left_time(),
            "ELAPSED 00:02.000   LEFT 00:00.000\nTOTAL 00:02.000"
        );
        assert_eq!(ui.get_right_time(), b);
        control(&ui, "DECK B cue").invoke_accessible_default_action();
        c.tick();
        assert_eq!(
            ui.get_right_time(),
            "ELAPSED 00:00.000   LEFT 00:02.000\nTOTAL 00:02.000"
        );
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(!engine.snapshot(DeckId::B).playing);
    }

    #[test]
    fn eq_gui_routes_both_decks_resets_and_preserves_transport_and_faders() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
        let wav = crate::test_support::wav_file(16384, 48000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(wav.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        ui.set_audio_enabled(true);
        ui.show().unwrap();
        control(&ui, "Show mixer EQ").invoke_accessible_default_action();
        assert!(ui.get_eq_open());
        control(&ui, "Deck A trim dB").set_accessible_value("-6");
        control(&ui, "Deck B trim dB").set_accessible_value("-12");
        assert_eq!(engine.eq_settings(DeckId::A).trim, -6.);
        assert_eq!(engine.eq_settings(DeckId::B).trim, -12.);
        ui.set_crossfader(0.);
        ui.invoke_mixer_changed();
        engine.play(DeckId::A);
        for _ in 0..1000 {
            output.next();
        }
        let expected = 0.5 * 10_f32.powf(-6. / 20.);
        assert!((output.next().unwrap() - expected).abs() < 1e-5);
        output.next();
        let before = engine.snapshot(DeckId::A);
        let volumes = engine.output_volumes();
        control(&ui, "Deck A low EQ dB").set_accessible_value("-24");
        control(&ui, "Deck B mid EQ dB").set_accessible_value("12");
        control(&ui, "Deck B high EQ dB").set_accessible_value("6");
        assert_eq!(engine.eq_settings(DeckId::A).low, -24.);
        assert_eq!(engine.eq_settings(DeckId::B).mid, 12.);
        assert_eq!(engine.eq_settings(DeckId::B).high, 6.);
        assert_eq!(engine.snapshot(DeckId::A).position, before.position);
        assert_eq!(engine.snapshot(DeckId::A).playing, before.playing);
        assert_eq!(engine.output_volumes(), volumes);
        control(&ui, "Deck A reset EQ").invoke_accessible_default_action();
        assert_eq!(
            engine.eq_settings(DeckId::A),
            crate::mixer_dsp::EqSettings::default()
        );
        assert_eq!(engine.eq_settings(DeckId::B).trim, -12.);
        ui.set_eq_low_a(f32::NAN);
        ui.invoke_eq_changed(0);
        assert_eq!(ui.get_eq_low_a(), 0.);
        assert_eq!(
            engine.eq_settings(DeckId::A),
            crate::mixer_dsp::EqSettings::default()
        );
        engine.pause(DeckId::B);
        engine.load_b(wav.path()).unwrap();
        assert_eq!(engine.eq_settings(DeckId::B).mid, 12.);
        ui.set_crossfader(1.);
        ui.invoke_mixer_changed();
        engine.play(DeckId::B);
        for _ in 0..4000 {
            output.next();
        }
        assert!((output.next().unwrap() - 0.5 * 10_f32.powf(-12. / 20.)).abs() < 1e-4);
        control(&ui, "Deck A bipolar filter").set_accessible_value("-0.5");
        control(&ui, "Deck B bipolar filter").set_accessible_value("0.5");
        assert_eq!(engine.eq_settings(DeckId::A).filter, -0.5);
        assert_eq!(engine.eq_settings(DeckId::B).filter, 0.5);
        for _ in 0..4000 {
            output.next();
        }
        assert!(
            output.next().unwrap().abs() < 1e-4,
            "high-pass removes the DC input on deck B"
        );
        engine.pause(DeckId::A);
        engine.load_a(wav.path()).unwrap();
        assert_eq!(engine.eq_settings(DeckId::A).filter, -0.5);
        control(&ui, "Deck B filter off").invoke_accessible_default_action();
        assert_eq!(engine.eq_settings(DeckId::B).filter, 0.);
        assert_eq!(engine.eq_settings(DeckId::B).mid, 12.);
        ui.set_eq_filter_a(f32::INFINITY);
        for _ in 0..4000 {
            output.next();
        }
        assert!(
            (output.next().unwrap() - 0.5 * 10_f32.powf(-12. / 20.)).abs() < 1e-4,
            "filter off restores deck B without clearing EQ/trim"
        );
        ui.invoke_eq_changed(0);
        assert_eq!(ui.get_eq_filter_a(), -0.5);
        control(&ui, "Deck A reset EQ").invoke_accessible_default_action();
        assert_eq!(engine.eq_settings(DeckId::A).filter, 0.);
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            for deck in ["A", "B"] {
                let item = control(&ui, &format!("Deck {deck} bipolar filter"));
                assert!(item.size().width > 0. && item.size().height > 0.);
                assert!(item.absolute_position().x + item.size().width <= width + 1.);
                assert!(item.absolute_position().y + item.size().height <= height + 1.);
                for name in ["trim", "low EQ", "mid EQ", "high EQ"] {
                    let item = control(&ui, &format!("Deck {deck} {name} dB"));
                    assert!(item.size().width > 0. && item.size().height > 0.);
                    assert!(item.absolute_position().x >= 0.);
                    assert!(item.absolute_position().x + item.size().width <= width + 1.);
                    assert!(item.absolute_position().y + item.size().height <= height + 1.);
                }
            }
        }
        finish_search(&c);
        ui.hide().unwrap();
    }

    fn control(ui: &AppWindow, label: &str) -> ElementHandle {
        let controls: Vec<_> = ElementHandle::find_by_accessible_label(ui, label).collect();
        assert_eq!(controls.len(), 1, "accessible control {label}");
        controls.into_iter().next().unwrap()
    }
    fn finish_search(c: &AudioController) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while c.search_in_progress() {
            assert!(Instant::now() < deadline, "search worker timed out");
            c.tick();
            std::thread::yield_now();
        }
    }
    fn finish(c: &AudioController) {
        assert!(c.pending.borrow().is_some(), "GUI started a load worker");
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while c.pending.borrow().is_some() {
            assert!(std::time::Instant::now() < deadline, "worker completed");
            // Exercise the same completion path as the application's GUI timer.
            c.tick();
            std::thread::sleep(Duration::from_millis(1));
        }
        finish_search(c);
    }
    fn finish_import(c: &AudioController) {
        // Other wait helpers call tick() too and may already have consumed this result.
        // This is an idempotent completion barrier, not an import-admission assertion.
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while c.pending_import.borrow().is_some() {
            assert!(std::time::Instant::now() < deadline);
            c.tick();
            std::thread::sleep(Duration::from_millis(1));
        }
        finish_search(c);
    }
    #[test]
    fn import_completion_barrier_accepts_results_consumed_by_an_earlier_tick() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        let wav = wav_file(8192, 48000);
        c.import_paths(vec![wav.path().into()]);
        assert!(
            c.pending_import.borrow().is_some(),
            "explicitly verify admission before polling"
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        while c.pending_import.borrow().is_some() {
            assert!(Instant::now() < deadline);
            c.tick();
            std::thread::yield_now();
        }
        finish_import(&c);
        finish_import(&c);
        assert_eq!(c.library.borrow().len(), 1);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert!(c.library.borrow().catalog().tracks[0].metadata.scanned);
        assert!(!ui.get_library_busy());
    }
    fn finish_storage(c: &AudioController) {
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            c.tick();
            if c.storage_ready.get()
                && !c.saving.get()
                && (!c.dirty.get() || c.save_failed.get() || c.persistence.borrow().is_none())
            {
                break;
            }
            assert!(Instant::now() < until, "storage worker did not finish");
            thread::sleep(Duration::from_millis(1));
        }
        finish_search(c);
    }
    #[test]
    fn alignment_choice_wires_both_phase_actions_and_releases_only_its_sync_follower() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let clock = Rc::new(Cell::new(Instant::now()));
        let source_clock = Rc::clone(&clock);
        let engine = Rc::new(DualDeckEngine::from_mixer_with_clock(
            mixer,
            1,
            48000,
            Box::new(move || source_clock.get()),
        ));
        let file = wav_file(4096, 384000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        for id in [DeckId::A, DeckId::B] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine.play(id);
        }
        let levels = engine.output_volumes();
        for (id, label, other) in [
            (DeckId::A, "DECK A", DeckId::B),
            (DeckId::B, "DECK B", DeckId::A),
        ] {
            for _ in 0..4 {
                control(&ui, &format!("{label} deck tools panel"))
                    .invoke_accessible_default_action();
            }
            engine.seek_position(id, Duration::from_secs(1)).unwrap();
            engine.seek_position(other, Duration::ZERO).unwrap();
            match id {
                DeckId::A => {
                    ui.set_left_alignment_index(1);
                    ui.invoke_alignment_mode_left();
                }
                DeckId::B => {
                    ui.set_right_alignment_index(1);
                    ui.invoke_alignment_mode_right();
                }
            }
            assert_eq!(engine.snapshot(id).position, Duration::from_secs(1));
            control(&ui, &format!("{label} phase sync")).invoke_accessible_default_action();
            assert_eq!(engine.snapshot(id).position, Duration::from_secs(2));
            assert_eq!(engine.snapshot(other).position, Duration::ZERO);
        }
        engine
            .seek_position(DeckId::A, Duration::from_secs(1))
            .unwrap();
        engine.seek_position(DeckId::B, Duration::ZERO).unwrap();
        c.refresh();
        for _ in 0..1000 {
            output.next();
        }
        clock.set(clock.get() + Duration::from_millis(1));
        c.refresh();
        let acquisition_before = engine.snapshot(DeckId::A).position;
        c.toggle_sync(DeckId::A).unwrap();
        assert_eq!(
            engine.snapshot(DeckId::A).position,
            acquisition_before + Duration::from_secs(1)
        );
        for _ in 0..3 {
            for _ in 0..1584 {
                output.next();
            }
            clock.set(clock.get() + Duration::from_millis(33));
            c.maintain_sync(clock.get());
            assert_eq!(
                c.sync_session.borrow().as_ref().unwrap().alignment,
                crate::phase_sync::Alignment::Bar4
            );
        }
        ui.set_left_alignment_index(-1);
        ui.invoke_alignment_mode_left();
        assert!(c.sync_session.borrow().is_some());
        assert_eq!(ui.get_left_alignment_index(), 1);
        ui.set_right_alignment_index(0);
        ui.invoke_alignment_mode_right();
        assert!(
            c.sync_session.borrow().is_some(),
            "reference view choice must not change the follower"
        );
        let before = engine.snapshot(DeckId::A).position;
        ui.set_left_alignment_index(0);
        ui.invoke_alignment_mode_left();
        assert!(c.sync_session.borrow().is_none());
        assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
        assert_eq!(engine.snapshot(DeckId::A).position, before);
        assert_eq!(engine.output_volumes(), levels);
        assert!(!c.dirty.get());
    }
    #[test]
    fn continuous_sync_cancels_on_grid_source_pause_loop_busy_and_held_input() {
        i_slint_backend_testing::init_no_event_loop();
        for action in 0..8 {
            let ui = AppWindow::new().unwrap();
            let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
            let clock = Rc::new(Cell::new(Instant::now()));
            let source_clock = Rc::clone(&clock);
            let engine = Rc::new(DualDeckEngine::from_mixer_with_clock(
                mixer,
                1,
                48000,
                Box::new(move || source_clock.get()),
            ));
            let file = wav_file(4096, 192000);
            engine.load_a(file.path()).unwrap();
            engine.load_b(file.path()).unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            for id in [DeckId::A, DeckId::B] {
                engine
                    .set_grid(
                        id,
                        Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                    )
                    .unwrap();
                engine.play(id);
            }
            c.refresh();
            for _ in 0..1000 {
                output.next();
            }
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(DeckId::A).unwrap();
            match action {
                0 => engine
                    .set_grid(
                        DeckId::B,
                        Some(crate::beatgrid::BeatGrid::new(121., Duration::ZERO).unwrap()),
                    )
                    .unwrap(),
                1 => engine.load_b(file.path()).unwrap(),
                2 => engine.pause(DeckId::B),
                3 => {
                    engine
                        .restore_loop(DeckId::B, Duration::ZERO, Duration::from_secs(1))
                        .unwrap();
                    engine.activate_loop(DeckId::B).unwrap();
                }
                4 => {
                    let (_tx, rx) = mpsc::channel();
                    *c.pending.borrow_mut() = Some((DeckId::B, rx));
                }
                5 => c.begin_grid_editor(DeckId::B).unwrap(),
                6 => {
                    ui.invoke_nudge_right(-1, true);
                    ui.invoke_nudge_right(1, true);
                }
                _ => {
                    engine.apply_sync_factor(DeckId::A, 1.01).unwrap();
                    drop(c);
                    assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
                    continue;
                }
            }
            c.maintain_sync(clock.get());
            assert!(c.sync_session.borrow().is_none(), "action {action}");
            assert_eq!(engine.snapshot(DeckId::A).sync_factor, 1.);
        }
    }
    #[test]
    #[ignore = "offline multi-minute sync qualification"]
    fn continuous_sync_two_minutes_of_actual_offline_pcm_in_both_directions() {
        i_slint_backend_testing::init_no_event_loop();
        // A constant, nonzero synthetic tone checks actual rendering without a sound device.
        let file = wav_file(4096, 48_000 * 130);
        for target in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
            let clock = Rc::new(Cell::new(Instant::now()));
            let source_clock = Rc::clone(&clock);
            let engine = Rc::new(DualDeckEngine::from_mixer_with_clock(
                mixer,
                1,
                48_000,
                Box::new(move || source_clock.get()),
            ));
            engine.load_a(file.path()).unwrap();
            engine.load_b(file.path()).unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            for (deck, bpm) in [(DeckId::A, 120.0), (DeckId::B, 121.2)] {
                engine
                    .set_grid(
                        deck,
                        Some(crate::beatgrid::BeatGrid::new(bpm, Duration::ZERO).unwrap()),
                    )
                    .unwrap();
                engine.play(deck);
            }
            c.refresh();
            for _ in 0..1000 {
                output.next().unwrap();
            }
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(target).unwrap();
            let base = engine.snapshot(target).requested_speed;
            let mut max_error = 0.0_f64;
            let mut nonzero = 0_usize;
            // 33 ms per control tick; simulated clock follows the rendered frame count.
            for tick in 0..3636 {
                for _ in 0..1584 {
                    let sample = output.next().unwrap();
                    assert!(sample.is_finite());
                    nonzero += usize::from(sample != 0.0);
                }
                clock.set(clock.get() + Duration::from_millis(33));
                if tick >= 909 {
                    let (error, _, _) = engine
                        .sync_observation(target, crate::phase_sync::Alignment::Beat)
                        .expect("fresh phase before controller command");
                    max_error = max_error.max(error.abs());
                }
                c.maintain_sync(clock.get());
                assert!(
                    c.sync_session.borrow().is_some(),
                    "sync ended at tick {tick}: {}",
                    ui.get_status()
                );
            }
            assert!(nonzero > 5_000_000);
            assert!(max_error < 0.10, "sustained beat phase error {max_error}");
            assert_eq!(engine.snapshot(target).requested_speed, base);
            assert!(engine.snapshot(DeckId::A).playing && engine.snapshot(DeckId::B).playing);
            c.toggle_sync(target).unwrap();
            assert_eq!(engine.snapshot(target).sync_factor, 1.0);
            println!(
                "offline target {target:?}: rendered {:.3}s, max phase error after 30s {:.6} beats",
                3636.0 * 0.033,
                max_error
            );
        }
    }

    #[test]
    fn continuous_sync_controls_feedback_freshness_and_manual_release_on_both_decks() {
        i_slint_backend_testing::init_no_event_loop();
        for target in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
            let clock = Rc::new(Cell::new(Instant::now()));
            let source_clock = Rc::clone(&clock);
            let engine = Rc::new(DualDeckEngine::from_mixer_with_clock(
                mixer,
                1,
                48000,
                Box::new(move || source_clock.get()),
            ));
            let file = wav_file(4096, 384000);
            engine.load_a(file.path()).unwrap();
            engine.load_b(file.path()).unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            for (id, bpm) in [(DeckId::A, 123.45), (DeckId::B, 128.5)] {
                engine
                    .set_grid(
                        id,
                        Some(crate::beatgrid::BeatGrid::new(bpm, Duration::ZERO).unwrap()),
                    )
                    .unwrap();
                engine.play(id);
            }
            let other = if target == DeckId::A {
                DeckId::B
            } else {
                DeckId::A
            };
            let label = if target == DeckId::A {
                "DECK A"
            } else {
                "DECK B"
            };
            for _ in 0..4 {
                control(&ui, &format!("{label} deck tools panel"))
                    .invoke_accessible_default_action();
            }
            c.refresh();
            for _ in 0..1000 {
                output.next();
            }
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            control(&ui, &format!("{label} continuous sync")).invoke_accessible_default_action();
            assert!(c.sync_session.borrow().is_some(), "{}", ui.get_status());
            let base = engine.snapshot(target).requested_speed;
            let reference = engine.snapshot(other).requested_speed;
            let levels = engine.output_volumes();
            for _ in 0..30 {
                for _ in 0..1584 {
                    output.next();
                }
                clock.set(clock.get() + Duration::from_millis(33));
                c.maintain_sync(clock.get());
                assert!(c.sync_session.borrow().is_some(), "{}", ui.get_status());
            }
            assert!((engine.snapshot(target).sync_factor - 1.).abs() > 1e-10);
            assert_eq!(engine.snapshot(target).requested_speed, base);
            assert_eq!(engine.snapshot(other).requested_speed, reference);
            assert_eq!(engine.output_volumes(), levels);
            let last = c.sync_session.borrow().as_ref().unwrap().last_good;
            clock.set(last + Duration::from_millis(1));
            c.maintain_sync(clock.get());
            assert_eq!(engine.snapshot(target).sync_factor, 1.);
            clock.set(last + Duration::from_millis(101));
            c.maintain_sync(clock.get());
            assert!(c.sync_session.borrow().is_none());
            c.refresh();
            for _ in 0..1000 {
                output.next();
            }
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(target).unwrap();
            assert!(
                c.toggle_sync(other).is_err(),
                "two followers would create a cycle"
            );
            c.toggle_sync(target).unwrap();
            assert!(c.sync_session.borrow().is_none());
            c.refresh();
            for _ in 0..1000 {
                output.next();
            }
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(target).unwrap();
            engine.set_speed(other, 1.01).unwrap();
            c.maintain_sync(clock.get());
            assert!(c.sync_session.borrow().is_none());
            assert_eq!(engine.snapshot(target).sync_factor, 1.);
            c.refresh();
            for _ in 0..1000 {
                output.next();
            }
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(target).unwrap();
            engine
                .seek_position(other, Duration::from_millis(1250))
                .unwrap();
            c.maintain_sync(clock.get());
            assert!(c.sync_session.borrow().is_none());
            c.refresh();
            for _ in 0..1000 {
                output.next();
            }
            clock.set(clock.get() + Duration::from_millis(1));
            c.refresh();
            c.toggle_sync(target).unwrap();
            c.close_library().unwrap();
            assert!(c.sync_session.borrow().is_none());
            assert_eq!(engine.snapshot(target).sync_factor, 1.);
            assert!(!c.dirty.get());
        }
    }
    #[test]
    fn phase_sync_buttons_validate_both_decks_and_preserve_inactive_loop_bounds() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(4096, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        for label in ["DECK A", "DECK B"] {
            for _ in 0..4 {
                control(&ui, &format!("{label} deck tools panel"))
                    .invoke_accessible_default_action();
            }
            assert_eq!(
                control(&ui, &format!("{label} phase sync")).accessible_enabled(),
                Some(false)
            );
        }
        for id in [DeckId::A, DeckId::B] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine
                .restore_loop(id, Duration::from_secs(1), Duration::from_secs(3))
                .unwrap();
            engine.play(id);
        }
        let levels = engine.output_volumes();
        for (id, label, other) in [
            (DeckId::A, "DECK A", DeckId::B),
            (DeckId::B, "DECK B", DeckId::A),
        ] {
            engine
                .seek_position(id, Duration::from_millis(1100))
                .unwrap();
            engine
                .seek_position(other, Duration::from_millis(125))
                .unwrap();
            c.refresh();
            assert_eq!(
                control(&ui, &format!("{label} phase sync")).accessible_enabled(),
                Some(true)
            );
            control(&ui, &format!("{label} phase sync")).invoke_accessible_default_action();
            let after = engine.snapshot(id);
            assert_eq!(after.position, Duration::from_millis(1125));
            assert!(after.playing);
            assert_eq!(after.loop_in, Some(Duration::from_secs(1)));
            assert_eq!(after.loop_out, Some(Duration::from_secs(3)));
            assert!(!after.loop_active);
            assert_eq!(engine.snapshot(other).position, Duration::from_millis(125));
        }
        assert_eq!(engine.output_volumes(), levels);
        assert!(!c.dirty.get());
        engine.activate_loop(DeckId::A).unwrap();
        ui.invoke_phase_sync_right();
        assert!(ui.get_status().contains("Exit loops"));
        engine.exit_loop(DeckId::A).unwrap();
        c.queue_pitch(DeckId::B, 2.).unwrap();
        ui.invoke_phase_sync_left();
        assert!(ui.get_status().contains("pending pitch"));
        assert!(c.pending_pitch.borrow()[1].is_some());
        c.apply_pending_pitch();
        engine.pause(DeckId::B);
        ui.invoke_phase_sync_left();
        assert!(ui.get_status().contains("Play both"));
        c.storage_ready.set(false);
        ui.invoke_phase_sync_left();
        assert!(ui.get_status().contains("Finish loading"));
    }
    #[test]
    fn match_tempo_controls_preserve_transport_and_reject_conflicting_requests() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(4096, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        for label in ["DECK A", "DECK B"] {
            for _ in 0..3 {
                control(&ui, &format!("{label} deck tools panel"))
                    .invoke_accessible_default_action();
            }
        }
        assert_eq!(
            control(&ui, "DECK A match tempo").accessible_enabled(),
            Some(false)
        );
        ui.invoke_match_tempo_left();
        assert!(ui.get_status().contains("manual grids"));
        for (id, bpm) in [(DeckId::A, 120.), (DeckId::B, 128.)] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(bpm, Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine
                .restore_loop(id, Duration::from_secs(1), Duration::from_secs(3))
                .unwrap();
            engine.seek_position(id, Duration::from_secs(2)).unwrap();
            engine.activate_loop(id).unwrap();
        }
        engine.play(DeckId::A);
        engine.set_speed(DeckId::B, 1.03).unwrap();
        c.refresh();
        let levels = engine.output_volumes();
        for (id, label) in [(DeckId::A, "DECK A"), (DeckId::B, "DECK B")] {
            let other = if id == DeckId::A {
                DeckId::B
            } else {
                DeckId::A
            };
            let before = engine.snapshot(id);
            let reference = engine.snapshot(other);
            let reference_bpm = reference.grid.unwrap().bpm() * reference.effective_speed;
            assert_eq!(
                control(&ui, &format!("{label} match tempo")).accessible_enabled(),
                Some(true)
            );
            control(&ui, &format!("{label} match tempo")).invoke_accessible_default_action();
            let after = engine.snapshot(id);
            assert_eq!(after.position, before.position);
            assert_eq!(after.playing, before.playing);
            assert_eq!(after.loop_in, before.loop_in);
            assert_eq!(after.loop_out, before.loop_out);
            assert!(after.loop_active);
            assert!(
                (after.grid.unwrap().bpm() * after.effective_speed - reference_bpm).abs() < 0.002
            );
            assert_eq!(
                engine.snapshot(other).requested_speed,
                reference.requested_speed
            );
        }
        assert_eq!(engine.output_volumes(), levels);
        assert!(!c.dirty.get());
        // A rejected action must neither consume a pending pitch gesture nor alter rate.
        c.queue_pitch(DeckId::B, 4.).unwrap();
        let speed = engine.snapshot(DeckId::A).requested_speed;
        ui.invoke_match_tempo_left();
        assert!(ui.get_status().contains("pending pitch"));
        assert!(c.pending_pitch.borrow()[1].is_some());
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, speed);
        c.apply_pending_pitch();
        c.begin_grid_editor(DeckId::B).unwrap();
        ui.invoke_match_tempo_left();
        assert!(ui.get_status().contains("grid edits"));
        c.cancel_grid_editor(DeckId::B);
        engine.set_nudge(DeckId::A, 1).unwrap();
        ui.invoke_match_tempo_right();
        assert!(ui.get_status().contains("Release nudge"));
        engine.set_nudge(DeckId::A, 0).unwrap();
        // Opposing held inputs are still an active gesture even at neutral net rate.
        ui.invoke_nudge_left(-1, true);
        ui.invoke_nudge_left(1, true);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert!(c.nudges.borrow()[0].is_some());
        ui.invoke_match_tempo_right();
        assert!(ui.get_status().contains("Release nudge"));
        ui.invoke_nudge_left(-1, false);
        ui.invoke_nudge_left(1, false);
        ui.set_left_pitch_range_index(0);
        c.change_pitch_range(DeckId::A).unwrap();
        engine.set_speed(DeckId::B, 1.2).unwrap();
        ui.invoke_match_tempo_left();
        assert!(ui.get_status().contains("Widen"));
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, speed);
        let (_tx, rx) = mpsc::channel();
        *c.pending.borrow_mut() = Some((DeckId::B, rx));
        c.refresh();
        assert_eq!(
            control(&ui, "DECK A match tempo").accessible_enabled(),
            Some(false)
        );
        ui.invoke_match_tempo_left();
        assert!(ui.get_status().contains("Finish loading"));
        c.pending.borrow_mut().take();
        c.storage_ready.set(false);
        ui.invoke_match_tempo_right();
        assert!(ui.get_status().contains("Finish loading"));
    }
    #[test]
    fn loop_toggle_and_reloop_buttons_follow_distinct_reference_actions_on_both_decks() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 192000);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        for id in [DeckId::A, DeckId::B] {
            engine
                .restore_loop(id, Duration::from_secs(1), Duration::from_secs(3))
                .unwrap();
            engine
                .seek_position(id, Duration::from_millis(2500))
                .unwrap();
        }
        engine.play(DeckId::A);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        for (id, label) in [(DeckId::A, "DECK A"), (DeckId::B, "DECK B")] {
            let playing = engine.snapshot(id).playing;
            control(&ui, &format!("{label} loop on/off")).invoke_accessible_default_action();
            assert!(engine.snapshot(id).loop_active);
            assert_eq!(engine.snapshot(id).position, Duration::from_millis(2500));
            assert_eq!(engine.snapshot(id).playing, playing);
            control(&ui, &format!("{label} loop on/off")).invoke_accessible_default_action();
            assert!(!engine.snapshot(id).loop_active);
            assert_eq!(engine.snapshot(id).position, Duration::from_millis(2500));
            control(&ui, &format!("{label} reloop")).invoke_accessible_default_action();
            assert!(engine.snapshot(id).loop_active);
            assert_eq!(engine.snapshot(id).position, Duration::from_secs(1));
            assert_eq!(engine.snapshot(id).playing, playing);
            assert_eq!(
                control(&ui, &format!("{label} reloop")).accessible_enabled(),
                Some(true)
            );
        }
        assert!(!c.dirty.get());
        c.storage_ready.set(false);
        c.refresh();
        assert_eq!(
            control(&ui, "DECK A reloop").accessible_enabled(),
            Some(false)
        );
        ui.invoke_reloop_left();
        assert!(ui.get_status().contains("Finish loading"));
    }
    #[test]
    fn cancel_background_jobs_preserves_decks_rejects_late_results_and_releases_single_slot() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let old = wav_file(8192, 96000);
        let next = wav_file(4096, 96000);
        let imported = wav_file(2048, 96000);
        engine.load_a(old.path()).unwrap();
        engine.load_b(old.path()).unwrap();
        engine.play(DeckId::A);
        let (release, wait) = mpsc::channel();
        let wait = Mutex::new(wait);
        let (entered, entry) = mpsc::channel();
        let path = next.path().to_path_buf();
        let picker = Arc::new(move || {
            entered.send(()).unwrap();
            wait.lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(15))
                .unwrap();
            Some(path.clone())
        });
        let (release_import, wait_import) = mpsc::channel();
        let wait_import = Mutex::new(wait_import);
        let (entered_import, entry_import) = mpsc::channel();
        let path = imported.path().to_path_buf();
        let library_picker = Arc::new(move || {
            entered_import.send(()).unwrap();
            wait_import
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(15))
                .unwrap();
            Some(vec![path.clone()])
        });
        let c = AudioController::wire(&ui, Rc::clone(&engine), picker, library_picker);
        ui.show().unwrap();
        let original = engine.snapshot(DeckId::B).detail.unwrap();
        let volumes = engine.output_volumes();
        ui.invoke_load_right();
        entry.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(ui.get_background_job_active());
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            let cancel = control(&ui, "Cancel background load or import");
            assert_eq!(cancel.accessible_enabled(), Some(true));
            assert!(cancel.size().width > 0. && cancel.size().height > 0.);
            assert!(cancel.absolute_position().x + cancel.size().width <= width + 0.1);
            assert!(cancel.absolute_position().y + cancel.size().height <= height + 0.1);
        }
        control(&ui, "Cancel background load or import").invoke_accessible_default_action();
        control(&ui, "Cancel background load or import").invoke_accessible_default_action();
        assert!(c.pending_cancel.borrow().as_ref().unwrap().is_cancelled());
        assert!(c.pending.borrow().is_some());
        ui.invoke_load_right();
        assert!(
            entry.try_recv().is_err(),
            "single job slot remains occupied until acknowledgement"
        );
        release.send(()).unwrap();
        finish(&c);
        assert!(Arc::ptr_eq(
            &original,
            &engine.snapshot(DeckId::B).detail.unwrap()
        ));
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(c.library.borrow().len(), 0);
        assert!(ui.get_status().contains("cancelled"));
        assert!(!ui.get_background_job_active());
        ui.invoke_load_right();
        entry.recv_timeout(Duration::from_secs(5)).unwrap();
        release.send(()).unwrap();
        finish(&c);
        let installed = engine.snapshot(DeckId::B).detail.unwrap();
        assert!(!Arc::ptr_eq(&original, &installed));
        let revision = c.library.borrow().revision();
        let count = c.library.borrow().len();
        ui.invoke_import_library();
        entry_import.recv_timeout(Duration::from_secs(5)).unwrap();
        control(&ui, "Cancel background load or import").invoke_accessible_default_action();
        release_import.send(()).unwrap();
        finish_import(&c);
        assert_eq!(c.library.borrow().len(), count);
        assert_eq!(c.library.borrow().revision(), revision);
        // Cancellation must win even after a complete result was already queued.
        let (sender, receiver) = mpsc::channel();
        *c.pending.borrow_mut() = Some((DeckId::B, receiver));
        *c.pending_cancel.borrow_mut() = Some(crate::cancellation::Cancellation::default());
        sender
            .send(Some(Ok((
                imported.path().to_path_buf(),
                PreparedTrack::open(imported.path()).unwrap(),
                crate::track_metadata::read(imported.path()),
            ))))
            .unwrap();
        ui.invoke_cancel_background_job();
        c.tick();
        assert!(Arc::ptr_eq(
            &installed,
            &engine.snapshot(DeckId::B).detail.unwrap()
        ));
        assert_eq!(c.library.borrow().revision(), revision);
        ui.invoke_load_right();
        entry.recv_timeout(Duration::from_secs(5)).unwrap();
        c.close_library().unwrap();
        assert!(c.pending_cancel.borrow().as_ref().unwrap().is_cancelled());
        release.send(()).unwrap();
        finish(&c);
        assert!(Arc::ptr_eq(
            &installed,
            &engine.snapshot(DeckId::B).detail.unwrap()
        ));
        assert_eq!(engine.output_volumes(), volumes);
    }
    #[test]
    fn deck_performance_survives_other_deck_picker_decode_and_library_import() {
        i_slint_backend_testing::init_no_event_loop();
        for (active, loading, label) in [
            (DeckId::A, DeckId::B, "DECK A"),
            (DeckId::B, DeckId::A, "DECK B"),
        ] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(1, 48000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
            let file = wav_file(8192, 48000 * 8);
            let next = wav_file(4096, 48000 * 8);
            engine.load_a(file.path()).unwrap();
            engine.load_b(file.path()).unwrap();
            let (release, wait) = mpsc::channel();
            let wait = Mutex::new(wait);
            let (entered, entry) = mpsc::channel();
            let new_path = next.path().to_path_buf();
            let picker = Arc::new(move || {
                entered.send(()).unwrap();
                wait.lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(15))
                    .unwrap();
                Some(new_path.clone())
            });
            let c = AudioController::wire(&ui, Rc::clone(&engine), picker, Arc::new(|| None));
            let profile = tempfile::tempdir().unwrap();
            c.restore_library(Ok(profile.path().join("library.sqlite")), Vec::new());
            finish_storage(&c);
            engine.play(active);
            c.tick();
            ui.show().unwrap();
            let pitch = |value| match active {
                DeckId::A => ui.invoke_pitch_left(value),
                DeckId::B => ui.invoke_pitch_right(value),
            };
            let nudge = |held| match active {
                DeckId::A => ui.invoke_nudge_left(1, held),
                DeckId::B => ui.invoke_nudge_right(1, held),
            };
            let hot_cue = || match active {
                DeckId::A => ui.invoke_hot_cue_left(0),
                DeckId::B => ui.invoke_hot_cue_right(0),
            };
            let volumes = engine.output_volumes();
            nudge(true);
            pitch(10.);
            match loading {
                DeckId::A => ui.invoke_load_left(),
                DeckId::B => ui.invoke_load_right(),
            }
            entry.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(c.pending.borrow().is_some());
            assert!(c.can_control_deck(active));
            assert!(!c.can_control_deck(loading));
            assert!(!c.can_change_crates());
            c.tick();
            assert_eq!(engine.snapshot(active).requested_speed, 1.1);
            assert_eq!(engine.snapshot(active).nudge_direction, 1);
            match loading {
                DeckId::A => ui.invoke_pitch_left(25.),
                DeckId::B => ui.invoke_pitch_right(25.),
            }
            assert!(c.pending_pitch.borrow()[AudioController::grid_index(loading)].is_none());
            nudge(false);
            engine
                .seek_position(active, Duration::from_secs(2))
                .unwrap();
            control(&ui, &format!("{label} loop in")).invoke_accessible_default_action();
            engine
                .seek_position(active, Duration::from_secs(4))
                .unwrap();
            c.tick();
            control(&ui, &format!("{label} loop out")).invoke_accessible_default_action();
            control(&ui, &format!("{label} loop on/off")).invoke_accessible_default_action();
            assert!(engine.snapshot(active).loop_active);
            hot_cue(); // Persist a cue while the other deck is waiting on its picker.
            let path = engine.snapshot(active).path.unwrap();
            assert!(c.library.borrow().hot_cues(&path).slots()[0].is_some());
            // A controlled import result must merge rather than replace cue/grid edits.
            let (import_tx, import_rx) = mpsc::channel();
            release.send(()).unwrap();
            finish(&c);
            assert!(engine.snapshot(active).playing);
            assert_eq!(engine.snapshot(active).requested_speed, 1.1);
            assert_eq!(engine.snapshot(loading).requested_speed, 1.);
            *c.pending_import.borrow_mut() = Some(import_rx);
            ui.set_library_busy(true);
            nudge(true);
            pitch(12.);
            c.tick();
            assert_eq!(engine.snapshot(active).nudge_direction, 1);
            assert_eq!(engine.snapshot(active).requested_speed, 1.12);
            nudge(false);
            hot_cue();
            assert!(!engine.snapshot(active).loop_active);
            assert_eq!(engine.snapshot(active).position, Duration::from_secs(2));
            match active {
                DeckId::A => {
                    ui.invoke_grid_left();
                    ui.set_left_grid_draft_bpm("120".into());
                    ui.invoke_save_grid_left();
                    ui.invoke_save_loop_left();
                }
                DeckId::B => {
                    ui.invoke_grid_right();
                    ui.set_right_grid_draft_bpm("120".into());
                    ui.invoke_save_grid_right();
                    ui.invoke_save_loop_right();
                }
            }
            assert_eq!(c.library.borrow().grid(&path).unwrap().bpm(), 120.);
            assert!(c.library.borrow().saved_loops(&path).slots()[0].is_some());
            import_tx
                .send(Some(Ok(crate::library::inspect_paths(vec![file
                    .path()
                    .to_path_buf()]))))
                .unwrap();
            finish_import(&c);
            finish_storage(&c);
            assert!(c.library.borrow().hot_cues(&path).slots()[0].is_some());
            assert_eq!(engine.output_volumes(), volumes);
            // Startup restore ownership remains exclusive even for direct callbacks.
            c.storage_ready.set(false);
            nudge(true);
            pitch(20.);
            c.tick();
            assert_eq!(engine.snapshot(active).nudge_direction, 0);
            assert_eq!(engine.snapshot(active).requested_speed, 1.12);
            c.storage_ready.set(true);
            c.close_library().unwrap();
            c.restore_library(Ok(profile.path().join("library.sqlite")), Vec::new());
            finish_storage(&c);
            assert!(c.library.borrow().hot_cues(&path).slots()[0].is_some());
            assert_eq!(c.library.borrow().grid(&path).unwrap().bpm(), 120.);
            let loops = c.library.borrow().saved_loops(&path);
            assert_eq!(
                loops.slots()[0].as_ref().unwrap().start(),
                Duration::from_secs(2)
            );
            assert_eq!(
                loops.slots()[0].as_ref().unwrap().end(),
                Duration::from_secs(4)
            );
            c.close_library().unwrap();
        }
    }
    #[test]
    fn momentary_nudge_pointer_keys_focus_panel_and_busy_cancellation() {
        use slint::platform::{Key, PointerEventButton, WindowEvent};
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        for _ in 0..4 {
            control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        }
        let pointer = |pressed: bool| {
            let button = control(&ui, "DECK A nudge up");
            let p = button.absolute_position();
            let size = button.size();
            let position =
                slint::LogicalPosition::new(p.x + size.width / 2., p.y + size.height / 2.);
            ui.window().dispatch_event(if pressed {
                WindowEvent::PointerPressed {
                    position,
                    button: PointerEventButton::Left,
                }
            } else {
                WindowEvent::PointerReleased {
                    position,
                    button: PointerEventButton::Left,
                }
            });
        };
        let volumes = engine.output_volumes();
        pointer(true);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 1);
        assert!(ui.get_left_pitch_caption().contains("Nudge +2%"));
        engine.set_speed(DeckId::A, 1.1).unwrap();
        pointer(false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert!((engine.snapshot(DeckId::A).effective_speed - 1.1).abs() < 0.0001);
        // Pointer press establishes focus for held Space/Return and deactivation.
        for key in [Key::Space, Key::Return] {
            ui.window()
                .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
            ui.window()
                .dispatch_event(WindowEvent::KeyPressRepeated { text: key.into() });
            assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 1);
            ui.window()
                .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
            assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        }
        pointer(true);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Space.into(),
        });
        let item = control(&ui, "DECK A nudge up");
        let position = item.absolute_position();
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Right,
        });
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Space.into(),
        });
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 1);
        pointer(false);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Space.into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Return.into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Return.into(),
        });
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 1);
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Space.into(),
        });
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        pointer(true);
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(false));
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        pointer(false);
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        ui.window().dispatch_event(WindowEvent::KeyPressRepeated {
            text: Key::Space.into(),
        });
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        pointer(true);
        control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        // Logical input sessions cannot cross loading, source, transport or shutdown boundaries.
        ui.invoke_nudge_left(-1, true);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, -1);
        c.storage_ready.set(false);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        ui.invoke_nudge_left(1, true);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        c.storage_ready.set(true);
        ui.invoke_nudge_left(-1, false);
        ui.invoke_nudge_left(1, true);
        engine.pause(DeckId::A);
        c.tick();
        engine.play(DeckId::A);
        ui.invoke_nudge_left(1, false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        ui.invoke_nudge_left(1, true);
        engine.load_a(file.path()).unwrap();
        c.tick();
        engine.play(DeckId::A);
        ui.invoke_nudge_left(1, false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        ui.invoke_nudge_left(-1, true);
        ui.invoke_nudge_right(1, true);
        c.close_library().unwrap();
        for id in [DeckId::A, DeckId::B] {
            assert_eq!(engine.snapshot(id).nudge_direction, 0);
            assert!(engine.snapshot(id).playing);
        }
        assert_eq!(engine.output_volumes(), volumes);
        assert!(!c.dirty.get());
    }
    #[test]
    fn nudge_input_conflicts_limits_repeat_and_instant_accessibility_never_latch() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.play(DeckId::A);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        ui.invoke_nudge_left(-1, true);
        ui.invoke_nudge_left(-1, true);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, -1);
        ui.invoke_nudge_left(1, true);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        ui.invoke_nudge_left(-1, false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 1);
        ui.invoke_nudge_left(1, false);
        assert!(c.nudges.borrow()[0].is_none());
        ui.invoke_nudge_left(-1, true);
        ui.invoke_nudge_left(1, true);
        ui.invoke_seek_left(50.);
        ui.invoke_nudge_left(-1, false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert!(c.nudges.borrow()[0].is_none());
        engine.set_speed(DeckId::A, 2.).unwrap();
        ui.invoke_nudge_left(-1, true);
        ui.invoke_nudge_left(1, true);
        // Releasing down would leave an invalid >2x up direction: cancel all.
        ui.invoke_nudge_left(-1, false);
        ui.invoke_nudge_left(1, false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert!(c.nudges.borrow()[0].is_none());
        ui.invoke_nudge_left(1, true);
        ui.invoke_nudge_left(1, false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        engine.set_speed(DeckId::A, 1.).unwrap();
        for _ in 0..4 {
            control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        }
        for label in [
            "DECK A nudge down",
            "DECK A nudge up",
            "DECK A cancel nudge",
        ] {
            control(&ui, label).invoke_accessible_default_action();
            assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        }
        ui.invoke_nudge_left(-1, true);
        for _ in 0..48000 * 10 {
            output.next();
        }
        assert!(!engine.snapshot(DeckId::A).playing);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
        assert!(c.nudges.borrow()[0].is_none());
        ui.invoke_nudge_left(-1, false);
        engine.play(DeckId::A);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
    }
    #[test]
    fn opposing_nudge_holds_cancel_after_hot_cue_and_loop_transport_but_not_errors() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.play(DeckId::A);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        let press = || {
            ui.invoke_nudge_left(-1, true);
            ui.invoke_nudge_left(1, true);
        };
        let release = || {
            ui.invoke_nudge_left(-1, false);
            assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 0);
            ui.invoke_nudge_left(1, false);
            assert!(c.nudges.borrow()[0].is_none());
        };
        ui.invoke_hot_cue_left(0); // Set, then recall the occupied slot below.
        press();
        ui.invoke_hot_cue_left(99); // Rejected: held ownership remains unchanged.
        assert_eq!(c.nudges.borrow()[0].as_ref().unwrap().held, [true; 2]);
        ui.invoke_nudge_left(-1, false);
        assert_eq!(engine.snapshot(DeckId::A).nudge_direction, 1);
        ui.invoke_nudge_left(1, false);
        press();
        ui.invoke_hot_cue_left(0);
        release();
        engine
            .restore_loop(DeckId::A, Duration::from_secs(1), Duration::from_secs(2))
            .unwrap();
        ui.invoke_save_loop_left();
        for command in 0..3 {
            engine
                .restore_loop(DeckId::A, Duration::from_secs(1), Duration::from_secs(2))
                .unwrap();
            engine.activate_loop(DeckId::A).unwrap();
            press();
            match command {
                0 => ui.invoke_loop_exit_left(),
                1 => ui.invoke_loop_in_left(),
                _ => ui.invoke_recall_loop_left(),
            }
            assert!(
                c.nudges.borrow()[0].is_none(),
                "transport command {command}"
            );
            release();
        }
        press();
        ui.invoke_loop_out_left(); // Current position equals In: invalid region.
        assert_eq!(c.nudges.borrow()[0].as_ref().unwrap().held, [true; 2]);
        ui.invoke_cancel_nudge_left();
        release();
    }
    #[test]
    fn pitch_ranges_preserve_audio_outside_values_and_coalesce_integer_fine_steps() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 8);
        let other = wav_file(4096, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        for _ in 0..3 {
            control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        }
        assert_eq!(ui.get_left_pitch_range_index(), 3);
        control(&ui, "DECK A pitch").set_accessible_value("25");
        c.tick();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let before = engine.snapshot(DeckId::A);
        let volumes = engine.output_volumes();
        ui.set_left_pitch_range_index(0);
        ui.invoke_pitch_range_left();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.25);
        assert_eq!(engine.snapshot(DeckId::A).position, before.position);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(ui.get_left_pitch_outside());
        assert_eq!(ui.get_left_pitch_percent(), 25.);
        assert!(ui.get_left_pitch_caption().starts_with("Outside range"));
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "DECK A pitch").count(),
            0
        );
        ui.invoke_pitch_up_left();
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.25);
        control(&ui, "DECK A reset pitch").invoke_accessible_default_action();
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert!(!ui.get_left_pitch_outside());
        assert_eq!(
            control(&ui, "DECK A pitch").accessible_value_minimum(),
            Some(-8.)
        );
        assert_eq!(
            control(&ui, "DECK A pitch").accessible_value_maximum(),
            Some(8.)
        );
        for _ in 0..100 {
            control(&ui, "DECK A pitch fine up").invoke_accessible_default_action();
        }
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.01);
        for _ in 0..100 {
            control(&ui, "DECK A pitch fine down").invoke_accessible_default_action();
        }
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        // Reaching the boundary must retain the last valid queued step, not discard it.
        ui.invoke_pitch_left(7.99);
        ui.invoke_pitch_up_left();
        ui.invoke_pitch_up_left();
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.08);
        assert!(!ui.get_left_pitch_can_up());
        ui.invoke_pitch_left(8.01);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.08);
        ui.set_left_pitch_range_index(1);
        ui.invoke_pitch_range_left();
        control(&ui, "DECK A pitch").set_accessible_value("16");
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.16);
        ui.set_left_pitch_range_index(2);
        ui.invoke_pitch_range_left();
        control(&ui, "DECK A pitch").set_accessible_value("-50");
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 0.5);
        ui.invoke_pitch_up_left();
        ui.set_left_pitch_range_index(3);
        ui.invoke_pitch_range_left();
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 0.5); // Range changes cancel queued gestures.
        control(&ui, "DECK A pitch").set_accessible_value("100");
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 2.);
        ui.set_left_pitch_range_index(-1);
        ui.invoke_pitch_range_left();
        assert_eq!(ui.get_left_pitch_range_index(), 3);
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 2.);
        ui.set_left_pitch_range_index(0);
        ui.invoke_pitch_range_left();
        assert!(ui.get_left_pitch_outside());
        engine.pause(DeckId::A);
        engine.load_a(other.path()).unwrap();
        c.tick();
        assert_eq!(ui.get_left_pitch_range_index(), 0);
        assert!(!ui.get_left_pitch_outside());
        assert_eq!(ui.get_left_pitch_percent(), 0.);
        assert_eq!(ui.get_right_pitch_range_index(), 3);
        assert_eq!(engine.snapshot(DeckId::B).requested_speed, 1.);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.output_volumes(), volumes);
        assert!(!c.dirty.get());
    }
    #[test]
    fn pitch_gui_coalesces_source_bound_requests_reset_bpm_and_loading_failures() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 8);
        let other = wav_file(4096, 48000 * 8);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        for _ in 0..3 {
            control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        }
        assert!(!control(&ui, "DECK A pitch").accessible_enabled().unwrap());
        ui.invoke_pitch_left(8.);
        assert!(ui.get_status().contains("Load"));
        assert!(c.pending_pitch.borrow()[0].is_none());
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        c.tick();
        assert_eq!(ui.get_left_bpm_caption(), "Source BPM unavailable");
        engine
            .set_grid(
                DeckId::A,
                Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
            )
            .unwrap();
        c.tick();
        for value in ["25", "50", "8"] {
            control(&ui, "DECK A pitch").set_accessible_value(value);
        }
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert!(c.pending_pitch.borrow()[0].is_some());
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.08);
        assert_eq!(ui.get_left_pitch_percent(), 8.);
        assert!(ui.get_left_bpm_caption().contains("120.00 → 129.60"));
        assert!(ui.get_left_pitch_caption().contains("effective +8.0000%"));
        assert!(c.pending_pitch.borrow()[0].is_none());
        engine
            .restore_loop(DeckId::A, Duration::from_secs(2), Duration::from_secs(4))
            .unwrap();
        engine.activate_loop(DeckId::A).unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let before = engine.snapshot(DeckId::A);
        let volumes = engine.output_volumes();
        control(&ui, "DECK A pitch").set_accessible_value("-25");
        c.tick();
        let after = engine.snapshot(DeckId::A);
        assert_eq!(after.requested_speed, 0.75);
        assert_eq!(after.position, before.position);
        assert_eq!(after.loop_out, before.loop_out);
        assert!(after.playing && after.loop_active);
        assert_eq!(after.grid, before.grid);
        assert_eq!(engine.output_volumes(), volumes);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).requested_speed, 1.);
        control(&ui, "DECK A reset pitch").invoke_accessible_default_action();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 0.75);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert_eq!(ui.get_left_pitch_percent(), 0.);
        ui.invoke_pitch_left(8.);
        ui.invoke_pitch_left(f32::NAN);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert!(c.pending_pitch.borrow()[0].is_none());
        ui.invoke_pitch_left(8.);
        c.storage_ready.set(false);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert!(ui.get_status().contains("loading"));
        c.storage_ready.set(true);
        engine.pause(DeckId::A);
        ui.invoke_pitch_left(16.);
        engine.load_a(other.path()).unwrap();
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.);
        assert_eq!(ui.get_left_pitch_percent(), 0.);
        assert!(ui.get_status().contains("replacement"));
        ui.invoke_pitch_left(16.);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.16);
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(engine
            .load_a(std::path::Path::new("missing-pitch-gui.wav"))
            .is_err());
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).requested_speed, 1.16);
        assert!(engine.snapshot(DeckId::B).playing);
        assert!(!c.dirty.get());
    }
    #[test]
    fn auto_loop_snap_checkbox_is_per_deck_explicit_and_uses_committed_anchor() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        assert!(!ui.get_left_auto_loop_snap());
        assert!(!control(&ui, "DECK A auto loop beat snap")
            .accessible_enabled()
            .unwrap());
        engine
            .set_grid(
                DeckId::A,
                Some(crate::beatgrid::BeatGrid::new(120., Duration::from_millis(250)).unwrap()),
            )
            .unwrap();
        c.tick();
        engine
            .seek_position(DeckId::A, Duration::from_millis(850))
            .unwrap();
        ui.set_left_auto_loop_index(3);
        ui.invoke_auto_loop_left();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_in,
            Some(Duration::from_millis(850))
        );
        control(&ui, "DECK A auto loop beat snap").invoke_accessible_default_action();
        assert!(ui.get_left_auto_loop_snap());
        assert!(!ui.get_right_auto_loop_snap());
        assert_eq!(
            engine.snapshot(DeckId::A).loop_in,
            Some(Duration::from_millis(850))
        ); // A mode toggle alone cannot move playback.
        ui.invoke_grid_left();
        ui.set_left_grid_draft_bpm("60".into());
        ui.invoke_edit_grid_bpm_left();
        control(&ui, "DECK A auto loop").invoke_accessible_default_action();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_in,
            Some(Duration::from_millis(750))
        );
        assert_eq!(
            engine.snapshot(DeckId::A).loop_out,
            Some(Duration::from_millis(1250))
        );
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).loop_in.is_none());
        assert!(!c.dirty.get());
        ui.invoke_cancel_grid_left();
        control(&ui, "DECK A auto loop beat snap").invoke_accessible_default_action();
        assert!(!ui.get_left_auto_loop_snap());
        engine
            .seek_position(DeckId::A, Duration::from_millis(850))
            .unwrap();
        ui.invoke_auto_loop_left();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_in,
            Some(Duration::from_millis(850))
        );
    }
    #[test]
    fn long_track_final_frame_loop_remains_visible_before_gui_rounding() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        ui.show().unwrap();
        let end = Duration::from_secs(600);
        let start = crate::looping::frame_time(600 * 48000 - 1, 48000);
        let overview = loop_overlay(Some(start), Some(end), end, Duration::ZERO, end, true);
        assert_eq!(overview.start, overview.end); // Both absolute percentages round to 100 in f32.
        assert!(overview.visible && overview.in_visible && overview.out_visible);
        let detail = loop_overlay(
            Some(start),
            Some(end),
            end,
            Duration::from_secs(568),
            end,
            true,
        );
        assert!(detail.visible && detail.start < detail.end);
        ui.set_left_loop_range(overview);
        ui.set_left_detail_loop_range(detail);
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            for (view, label) in [
                ("DECK A waveform overview", "DECK A overview loop range"),
                ("DECK A detailed waveform", "DECK A detail loop range"),
            ] {
                let view = control(&ui, view);
                let shade = control(&ui, label);
                assert!(shade.size().width >= 0.99);
                assert!(
                    shade.absolute_position().x + shade.size().width
                        <= view.absolute_position().x + view.size().width + 0.1
                );
            }
            assert_eq!(
                ElementHandle::find_by_accessible_label(&ui, "DECK A overview loop in").count(),
                1
            );
        }
        for (view_start, view_end) in [(Duration::ZERO, start), (end, Duration::from_secs(601))] {
            let overlay = loop_overlay(
                Some(start),
                Some(end),
                Duration::from_secs(601),
                view_start,
                view_end,
                false,
            );
            assert!(!overlay.visible);
            assert!(!overlay.in_visible);
            assert!(!overlay.out_visible);
        }
        assert!(
            !loop_overlay(
                Some(start),
                Some(end),
                Duration::from_secs(599),
                Duration::ZERO,
                Duration::from_secs(599),
                false
            )
            .valid
        );
    }
    #[test]
    fn loop_overlays_clip_zoom_follow_state_and_preserve_other_deck() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 30);
        let other = wav_file(4096, 48000 * 30);
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        let count = |label: &str| ElementHandle::find_by_accessible_label(&ui, label).count();
        assert_eq!(count("DECK A overview loop range"), 0);
        assert!(!ui.get_left_loop_range().valid);
        engine
            .seek_position(DeckId::A, Duration::from_secs(10))
            .unwrap();
        ui.invoke_loop_in_left();
        assert_eq!(count("DECK A overview loop in"), 0); // No complete interval yet.
        engine
            .seek_position(DeckId::A, Duration::from_secs(20))
            .unwrap();
        ui.invoke_loop_out_left();
        assert!(ui.get_left_loop_range().valid);
        assert!(!ui.get_left_loop_range().active);
        assert!((ui.get_left_loop_range().start - 100. / 3.).abs() < 0.001);
        assert!((ui.get_left_loop_range().end - 200. / 3.).abs() < 0.001);
        assert_eq!(count("DECK B overview loop range"), 0);
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            engine
                .seek_position(DeckId::A, Duration::from_secs(15))
                .unwrap();
            c.tick();
            let view = control(&ui, "DECK A waveform overview");
            let shade = control(&ui, "DECK A overview loop range");
            assert!(
                (shade.absolute_position().x - view.absolute_position().x - view.size().width / 3.)
                    .abs()
                    < 0.1
            );
            assert!((shade.size().width - view.size().width / 3.).abs() < 0.1);
            for (label, part) in [
                ("DECK A overview loop in", 1. / 3.),
                ("DECK A overview loop out", 2. / 3.),
            ] {
                let marker = control(&ui, label);
                assert!(
                    (marker.absolute_position().x
                        - view.absolute_position().x
                        - part * (view.size().width - marker.size().width))
                        .abs()
                        < 0.1
                );
            }
            let detail = control(&ui, "DECK A detailed waveform");
            let shade = control(&ui, "DECK A detail loop range");
            assert!((shade.absolute_position().x - detail.absolute_position().x).abs() < 0.1);
            assert!((shade.size().width - detail.size().width).abs() < 0.1);
            assert_eq!(count("DECK A detail loop in"), 0);
            assert_eq!(count("DECK A detail loop out"), 0);
            // Right- and left-clipped ranges expose just the endpoint inside the viewport.
            engine
                .seek_position(DeckId::A, Duration::from_secs(10))
                .unwrap();
            c.tick();
            let shade = control(&ui, "DECK A detail loop range");
            assert!(shade.absolute_position().x > detail.absolute_position().x);
            assert!((shade.size().width - detail.size().width / 2.).abs() < 0.1);
            assert_eq!(count("DECK A detail loop in"), 1);
            assert_eq!(count("DECK A detail loop out"), 0);
            engine
                .seek_position(DeckId::A, Duration::from_secs(20))
                .unwrap();
            c.tick();
            let shade = control(&ui, "DECK A detail loop range");
            assert!((shade.absolute_position().x - detail.absolute_position().x).abs() < 0.1);
            assert!((shade.size().width - detail.size().width / 2.).abs() < 0.1);
            assert_eq!(count("DECK A detail loop in"), 0);
            assert_eq!(count("DECK A detail loop out"), 1);
        }
        // A touching exclusive Out boundary is not an overlapping region.
        engine
            .restore_loop(DeckId::A, Duration::from_secs(2), Duration::from_secs(4))
            .unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_secs(8))
            .unwrap();
        c.tick();
        assert_eq!(count("DECK A detail loop range"), 0);
        engine.seek_position(DeckId::A, Duration::ZERO).unwrap();
        c.tick();
        assert_eq!(count("DECK A detail loop range"), 1);
        ui.set_left_detail_window(32);
        ui.invoke_zoom_left();
        assert_eq!(count("DECK A detail loop in"), 1);
        assert_eq!(count("DECK A detail loop out"), 1);
        let position = engine.snapshot(DeckId::A).position;
        engine.play(DeckId::B);
        let volumes = engine.output_volumes();
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.output_volumes(), volumes);
        ui.invoke_loop_activate_left();
        assert!(ui.get_left_loop_range().active);
        assert!(ui.get_left_loop_range().description.contains("Active"));
        ui.invoke_loop_exit_left();
        assert!(!ui.get_left_loop_range().active);
        assert!(ui.get_left_loop_range().valid);
        assert_eq!(count("DECK A overview loop range"), 1);
        // One complete frame at EOF remains visible without extending beyond the waveform.
        let end = engine.snapshot(DeckId::A).duration;
        let start = end - Duration::from_nanos(20833);
        engine.restore_loop(DeckId::A, start, end).unwrap();
        c.tick();
        for (view, label) in [
            ("DECK A waveform overview", "DECK A overview loop range"),
            ("DECK A detailed waveform", "DECK A detail loop range"),
        ] {
            let view = control(&ui, view);
            let shade = control(&ui, label);
            assert!(shade.size().width >= 0.99);
            assert!(
                shade.absolute_position().x + shade.size().width
                    <= view.absolute_position().x + view.size().width + 0.1
            );
        }
        assert!(engine
            .load_a(std::path::Path::new("missing-loop-overlay.wav"))
            .is_err());
        c.tick();
        assert!(ui.get_left_loop_range().valid);
        engine.load_a(other.path()).unwrap();
        c.tick();
        assert!(!ui.get_left_loop_range().valid);
        assert_eq!(count("DECK A overview loop range"), 0);
        assert!(engine.snapshot(DeckId::B).playing);
        assert!(!c.dirty.get());
    }
    #[test]
    fn auto_loop_gui_uses_committed_grid_and_preserves_failures_and_saved_slots() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(8192, 48000 * 12);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        assert!(!control(&ui, "DECK A auto loop")
            .accessible_enabled()
            .unwrap());
        ui.invoke_auto_loop_left();
        assert!(ui.get_status().contains("manual beatgrid"));
        assert!(engine.snapshot(DeckId::A).loop_in.is_none());
        let grid = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
        engine.set_grid(DeckId::A, Some(grid)).unwrap();
        c.tick();
        // An unsaved grid draft cannot change the BPM used by Auto Loop.
        ui.invoke_grid_left();
        ui.set_left_grid_draft_bpm("60".into());
        ui.invoke_edit_grid_bpm_left();
        engine
            .seek_position(DeckId::A, Duration::from_millis(1250))
            .unwrap();
        ui.set_left_auto_loop_index(5);
        control(&ui, "DECK A auto loop").invoke_accessible_default_action();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_out,
            Some(Duration::from_millis(3250))
        );
        assert!(!engine.snapshot(DeckId::A).playing);
        control(&ui, "DECK A half loop").invoke_accessible_default_action();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_out,
            Some(Duration::from_millis(2250))
        );
        control(&ui, "DECK A double loop").invoke_accessible_default_action();
        assert_eq!(
            engine.snapshot(DeckId::A).loop_out,
            Some(Duration::from_millis(3250))
        );
        ui.invoke_cancel_grid_left();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let before = engine.snapshot(DeckId::A);
        ui.set_left_auto_loop_index(8);
        ui.invoke_auto_loop_left();
        assert_eq!(engine.snapshot(DeckId::A).loop_out, before.loop_out);
        assert_eq!(engine.snapshot(DeckId::A).position, before.position);
        assert!(engine.snapshot(DeckId::A).playing);
        ui.set_left_auto_loop_index(-1);
        ui.invoke_auto_loop_left();
        assert!(ui.get_status().contains("supported"));
        c.storage_ready.set(false);
        ui.invoke_half_loop_left();
        assert_eq!(engine.snapshot(DeckId::A).loop_out, before.loop_out);
        c.storage_ready.set(true);
        assert!(engine.snapshot(DeckId::B).loop_in.is_none());
        assert!(engine.snapshot(DeckId::B).playing);
        assert!(!c.dirty.get());
        // Auto-created regions use the existing saved-slot path without persisting active playback.
        ui.invoke_save_loop_left();
        assert!(c.library.borrow().catalog().tracks[0].saved_loops.slots()[0].is_some());
    }
    #[test]
    fn saved_loop_gui_save_recall_delete_shared_identity_bounds_and_restart() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let file = wav_file(8192, 48_000 * 8);
        let other = wav_file(4096, 48_000 * 8);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        ui.show().unwrap();
        control(&ui, "DECK A deck tools panel").invoke_accessible_default_action();
        control(&ui, "DECK B deck tools panel").invoke_accessible_default_action();
        ui.invoke_save_loop_left();
        assert!(ui.get_status().contains("In"));
        engine.seek_percent(DeckId::A, 25.).unwrap();
        ui.invoke_loop_in_left();
        engine.seek_percent(DeckId::A, 50.).unwrap();
        ui.invoke_loop_out_left();
        control(&ui, "DECK A saved loop name").set_accessible_value("繰り返し 🎵");
        control(&ui, "DECK A save loop slot").invoke_accessible_default_action();
        finish_storage(&c);
        assert!(ui.get_right_saved_loops().row_data(0).unwrap().occupied);
        assert!(ui
            .get_right_saved_loops()
            .row_data(0)
            .unwrap()
            .description
            .contains("繰り返し 🎵"));
        let path = engine.snapshot(DeckId::A).path.unwrap();
        let saved = c.library.borrow().saved_loops(&path);
        ui.invoke_save_loop_left();
        assert!(ui.get_status().contains("occupied"));
        assert_eq!(c.library.borrow().saved_loops(&path), saved);
        let volumes = engine.output_volumes();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let position = engine.snapshot(DeckId::B).position;
        control(&ui, "DECK B recall loop slot").invoke_accessible_default_action();
        assert_eq!(
            engine.snapshot(DeckId::B).loop_in,
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            engine.snapshot(DeckId::B).loop_out,
            Some(Duration::from_secs(4))
        );
        assert!(!engine.snapshot(DeckId::B).loop_active);
        assert_eq!(engine.snapshot(DeckId::B).position, position);
        assert!(engine.snapshot(DeckId::B).playing);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(!c.dirty.get());
        ui.invoke_loop_activate_right();
        assert!(engine.snapshot(DeckId::B).loop_active);
        for (slot, start, end) in [
            (1, Duration::ZERO, Duration::from_secs(9)),
            (2, Duration::from_nanos(1), Duration::from_nanos(2)),
        ] {
            c.library
                .borrow_mut()
                .set_saved_loop(
                    &path,
                    slot,
                    Some(
                        crate::saved_loops::SavedLoop::new(start, end, "Invalid for source".into())
                            .unwrap(),
                    ),
                )
                .unwrap();
            c.library_changed();
            c.refresh();
            ui.set_right_saved_loop_slot(slot as i32 + 1);
            let before = engine.snapshot(DeckId::B);
            ui.invoke_recall_loop_right();
            let after = engine.snapshot(DeckId::B);
            assert_eq!(after.loop_in, before.loop_in);
            assert_eq!(after.loop_out, before.loop_out);
            assert_eq!(after.position, before.position);
            assert!(after.loop_active);
            assert!(after.playing);
        }
        ui.set_right_saved_loop_slot(i32::MIN);
        assert!(c.change_saved_loop(DeckId::B, 1).is_err());
        ui.set_right_saved_loop_slot(1);
        c.storage_ready.set(false);
        assert!(c.change_saved_loop(DeckId::B, 2).is_err());
        c.storage_ready.set(true);
        control(&ui, "DECK B delete loop slot").invoke_accessible_default_action();
        finish_storage(&c);
        assert!(!ui.get_left_saved_loops().row_data(0).unwrap().occupied);
        assert!(engine.snapshot(DeckId::B).loop_active);
        ui.invoke_save_loop_right();
        finish_storage(&c);
        assert!(ui.get_left_saved_loops().row_data(0).unwrap().occupied);
        engine.pause(DeckId::A);
        engine.load_a(other.path()).unwrap();
        c.tick();
        assert!(!ui.get_left_saved_loops().row_data(0).unwrap().occupied);
        assert_eq!(ui.get_left_saved_loop_name(), "");
        assert_eq!(engine.output_volumes(), volumes);
        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output2) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        engine.load_a(file.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        assert!(ui.get_left_saved_loops().row_data(0).unwrap().occupied);
        assert!(!engine.snapshot(DeckId::A).loop_active);
        ui.invoke_recall_loop_left();
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(
            engine.snapshot(DeckId::A).loop_in,
            Some(Duration::from_secs(2))
        );
        ui.invoke_delete_loop_left();
        c.close_library().unwrap();
        drop(c);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        assert!(!ui.get_left_saved_loops().row_data(0).unwrap().occupied);
        c.close_library().unwrap();
    }
    #[test]
    fn manual_loop_gui_controls_state_transport_and_replacement_are_isolated() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let file = wav_file(8192, 48_000 * 8);
        let other = wav_file(4096, 48_000 * 8);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        assert!(!control(&ui, "DECK A loop in").accessible_enabled().unwrap());
        ui.invoke_loop_activate_left();
        assert!(ui.get_status().contains("Load"));
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        c.tick();
        ui.set_channel_a(0.6);
        ui.set_channel_b(0.8);
        ui.set_crossfader(0.3);
        ui.invoke_mixer_changed();
        let volumes = engine.output_volumes();
        assert!(!control(&ui, "DECK A loop out")
            .accessible_enabled()
            .unwrap());
        engine.seek_percent(DeckId::A, 25.).unwrap();
        control(&ui, "DECK A loop in").invoke_accessible_default_action();
        assert!(ui.get_left_loop_has_in());
        assert!(!ui.get_left_loop_ready());
        control(&ui, "DECK A loop out").invoke_accessible_default_action();
        assert!(!ui.get_left_loop_ready());
        assert!(ui.get_status().contains("In < Out"));
        engine.seek_percent(DeckId::A, 50.).unwrap();
        control(&ui, "DECK A loop out").invoke_accessible_default_action();
        assert!(ui.get_left_loop_ready());
        assert!(!ui.get_left_loop_active());
        control(&ui, "DECK A loop on/off").invoke_accessible_default_action();
        assert!(ui.get_left_loop_active());
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(2));
        assert!(!ui.get_right_loop_has_in());
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        engine.play(DeckId::B);
        output.by_ref().take(48_000 * 5).for_each(drop);
        c.tick();
        assert!(ui.get_left_loop_active());
        assert!(engine.snapshot(DeckId::A).playing);
        assert!((Duration::from_secs(2)..Duration::from_secs(4))
            .contains(&engine.snapshot(DeckId::A).position));
        assert!(ui.get_left_loop_caption().contains("ACTIVE"));
        assert_eq!(engine.output_volumes(), volumes);
        assert!(engine.snapshot(DeckId::B).playing);
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        let paused = engine.snapshot(DeckId::A).position;
        output.by_ref().take(4096).for_each(drop);
        c.tick();
        assert_eq!(engine.snapshot(DeckId::A).position, paused);
        assert!(ui.get_left_loop_active());
        control(&ui, "DECK A loop on/off").invoke_accessible_default_action();
        assert!(!ui.get_left_loop_active());
        assert!(ui.get_left_loop_ready());
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, paused);
        ui.invoke_loop_activate_left();
        control(&ui, "DECK A position").set_accessible_value("75");
        assert!(!ui.get_left_loop_active());
        assert!(ui.get_left_loop_ready());
        ui.invoke_loop_activate_left();
        control(&ui, "DECK A cue").invoke_accessible_default_action();
        assert!(!ui.get_left_loop_active());
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::ZERO);
        assert!(ui.get_left_loop_ready());
        ui.invoke_loop_activate_left();
        assert!(engine
            .load_a(std::path::Path::new("missing-loop-ui.wav"))
            .is_err());
        c.tick();
        assert!(ui.get_left_loop_active());
        // Simulate a background operation: both callbacks and button states must reject edits.
        c.storage_ready.set(false);
        c.publish_library();
        let original = engine.snapshot(DeckId::A).loop_in;
        ui.invoke_loop_in_left();
        assert_eq!(engine.snapshot(DeckId::A).loop_in, original);
        assert!(ui.get_status().contains("Finish loading"));
        c.storage_ready.set(true);
        c.publish_library();
        engine.load_a(other.path()).unwrap();
        c.tick();
        assert!(!ui.get_left_loop_active());
        assert!(!ui.get_left_loop_has_in());
        assert!(!ui.get_left_loop_ready());
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.output_volumes(), volumes);
        assert!(!c.dirty.get());
    }
    #[test]
    fn cue_editor_cancel_conflict_source_lifetime_markers_and_restart() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let file = wav_file(8192, 48_000 * 30);
        let bad = TempFile::new("mp3", b"invalid");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        ui.show().unwrap();
        engine.seek_percent(DeckId::A, 50.).unwrap();
        ui.invoke_hot_cue_left(0);
        finish_storage(&c);
        let path = engine.snapshot(DeckId::A).path.unwrap();
        let original = c.library.borrow().hot_cues(&path);
        control(&ui, "DECK A edit hot cue").invoke_accessible_default_action();
        control(&ui, "DECK A cue name").set_accessible_value("入り口 🎵");
        control(&ui, "DECK A cue RGB").set_accessible_value("#12abef");
        assert_eq!(c.library.borrow().hot_cues(&path), original);
        control(&ui, "DECK A cancel hot cue").invoke_accessible_default_action();
        assert_eq!(c.library.borrow().hot_cues(&path), original);
        assert!(!c.dirty.get());
        ui.invoke_edit_hot_cue_left(0);
        ui.invoke_edit_hot_cue_right(0);
        control(&ui, "DECK A cue RGB").set_accessible_value("GGGGGG");
        ui.invoke_save_hot_cue_left();
        assert!(ui.get_left_cue_editing());
        assert_eq!(c.library.borrow().hot_cues(&path), original);
        control(&ui, "DECK A cue RGB").set_accessible_value("#12abef");
        ui.set_left_cue_name("x".repeat(129).into());
        ui.invoke_save_hot_cue_left();
        assert!(ui.get_left_cue_editing());
        assert_eq!(c.library.borrow().hot_cues(&path), original);
        control(&ui, "DECK A cue name").set_accessible_value("入り口 🎵");
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        let volumes = engine.output_volumes();
        let position = engine.snapshot(DeckId::A).position;
        control(&ui, "DECK A save hot cue").invoke_accessible_default_action();
        finish_storage(&c);
        assert!(!ui.get_left_cue_editing());
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.output_volumes(), volumes);
        let cue = c.library.borrow().hot_cues(&path).slots()[0]
            .clone()
            .unwrap();
        assert_eq!(cue.position(), Duration::from_secs(15));
        assert_eq!(cue.name(), "入り口 🎵");
        assert_eq!(cue.rgb(), 0x12abef);
        assert_eq!(
            ui.get_right_hot_cues().row_data(0).unwrap().name,
            "入り口 🎵"
        );
        ui.invoke_save_hot_cue_right();
        assert!(ui.get_status().contains("changed"));
        assert!(ui.get_right_cue_editing());
        ui.invoke_cancel_hot_cue_right();
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            for (view, label, start, end) in [
                (
                    "DECK A waveform overview",
                    "DECK A overview cue marker 1",
                    0.,
                    100.,
                ),
                (
                    "DECK A detailed waveform",
                    "DECK A detail cue marker 1",
                    ui.get_left_detail_start(),
                    ui.get_left_detail_end(),
                ),
            ] {
                let view = control(&ui, view);
                let marker = control(&ui, label);
                let expected = view.absolute_position().x
                    + (50. - start) / (end - start) * (view.size().width - marker.size().width);
                assert!((marker.absolute_position().x - expected).abs() < 0.1);
            }
        }
        engine.seek_percent(DeckId::A, 0.).unwrap();
        c.tick();
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "DECK A detail cue marker 1").count(),
            0
        );
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "DECK A overview cue marker 1").count(),
            1
        );
        ui.set_left_detail_window(32);
        ui.invoke_zoom_left();
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "DECK A detail cue marker 1").count(),
            1
        );
        ui.invoke_edit_hot_cue_left(0);
        engine.pause(DeckId::A);
        assert!(engine.load_a(bad.path()).is_err());
        c.tick();
        assert!(ui.get_left_cue_editing());
        engine.load_a(file.path()).unwrap();
        c.tick();
        assert!(!ui.get_left_cue_editing());
        ui.invoke_edit_hot_cue_left(0);
        ui.invoke_delete_hot_cue_right(0);
        ui.invoke_save_hot_cue_left();
        assert!(ui.get_status().contains("changed"));
        ui.invoke_cancel_hot_cue_left();
        ui.invoke_hot_cue_right(0);
        ui.invoke_edit_hot_cue_right(0);
        ui.set_right_cue_name("Restored 🎵".into());
        ui.set_right_cue_rgb("ABCDEF".into());
        ui.invoke_save_hot_cue_right();
        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output2) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        engine.load_a(file.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        let row = ui.get_left_hot_cues().row_data(0).unwrap();
        assert_eq!(row.name, "Restored 🎵");
        assert_eq!(row.tint, slint::Color::from_rgb_u8(0xab, 0xcd, 0xef));
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "DECK A overview cue marker 1").count(),
            1
        );
        c.close_library().unwrap();
    }
    #[test]
    fn quantized_new_cues_preserve_transport_recall_exactly_and_survive_sqlite_restart() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let a = wav_file(8192, 384000);
        let b = wav_file(4096, 384000);
        let a_path = std::fs::canonicalize(a.path()).unwrap();
        let b_path = std::fs::canonicalize(b.path()).unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        engine.load_a(a_path.as_path()).unwrap();
        engine.load_b(b_path.as_path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        let grid = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
        c.library.borrow_mut().insert([
            a_path.as_path().to_path_buf(),
            b_path.as_path().to_path_buf(),
        ]);
        for path in [a_path.as_path(), b_path.as_path()] {
            c.library.borrow_mut().set_grid(path, Some(grid)).unwrap();
        }
        c.library_changed();
        finish_storage(&c);
        for (id, label, path) in [
            (DeckId::A, "DECK A", a_path.as_path()),
            (DeckId::B, "DECK B", b_path.as_path()),
        ] {
            engine.set_grid(id, Some(grid)).unwrap();
            engine
                .seek_position(id, Duration::from_millis(1250))
                .unwrap();
            engine
                .restore_loop(id, Duration::from_secs(1), Duration::from_secs(3))
                .unwrap();
            engine.activate_loop(id).unwrap();
            if id == DeckId::A {
                engine.play(id);
                engine.set_nudge(id, 1).unwrap();
            }
            c.refresh();
            control(&ui, &format!("{label} quantize new cues")).invoke_accessible_default_action();
            let before = engine.snapshot(id);
            let gains = engine.output_volumes();
            control(&ui, &format!("{label} hot cue 1")).invoke_accessible_default_action();
            assert!(
                c.library.borrow().hot_cues(path).slots()[0].is_some(),
                "{label}: {}; quantA={} quantB={} enabled={:?}",
                ui.get_status(),
                ui.get_left_quantize_cues(),
                ui.get_right_quantize_cues(),
                control(&ui, &format!("{label} hot cue 1")).accessible_enabled()
            );
            assert_eq!(
                c.library.borrow().hot_cues(path).slots()[0]
                    .as_ref()
                    .unwrap()
                    .position(),
                Duration::from_millis(1500)
            );
            let after = engine.snapshot(id);
            assert_eq!(after.position, before.position);
            assert_eq!(after.playing, before.playing);
            assert_eq!(after.nudge_direction, before.nudge_direction);
            assert_eq!(after.loop_active, before.loop_active);
            assert_eq!(engine.output_volumes(), gains);
            engine.set_grid(id, None).unwrap();
            engine
                .seek_position(id, Duration::from_millis(2200))
                .unwrap();
            c.refresh();
            assert_eq!(
                control(&ui, &format!("{label} quantize new cues")).accessible_enabled(),
                Some(true)
            );
            control(&ui, &format!("{label} hot cue 1")).invoke_accessible_default_action();
            assert_eq!(engine.snapshot(id).position, Duration::from_millis(1500));
            control(&ui, &format!("{label} quantize new cues")).invoke_accessible_default_action();
        }
        engine
            .seek_position(DeckId::A, Duration::new(1, 234567890))
            .unwrap();
        let exact = engine.snapshot(DeckId::A).position;
        ui.invoke_hot_cue_left(1);
        assert_eq!(
            c.library.borrow().hot_cues(a_path.as_path()).slots()[1]
                .as_ref()
                .unwrap()
                .position(),
            exact
        );
        finish_storage(&c);
        c.close_library().unwrap();
        let ui2 = AppWindow::new().unwrap();
        let (mixer, _output2) = rodio::mixer::mixer(1, 48000);
        let engine2 = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        engine2.load_a(a_path.as_path()).unwrap();
        engine2.load_b(b_path.as_path()).unwrap();
        let restored =
            AudioController::wire(&ui2, engine2.clone(), Arc::new(|| None), Arc::new(|| None));
        restored.restore_library(Ok(db), Vec::new());
        finish_storage(&restored);
        for path in [a_path.as_path(), b_path.as_path()] {
            assert_eq!(
                restored.library.borrow().hot_cues(path).slots()[0]
                    .as_ref()
                    .unwrap()
                    .position(),
                Duration::from_millis(1500)
            );
        }
        assert_eq!(
            restored.library.borrow().hot_cues(a_path.as_path()).slots()[1]
                .as_ref()
                .unwrap()
                .position(),
            exact
        );
        assert!(!ui2.get_left_quantize_cues() && !ui2.get_right_quantize_cues());
        restored.close_library().unwrap();
    }
    #[test]
    fn quantized_cue_fractional_bpm_at_44100hz_recalls_the_exact_source_frame() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = crate::test_support::wav_samples(&vec![4096; 88200], 44100, 1);
        engine.load_a(file.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        engine
            .set_grid(
                DeckId::A,
                Some(crate::beatgrid::BeatGrid::new(123.45, Duration::ZERO).unwrap()),
            )
            .unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_millis(730))
            .unwrap();
        ui.set_left_quantize_cues(true);
        c.change_hot_cue(DeckId::A, 0, false).unwrap();
        let path = engine.snapshot(DeckId::A).path.unwrap();
        let cue = c.library.borrow().hot_cues(&path).slots()[0]
            .clone()
            .unwrap();
        assert_eq!(cue.position(), Duration::from_nanos(972063493));
        engine
            .seek_position(DeckId::A, Duration::from_millis(1500))
            .unwrap();
        c.change_hot_cue(DeckId::A, 0, false).unwrap();
        assert_eq!(
            crate::looping::frame_at(engine.snapshot(DeckId::A).position, 44100),
            42868
        );
    }
    #[test]
    fn quantized_cue_rejections_do_not_insert_or_dirty_library() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let file = wav_file(4096, 96000);
        engine.load_a(file.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        ui.set_left_quantize_cues(true);
        assert!(c
            .change_hot_cue(DeckId::A, 0, false)
            .unwrap_err()
            .contains("manual grid"));
        engine
            .set_grid(
                DeckId::A,
                Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
            )
            .unwrap();
        c.begin_grid_editor(DeckId::A).unwrap();
        assert!(c
            .change_hot_cue(DeckId::A, 0, false)
            .unwrap_err()
            .contains("draft"));
        c.cancel_grid_editor(DeckId::A);
        engine
            .set_grid(
                DeckId::A,
                Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
            )
            .unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_millis(1900))
            .unwrap();
        let position = engine.snapshot(DeckId::A).position;
        assert!(c
            .change_hot_cue(DeckId::A, 0, false)
            .unwrap_err()
            .contains("outside"));
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        engine.seek_percent(DeckId::A, 100.).unwrap();
        assert!(c.change_hot_cue(DeckId::A, 0, false).is_err());
        c.storage_ready.set(false);
        assert!(c.change_hot_cue(DeckId::A, 0, false).is_err());
        assert_eq!(c.library.borrow().len(), 0);
        assert_eq!(c.library.borrow().revision(), 0);
        assert!(!c.dirty.get());
    }
    #[test]
    fn device_scan_before_profile_load_restores_saved_row_and_warns_if_missing() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let selected = crate::audio_devices::DeviceSelector {
            backend: "Test".into(),
            name: "Speakers".into(),
            endpoint_id: Some("stable-endpoint".into()),
        };
        let worker = crate::library_store::Worker::start(db.clone()).unwrap();
        worker.events.recv().unwrap();
        worker
            .save_startup_audio(crate::audio_devices::StartupSelection {
                version: 1,
                device: Some(selected.clone()),
            })
            .unwrap();
        worker.events.recv().unwrap();
        worker.close_delta(None).unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(2, 48000);
        let selector = selected.clone();
        let c = AudioController::wire_with_device_catalog(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000)),
            Arc::new(|| None),
            Arc::new(|| None),
            Arc::new(move || {
                Ok(vec![crate::audio_devices::DeviceChoice {
                    selector: crate::audio_devices::DeviceSelector {
                        backend: selector.backend.clone(),
                        name: "Renamed speakers".into(),
                        endpoint_id: selector.endpoint_id.clone(),
                    },
                    channels: 2,
                }])
            }),
        );
        c.restore_library(Ok(db), vec![]);
        c.start_device_scan();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while c.device_scan.borrow().is_some() {
            assert!(std::time::Instant::now() < deadline);
            c.poll_device_scan();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!c.storage_ready.get());
        assert_eq!(ui.get_audio_device_choice(), 0);
        finish_storage(&c);
        assert_eq!(ui.get_audio_device_choice(), 1);
        assert!(ui.get_audio_device_status().contains("ID-bound"));
        c.device_choices.borrow_mut().clear();
        c.sync_saved_device_choice();
        assert!(ui.get_audio_device_status().contains("Speakers"));
        assert!(ui.get_audio_device_status().contains("preserved"));
        assert_eq!(c.saved_startup_audio.borrow().device, Some(selected));
        ui.set_audio_startup_warning("Selected device unavailable; default in use".into());
        ui.show().unwrap();
        assert!(!ui.get_audio_device_open());
        assert!(control(&ui, "Audio startup fallback warning").size().height > 0.);
        ui.hide().unwrap();
        c.close_library().unwrap();
    }
    #[test]
    fn startup_device_gui_scans_off_thread_and_saves_without_switching_playback() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(4, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 4, 48000));
        let main_thread = std::thread::current().id();
        let catalog: DeviceCatalog = Arc::new(move || {
            assert_ne!(std::thread::current().id(), main_thread);
            Ok(vec![crate::audio_devices::DeviceChoice {
                selector: crate::audio_devices::DeviceSelector {
                    backend: "Test".into(),
                    name: "Speakers".into(),
                    endpoint_id: None,
                },
                channels: 2,
            }])
        });
        let c = AudioController::wire_with_device_catalog(
            &ui,
            engine.clone(),
            Arc::new(|| None),
            Arc::new(|| None),
            catalog,
        );
        c.restore_library(Ok(db.clone()), vec![]);
        finish_storage(&c);
        let track = wav_file(8192, 192000);
        engine.load_a(track.path()).unwrap();
        engine.play(DeckId::A);
        ui.show().unwrap();
        control(&ui, "Show audio device settings").invoke_accessible_default_action();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while c.device_scan.borrow().is_some() {
            assert!(std::time::Instant::now() < deadline);
            c.tick();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(ui.get_audio_device_names().row_count(), 2);
        ui.set_audio_device_choice(1);
        ui.invoke_audio_device_save();
        while c.startup_audio_saving.get() {
            assert!(std::time::Instant::now() < deadline);
            c.tick();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(ui.get_audio_device_status().contains("saved"));
        assert_eq!(
            crate::library_store::read_startup_audio(&db)
                .unwrap()
                .device
                .unwrap()
                .name,
            "Speakers"
        );
        c.device_choices.borrow_mut().clear();
        c.device_choices
            .borrow_mut()
            .push(crate::audio_devices::DeviceChoice {
                selector: crate::audio_devices::DeviceSelector {
                    backend: "WASAPI".into(),
                    name: "Speakers".into(),
                    endpoint_id: None,
                },
                channels: 2,
            });
        ui.set_audio_device_choice(1);
        ui.invoke_audio_device_save();
        assert!(ui
            .get_audio_device_status()
            .contains("could not identify this output uniquely"));
        assert!(!c.startup_audio_saving.get());
        assert_eq!(
            crate::library_store::read_startup_audio(&db)
                .unwrap()
                .device
                .unwrap()
                .name,
            "Speakers"
        );
        assert_eq!(
            crate::library_store::read_startup_audio(&db)
                .unwrap()
                .device
                .unwrap()
                .backend,
            "Test"
        );
        assert_eq!(engine.output_channels(), 4);
        assert!(engine.snapshot(DeckId::A).playing);
        output.by_ref().take(10000).for_each(drop);
        assert!(engine.snapshot(DeckId::A).position > Duration::ZERO);
        assert_eq!(engine.output_pair(), 0);
        ui.set_audio_device_choice(-1);
        ui.invoke_audio_device_save();
        assert!(ui.get_audio_device_status().contains("Invalid"));
        assert!(!c.startup_audio_saving.get());
        ui.hide().unwrap();
        c.close_library().unwrap();
    }
    #[test]
    fn track_drop_rejects_invalid_payloads_busy_and_playing_decks() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(2, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
        let c = AudioController::wire(
            &ui,
            engine.clone(),
            Arc::new(|| panic!("drop must not open picker")),
            Arc::new(|| None),
        );
        let wav = wav_file(8192, 192000);
        let data = ui.invoke_track_transfer(wav.path().to_string_lossy().as_ref().into());
        assert!(ui.invoke_accepts_track_drop(0, data.clone()));
        assert!(!ui.invoke_accepts_track_drop(2, data.clone()));
        assert!(!ui.invoke_accepts_track_drop(
            0,
            slint::DataTransfer::from(slint::SharedString::from("text is not a file"))
        ));
        let mut multiple = slint::DataTransfer::default();
        multiple.set_file_paths([wav.path(), wav.path()]);
        assert!(!ui.invoke_drop_track(0, multiple));
        assert!(ui.invoke_drop_track(0, data.clone()));
        assert!(!ui.invoke_drop_track(1, data.clone()));
        finish(&c);
        engine.play(DeckId::A);
        let before = engine.snapshot(DeckId::A);
        assert!(!ui.invoke_drop_track(0, data.clone()));
        assert_eq!(engine.snapshot(DeckId::A).path, before.path);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(ui.invoke_drop_track(1, data));
        finish(&c);
        assert!(engine.snapshot(DeckId::B).path.is_some());
        assert!(!engine.snapshot(DeckId::B).playing);
    }
    #[test]
    fn external_drop_ingress_loads_both_decks_and_preserves_guards() {
        use crate::external_drop::{drop_file, refresh_hover, FileGesture};
        i_slint_backend_testing::init_no_event_loop();
        for (width, height) in [(850., 740.), (1280., 860.)] {
            let ui = AppWindow::new().unwrap();
            ui.window().set_size(slint::LogicalSize::new(width, height));
            let (mixer, _output) = rodio::mixer::mixer(2, 48000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
            let c = AudioController::wire(
                &ui,
                engine.clone(),
                Arc::new(|| panic!("external drop must not open picker")),
                Arc::new(|| None),
            );
            let wav = wav_file(8192, 192000);
            let original = std::fs::read(wav.path()).unwrap();
            ui.show().unwrap();
            let point = |label| {
                let origin = control(&ui, label).absolute_position();
                slint::LogicalPosition::new(origin.x + 5., origin.y + 5.)
            };
            let a = point("DECK A load track");
            let b = point("DECK B load track");
            let mut gesture = FileGesture::default();
            for position in [a, b] {
                gesture.hover(wav.path().to_owned());
                assert!(drop_file(&ui, &mut gesture, wav.path(), Ok(position)));
                gesture.hover(wav.path().to_owned());
                assert!(!drop_file(&ui, &mut gesture, wav.path(), Ok(position)));
                assert!(ui.get_status().contains("Wait"));
                finish(&c);
            }
            assert!(!engine.snapshot(DeckId::A).playing);
            assert!(!engine.snapshot(DeckId::B).playing);
            gesture.hover(wav.path().to_owned());
            refresh_hover(&ui, &gesture, Some(a));
            assert_eq!(ui.get_external_drop_deck(), 0);
            engine.play(DeckId::A);
            let before = engine.snapshot(DeckId::A);
            assert!(!drop_file(&ui, &mut gesture, wav.path(), Ok(a)));
            assert!(ui.get_status().contains("Pause"));
            assert_eq!(engine.snapshot(DeckId::A).path, before.path);
            assert!(engine.snapshot(DeckId::A).playing);
            gesture.hover(wav.path().to_owned());
            assert!(drop_file(&ui, &mut gesture, wav.path(), Ok(b)));
            finish(&c);
            assert!(!engine.snapshot(DeckId::B).playing);
            let before_b = engine.snapshot(DeckId::B).path;
            gesture.hover(wav.path().to_owned());
            gesture.hover(wav.path().to_owned());
            for _ in 0..2 {
                assert!(!drop_file(&ui, &mut gesture, wav.path(), Ok(b)));
                assert!(c.pending.borrow().is_none());
            }
            let invalid = wav.path().with_extension("txt");
            gesture.hover(invalid.clone());
            assert!(!drop_file(&ui, &mut gesture, &invalid, Ok(b)));
            assert_eq!(engine.snapshot(DeckId::B).path, before_b);
            let missing = wav.path().with_file_name("missing-external-drop.wav");
            gesture.hover(missing.clone());
            assert!(drop_file(&ui, &mut gesture, &missing, Ok(b)));
            finish(&c);
            assert_eq!(engine.snapshot(DeckId::B).path, before_b);
            assert!(!engine.snapshot(DeckId::B).playing);
            assert_eq!(std::fs::read(wav.path()).unwrap(), original);
            engine.pause(DeckId::A);
            ui.hide().unwrap();
        }
    }

    #[test]
    fn library_pointer_drag_loads_both_decks_at_supported_sizes() {
        use slint::platform::{PointerEventButton, WindowEvent};
        i_slint_backend_testing::init_no_event_loop();
        for (width, height) in [(850., 740.), (1280., 860.)] {
            let ui = AppWindow::new().unwrap();
            ui.window().set_size(slint::LogicalSize::new(width, height));
            let (mixer, _output) = rodio::mixer::mixer(2, 48000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
            let c = AudioController::wire(
                &ui,
                engine.clone(),
                Arc::new(|| panic!("drag must not open picker")),
                Arc::new(|| None),
            );
            let wav = wav_file(8192, 192000);
            c.library.borrow_mut().insert([wav.path().to_owned()]);
            c.publish_library();
            finish_search(&c);
            ui.show().unwrap();
            for (id, label) in [
                (DeckId::A, "DECK A load track"),
                (DeckId::B, "DECK B load track"),
            ] {
                let row = control(&ui, "Library track 1");
                let source = row.absolute_position();
                let button = control(&ui, label);
                let dest = button.absolute_position();
                let from = slint::LogicalPosition::new(source.x + 20., source.y + 15.);
                let target =
                    slint::LogicalPosition::new(dest.x + 20., dest.y + button.size().height + 8.);
                ui.window().dispatch_event(WindowEvent::PointerPressed {
                    position: from,
                    button: PointerEventButton::Left,
                });
                ui.window().dispatch_event(WindowEvent::PointerMoved {
                    position: slint::LogicalPosition::new(from.x + 20., from.y),
                });
                ui.window()
                    .dispatch_event(WindowEvent::PointerMoved { position: target });
                ui.window().dispatch_event(WindowEvent::PointerReleased {
                    position: target,
                    button: PointerEventButton::Left,
                });
                assert!(
                    c.pending.borrow().is_some(),
                    "drag did not reach {id:?} at {width}"
                );
                finish(&c);
                assert!(engine.snapshot(id).path.is_some());
                assert!(!engine.snapshot(id).playing);
            }
            ui.hide().unwrap();
        }
    }
    #[test]
    fn standalone_default_output_plays_both_decks_without_midi() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _old_output) = rodio::mixer::mixer(2, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
        let rendered = Rc::new(RefCell::new(None));
        let sink = rendered.clone();
        let c = AudioController::wire_with_audio_services(
            &ui,
            engine.clone(),
            Arc::new(|| None),
            Arc::new(|| None),
            Arc::new(|| Ok(vec![])),
            Rc::new(move |selector| {
                assert!(
                    selector.is_none(),
                    "System default must not require a named controller"
                );
                let (mixer, output) = rodio::mixer::mixer(2, 48000);
                *sink.borrow_mut() = Some(output);
                Ok(DualDeckEngine::from_mixer(mixer, 2, 48000))
            }),
        );
        let track = wav_file(8192, 192000);
        engine.load_a(track.path()).unwrap();
        engine.load_b(track.path()).unwrap();
        ui.set_audio_device_choice(0);
        ui.invoke_audio_device_apply();
        c.poll_device_switch();
        assert_eq!(c.engine.borrow().output_channels(), 2);
        assert!(!c.engine.borrow().snapshot(DeckId::A).playing);
        assert!(!c.engine.borrow().snapshot(DeckId::B).playing);
        ui.set_channel_a(0.4);
        ui.set_channel_b(0.8);
        ui.set_crossfader(0.5);
        ui.invoke_mixer_changed();
        ui.invoke_toggle_left();
        ui.invoke_toggle_right();
        let samples = rendered
            .borrow_mut()
            .as_mut()
            .unwrap()
            .by_ref()
            .take(4800)
            .collect::<Vec<_>>();
        assert!(samples.iter().all(|sample| sample.is_finite()));
        assert!(samples
            .iter()
            .skip(2400)
            .all(|sample| (*sample - 0.15).abs() < 1e-5));
        c.tick();
        assert!(ui.get_left_position() > 0.);
        assert!(ui.get_right_position() > 0.);
        ui.invoke_toggle_left();
        ui.invoke_toggle_right();
        assert!(!c.engine.borrow().snapshot(DeckId::A).playing);
        assert!(!c.engine.borrow().snapshot(DeckId::B).playing);
    }

    #[test]
    fn failed_output_pauses_and_reopen_preserves_tracks_but_rejects_other_identity() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let key = crate::output_preferences::OutputDeviceKey {
            endpoint_id: Some("endpoint-a".into()),
            backend: "Test".into(),
            name: "Output".into(),
            channels: 2,
        };
        let (mixer, _output) = rodio::mixer::mixer(2, 48000);
        let engine = Rc::new(
            DualDeckEngine::from_mixer(mixer, 2, 48000).with_output_device_key(key.clone()),
        );
        let wrong = Rc::new(Cell::new(true));
        let wrong_open = wrong.clone();
        let c = AudioController::wire_with_audio_services(
            &ui,
            engine.clone(),
            Arc::new(|| None),
            Arc::new(|| None),
            Arc::new(|| Ok(vec![])),
            Rc::new(move |selector| {
                assert_eq!(selector.unwrap().name, "Output");
                let (mixer, _output) = rodio::mixer::mixer(2, 48000);
                let mut actual = key.clone();
                if wrong_open.get() {
                    actual.endpoint_id = Some("endpoint-b".into());
                }
                Ok(DualDeckEngine::from_mixer(mixer, 2, 48000).with_output_device_key(actual))
            }),
        );
        let track = wav_file(8192, 192000);
        engine.load_a(track.path()).unwrap();
        engine.load_b(track.path()).unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_millis(250))
            .unwrap();
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        engine.inject_output_failure();
        c.tick();
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(!engine.snapshot(DeckId::B).playing);
        assert!(ui
            .get_audio_startup_warning()
            .contains("Audio output failed"));
        ui.invoke_toggle_left();
        engine.play(DeckId::B);
        assert!(!engine.snapshot(DeckId::A).playing);
        assert!(!engine.snapshot(DeckId::B).playing);
        ui.invoke_audio_device_reopen();
        c.poll_device_switch();
        assert!(Rc::ptr_eq(&c.engine.borrow(), &engine));
        assert!(ui
            .get_audio_device_status()
            .contains("identity or channel shape changed"));
        wrong.set(false);
        ui.invoke_audio_device_reopen();
        c.poll_device_switch();
        assert!(!Rc::ptr_eq(&c.engine.borrow(), &engine));
        assert!(!c.engine.borrow().output_failed());
        assert_eq!(
            c.engine.borrow().snapshot(DeckId::A).position,
            engine.snapshot(DeckId::A).position
        );
        assert_eq!(
            c.engine.borrow().snapshot(DeckId::B).path,
            engine.snapshot(DeckId::B).path
        );
        assert!(!c.engine.borrow().snapshot(DeckId::A).playing);
        assert!(!c.engine.borrow().snapshot(DeckId::B).playing);
        assert!(ui.get_audio_startup_warning().is_empty());
        ui.invoke_toggle_left();
        assert!(c.engine.borrow().snapshot(DeckId::A).playing);
    }
    #[test]
    fn paused_device_switch_is_transactional_and_replaces_shared_owner() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(4, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 4, 48000));
        let weak = Rc::downgrade(&engine);
        let fail = Rc::new(Cell::new(true));
        let fail_open = fail.clone();
        let c = AudioController::wire_with_audio_services(
            &ui,
            engine.clone(),
            Arc::new(|| None),
            Arc::new(|| None),
            Arc::new(|| Ok(vec![])),
            Rc::new(move |_| {
                if fail_open.get() {
                    return Err("injected open failure".into());
                }
                let (mixer, _output) = rodio::mixer::mixer(2, 44100);
                Ok(DualDeckEngine::from_mixer(mixer, 2, 44100))
            }),
        );
        let track = wav_file(8192, 192000);
        engine.load_a(track.path()).unwrap();
        engine
            .seek_position(DeckId::A, Duration::from_millis(250))
            .unwrap();
        ui.set_audio_device_choice(0);
        engine.play(DeckId::A);
        ui.invoke_audio_device_apply();
        assert!(c.pending_device_switch.borrow().is_none());
        assert!(ui.get_audio_device_status().contains("Pause both"));
        engine.pause(DeckId::A);
        ui.invoke_audio_device_apply();
        c.poll_device_switch();
        assert!(Rc::ptr_eq(&c.engine.borrow(), &engine));
        assert!(ui
            .get_audio_device_status()
            .contains("previous device retained"));
        let owner = c.audio_owner();
        fail.set(false);
        ui.invoke_audio_device_apply();
        c.poll_device_switch();
        assert!(!Rc::ptr_eq(&owner.borrow(), &engine));
        assert_eq!(owner.borrow().output_channels(), 2);
        assert_eq!(
            owner.borrow().snapshot(DeckId::A).position,
            engine.snapshot(DeckId::A).position
        );
        assert_eq!(
            owner.borrow().snapshot(DeckId::A).path,
            engine.snapshot(DeckId::A).path
        );
        assert!(!owner.borrow().snapshot(DeckId::A).playing);
        assert!(!ui.get_audio_device_switching());
        drop(engine);
        assert!(weak.upgrade().is_none());
        ui.invoke_toggle_left();
        assert!(owner.borrow().snapshot(DeckId::A).playing);
    }
    #[test]
    fn monitor_profiles_restore_latest_ack_and_close_flush_only_for_matching_device() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        for (round, name) in [(0, "MC4000"), (1, "MC4000"), (2, "Other")] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(4, 48000);
            let engine = Rc::new(
                DualDeckEngine::from_mixer(mixer, 4, 48000).with_output_device_key(
                    crate::output_preferences::OutputDeviceKey {
                        endpoint_id: None,
                        backend: "Test".into(),
                        name: name.into(),
                        channels: 4,
                    },
                ),
            );
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            c.restore_library(Ok(db.clone()), vec![]);
            ui.set_monitor_gain(77.);
            ui.invoke_monitor_changed();
            ui.set_monitor_cue_a(true);
            ui.invoke_monitor_cue_changed(0);
            assert_eq!(engine.monitor_settings().gain, 1.);
            assert!(!engine.headphone_cue(DeckId::A));
            finish_storage(&c);
            assert!(!engine.snapshot(DeckId::A).loaded && !engine.snapshot(DeckId::B).playing);
            if round == 1 {
                assert!((engine.monitor_settings().gain - 0.6).abs() < 1e-6);
                assert_eq!(engine.monitor_settings().mix, 0.5);
                assert!(!engine.headphone_cue(DeckId::A) && engine.headphone_cue(DeckId::B));
                assert!(ui.get_monitor_status().contains("restored"));
            } else {
                assert_eq!(
                    engine.monitor_settings(),
                    crate::output_routing::MonitorSettings::default()
                );
                assert!(!engine.headphone_cue(DeckId::A) && !engine.headphone_cue(DeckId::B));
            }
            if round == 0 {
                ui.set_monitor_gain(40.);
                ui.set_monitor_mix(25.);
                ui.invoke_monitor_changed();
                ui.set_monitor_cue_a(true);
                ui.invoke_monitor_cue_changed(0);
                c.save_library();
                assert!(c.saving.get());
                ui.set_monitor_gain(20.);
                ui.set_monitor_mix(75.);
                ui.invoke_monitor_changed();
                ui.set_monitor_cue_b(true);
                ui.invoke_monitor_cue_changed(1);
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while c.saving.get() {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "monitor save acknowledgement timed out"
                    );
                    c.poll_storage();
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert_eq!(ui.get_output_profile_status(), "Profile changed");
                finish_storage(&c);
                assert_eq!(
                    c.saved_output
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .monitor
                        .unwrap()
                        .mix,
                    0.75
                );
                ui.set_monitor_gain(60.);
                ui.set_monitor_mix(50.);
                ui.invoke_monitor_changed();
                ui.set_monitor_cue_a(false);
                ui.invoke_monitor_cue_changed(0);
                // No tick: close must capture the final monitor/cue state itself.
            }
            c.close_library().unwrap();
        }
    }
    #[test]
    fn malformed_monitor_profile_is_preserved_while_unrelated_preferences_save() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let worker = crate::library_store::Worker::start(db.clone()).unwrap();
        worker.events.recv().unwrap();
        worker.close_delta(None).unwrap();
        let raw = r#"{"version":1,"routes":[{"device":{"backend":"Test","name":"MC4000","channels":4},"pair":0,"monitor":{"gain":2,"mix":0,"cues":[false,false]}}]}"#;
        let sql = rusqlite::Connection::open(&db).unwrap();
        sql.execute(
            "INSERT INTO library_meta(key,value) VALUES('audio_output_routes_v1',?1)",
            [raw],
        )
        .unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(4, 48000);
        let engine = Rc::new(
            DualDeckEngine::from_mixer(mixer, 4, 48000).with_output_device_key(
                crate::output_preferences::OutputDeviceKey {
                    endpoint_id: None,
                    backend: "Test".into(),
                    name: "MC4000".into(),
                    channels: 4,
                },
            ),
        );
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db), vec![]);
        finish_storage(&c);
        assert!(!c.output_profiles_writable.get());
        ui.set_monitor_gain(50.);
        ui.set_monitor_mix(50.);
        ui.invoke_monitor_changed();
        assert!(ui.get_monitor_status().contains("session only"));
        ui.set_quantize_enabled(false);
        finish_storage(&c);
        c.close_library().unwrap();
        let saved: String = sql
            .query_row(
                "SELECT value FROM library_meta WHERE key='audio_output_routes_v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(saved, raw);
        let prefs: String = sql
            .query_row(
                "SELECT value FROM library_meta WHERE key='quantize_preferences_v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            !serde_json::from_str::<crate::preferences::QuantizePreferences>(&prefs)
                .unwrap()
                .enabled
        );
    }
    #[test]
    fn output_profiles_restore_matching_device_and_keep_other_profiles() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        for (name, channels, expected, edit) in [
            (Some("MC4000"), 4, 0, true),
            (Some("Other"), 4, 0, true),
            (Some("MC4000"), 2, 0, false),
            (None, 4, 0, false),
            (Some("MC4000"), 4, 1, false),
        ] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(channels, 48000);
            let mut engine = DualDeckEngine::from_mixer(mixer, channels, 48000);
            if let Some(name) = name {
                engine =
                    engine.with_output_device_key(crate::output_preferences::OutputDeviceKey {
                        endpoint_id: None,
                        backend: "Test".into(),
                        name: name.into(),
                        channels,
                    });
            }
            let engine = Rc::new(engine);
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            c.restore_library(Ok(db.clone()), vec![]);
            assert_eq!(
                control(&ui, "Audio output pair").accessible_enabled(),
                Some(false)
            );
            finish_storage(&c);
            assert_eq!(engine.output_pair(), expected);
            assert_eq!(ui.get_output_pair_index(), i32::from(expected));
            assert!(!engine.snapshot(DeckId::A).loaded);
            assert!(!engine.snapshot(DeckId::B).playing);
            if edit {
                ui.set_output_pair_index(1);
                ui.invoke_output_pair_changed();
                c.save_library();
                assert!(c.saving.get());
                ui.set_output_pair_index(0);
                ui.invoke_output_pair_changed();
                finish_storage(&c);
                assert_eq!(c.saved_output.borrow().as_ref().unwrap().pair, 0);
                ui.set_output_pair_index(1);
                ui.invoke_output_pair_changed();
            }
            c.close_library().unwrap();
        }
    }
    #[test]
    fn output_profile_close_revert_and_retry_preserve_latest_route() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        for retry in [false, true] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(4, 48000);
            let engine = Rc::new(
                DualDeckEngine::from_mixer(mixer, 4, 48000).with_output_device_key(
                    crate::output_preferences::OutputDeviceKey {
                        endpoint_id: None,
                        backend: "Test".into(),
                        name: "MC4000".into(),
                        channels: 4,
                    },
                ),
            );
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            c.restore_library(Ok(db.clone()), vec![]);
            finish_storage(&c);
            assert_eq!(engine.output_pair(), 0);
            if !retry {
                ui.set_output_pair_index(1);
                ui.invoke_output_pair_changed();
                c.save_library();
                assert!(c.saving.get());
                ui.set_output_pair_index(0);
                ui.invoke_output_pair_changed();
                c.close_library().unwrap();
            } else {
                let connection = rusqlite::Connection::open(&db).unwrap();
                connection.execute_batch("CREATE TRIGGER reject_routes BEFORE INSERT ON library_meta WHEN NEW.key='audio_output_routes_v1' BEGIN SELECT RAISE(ABORT,'injected route write'); END;").unwrap();
                ui.set_output_pair_index(1);
                ui.invoke_output_pair_changed();
                finish_storage(&c);
                assert!(c.save_failed.get());
                assert_eq!(ui.get_output_profile_status(), "Route not saved");
                connection
                    .execute_batch("DROP TRIGGER reject_routes")
                    .unwrap();
                ui.invoke_retry_library_save();
                finish_storage(&c);
                assert!(!c.save_failed.get());
                assert_eq!(c.saved_output.borrow().as_ref().unwrap().pair, 1);
                c.close_library().unwrap();
            }
        }
    }
    #[test]
    fn full_output_profiles_allow_other_saves_and_existing_device_updates() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let worker = crate::library_store::Worker::start(db.clone()).unwrap();
        worker.events.recv().unwrap();
        worker.close_delta(None).unwrap();
        let mut profiles = crate::output_preferences::OutputRoutes::default();
        for i in 0..32 {
            profiles
                .update(crate::output_preferences::OutputRoute {
                    device: crate::output_preferences::OutputDeviceKey {
                        endpoint_id: None,
                        backend: "Test".into(),
                        name: format!("Device{i}"),
                        channels: 4,
                    },
                    pair: 0,

                    monitor: None,
                })
                .unwrap();
        }
        let raw = serde_json::to_string(&profiles).unwrap();
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection
            .execute(
                "INSERT INTO library_meta(key,value) VALUES('audio_output_routes_v1',?1)",
                [&raw],
            )
            .unwrap();
        for (name, session_only) in [("MC4000", true), ("Device0", false)] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(4, 48000);
            let engine = Rc::new(
                DualDeckEngine::from_mixer(mixer, 4, 48000).with_output_device_key(
                    crate::output_preferences::OutputDeviceKey {
                        endpoint_id: None,
                        backend: "Test".into(),
                        name: name.into(),
                        channels: 4,
                    },
                ),
            );
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            c.restore_library(Ok(db.clone()), vec![]);
            finish_storage(&c);
            assert_eq!(c.output_profiles_writable.get(), !session_only);
            ui.set_output_pair_index(1);
            ui.invoke_output_pair_changed();
            ui.set_quantize_enabled(!ui.get_quantize_enabled());
            let file = wav_file(4096, 480);
            c.library.borrow_mut().insert([file.path().to_path_buf()]);
            c.library_changed();
            finish_storage(&c);
            assert!(!c.save_failed.get());
            c.close_library().unwrap();
            let saved: String = connection
                .query_row(
                    "SELECT value FROM library_meta WHERE key='audio_output_routes_v1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            if session_only {
                assert_eq!(saved, raw);
            } else {
                let profiles: crate::output_preferences::OutputRoutes =
                    serde_json::from_str(&saved).unwrap();
                assert_eq!(profiles.routes.len(), 32);
                assert_eq!(
                    profiles.pair_for(engine.output_device_key().unwrap()),
                    Some(1)
                );
            }
        }
    }
    #[test]
    fn corrupt_output_profile_stays_session_only_while_other_preferences_save() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let worker = crate::library_store::Worker::start(db.clone()).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            crate::library_store::Event::Loaded(Ok(_))
        ));
        worker.close_delta(None).unwrap();
        let connection = rusqlite::Connection::open(&db).unwrap();
        let raw = r#"{"version":2,"routes":[]}"#;
        connection
            .execute(
                "INSERT INTO library_meta(key,value) VALUES('audio_output_routes_v1',?1)",
                [raw],
            )
            .unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(4, 48000);
        let engine = Rc::new(
            DualDeckEngine::from_mixer(mixer, 4, 48000).with_output_device_key(
                crate::output_preferences::OutputDeviceKey {
                    endpoint_id: None,
                    backend: "Test".into(),
                    name: "MC4000".into(),
                    channels: 4,
                },
            ),
        );
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db), vec![]);
        finish_storage(&c);
        assert_eq!(ui.get_output_profile_status(), "Session only");
        assert!(ui.get_library_storage().contains("preserved"));
        ui.set_output_pair_index(1);
        ui.invoke_output_pair_changed();
        ui.set_quantize_enabled(false);
        finish_storage(&c);
        assert_eq!(engine.output_pair(), 1);
        assert!(!c.saved_quantize.get().enabled);
        assert!(!c.save_failed.get());
        assert_eq!(ui.get_output_profile_status(), "Session only");
        c.close_library().unwrap();
        let preserved: String = connection
            .query_row(
                "SELECT value FROM library_meta WHERE key='audio_output_routes_v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(preserved, raw);
    }
    #[test]
    fn output_pair_selection_routes_stereo_without_changing_deck_state() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(4, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 4, 48000));
        let file = crate::test_support::wav_samples(&[12000_i16, -6000].repeat(48000), 48000, 2);
        engine.load_a(file.path()).unwrap();
        let _c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        ui.set_crossfader(0.);
        ui.invoke_mixer_changed();
        engine.play(DeckId::A);
        for _ in 0..4096 {
            output.next();
        }
        let normal: [f32; 4] = std::array::from_fn(|_| output.next().unwrap());
        assert!(normal[0] > 0.3 && normal[1] < -0.15);
        assert_eq!(&normal[2..], &[0., 0.]);
        let before = engine.snapshot(DeckId::A);
        let gains = engine.output_volumes();
        ui.set_output_pair_index(1);
        ui.invoke_output_pair_changed();
        assert_eq!(engine.output_pair(), 1);
        let after = engine.snapshot(DeckId::A);
        assert_eq!(after.position, before.position);
        assert_eq!(after.playing, before.playing);
        assert_eq!(after.requested_speed, before.requested_speed);
        assert_eq!(engine.output_volumes(), gains);
        let clock = engine.rendered_frames();
        let left = output.next().unwrap();
        assert_eq!(engine.rendered_frames(), clock);
        let routed: [f32; 4] = [
            left,
            output.next().unwrap(),
            output.next().unwrap(),
            output.next().unwrap(),
        ];
        assert_eq!(engine.rendered_frames(), clock + 1);
        assert_eq!(routed, [0., 0., normal[0], normal[1]]);
        ui.set_output_pair_index(99);
        ui.invoke_output_pair_changed();
        assert_eq!(ui.get_output_pair_index(), 1);
        assert_eq!(engine.output_pair(), 1);
        assert!(ui.get_status().contains("unavailable"));
        let allocations = crate::looping::counted_allocations(|| {
            for i in 0..10000 {
                if i % 32 == 0 {
                    engine.set_output_pair(((i / 32) % 2) as u16).unwrap();
                }
                output.next();
            }
        });
        assert_eq!(allocations, 0);
    }
    #[test]
    fn quantize_preferences_restart_retains_latest_changes_and_close_flush() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(2, 48000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), vec![]);
        assert_eq!(
            control(&ui, "Global Quantize").accessible_enabled(),
            Some(false)
        );
        finish_storage(&c);
        assert_eq!(
            c.quantize_preferences().unwrap(),
            crate::preferences::QuantizePreferences::default()
        );
        assert_eq!(
            control(&ui, "Global Quantize").accessible_enabled(),
            Some(true)
        );
        ui.set_quantize_enabled(false);
        ui.set_left_quantize_cues(true);
        ui.set_right_quantize_triggers(true);
        ui.set_left_quantize_step(0);
        ui.set_right_quantize_step(5);
        c.save_library();
        assert!(c.saving.get());
        ui.set_left_quantize_triggers(true);
        ui.set_left_auto_loop_snap(true);
        ui.set_right_auto_loop_snap(true);
        ui.set_right_quantize_cues(true);
        finish_storage(&c);
        assert_eq!(c.saved_quantize.get(), c.quantize_preferences().unwrap());
        assert!(!c.dirty.get());
        // A last change without another GUI tick must still be flushed on close.
        ui.set_right_quantize_step(2);
        let expected = c.quantize_preferences().unwrap();
        c.close_library().unwrap();
        let ui2 = AppWindow::new().unwrap();
        let (mixer, _output2) = rodio::mixer::mixer(2, 48000);
        let engine2 = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
        let c2 = AudioController::wire(&ui2, engine2.clone(), Arc::new(|| None), Arc::new(|| None));
        c2.restore_library(Ok(db), vec![]);
        finish_storage(&c2);
        assert_eq!(c2.quantize_preferences().unwrap(), expected);
        assert!(!engine2.snapshot(DeckId::A).loaded && !engine2.snapshot(DeckId::B).playing);
        assert!(!c2.dirty.get());
        c2.close_library().unwrap();
    }
    #[test]
    fn quantize_preference_failure_retry_and_invalid_value_preserve_committed_state() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(2, 48000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), vec![]);
        finish_storage(&c);
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_preferences BEFORE INSERT ON library_meta WHEN NEW.key='quantize_preferences_v1' BEGIN SELECT RAISE(ABORT,'injected preference write'); END;").unwrap();
        ui.set_quantize_enabled(false);
        ui.set_left_quantize_step(0);
        let expected = c.quantize_preferences().unwrap();
        finish_storage(&c);
        assert!(c.save_failed.get() && c.dirty.get());
        assert!(ui.get_library_save_failed());
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM library_meta WHERE key='quantize_preferences_v1'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        connection
            .execute_batch("DROP TRIGGER reject_preferences")
            .unwrap();
        ui.invoke_retry_library_save();
        finish_storage(&c);
        assert!(!c.save_failed.get() && !c.dirty.get());
        assert_eq!(c.saved_quantize.get(), expected);
        ui.set_left_quantize_step(99);
        finish_storage(&c);
        assert!(c.save_failed.get());
        let persisted: String = connection
            .query_row(
                "SELECT value FROM library_meta WHERE key='quantize_preferences_v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<crate::preferences::QuantizePreferences>(&persisted).unwrap(),
            expected
        );
        ui.set_left_quantize_step(3);
        finish_storage(&c);
        assert!(!c.save_failed.get());
        c.close_library().unwrap();
    }
    #[test]
    fn quantize_preference_close_preserves_revert_and_does_not_overwrite_during_restore() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        for closing_during_restore in [false, true] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(2, 48000);
            let c = AudioController::wire(
                &ui,
                Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000)),
                Arc::new(|| None),
                Arc::new(|| None),
            );
            c.restore_library(Ok(db.clone()), vec![]);
            if !closing_during_restore {
                finish_storage(&c);
                ui.set_quantize_enabled(false);
                c.save_library();
                assert!(c.saving.get());
                ui.set_quantize_enabled(true);
            } else {
                ui.set_quantize_enabled(false);
            }
            c.close_library().unwrap();
            let worker = crate::library_store::Worker::start(db.clone()).unwrap();
            let crate::library_store::Event::Loaded(Ok(loaded)) = worker.events.recv().unwrap()
            else {
                panic!("restore")
            };
            assert!(loaded.quantize_preferences.enabled);
            worker.close_delta(None).unwrap();
        }
    }
    #[test]
    fn programmatic_global_quantize_off_cancels_both_queued_cues() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
        let file = wav_file(4096, 48000 * 3);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        c.change_hot_cue(DeckId::A, 0, false).unwrap();
        ui.set_left_quantize_triggers(true);
        ui.set_right_quantize_triggers(true);
        for id in [DeckId::A, DeckId::B] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine
                .seek_position(id, Duration::from_millis(200))
                .unwrap();
            engine.play(id);
        }
        for _ in 0..128 {
            output.next();
        }
        for id in [DeckId::A, DeckId::B] {
            c.change_hot_cue(id, 0, false).unwrap();
        }
        let before = [
            engine.snapshot(DeckId::A).position,
            engine.snapshot(DeckId::B).position,
        ];
        ui.set_quantize_enabled(false);
        slint::platform::update_timers_and_animations();
        for _ in 0..48000 * 2 {
            output.next();
        }
        for (index, id) in [DeckId::A, DeckId::B].into_iter().enumerate() {
            let expected = before[index] + Duration::from_secs(1);
            assert!(engine.snapshot(id).position.abs_diff(expected) < Duration::from_millis(2));
        }
        ui.set_quantize_enabled(true);
        slint::platform::update_timers_and_animations();
        engine.apply_sync_factor(DeckId::A, 1.01).unwrap();
        engine.set_nudge(DeckId::B, 1).unwrap();
        let active = [engine.snapshot(DeckId::A), engine.snapshot(DeckId::B)];
        ui.set_quantize_enabled(false);
        slint::platform::update_timers_and_animations();
        for (index, id) in [DeckId::A, DeckId::B].into_iter().enumerate() {
            let current = engine.snapshot(id);
            assert_eq!(current.position, active[index].position);
            assert_eq!(current.effective_speed, active[index].effective_speed);
            assert_eq!(current.sync_factor, active[index].sync_factor);
            assert_eq!(current.nudge_direction, active[index].nudge_direction);
            assert!(current.playing);
        }
    }
    #[test]
    fn global_quantize_bypasses_both_decks_preserves_preferences_and_cancels_triggers() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
        let a = wav_file(4096, 48000 * 8);
        let b = wav_file(8192, 48000 * 8);
        engine.load_a(a.path()).unwrap();
        engine.load_b(b.path()).unwrap();
        for id in [DeckId::A, DeckId::B] {
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
        }
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        for id in [DeckId::A, DeckId::B] {
            let path = engine.snapshot(id).path.unwrap();
            c.library.borrow_mut().insert([path.to_path_buf()]);
            c.library
                .borrow_mut()
                .set_hot_cue(
                    &path,
                    0,
                    Some(
                        crate::hot_cues::HotCue::new(
                            Duration::from_secs(1),
                            "saved".into(),
                            0xffffff,
                        )
                        .unwrap(),
                    ),
                )
                .unwrap();
        }
        ui.set_left_quantize_cues(true);
        ui.set_right_quantize_cues(true);
        ui.set_left_quantize_triggers(true);
        ui.set_right_quantize_triggers(true);
        ui.set_left_auto_loop_snap(true);
        ui.set_right_auto_loop_snap(true);
        ui.set_left_quantize_step(0);
        ui.set_right_quantize_step(5);
        c.tick();
        ui.show().unwrap();
        slint::platform::update_timers_and_animations();
        assert!(ui.get_quantize_enabled());
        for id in [DeckId::A, DeckId::B] {
            engine
                .seek_position(id, Duration::from_millis(200))
                .unwrap();
            engine.play(id);
        }
        for _ in 0..128 {
            output.next();
        }
        for id in [DeckId::A, DeckId::B] {
            c.change_hot_cue(id, 0, false).unwrap();
        }
        let before = [engine.snapshot(DeckId::A), engine.snapshot(DeckId::B)];
        let gains = engine.output_volumes();
        control(&ui, "Global Quantize").invoke_accessible_default_action();
        assert!(!ui.get_quantize_enabled());
        for (index, id) in [DeckId::A, DeckId::B].into_iter().enumerate() {
            let current = engine.snapshot(id);
            assert_eq!(current.position, before[index].position);
            assert_eq!(current.grid, before[index].grid);
            assert_eq!(current.requested_speed, before[index].requested_speed);
            assert!(current.playing);
        }
        // Do not process change handlers first: the toolbar event must cancel immediately.
        for _ in 0..48000 * 2 * 2 {
            output.next();
        }
        for (index, id) in [DeckId::A, DeckId::B].into_iter().enumerate() {
            let expected = before[index].position + Duration::from_secs(2);
            assert!(engine.snapshot(id).position.abs_diff(expected) < Duration::from_millis(2));
        }
        assert_eq!(engine.output_volumes(), gains);
        assert!(ui.get_left_quantize_cues() && ui.get_right_quantize_cues());
        assert!(ui.get_left_quantize_triggers() && ui.get_right_quantize_triggers());
        assert!(ui.get_left_auto_loop_snap() && ui.get_right_auto_loop_snap());
        assert_eq!(
            (ui.get_left_quantize_step(), ui.get_right_quantize_step()),
            (0, 5)
        );
        for (id, label) in [(DeckId::A, "DECK A"), (DeckId::B, "DECK B")] {
            assert_eq!(
                control(&ui, &format!("{label} quantize new cues")).accessible_enabled(),
                Some(false)
            );
            assert_eq!(
                control(&ui, &format!("{label} quantization step")).accessible_enabled(),
                Some(false)
            );
            engine.pause(id);
            engine
                .seek_position(id, Duration::from_nanos(1_123_456_789))
                .unwrap();
            let raw = engine.snapshot(id).position;
            c.change_hot_cue(id, 1, false).unwrap();
            let path = engine.snapshot(id).path.unwrap();
            assert_eq!(
                c.library.borrow().hot_cues(&path).slots()[1]
                    .as_ref()
                    .unwrap()
                    .position(),
                raw
            );
            c.change_hot_cue(id, 0, false).unwrap();
            assert_eq!(engine.snapshot(id).position, Duration::from_secs(1));
            engine
                .seek_position(id, Duration::from_millis(850))
                .unwrap();
            match id {
                DeckId::A => ui.invoke_auto_loop_left(),
                DeckId::B => ui.invoke_auto_loop_right(),
            }
            assert_eq!(
                engine.snapshot(id).loop_in,
                Some(Duration::from_millis(850))
            );
        }
        control(&ui, "Global Quantize").invoke_accessible_default_action();
        assert!(ui.get_quantize_enabled());
        for id in [DeckId::A, DeckId::B] {
            assert_eq!(
                engine.snapshot(id).loop_in,
                Some(Duration::from_millis(850))
            );
            engine.exit_loop(id).unwrap();
            engine
                .seek_position(id, Duration::from_millis(850))
                .unwrap();
            match id {
                DeckId::A => ui.invoke_auto_loop_left(),
                DeckId::B => ui.invoke_auto_loop_right(),
            }
            assert_eq!(engine.snapshot(id).loop_in, Some(Duration::from_secs(1)));
            engine.exit_loop(id).unwrap();
            engine
                .seek_position(id, Duration::from_millis(1350))
                .unwrap();
            c.change_hot_cue(id, 2, false).unwrap();
            let path = engine.snapshot(id).path.unwrap();
            let expected = match id {
                DeckId::A => Duration::from_millis(1375),
                DeckId::B => Duration::from_secs(2),
            };
            assert_eq!(
                c.library.borrow().hot_cues(&path).slots()[2]
                    .as_ref()
                    .unwrap()
                    .position(),
                expected
            );
        }
        ui.hide().unwrap();
    }
    #[test]
    fn quantization_selector_cancels_pending_recall_and_keeps_other_deck_choice() {
        i_slint_backend_testing::init_no_event_loop();
        for id in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
            let file = wav_file(4096, 48000 * 5);
            engine.load_a(file.path()).unwrap();
            engine.load_b(file.path()).unwrap();
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            let path = engine.snapshot(id).path.unwrap();
            c.library.borrow_mut().insert([path.to_path_buf()]);
            c.library
                .borrow_mut()
                .set_hot_cue(
                    &path,
                    0,
                    Some(
                        crate::hot_cues::HotCue::new(Duration::ZERO, "zero".into(), 0xffffff)
                            .unwrap(),
                    ),
                )
                .unwrap();
            c.tick();
            ui.show().unwrap();
            let label = match id {
                DeckId::A => "DECK A",
                DeckId::B => "DECK B",
            };
            let selector = control(&ui, &format!("{label} quantization step"));
            assert_eq!(selector.accessible_enabled(), Some(true));
            match id {
                DeckId::A => ui.set_left_quantize_step(5),
                DeckId::B => ui.set_right_quantize_step(5),
            }
            slint::platform::update_timers_and_animations();
            let actual = match id {
                DeckId::A => ui.get_left_quantize_step(),
                DeckId::B => ui.get_right_quantize_step(),
            };
            assert_eq!(actual, 5);
            match id {
                DeckId::A => ui.set_left_quantize_triggers(true),
                DeckId::B => ui.set_right_quantize_triggers(true),
            }
            engine
                .seek_position(id, Duration::from_millis(200))
                .unwrap();
            engine.play(id);
            for _ in 0..128 {
                output.next();
            }
            c.change_hot_cue(id, 0, false).unwrap();
            match id {
                DeckId::A => ui.set_left_quantize_step(1),
                DeckId::B => ui.set_right_quantize_step(1),
            }
            slint::platform::update_timers_and_animations();
            let actual = match id {
                DeckId::A => ui.get_left_quantize_step(),
                DeckId::B => ui.get_right_quantize_step(),
            };
            assert_eq!(actual, 1);
            for _ in 0..48000 * 2 * 2 {
                output.next();
            }
            assert!(
                engine.snapshot(id).position > Duration::from_secs(2),
                "old queued cue must not jump to zero"
            );
            let other = match id {
                DeckId::A => ui.get_right_quantize_step(),
                DeckId::B => ui.get_left_quantize_step(),
            };
            assert_eq!(other, 3);
            ui.hide().unwrap();
        }
    }
    #[test]
    fn selectable_quantization_places_and_schedules_cues_on_both_decks() {
        i_slint_backend_testing::init_no_event_loop();
        for id in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, mut output) = rodio::mixer::mixer(2, 48000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48000));
            let file = wav_file(4096, 48000 * 8);
            engine.load_a(file.path()).unwrap();
            engine.load_b(file.path()).unwrap();
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            let path = engine.snapshot(id).path.unwrap();
            c.library.borrow_mut().insert([path.to_path_buf()]);
            match id {
                DeckId::A => ui.set_left_quantize_cues(true),
                DeckId::B => ui.set_right_quantize_cues(true),
            }
            for (index, expected_ms) in [312.5, 250., 250., 500., 0., 0.].into_iter().enumerate() {
                engine.pause(id);
                match id {
                    DeckId::A => {
                        ui.set_left_quantize_step(index as i32);
                        ui.set_left_quantize_triggers(false);
                    }
                    DeckId::B => {
                        ui.set_right_quantize_step(index as i32);
                        ui.set_right_quantize_triggers(false);
                    }
                }
                engine
                    .seek_position(id, Duration::from_millis(290))
                    .unwrap();
                c.change_hot_cue(id, index as i32, false).unwrap();
                let target = c.library.borrow().hot_cues(&path).slots()[index]
                    .as_ref()
                    .unwrap()
                    .position();
                assert_eq!(target, Duration::from_secs_f64(expected_ms / 1000.));
                c.change_hot_cue(id, index as i32, false).unwrap();
                assert_eq!(
                    engine.snapshot(id).position,
                    target,
                    "saved target is recalled exactly"
                );
                engine
                    .seek_position(id, Duration::from_millis(100))
                    .unwrap();
                engine.play(id);
                for _ in 0..128 {
                    output.next();
                }
                match id {
                    DeckId::A => ui.set_left_quantize_triggers(true),
                    DeckId::B => ui.set_right_quantize_triggers(true),
                }
                c.change_hot_cue(id, index as i32, false).unwrap();
                let deadline = [0.125, 0.125, 0.25, 0.5, 1., 2.][index];
                let before = ((deadline - 0.1) * 48000.) as usize - 200;
                for _ in 0..before * 2 {
                    output.next();
                }
                let position = engine.snapshot(id).position.as_secs_f64();
                assert!(
                    position > 0.1 && position < deadline,
                    "step {index}: {position}"
                );
                for _ in 0..1000 {
                    output.next();
                }
                let position = engine.snapshot(id).position;
                assert!(
                    position >= target && position < target + Duration::from_millis(20),
                    "step {index}: {position:?} target {target:?}"
                );
            }
            match id {
                DeckId::A => ui.set_left_quantize_step(99),
                DeckId::B => ui.set_right_quantize_step(99),
            }
            let before = engine.snapshot(id).position;
            assert!(c.change_hot_cue(id, 0, false).is_err());
            assert_eq!(engine.snapshot(id).position, before);
            assert!(c.change_hot_cue(id, 6, false).is_err());
            assert!(c.library.borrow().hot_cues(&path).slots()[6].is_none());
            match id {
                DeckId::A => ui.set_left_quantize_step(5),
                DeckId::B => ui.set_right_quantize_step(5),
            }
            engine.pause(id);
            engine
                .seek_position(id, Duration::from_millis(7900))
                .unwrap();
            assert!(
                c.change_hot_cue(id, 6, false).is_err(),
                "nearest grid point is EOF"
            );
            assert!(c.library.borrow().hot_cues(&path).slots()[6].is_none());
            engine.play(id);
            for _ in 0..128 {
                output.next();
            }
            let before = engine.snapshot(id).position;
            assert!(
                c.change_hot_cue(id, 0, false).is_err(),
                "next grid point is EOF"
            );
            assert_eq!(engine.snapshot(id).position, before);
        }
    }
    #[test]
    fn quantized_hot_cue_recall_waits_for_next_rendered_beat_on_both_decks() {
        i_slint_backend_testing::init_no_event_loop();
        for id in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
            let file = wav_file(4096, 144_000);
            engine.load_a(file.path()).unwrap();
            engine.load_b(file.path()).unwrap();
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120.0, Duration::ZERO).unwrap()),
                )
                .unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            let cue_path = engine.snapshot(id).path.unwrap();
            c.library.borrow_mut().insert([cue_path.to_path_buf()]);
            c.library
                .borrow_mut()
                .set_hot_cue(
                    &cue_path,
                    0,
                    Some(
                        crate::hot_cues::HotCue::new(
                            Duration::from_secs(1),
                            "jump".into(),
                            0xffffff,
                        )
                        .unwrap(),
                    ),
                )
                .unwrap();
            match id {
                DeckId::A => ui.set_left_quantize_triggers(true),
                DeckId::B => ui.set_right_quantize_triggers(true),
            }
            assert!(c.change_hot_cue(id, 0, false).is_err());
            engine.play(id);
            for _ in 0..1000 {
                output.next();
            }
            c.change_hot_cue(id, 0, false).unwrap();
            assert!(engine.snapshot(id).position < Duration::from_millis(100));
            for _ in 0..24_000 {
                output.next();
            }
            let position = engine.snapshot(id).position;
            assert!(
                position >= Duration::from_secs(1) && position < Duration::from_millis(1100),
                "{position:?}"
            );
        }
    }

    #[test]
    fn hot_cue_number_keys_preview_until_release_and_play_promotes_on_both_decks() {
        use slint::{platform::WindowEvent, Model};
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let file = wav_file(8192, 48_000 * 8);
        let other = wav_file(4096, 48_000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        for (id, position) in [
            (DeckId::A, Duration::from_secs(1)),
            (DeckId::B, Duration::from_secs(2)),
        ] {
            let path = engine.snapshot(id).path.unwrap();
            c.library.borrow_mut().insert([path.to_path_buf()]);
            c.library
                .borrow_mut()
                .set_hot_cue(
                    &path,
                    0,
                    Some(
                        crate::hot_cues::HotCue::new(position, "preview".into(), 0xffffff).unwrap(),
                    ),
                )
                .unwrap();
        }
        let path_a = engine.snapshot(DeckId::A).path.unwrap();
        c.library
            .borrow_mut()
            .set_hot_cue(
                &path_a,
                7,
                Some(
                    crate::hot_cues::HotCue::new(
                        Duration::from_secs(3),
                        "keyboard pad".into(),
                        0xffffff,
                    )
                    .unwrap(),
                ),
            )
            .unwrap();
        c.refresh();
        ui.show().unwrap();
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));

        engine
            .seek_position(DeckId::A, Duration::from_secs(4))
            .unwrap();
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "1".into() });
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(!engine.snapshot(DeckId::B).playing);
        assert!(c.library.borrow().catalog().history.is_empty());
        for _ in 0..2400 {
            output.next();
        }
        let preview_position = engine.snapshot(DeckId::A).position;
        assert!(preview_position > Duration::from_millis(1040));
        ui.window()
            .dispatch_event(WindowEvent::KeyPressRepeated { text: "1".into() });
        assert_eq!(engine.snapshot(DeckId::A).position, preview_position);
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "1".into() });
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));
        assert!(c.library.borrow().catalog().history.is_empty());

        c.hold_hot_cue(DeckId::A, 0, true, 1).unwrap();
        c.hold_hot_cue(DeckId::A, 0, true, 5).unwrap();
        c.hold_hot_cue(DeckId::A, 0, false, 5).unwrap();
        assert!(engine.snapshot(DeckId::A).playing);
        c.hold_hot_cue(DeckId::A, 0, false, 1).unwrap();
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));

        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "1".into() });
        for _ in 0..2400 {
            output.next();
        }
        ui.invoke_toggle_left();
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(c.library.borrow().catalog().history[0].play_count, 1);
        ui.invoke_toggle_left();
        assert!(!engine.snapshot(DeckId::A).playing);
        ui.invoke_toggle_left();
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(c.library.borrow().catalog().history[0].play_count, 2);
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "1".into() });
        assert!(engine.snapshot(DeckId::A).playing);
        for _ in 0..4800 {
            output.next();
        }
        assert!(engine.snapshot(DeckId::A).position > Duration::from_millis(1100));
        ui.invoke_select_crate(crate::library::HISTORY_SCOPE);
        assert_eq!(
            c.library.borrow().active_crate(),
            crate::library::HISTORY_SCOPE
        );
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 1);

        engine
            .seek_position(DeckId::B, Duration::from_secs(5))
            .unwrap();
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "6".into() });
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(c.library.borrow().catalog().history.len(), 1);
        for _ in 0..2400 {
            output.next();
        }
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "6".into() });
        assert!(!engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).position, Duration::from_secs(2));

        let waveform = control(&ui, "DECK B detailed waveform");
        let p = waveform.absolute_position();
        let size = waveform.size();
        let waveform_position =
            slint::LogicalPosition::new(p.x + size.width / 2., p.y + size.height / 2.);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: waveform_position,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: waveform_position,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "6".into() });
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(false));
        c.handle_window_deactivated();
        c.tick();
        assert!(!engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).position, Duration::from_secs(2));
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "6".into() });
        assert_eq!(ui.get_active_hot_cue_key(), -1);

        engine.pause(DeckId::A);
        let pad = control(&ui, "DECK A hot cue 1");
        let p = pad.absolute_position();
        let size = pad.size();
        let position = slint::LogicalPosition::new(p.x + size.width / 2., p.y + size.height / 2.);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
        assert!(engine.snapshot(DeckId::A).playing);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Right,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Right,
        });
        assert!(engine.snapshot(DeckId::A).playing);
        for _ in 0..2400 {
            output.next();
        }
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));

        engine
            .seek_position(DeckId::A, Duration::from_secs(4))
            .unwrap();
        let search = control(&ui, "Library search");
        let p = search.absolute_position();
        let size = search.size();
        let search_position =
            slint::LogicalPosition::new(p.x + size.width / 2., p.y + size.height / 2.);
        let detail = control(&ui, "DECK A detailed waveform");
        let detail_position = detail.absolute_position();
        let detail_size = detail.size();
        let detail_center = slint::LogicalPosition::new(
            detail_position.x + detail_size.width / 2.,
            detail_position.y + detail_size.height / 2.,
        );
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: detail_center,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: detail_center,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "1".into() });
        assert!(engine.snapshot(DeckId::A).playing);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: search_position,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: search_position,
            button: slint::platform::PointerEventButton::Left,
        });
        c.handle_global_hot_cue_key_released(0);
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(1));
        assert_eq!(ui.get_active_hot_cue_key(), -1);
        engine
            .seek_position(DeckId::A, Duration::from_secs(4))
            .unwrap();
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "1".into() });
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "1".into() });
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(4));
        assert_eq!(ui.get_library_query().as_str(), "1");

        let slot_eight = control(&ui, "DECK A hot cue 8");
        let p = slot_eight.absolute_position();
        let size = slot_eight.size();
        let slot_position =
            slint::LogicalPosition::new(p.x + size.width / 2., p.y + size.height / 2.);
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: slot_position,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: slot_position,
            button: slint::platform::PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: slint::platform::Key::Space.into(),
        });
        assert!(engine.snapshot(DeckId::A).playing);
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: slint::platform::Key::Space.into(),
        });
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, Duration::from_secs(3));
    }

    #[test]
    fn overlapping_hot_cues_keep_distinct_waveform_label_lanes() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let file = wav_file(8192, 48_000 * 8);
        engine.load_a(file.path()).unwrap();
        let c = AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
        let path = engine.snapshot(DeckId::A).path.unwrap();
        c.library.borrow_mut().insert([path.to_path_buf()]);
        for slot in 0..crate::hot_cues::SLOT_COUNT {
            c.library
                .borrow_mut()
                .set_hot_cue(
                    &path,
                    slot,
                    Some(
                        crate::hot_cues::HotCue::new(
                            Duration::from_millis(7600),
                            format!("Cue {}", slot + 1),
                            0xffffff,
                        )
                        .unwrap(),
                    ),
                )
                .unwrap();
        }
        ui.show().unwrap();
        c.refresh();
        let model = ui.get_left_hot_cues();
        let rows = (0..crate::hot_cues::SLOT_COUNT)
            .map(|slot| model.row_data(slot).unwrap())
            .collect::<Vec<_>>();
        assert!(rows.iter().all(|row| row.valid && row.position == 95.));
        assert_eq!(
            rows.iter()
                .map(|row| (row.label_row, row.label_column))
                .collect::<std::collections::HashSet<_>>()
                .len(),
            crate::hot_cues::SLOT_COUNT
        );
        let waveform = control(&ui, "DECK A waveform overview");
        let waveform_position = waveform.absolute_position();
        let waveform_size = waveform.size();
        for slot in 1..=crate::hot_cues::SLOT_COUNT {
            let label = control(&ui, &format!("DECK A overview cue label {slot}"));
            let position = label.absolute_position();
            let size = label.size();
            assert!(position.x >= waveform_position.x, "cue {slot} left edge");
            assert!(
                position.x + size.width <= waveform_position.x + waveform_size.width,
                "cue {slot} right edge"
            );
            assert!(position.y >= waveform_position.y, "cue {slot} top edge");
            assert!(
                position.y + size.height <= waveform_position.y + waveform_size.height,
                "cue {slot} bottom edge"
            );
        }
    }

    #[test]
    fn hot_cue_pads_set_recall_delete_share_and_restore_without_transport_changes() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let file = wav_file(8192, 48_000 * 8);
        let other = wav_file(4096, 48_000 * 8);
        let bad = TempFile::new("mp3", b"invalid");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        ui.show().unwrap();
        ui.set_channel_a(0.6);
        ui.set_channel_b(0.8);
        ui.set_crossfader(0.3);
        ui.invoke_mixer_changed();
        let gains = engine.output_volumes();
        engine
            .seek_position(DeckId::A, Duration::new(2, 123456789))
            .unwrap();
        let position = engine.snapshot(DeckId::A).position;
        control(&ui, "DECK A hot cue 1").invoke_accessible_default_action();
        finish_storage(&c);
        assert!(ui.get_left_hot_cues().row_data(0).unwrap().occupied);
        assert!(!ui.get_right_hot_cues().row_data(0).unwrap().occupied);
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        assert!(!engine.snapshot(DeckId::A).playing);
        engine.seek_percent(DeckId::A, 70.).unwrap();
        control(&ui, "DECK A hot cue 1").invoke_accessible_default_action();
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        engine.play(DeckId::A);
        engine.play(DeckId::B);
        engine.seek_percent(DeckId::A, 80.).unwrap();
        control(&ui, "DECK A hot cue 1").invoke_accessible_default_action();
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).playing);
        assert_eq!(engine.snapshot(DeckId::B).position, Duration::ZERO);
        assert_eq!(engine.output_volumes(), gains);
        assert!(!c.dirty.get()); // Recalling does not write the catalog.
        engine.pause(DeckId::B);
        engine.load_b(file.path()).unwrap();
        c.tick();
        assert!(ui.get_right_hot_cues().row_data(0).unwrap().occupied);
        control(&ui, "DECK B hot cue 8").invoke_accessible_default_action();
        finish_storage(&c);
        assert!(ui.get_left_hot_cues().row_data(7).unwrap().occupied);
        control(&ui, "DECK A delete hot cue slot").set_accessible_value("8");
        control(&ui, "DECK A delete hot cue").invoke_accessible_default_action();
        finish_storage(&c);
        assert!(!ui.get_right_hot_cues().row_data(7).unwrap().occupied);
        assert!(ui.get_right_hot_cues().row_data(0).unwrap().occupied);
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.output_volumes(), gains);
        engine.pause(DeckId::A);
        assert!(engine.load_a(bad.path()).is_err());
        c.tick();
        assert!(ui.get_left_hot_cues().row_data(0).unwrap().occupied);
        engine.load_a(other.path()).unwrap();
        c.tick();
        assert!(!ui.get_left_hot_cues().row_data(0).unwrap().occupied);
        assert!(c.change_hot_cue(DeckId::A, -1, false).is_err());
        assert!(c.change_hot_cue(DeckId::A, 8, false).is_err());
        engine.seek_percent(DeckId::A, 100.).unwrap();
        assert!(c.change_hot_cue(DeckId::A, 0, false).is_err());
        let path = engine.snapshot(DeckId::A).path.unwrap();
        c.library.borrow_mut().insert([path.to_path_buf()]);
        c.library
            .borrow_mut()
            .set_hot_cue(
                &path,
                0,
                Some(
                    crate::hot_cues::HotCue::new(Duration::from_secs(9), "Old file".into(), 0)
                        .unwrap(),
                ),
            )
            .unwrap();
        let before = engine.snapshot(DeckId::A).position;
        assert!(c
            .change_hot_cue(DeckId::A, 0, false)
            .unwrap_err()
            .contains("outside"));
        assert_eq!(engine.snapshot(DeckId::A).position, before);
        c.tick();
        assert!(!ui.get_left_hot_cues().row_data(0).unwrap().valid);
        assert_eq!(
            ElementHandle::find_by_accessible_label(&ui, "DECK A overview cue marker 1").count(),
            0
        );
        c.library_changed();
        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output2) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        engine.load_b(file.path()).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        assert!(ui.get_right_hot_cues().row_data(0).unwrap().occupied);
        assert!(!ui.get_right_hot_cues().row_data(7).unwrap().occupied);
        control(&ui, "DECK B hot cue 1").invoke_accessible_default_action();
        assert_eq!(engine.snapshot(DeckId::B).position, position);
        assert!(!engine.snapshot(DeckId::B).playing);
        control(&ui, "DECK B delete hot cue").invoke_accessible_default_action();
        c.close_library().unwrap();
        drop(c);
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(dir.path().join("library.sqlite3")), Vec::new());
        finish_storage(&c);
        assert!(!ui.get_right_hot_cues().row_data(0).unwrap().occupied);
        c.close_library().unwrap();
    }
    #[test]
    fn durable_library_restores_real_track_ids_missing_rows_and_loads_after_restart() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        let wav = wav_file(8192, 48_000);
        let missing = dir.path().join("will-disappear.wav");
        std::fs::copy(wav.path(), &missing).unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let ui = AppWindow::new().unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(path.clone()), vec![fixture(), missing.clone()]);
        assert!(ui.get_library_busy());
        finish_storage(&c);
        finish_import(&c);
        finish_storage(&c);
        assert_eq!(ui.get_library_rows().row_count(), 2);
        assert_eq!(ui.get_library_storage(), "Library saved");
        // A later import may be newer than the outstanding disk snapshot at shutdown.
        c.import_paths(vec![wav.path().into()]);
        finish_import(&c);
        c.close_library().unwrap();
        drop(c);
        std::fs::remove_file(&missing).unwrap();
        let ui = AppWindow::new().unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(path.clone()), Vec::new());
        finish_storage(&c);
        assert_eq!(ui.get_library_rows().row_count(), 3);
        let missing_row = ui
            .get_library_rows()
            .iter()
            .find(|row| row.id == 2)
            .unwrap();
        assert!(!missing_row.available);
        assert!(ui.get_library_count().contains("1 missing"));
        ui.invoke_select_library(1);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(ui.get_left_title().ends_with("tone-9s.mp3"));
        assert!(!engine.snapshot(DeckId::A).playing);
        assert_eq!(c.library.borrow().selected_id(), 1);
        ui.invoke_select_library(2);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(ui.get_left_title().ends_with("tone-9s.mp3"));
        assert!(!ui.get_status().contains("loaded and paused"));
        c.close_library().unwrap();
    }
    #[test]
    fn durable_library_migrates_json_into_sqlite_and_loads_same_id_after_restart() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("library.json");
        let catalog = crate::library::Catalog {
            version: 1,
            crates: Default::default(),
            tracks: vec![crate::library::SavedTrack {
                hot_cues: Default::default(),
                saved_loops: Default::default(),
                id: 73,
                path: fixture(),
                metadata: Default::default(),
                grid: None,
            }],
            history: Vec::new(),
        };
        let original = serde_json::to_vec(&catalog).unwrap();
        std::fs::write(&legacy, &original).unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(legacy.clone()), Vec::new());
        finish_storage(&c);
        assert!(ui.get_library_storage().contains("Migrated JSON library"));
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 73);
        ui.invoke_select_library(73);
        ui.invoke_load_library_right();
        finish(&c);
        assert!(ui.get_right_title().ends_with("tone-9s.mp3"));
        assert!(!ui.get_right_playing());
        c.close_library().unwrap();
        assert_eq!(std::fs::read(&legacy).unwrap(), original);
        drop(c);
        // Initialized SQLite wins even if the retained legacy input later becomes unreadable.
        std::fs::write(&legacy, b"stale invalid legacy").unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _restarted_output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(&ui, engine, Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(legacy.with_extension("sqlite3")), Vec::new());
        finish_storage(&c);
        assert_eq!(ui.get_library_storage(), "SQLite library restored");
        assert!(!ui.get_right_loaded());
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 73);
        ui.invoke_select_library(73);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(ui.get_left_title().ends_with("tone-9s.mp3"));
        c.close_library().unwrap();
    }
    #[test]
    fn additional_audio_formats_import_restore_and_load_into_both_decks() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("formats.sqlite3");
        let names = [
            "format-aac.m4a",
            "format-alac.m4a",
            "format-flac.flac",
            "format-aiff.aiff",
            "format-vorbis.ogg",
            "format-opus.opus",
            "format-opus-mono.opus",
        ];
        let mut paths = Vec::new();
        for name in names {
            let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            let destination = dir.path().join(name);
            std::fs::copy(source, &destination).unwrap();
            let analysis = crate::audio::analyze_file(&destination).unwrap();
            assert_eq!(analysis.sample_rate, 48_000, "{name}");
            assert_eq!(
                analysis.channels,
                if name == "format-opus-mono.opus" {
                    1
                } else {
                    2
                },
                "{name}"
            );
            assert!((40_000..=52_000).contains(&analysis.frames), "{name}");
            assert!(analysis.peak > 0.01, "{name}");
            let metadata = crate::track_metadata::read(&destination);
            assert!(metadata.error.is_none(), "{name}: {:?}", metadata.error);
            if name != "format-aiff.aiff" {
                assert_eq!(metadata.title, "Fixture Tone", "{name}");
                assert_eq!(metadata.artist, "Fixture Artist", "{name}");
            }
            paths.push(destination);
        }
        let invalid = crate::test_support::TempFile::new("m4a", b"not a supported media container");
        assert!(crate::audio::analyze_file(invalid.path()).is_err());
        assert!(crate::playback::PreparedTrack::open(invalid.path()).is_err());
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(2, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        c.import_paths(paths);
        finish_import(&c);
        finish_storage(&c);
        assert_eq!(ui.get_library_rows().row_count(), names.len());
        for (index, name) in names.iter().enumerate() {
            let id = index as i32 + 1;
            ui.invoke_select_library(id);
            ui.invoke_load_library_left();
            finish(&c);
            assert!(ui.get_left_title().ends_with(name), "{name}");
            ui.invoke_select_library(id);
            ui.invoke_load_library_right();
            finish(&c);
            assert!(ui.get_right_title().ends_with(name), "{name}");
            for (deck, crossfader) in [(DeckId::A, 0.0), (DeckId::B, 1.0)] {
                engine.set_levels(MixerLevels {
                    crossfader,
                    channel_a: 1.0,
                    channel_b: 1.0,
                });
                engine.play(deck);
                let peak = output
                    .by_ref()
                    .take(24_000)
                    .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
                assert!(peak > 0.001, "no rendered audio for {name} deck {deck:?}");
                engine.pause(deck);
            }
        }
        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(2, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 2, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        assert_eq!(ui.get_library_rows().row_count(), names.len());
        ui.invoke_select_library(3);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(ui.get_left_title().ends_with("format-flac.flac"));
        c.close_library().unwrap();
    }

    #[test]
    fn metadata_table_search_sort_reload_and_paused_loading_preserve_identity() {
        use crate::test_support::{tagged_mp3, tagged_wav};
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let mp3 = tagged_mp3("Zulu title", "Artist Ñ", "First album");
        let wav = tagged_wav("Alpha title", "Other artist", "Second album");
        let paths = vec![mp3.path().to_owned(), wav.path().to_owned()];
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let c = AudioController::wire(&ui, engine, Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db.clone()), paths);
        finish_storage(&c);
        finish_import(&c);
        finish_storage(&c);
        let rows: Vec<_> = ui.get_library_rows().iter().collect();
        assert_eq!(rows[0].name, "Alpha title");
        assert_eq!(rows[0].id, 2);
        assert_eq!(rows[1].artist, "Artist Ñ");
        assert_eq!(rows[1].album, "First album");
        assert!(rows[1].duration.starts_with('~'));
        control(&ui, "Library search").set_accessible_value("ARTIST Ñ");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        ui.invoke_select_library(1);
        control(&ui, "Clear library search").invoke_accessible_default_action();
        finish_search(&c);
        control(&ui, "Sort library by duration").invoke_accessible_default_action();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 2);
        assert_eq!(ui.get_selected_track(), 1);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(ui.get_left_title().ends_with(".mp3"));
        assert!(!ui.get_left_playing());
        let row = ui.get_library_rows().iter().find(|r| r.id == 1).unwrap();
        assert!(!row.duration.starts_with('~'));
        assert_eq!(row.name, "Zulu title");
        finish_storage(&c);
        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output2) = rodio::mixer::mixer(1, 48000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        let row = ui.get_library_rows().iter().find(|r| r.id == 1).unwrap();
        assert_eq!(row.name, "Zulu title");
        assert_eq!(row.artist, "Artist Ñ");
        assert!(!row.duration.starts_with('~'));
        control(&ui, "Library search").set_accessible_value("second album");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 2);
        ui.invoke_select_library(2);
        ui.invoke_load_library_right();
        finish(&c);
        assert!(ui.get_right_title().ends_with(".wav"));
        assert!(!ui.get_right_playing());
        c.close_library().unwrap();
    }

    #[test]
    fn smart_crate_ui_filters_and_restores_saved_metadata_query() {
        use crate::test_support::tagged_wav;
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let first = tagged_wav("Morning Mix", "Artist Ñ", "First Album");
        let second = tagged_wav("Evening Mix", "Other Artist", "Second Album");
        let imported_match = tagged_wav("Late Set", "Artist Ñ", "Third Album");
        let imported_nonmatch = tagged_wav("Quiet Set", "Other", "Fourth Album");
        let paths = vec![first.path().to_owned(), second.path().to_owned()];
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(&ui, engine, Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db.clone()), paths.clone());
        finish_storage(&c);
        finish_import(&c);
        finish_storage(&c);
        ui.show().unwrap();

        control(&ui, "New smart crate").invoke_accessible_default_action();
        control(&ui, "Crate name").set_accessible_value("Artists from North");
        control(&ui, "Smart crate query").set_accessible_value("ARTIST Ñ");
        control(&ui, "Save crate name").invoke_accessible_default_action();
        finish_storage(&c);
        finish_search(&c);
        assert!(ui.get_selected_crate_smart());
        assert!(ui.get_crate_rows().row_data(0).unwrap().smart);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(
            ui.get_library_rows().row_data(0).unwrap().artist,
            "Artist Ñ"
        );
        c.import_paths(vec![
            imported_match.path().to_owned(),
            imported_nonmatch.path().to_owned(),
        ]);
        finish_import(&c);
        finish_storage(&c);
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 2);
        assert!(ui
            .get_library_rows()
            .iter()
            .all(|row| row.artist == "Artist Ñ"));
        assert!(
            c.library.borrow().crates().entries[0].tracks.is_empty(),
            "smart crates do not gain static memberships on import"
        );
        assert!(!ui.get_status().contains("assignment failed"));
        control(&ui, "Library search").set_accessible_value("First Album");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        control(&ui, "Library search").set_accessible_value("Second Album");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 0);

        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        ui.invoke_select_crate(1);
        finish_search(&c);
        assert!(ui.get_selected_crate_smart());
        assert_eq!(ui.get_library_rows().row_count(), 2);
        assert_eq!(
            ui.get_library_rows().row_data(0).unwrap().artist,
            "Artist Ñ"
        );
    }

    #[test]
    fn crate_gui_import_assignment_confirmation_restart_and_playing_deck_are_isolated() {
        use slint::Model;
        fn create(ui: &AppWindow, name: &str) {
            control(ui, "New crate").invoke_accessible_default_action();
            control(ui, "Crate name").set_accessible_value(name);
            control(ui, "Save crate name").invoke_accessible_default_action();
        }
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let wav = wav_file(8192, 48_000);
        let third = wav_file(4096, 24_000);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        ui.invoke_new_crate();
        assert!(!ui.get_crate_editor_open());
        finish_storage(&c);
        ui.show().unwrap();
        c.import_paths(vec![fixture(), wav.path().to_owned()]);
        finish_import(&c);
        create(&ui, "Música 日本");
        finish_search(&c);
        assert_eq!(ui.get_selected_crate(), 1);
        assert_eq!(ui.get_crate_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_count(), 0);
        assert!(ui.get_library_empty().contains("crate is empty"));
        control(&ui, "All library tracks").invoke_accessible_default_action();
        finish_search(&c);
        control(&ui, "Library track 1").invoke_accessible_default_action();
        control(&ui, "Add selected track to crate").invoke_accessible_default_action();
        control(&ui, "Add selected track to crate").invoke_accessible_default_action();
        assert_eq!(ui.get_crate_rows().row_data(0).unwrap().count, 1);
        control(&ui, "Crate 1").invoke_accessible_default_action();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        control(&ui, "Library search").set_accessible_value("no matching member");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 0);
        control(&ui, "Clear library search").invoke_accessible_default_action();
        finish_search(&c);
        ui.invoke_select_library(1);
        ui.invoke_load_library_left();
        finish(&c);
        ui.invoke_toggle_left();
        let title = ui.get_left_title();
        assert!(ui.get_left_playing());
        control(&ui, "Rename crate").invoke_accessible_default_action();
        control(&ui, "Crate name").set_accessible_value("Festival");
        control(&ui, "Save crate name").invoke_accessible_default_action();
        assert_eq!(ui.get_selected_crate(), 1);
        assert_eq!(ui.get_crate_rows().row_data(0).unwrap().name, "Festival");
        control(&ui, "New crate").invoke_accessible_default_action();
        control(&ui, "Save crate name").invoke_accessible_default_action();
        assert!(ui.get_crate_editor_open());
        assert!(ui.get_status().contains("printable"));
        control(&ui, "Cancel crate dialog").invoke_accessible_default_action();
        create(&ui, "Warmup");
        finish_search(&c);
        assert_eq!(ui.get_selected_crate(), 2);
        // An import belongs to the captured crate, even if a later callback changes the filter.
        c.import_paths(vec![third.path().to_owned()]);
        ui.invoke_select_crate(1);
        finish_search(&c);
        finish_import(&c);
        assert_eq!(
            c.library
                .borrow()
                .crates()
                .entries
                .iter()
                .find(|e| e.id == 2)
                .unwrap()
                .tracks,
            std::collections::HashSet::from([3])
        );
        control(&ui, "All library tracks").invoke_accessible_default_action();
        finish_search(&c);
        ui.invoke_select_library(1);
        ui.set_target_crate_index(1);
        ui.invoke_target_crate(1);
        control(&ui, "Add selected track to crate").invoke_accessible_default_action();
        assert_eq!(c.crate_target.get(), 2);
        ui.invoke_select_crate(1);
        finish_search(&c);
        control(&ui, "Rename crate").invoke_accessible_default_action();
        control(&ui, "Crate name").set_accessible_value("Zulu");
        control(&ui, "Save crate name").invoke_accessible_default_action();
        assert_eq!(c.crate_target.get(), 2);
        assert_eq!(ui.get_target_crate_index(), 0);
        control(&ui, "Delete crate").invoke_accessible_default_action();
        assert!(ui.get_crate_delete_pending());
        assert_eq!(ui.get_crate_rows().row_count(), 2);
        control(&ui, "Cancel crate dialog").invoke_accessible_default_action();
        assert_eq!(ui.get_crate_rows().row_count(), 2);
        ui.invoke_delete_crate();
        ui.invoke_select_crate(2);
        finish_search(&c);
        ui.invoke_confirm_delete_crate();
        assert_eq!(ui.get_crate_rows().row_count(), 2);
        ui.invoke_select_crate(1);
        finish_search(&c);
        control(&ui, "Delete crate").invoke_accessible_default_action();
        control(&ui, "Confirm crate deletion").invoke_accessible_default_action();
        finish_search(&c);
        assert_eq!(ui.get_crate_rows().row_count(), 1);
        assert_eq!(ui.get_selected_crate(), 0);
        assert_eq!(ui.get_library_rows().row_count(), 3);
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_title(), title);
        assert!(fixture().is_file() && wav.path().is_file() && third.path().is_file());
        ui.invoke_select_crate(2);
        finish_search(&c);
        ui.invoke_select_library(1);
        control(&ui, "Remove selected track from crate").invoke_accessible_default_action();
        finish_search(&c);
        assert_eq!(ui.get_selected_track(), -1);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 3);
        assert_eq!(c.library.borrow().len(), 3);
        assert!(ui.get_left_playing());
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            let navigation = control(&ui, "Crate navigation").absolute_position();
            let status = control(&ui, "Application status");
            assert!(navigation.x >= 0.);
            assert!(status.absolute_position().y + status.size().height <= height + 1.);
        }
        finish_storage(&c);
        c.close_library().unwrap();
        ui.hide().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _restarted) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        assert_eq!(ui.get_crate_rows().row_count(), 1);
        assert_eq!(ui.get_crate_rows().row_data(0).unwrap().name, "Warmup");
        assert_eq!(ui.get_selected_crate(), 0);
        assert_eq!(ui.get_library_rows().row_count(), 3);
        ui.invoke_select_crate(2);
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 3);
        ui.invoke_select_library(3);
        ui.invoke_load_library_right();
        finish(&c);
        assert!(ui.get_right_title().ends_with(".wav"));
        assert!(!ui.get_right_playing());
        c.close_library().unwrap();
    }

    #[test]
    fn subcrate_gui_persists_hierarchy_and_promotes_children_after_delete() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let audio = wav_file(8192, 48_000);
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        ui.show().unwrap();
        c.import_paths(vec![audio.path().to_owned()]);
        finish_import(&c);

        control(&ui, "New crate").invoke_accessible_default_action();
        control(&ui, "Crate name").set_accessible_value("Parent");
        control(&ui, "Save crate name").invoke_accessible_default_action();
        finish_search(&c);
        ui.invoke_select_crate(1);
        control(&ui, "New subcrate").invoke_accessible_default_action();
        assert!(ui.get_crate_dialog_title().contains("Parent"));
        control(&ui, "Crate name").set_accessible_value("Child");
        control(&ui, "Save crate name").invoke_accessible_default_action();
        finish_search(&c);
        ui.invoke_select_crate(2);
        control(&ui, "New subcrate").invoke_accessible_default_action();
        control(&ui, "Crate name").set_accessible_value("Nested");
        control(&ui, "Save crate name").invoke_accessible_default_action();
        finish_search(&c);

        assert_eq!(ui.get_crate_rows().row_count(), 3);
        assert_eq!(ui.get_crate_rows().row_data(1).unwrap().depth, 1);
        assert_eq!(ui.get_crate_rows().row_data(2).unwrap().depth, 2);
        control(&ui, "All library tracks").invoke_accessible_default_action();
        finish_search(&c);
        ui.invoke_select_library(1);
        control(&ui, "Add selected track to crate").invoke_accessible_default_action();
        assert_eq!(
            c.library.borrow().crates().entries[2].tracks,
            std::collections::HashSet::from([1])
        );

        ui.invoke_select_crate(2);
        finish_search(&c);
        control(&ui, "Delete crate").invoke_accessible_default_action();
        assert!(ui.get_crate_delete_pending());
        assert_eq!(ui.get_crate_rows().row_count(), 3);
        control(&ui, "Confirm crate deletion").invoke_accessible_default_action();
        finish_search(&c);
        let catalog = c.library.borrow();
        assert!(catalog.crates().entries.iter().all(|entry| entry.id != 2));
        assert_eq!(
            catalog
                .crates()
                .entries
                .iter()
                .find(|entry| entry.id == 3)
                .unwrap()
                .parent_id,
            Some(1)
        );
        assert_eq!(
            catalog
                .crates()
                .entries
                .iter()
                .find(|entry| entry.id == 3)
                .unwrap()
                .tracks,
            std::collections::HashSet::from([1])
        );
        drop(catalog);
        finish_storage(&c);
        c.close_library().unwrap();
        ui.hide().unwrap();
        drop(c);

        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        assert_eq!(ui.get_crate_rows().row_count(), 2);
        assert_eq!(ui.get_crate_rows().row_data(1).unwrap().name, "Nested");
        assert_eq!(ui.get_crate_rows().row_data(1).unwrap().depth, 1);
        ui.invoke_select_crate(3);
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 1);
        c.close_library().unwrap();
    }

    #[test]
    fn mixed_meter_gui_preview_save_cancel_and_remove_on_both_decks() {
        i_slint_backend_testing::init_no_event_loop();
        for id in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(1, 48000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
            let file = wav_file(8192, 48000 * 10);
            match id {
                DeckId::A => engine.load_a(file.path()).unwrap(),
                DeckId::B => engine.load_b(file.path()).unwrap(),
            }
            engine
                .set_grid(
                    id,
                    Some(crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap()),
                )
                .unwrap();
            engine.play(id);
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            for save in [false, true] {
                match id {
                    DeckId::A => ui.invoke_grid_left(),
                    DeckId::B => ui.invoke_grid_right(),
                }
                match id {
                    DeckId::A => {
                        ui.set_left_grid_draft_beats(3);
                        ui.set_left_grid_draft_denominator_index(2);
                    }
                    DeckId::B => {
                        ui.set_right_grid_draft_beats(3);
                        ui.set_right_grid_draft_denominator_index(2);
                    }
                }
                for (bar, beats, denominator_index) in [(2, 3, 1), (3, 5, 2)] {
                    match id {
                        DeckId::A => {
                            ui.set_left_grid_meter_bar(bar);
                            ui.set_left_grid_meter_beats(beats);
                            ui.set_left_grid_meter_denominator_index(denominator_index);
                            ui.invoke_grid_meter_set_left();
                        }
                        DeckId::B => {
                            ui.set_right_grid_meter_bar(bar);
                            ui.set_right_grid_meter_beats(beats);
                            ui.set_right_grid_meter_denominator_index(denominator_index);
                            ui.invoke_grid_meter_set_right();
                        }
                    }
                }
                assert!(engine.snapshot(id).grid.unwrap().meter_changes().is_empty());
                let draft = c.grid_editors.borrow()[AudioController::grid_index(id)]
                    .as_ref()
                    .unwrap()
                    .draft
                    .value()
                    .unwrap();
                assert_eq!(draft.bar_start_beat(4).unwrap(), 7.);
                assert!(match id {
                    DeckId::A => ui.get_left_grid_meter_summary(),
                    DeckId::B => ui.get_right_grid_meter_summary(),
                }
                .contains("3: 5/8"));
                assert!(match id {
                    DeckId::A => ui.get_left_grid_meter_summary(),
                    DeckId::B => ui.get_right_grid_meter_summary(),
                }
                .contains("1: 3/8"));
                match (id, save) {
                    (DeckId::A, false) => ui.invoke_cancel_grid_left(),
                    (DeckId::B, false) => ui.invoke_cancel_grid_right(),
                    (DeckId::A, true) => ui.invoke_save_grid_left(),
                    (DeckId::B, true) => ui.invoke_save_grid_right(),
                }
                assert_eq!(
                    engine.snapshot(id).grid.unwrap().meter_changes().len(),
                    if save { 2 } else { 0 }
                );
            }
            let path = engine.snapshot(id).path.unwrap();
            assert_eq!(c.library.borrow().grid(&path), engine.snapshot(id).grid);
            let before = engine.snapshot(id).position;
            match id {
                DeckId::A => {
                    ui.invoke_grid_left();
                    ui.set_left_grid_meter_bar(2);
                    ui.invoke_grid_meter_remove_left();
                    ui.invoke_save_grid_left();
                }
                DeckId::B => {
                    ui.invoke_grid_right();
                    ui.set_right_grid_meter_bar(2);
                    ui.invoke_grid_meter_remove_right();
                    ui.invoke_save_grid_right();
                }
            }
            assert_eq!(
                engine.snapshot(id).grid.unwrap().bar_start_beat(3).unwrap(),
                3.
            );
            assert_eq!(engine.snapshot(id).position, before);
            assert!(engine.snapshot(id).playing);
        }
    }

    #[test]
    fn variable_tempo_gui_preview_save_cancel_and_remove_on_both_decks() {
        i_slint_backend_testing::init_no_event_loop();
        for id in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
            let file = wav_file(8192, 48_000 * 10);
            match id {
                DeckId::A => engine.load_a(file.path()).unwrap(),
                DeckId::B => engine.load_b(file.path()).unwrap(),
            }
            let base = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
            engine.set_grid(id, Some(base)).unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));
            for save in [false, true] {
                match id {
                    DeckId::A => ui.invoke_grid_left(),
                    DeckId::B => ui.invoke_grid_right(),
                }
                for (beat, bpm) in [(8, "90"), (4, "105,5")] {
                    match id {
                        DeckId::A => {
                            ui.set_left_grid_tempo_beat(beat);
                            ui.set_left_grid_tempo_bpm(bpm.into());
                            ui.invoke_grid_tempo_set_left();
                        }
                        DeckId::B => {
                            ui.set_right_grid_tempo_beat(beat);
                            ui.set_right_grid_tempo_bpm(bpm.into());
                            ui.invoke_grid_tempo_set_right();
                        }
                    }
                }
                match id {
                    DeckId::A => {
                        ui.set_left_grid_draft_beats(3);
                        ui.set_left_grid_tempo_beat(6);
                        ui.set_left_grid_tempo_bpm("999".into());
                        ui.invoke_grid_tempo_set_left();
                        ui.set_left_grid_draft_beats(4);
                    }
                    DeckId::B => {
                        ui.set_right_grid_draft_beats(3);
                        ui.set_right_grid_tempo_beat(6);
                        ui.set_right_grid_tempo_bpm("999".into());
                        ui.invoke_grid_tempo_set_right();
                        ui.set_right_grid_draft_beats(4);
                    }
                }
                assert!(engine.snapshot(id).grid.unwrap().is_constant_tempo());
                let draft = c.grid_editors.borrow()[AudioController::grid_index(id)]
                    .as_ref()
                    .unwrap()
                    .draft
                    .value()
                    .unwrap();
                assert_eq!(
                    draft.tempo_changes(),
                    &[
                        crate::beatgrid::TempoChange {
                            beat: 4,
                            bpm: 105.5
                        },
                        crate::beatgrid::TempoChange { beat: 8, bpm: 90. }
                    ]
                );
                assert_eq!(draft.beats_per_bar(), 4);
                assert!(match id {
                    DeckId::A => ui.get_left_grid_tempo_summary(),
                    DeckId::B => ui.get_right_grid_tempo_summary(),
                }
                .contains("Beat 4: 105.5 BPM"));
                match (id, save) {
                    (DeckId::A, false) => ui.invoke_cancel_grid_left(),
                    (DeckId::B, false) => ui.invoke_cancel_grid_right(),
                    (DeckId::A, true) => ui.invoke_save_grid_left(),
                    (DeckId::B, true) => ui.invoke_save_grid_right(),
                }
                assert_eq!(
                    engine.snapshot(id).grid.unwrap().tempo_changes().len(),
                    if save { 2 } else { 0 }
                );
            }
            match id {
                DeckId::A => {
                    ui.invoke_grid_left();
                    ui.set_left_grid_tempo_beat(4);
                    ui.invoke_grid_tempo_remove_left();
                    ui.invoke_save_grid_left();
                }
                DeckId::B => {
                    ui.invoke_grid_right();
                    ui.set_right_grid_tempo_beat(4);
                    ui.invoke_grid_tempo_remove_right();
                    ui.invoke_save_grid_right();
                }
            }
            assert_eq!(
                engine.snapshot(id).grid.unwrap().tempo_changes(),
                &[crate::beatgrid::TempoChange { beat: 8, bpm: 90. }]
            );
        }
    }

    #[test]
    fn warp_marker_gui_preview_save_cancel_and_remove_on_both_decks() {
        i_slint_backend_testing::init_no_event_loop();
        for id in [DeckId::A, DeckId::B] {
            let ui = AppWindow::new().unwrap();
            let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
            let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
            let file = wav_file(8192, 48_000 * 10);
            match id {
                DeckId::A => engine.load_a(file.path()).unwrap(),
                DeckId::B => engine.load_b(file.path()).unwrap(),
            }
            let base = crate::beatgrid::BeatGrid::new(120., Duration::ZERO).unwrap();
            engine.set_grid(id, Some(base)).unwrap();
            let c =
                AudioController::wire(&ui, engine.clone(), Arc::new(|| None), Arc::new(|| None));

            for save in [false, true] {
                match id {
                    DeckId::A => {
                        ui.invoke_grid_left();
                        ui.set_left_grid_warp_beat(6);
                    }
                    DeckId::B => {
                        ui.invoke_grid_right();
                        ui.set_right_grid_warp_beat(6);
                    }
                }
                engine
                    .seek_position(id, Duration::from_millis(3_250))
                    .unwrap();
                match id {
                    DeckId::A => ui.invoke_grid_warp_set_left(),
                    DeckId::B => ui.invoke_grid_warp_set_right(),
                }
                assert!(engine.snapshot(id).grid.unwrap().warp_markers().is_empty());
                let draft = c.grid_editors.borrow()[AudioController::grid_index(id)]
                    .as_ref()
                    .unwrap()
                    .draft
                    .value()
                    .unwrap();
                assert_eq!(
                    draft.warp_markers(),
                    &[crate::beatgrid::WarpMarker {
                        beat: 6,
                        position_ns: 3_250_000_000,
                    }]
                );
                let summary = match id {
                    DeckId::A => ui.get_left_grid_warp_summary(),
                    DeckId::B => ui.get_right_grid_warp_summary(),
                };
                assert!(summary.contains("Beat 6: 3.250s"));
                let lines = match id {
                    DeckId::A => ui.get_left_beats(),
                    DeckId::B => ui.get_right_beats(),
                };
                assert!((0..lines.row_count())
                    .any(|index| { lines.row_data(index).is_some_and(|line| line.warp_marker) }));

                match (id, save) {
                    (DeckId::A, false) => ui.invoke_cancel_grid_left(),
                    (DeckId::B, false) => ui.invoke_cancel_grid_right(),
                    (DeckId::A, true) => ui.invoke_save_grid_left(),
                    (DeckId::B, true) => ui.invoke_save_grid_right(),
                }
                assert_eq!(
                    engine.snapshot(id).grid.unwrap().warp_markers().len(),
                    usize::from(save)
                );
            }

            match id {
                DeckId::A => {
                    ui.invoke_grid_left();
                    ui.set_left_grid_warp_beat(6);
                    ui.invoke_grid_warp_remove_left();
                    ui.invoke_save_grid_left();
                }
                DeckId::B => {
                    ui.invoke_grid_right();
                    ui.set_right_grid_warp_beat(6);
                    ui.invoke_grid_warp_remove_right();
                    ui.invoke_save_grid_right();
                }
            }
            let path = engine.snapshot(id).path.unwrap();
            assert!(engine.snapshot(id).grid.unwrap().warp_markers().is_empty());
            assert_eq!(c.library.borrow().grid(&path), engine.snapshot(id).grid);
        }
    }

    #[test]
    fn grid_editor_drafts_invalid_input_cancel_and_clear_preserve_transport() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let file = wav_file(8192, 48_000 * 8);
        let other = wav_file(4096, 48_000 * 8);
        engine.load_a(file.path()).unwrap();
        engine.load_b(other.path()).unwrap();
        engine.seek_percent(DeckId::A, 25.).unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        ui.set_channel_a(0.6);
        ui.set_channel_b(0.8);
        ui.set_crossfader(0.3);
        ui.invoke_mixer_changed();
        let position = engine.snapshot(DeckId::A).position;
        control(&ui, "DECK A edit beatgrid").invoke_accessible_default_action();
        assert!(ui.get_left_grid_editing());
        assert!(engine.snapshot(DeckId::A).grid.is_none());
        assert!(!c.dirty.get());
        assert!(ui.get_left_beats().row_count() > 0);
        control(&ui, "DECK A zoom in").invoke_accessible_default_action();
        assert_eq!(ui.get_left_detail_window(), 4);
        assert!(engine.snapshot(DeckId::A).grid.is_none());
        assert!(ui
            .get_left_beats()
            .iter()
            .any(|line| line.first && line.bar == 1));
        assert!(ElementHandle::find_by_accessible_label(&ui, "DECK A bar label").count() > 0);
        control(&ui, "DECK A draft BPM").set_accessible_value("128.5");
        control(&ui, "DECK A shift grid later").invoke_accessible_default_action();
        let draft = c.grid_editors.borrow()[0]
            .as_ref()
            .unwrap()
            .draft
            .value()
            .unwrap();
        assert_eq!(draft.anchor(), position + Duration::from_millis(10));
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        assert!(engine.snapshot(DeckId::B).grid.is_none());
        control(&ui, "DECK A cancel beatgrid").invoke_accessible_default_action();
        assert!(!ui.get_left_grid_editing());
        assert_eq!(ui.get_left_beats().row_count(), 0);
        assert!(!c.dirty.get());
        ui.invoke_grid_left();
        control(&ui, "DECK A draft BPM").set_accessible_value("NaN");
        ui.invoke_save_grid_left();
        assert!(ui.get_left_grid_editing());
        assert!(engine.snapshot(DeckId::A).grid.is_none());
        assert!(!c.dirty.get());
        control(&ui, "DECK A draft BPM").set_accessible_value("128,5");
        control(&ui, "DECK A anchor draft at playhead").invoke_accessible_default_action();
        engine.play(DeckId::A);
        control(&ui, "DECK A save beatgrid").invoke_accessible_default_action();
        let committed = engine.snapshot(DeckId::A).grid.unwrap();
        assert_eq!(committed.bpm(), 128.5);
        assert_eq!(committed.anchor(), position);
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(engine.snapshot(DeckId::B).grid.is_none());
        assert_eq!(ui.get_channel_a(), 0.6);
        assert_eq!(ui.get_crossfader(), 0.3);
        control(&ui, "DECK A clear beatgrid").invoke_accessible_default_action();
        assert!(ui.get_left_grid_editing());
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(committed));
        ui.invoke_cancel_grid_left();
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(committed));
        ui.invoke_clear_grid_left();
        control(&ui, "DECK A save beatgrid").invoke_accessible_default_action();
        assert!(engine.snapshot(DeckId::A).grid.is_none());
        assert!(engine.snapshot(DeckId::A).playing);
        assert_eq!(engine.snapshot(DeckId::A).position, position);
        assert_eq!(ui.get_channel_b(), 0.8);
        ui.hide().unwrap();
    }
    #[test]
    fn shared_track_grid_conflict_reload_and_restart_keep_explicit_saved_value() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library.sqlite3");
        let file = wav_file(8192, 48_000 * 8);
        let bad = TempFile::new("mp3", b"invalid");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(
            Ok(db.clone()),
            vec![file.path().to_owned(), bad.path().to_owned()],
        );
        finish_storage(&c);
        finish_import(&c);
        finish_storage(&c);
        ui.invoke_select_library(1);
        ui.invoke_load_library_left();
        finish(&c);
        ui.invoke_load_library_right();
        finish(&c);
        finish_storage(&c);
        engine.seek_percent(DeckId::A, 25.).unwrap();
        c.tick();
        ui.invoke_grid_left();
        control(&ui, "DECK A draft BPM").set_accessible_value("128.5");
        ui.invoke_grid_anchor_left();
        ui.invoke_grid_right();
        control(&ui, "DECK B draft BPM").set_accessible_value("130");
        ui.invoke_save_grid_left();
        finish_storage(&c);
        let saved = engine.snapshot(DeckId::A).grid.unwrap();
        assert_eq!(saved.bpm(), 128.5);
        assert_eq!(saved.anchor(), Duration::from_secs(2));
        assert_eq!(engine.snapshot(DeckId::B).grid, Some(saved));
        ui.invoke_save_grid_right();
        assert!(ui.get_status().contains("changed"));
        assert!(ui.get_right_grid_editing());
        assert_eq!(engine.snapshot(DeckId::B).grid, Some(saved));
        assert!(!c.dirty.get());
        ui.invoke_cancel_grid_right();
        assert!(!ui.get_right_grid_editing());
        ui.invoke_grid_left();
        control(&ui, "DECK A draft BPM").set_accessible_value("140");
        ui.invoke_select_library(2);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(ui.get_left_grid_editing());
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(saved));
        ui.invoke_select_library(1);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(!ui.get_left_grid_editing());
        assert_eq!(engine.snapshot(DeckId::A).grid, Some(saved));
        c.close_library().unwrap();
        drop(c);
        let ui = AppWindow::new().unwrap();
        let (mixer, _new_output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        ui.invoke_select_library(1);
        ui.invoke_load_library_right();
        finish(&c);
        assert_eq!(engine.snapshot(DeckId::B).grid, Some(saved));
        assert!(!ui.get_right_playing());
        ui.invoke_clear_grid_right();
        assert_eq!(engine.snapshot(DeckId::B).grid, Some(saved));
        ui.invoke_save_grid_right();
        finish_storage(&c);
        assert!(engine.snapshot(DeckId::B).grid.is_none());
        ui.invoke_load_library_right();
        finish(&c);
        assert!(engine.snapshot(DeckId::B).grid.is_none());
        c.close_library().unwrap();
    }
    #[test]
    fn delta_save_ack_keeps_newer_edits_and_failed_delta_retries_latest_state() {
        i_slint_backend_testing::init_no_event_loop();
        let profile = tempfile::tempdir().unwrap();
        let db = profile.path().join("library.sqlite3");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        let file = wav_file(8192, 48000);
        c.library.borrow_mut().insert([file.path().to_path_buf()]);
        c.library_changed();
        c.save_library();
        assert!(c.saving.get());
        let first = c.inflight_revision.get().unwrap();
        let cue =
            |name| crate::hot_cues::HotCue::new(Duration::from_millis(50), name, 0x112233).unwrap();
        c.library
            .borrow_mut()
            .set_hot_cue(file.path(), 0, Some(cue("Newer".into())))
            .unwrap();
        c.library_changed();
        let until = Instant::now() + Duration::from_secs(15);
        while c.saving.get() {
            c.poll_storage();
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(c.dirty.get());
        let changed = c.library.borrow().changes();
        assert!(changed.revision > first);
        assert_eq!(changed.tracks.len(), 1);
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_delta BEFORE INSERT ON track_hot_cues BEGIN SELECT RAISE(ABORT,'injected delta failure'); END;").unwrap();
        c.save_library();
        finish_storage(&c);
        assert!(c.save_failed.get() && c.dirty.get());
        assert_eq!(c.library.borrow().changes().tracks.len(), 1);
        c.library
            .borrow_mut()
            .set_hot_cue(file.path(), 0, Some(cue("Latest".into())))
            .unwrap();
        c.library_changed();
        connection
            .execute_batch("DROP TRIGGER reject_delta")
            .unwrap();
        ui.invoke_retry_library_save();
        finish_storage(&c);
        assert!(!c.library.borrow().has_changes());
        assert!(!c.save_failed.get());
        c.close_library().unwrap();
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        assert_eq!(
            c.library.borrow().hot_cues(file.path()).slots()[0]
                .as_ref()
                .unwrap()
                .name(),
            "Latest"
        );
        c.close_library().unwrap();
    }
    #[test]
    fn backfill_write_failure_keeps_loaded_catalog_and_retry_persists_enrichment() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let profile = tempfile::tempdir().unwrap();
        let db = profile.path().join("library.sqlite3");
        let file = crate::test_support::tagged_wav("Retained title", "Artist", "Album");
        let seed = crate::library::Catalog {
            version: 1,
            crates: Default::default(),
            tracks: vec![crate::library::SavedTrack {
                id: 17,
                path: file.path().to_path_buf(),
                metadata: Default::default(),
                grid: None,
                hot_cues: Default::default(),
                saved_loops: Default::default(),
            }],
            history: Vec::new(),
        };
        let worker = crate::library_store::Worker::start(db.clone()).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            crate::library_store::Event::Loaded(Ok(_))
        ));
        worker.save(seed).unwrap();
        assert!(matches!(
            worker.events.recv().unwrap(),
            crate::library_store::Event::Saved(Ok(()))
        ));
        worker.close(None).unwrap();
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_backfill BEFORE INSERT ON track_metadata WHEN NEW.scanned=1 BEGIN SELECT RAISE(ABORT,'injected backfill failure'); END;").unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48000));
        let c = AudioController::wire(&ui, engine, Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(db.clone()), Vec::new());
        finish_storage(&c);
        assert!(c.storage_ready.get());
        assert!(c.persistence.borrow().is_some());
        assert!(c.dirty.get() && c.save_failed.get());
        assert!(ui.get_library_save_failed());
        assert!(ui
            .get_library_storage()
            .contains("catalog remains available"));
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 17);
        assert_eq!(
            ui.get_library_rows().row_data(0).unwrap().name,
            "Retained title"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT scanned FROM track_metadata WHERE track_id=17",
                    [],
                    |r| r.get::<_, i32>(0)
                )
                .unwrap(),
            0
        );
        // Failed optional writes must not prevent loading or subsequent retry.
        ui.invoke_select_library(17);
        ui.invoke_load_library_left();
        finish(&c);
        assert!(ui.get_left_loaded());
        finish_storage(&c);
        connection
            .execute_batch("DROP TRIGGER reject_backfill")
            .unwrap();
        ui.invoke_retry_library_save();
        finish_storage(&c);
        assert!(!c.dirty.get() && !c.save_failed.get());
        assert!(!ui.get_library_save_failed());
        assert_eq!(
            connection
                .query_row(
                    "SELECT scanned FROM track_metadata WHERE track_id=17",
                    [],
                    |r| r.get::<_, i32>(0)
                )
                .unwrap(),
            1
        );
        c.close_library().unwrap();
        c.restore_library(Ok(db), Vec::new());
        finish_storage(&c);
        assert_eq!(
            ui.get_library_rows().row_data(0).unwrap().name,
            "Retained title"
        );
        assert!(!ui.get_library_save_failed());
        c.close_library().unwrap();
    }
    #[test]
    fn durable_library_reports_write_failure_retry_and_damaged_startup_without_data_loss() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        c.restore_library(Ok(path.clone()), Vec::new());
        finish_storage(&c);
        let blocker = rusqlite::Connection::open(&path).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        c.import_paths(vec![fixture()]);
        finish_import(&c);
        finish_storage(&c);
        assert!(ui.get_library_save_failed());
        assert!(ui.get_library_storage().contains("NOT saved"));
        blocker.execute_batch("ROLLBACK").unwrap();
        drop(blocker);
        ui.invoke_retry_library_save();
        finish_storage(&c);
        assert!(!ui.get_library_save_failed());
        assert_eq!(ui.get_library_storage(), "Library saved");
        c.close_library().unwrap();
        std::fs::remove_file(crate::library_store::side_path(&path, ".bak")).unwrap();
        std::fs::write(&path, b"unrecoverable catalog").unwrap();
        let ui = AppWindow::new().unwrap();
        let c = AudioController::wire(&ui, engine, Arc::new(|| None), Arc::new(|| None));
        c.restore_library(Ok(path.clone()), Vec::new());
        finish_storage(&c);
        assert!(ui.get_library_storage().contains("session only"));
        c.import_paths(vec![fixture()]);
        finish_import(&c);
        c.close_library().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"unrecoverable catalog");
    }
    #[test]
    fn extended_workspace_library_search_loads_correct_tracks_and_protects_playing_decks() {
        use slint::{
            platform::{Key, WindowEvent},
            Model,
        };
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let wav = wav_file(8192, 48_000);
        let bad = TempFile::new("mp3", b"invalid music");
        let unsupported = TempFile::new("txt", b"not music");
        let batches = Arc::new(Mutex::new(VecDeque::from([
            Some(vec![
                wav.path().to_owned(),
                fixture(),
                bad.path().to_owned(),
                wav.path().to_owned(),
                unsupported.path().to_owned(),
                PathBuf::from("missing.wav"),
            ]),
            None,
        ])));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(move || batches.lock().unwrap().pop_front().unwrap()),
        );
        ui.show().unwrap();
        assert_eq!(ui.get_library_rows().row_count(), 0);
        control(&ui, "Import library files").invoke_accessible_default_action();
        assert!(ui.get_library_busy());
        finish_import(&c);
        assert_eq!(ui.get_library_rows().row_count(), 3);
        assert!(ui.get_status().contains("3 skipped"));
        assert_eq!(ui.get_selected_track(), -1);
        control(&ui, "Library track 2").invoke_accessible_default_action();
        control(&ui, "Sort library by name").invoke_accessible_default_action();
        finish_search(&c);
        assert_eq!(ui.get_selected_track(), 2);
        control(&ui, "Load selected into deck A").invoke_accessible_default_action();
        finish(&c);
        assert!(ui.get_left_title().ends_with("tone-9s.mp3"));
        assert!(!ui.get_right_loaded());
        control(&ui, "Library search").set_accessible_value(".WAV");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(ui.get_selected_track(), -1);
        assert_eq!(
            control(&ui, "Load selected into deck A").accessible_enabled(),
            Some(false)
        );
        control(&ui, "Library track 1").invoke_accessible_default_action();
        control(&ui, "Load selected into deck B").invoke_accessible_default_action();
        finish(&c);
        assert!(ui.get_right_title().ends_with(".wav"));
        control(&ui, "Library search").set_accessible_value("no matching file");
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 0);
        assert!(ui.get_library_empty().contains("No tracks match"));
        control(&ui, "Clear library search").invoke_accessible_default_action();
        finish_search(&c);
        assert_eq!(ui.get_library_rows().row_count(), 3);
        control(&ui, "Library track 1").invoke_accessible_default_action();
        let ordered_ids: Vec<_> = ui.get_library_rows().iter().map(|row| row.id).collect();
        let selected_index = ordered_ids.iter().position(|id| *id == 1).unwrap();
        let next_id = ordered_ids[(selected_index + 1).min(ordered_ids.len() - 1)];
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::DownArrow.into(),
        });
        assert_eq!(ui.get_selected_track(), next_id);
        control(&ui, "Library track 1").invoke_accessible_default_action();
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        let title = ui.get_left_title();
        assert_eq!(
            control(&ui, "Load selected into deck A").accessible_enabled(),
            Some(false)
        );
        ui.invoke_load_library_left();
        assert!(c.pending.borrow().is_none());
        assert!(ui.get_status().contains("Pause deck A"));
        assert_eq!(ui.get_left_title(), title);
        ui.invoke_toggle_left();
        ui.invoke_load_library_left();
        ui.invoke_toggle_left(); // Starts old track while a replacement is being decoded.
        finish(&c);
        assert_eq!(ui.get_left_title(), title);
        assert!(ui.get_left_playing());
        assert!(ui.get_status().contains("Load discarded"));
        ui.invoke_toggle_left();
        control(&ui, "Library track 3").invoke_accessible_default_action();
        control(&ui, "Load selected into deck A").invoke_accessible_default_action();
        finish(&c);
        assert_eq!(ui.get_left_title(), title);
        assert!(ui.get_status().starts_with("Deck A:"));
        control(&ui, "Import library files").invoke_accessible_default_action();
        finish_import(&c);
        assert_eq!(ui.get_library_rows().row_count(), 3);
        assert_eq!(ui.get_status(), "Library import cancelled");
        for (width, height) in [(850.0, 740.0), (1280.0, 860.0)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            let quantize = control(&ui, "Global Quantize");
            let output_pair = control(&ui, "Audio output pair");
            assert!(output_pair.size().width > 0. && output_pair.size().height > 0.);
            assert!(output_pair.absolute_position().x >= 0.);
            assert!(output_pair.absolute_position().x + output_pair.size().width <= width + 0.1);
            assert!(quantize.size().width > 0. && quantize.size().height > 0.);
            assert!(quantize.absolute_position().x >= 0. && quantize.absolute_position().y >= 0.);
            assert!(quantize.absolute_position().x + quantize.size().width <= width + 0.1);
            assert!(quantize.absolute_position().y + quantize.size().height <= height + 0.1);
            let shared = control(&ui, "Shared waveform area");
            let library = control(&ui, "Track library area");
            let status = control(&ui, "Application status");
            let left = control(&ui, "DECK A detailed waveform");
            let right = control(&ui, "DECK B detailed waveform");
            let overview = control(&ui, "DECK A waveform overview");
            assert!(overview.absolute_position().y < shared.absolute_position().y);
            for deck in ["DECK A", "DECK B"] {
                for slot in 1..=8 {
                    let pad = control(&ui, &format!("{deck} hot cue {slot}"));
                    assert!(pad.size().width > 0. && pad.size().height > 0.);
                    assert!(pad.absolute_position().x >= 0.);
                    assert!(pad.absolute_position().x + pad.size().width <= width + 0.1);
                    assert!(
                        pad.absolute_position().y + pad.size().height
                            <= shared.absolute_position().y
                    );
                }
                for action in ["loop in", "loop out", "loop on/off", "reloop"] {
                    let button = control(&ui, &format!("{deck} {action}"));
                    assert!(button.size().width > 0. && button.size().height > 0.);
                    assert!(button.absolute_position().x >= 0.);
                    assert!(button.absolute_position().x + button.size().width <= width + 0.1);
                    assert!(
                        button.absolute_position().y + button.size().height
                            <= shared.absolute_position().y
                    );
                }
                control(&ui, &format!("{deck} deck tools panel"))
                    .invoke_accessible_default_action();
                for action in [
                    "saved loop slot",
                    "saved loop name",
                    "save loop slot",
                    "recall loop slot",
                    "delete loop slot",
                ] {
                    let button = control(&ui, &format!("{deck} {action}"));
                    assert!(button.absolute_position().x >= 0.);
                    assert!(button.absolute_position().x + button.size().width <= width + 0.1);
                    assert!(
                        button.absolute_position().y + button.size().height
                            <= shared.absolute_position().y
                    );
                }
                control(&ui, &format!("{deck} deck tools panel"))
                    .invoke_accessible_default_action();
                for action in [
                    "auto loop length",
                    "auto loop",
                    "half loop",
                    "double loop",
                    "auto loop beat snap",
                ] {
                    let button = control(&ui, &format!("{deck} {action}"));
                    assert!(button.absolute_position().x >= 0.);
                    assert!(button.absolute_position().x + button.size().width <= width + 0.1);
                    assert!(
                        button.absolute_position().y + button.size().height
                            <= shared.absolute_position().y
                    );
                }
                control(&ui, &format!("{deck} deck tools panel"))
                    .invoke_accessible_default_action();
                for action in [
                    "pitch",
                    "reset pitch",
                    "pitch values",
                    "source and current BPM",
                    "pitch range",
                    "pitch fine down",
                    "pitch fine up",
                    "match tempo",
                ] {
                    let item = control(&ui, &format!("{deck} {action}"));
                    assert!(item.absolute_position().x >= 0.);
                    assert!(item.absolute_position().x + item.size().width <= width + 0.1);
                    assert!(
                        item.absolute_position().y + item.size().height
                            <= shared.absolute_position().y
                    );
                }
                control(&ui, &format!("{deck} deck tools panel"))
                    .invoke_accessible_default_action();
                for action in [
                    "nudge down",
                    "nudge up",
                    "cancel nudge",
                    "phase sync",
                    "continuous sync",
                    "phase alignment mode",
                ] {
                    let item = control(&ui, &format!("{deck} {action}"));
                    assert!(item.size().width > 0. && item.size().height > 0.);
                    assert!(item.absolute_position().x >= 0.);
                    assert!(item.absolute_position().x + item.size().width <= width + 0.1);
                    assert!(
                        item.absolute_position().y + item.size().height
                            <= shared.absolute_position().y
                    );
                }
                control(&ui, &format!("{deck} deck tools panel"))
                    .invoke_accessible_default_action();
                let quantize = control(&ui, &format!("{deck} quantize new cues"));
                let step = control(&ui, &format!("{deck} quantization step"));
                assert!(step.size().width > 0. && step.size().height > 0.);
                assert!(step.absolute_position().x >= 0.);
                assert!(step.absolute_position().x + step.size().width <= width + 0.1);
                assert!(
                    step.absolute_position().y + step.size().height <= shared.absolute_position().y
                );
                assert!(quantize.size().width > 0. && quantize.size().height > 0.);
                assert!(quantize.absolute_position().x >= 0.);
                assert!(quantize.absolute_position().x + quantize.size().width <= width + 0.1);
                assert!(
                    quantize.absolute_position().y + quantize.size().height
                        <= shared.absolute_position().y
                );
                let delete = control(&ui, &format!("{deck} delete hot cue"));
                assert!(
                    delete.absolute_position().y + delete.size().height
                        <= shared.absolute_position().y
                );
            }
            assert!((left.absolute_position().x - right.absolute_position().x).abs() < 0.1);
            assert!((left.size().width - right.size().width).abs() < 0.1);
            assert!(left.size().width > overview.size().width * 1.5);
            assert!(left.absolute_position().y < right.absolute_position().y);
            assert!(
                library.absolute_position().y
                    >= shared.absolute_position().y + shared.size().height - 0.1
            );
            assert!(library.size().height >= 159.0);
            assert!(
                status.absolute_position().y
                    >= library.absolute_position().y + library.size().height - 0.1
            );
            assert!(status.absolute_position().y + status.size().height <= height + 0.1);
        }
        ui.hide().unwrap();
    }
    #[test]
    fn estimated_bpm_is_a_track_suggestion_and_never_automatically_applies_a_grid() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let pulse = |bpm| {
            let samples: Vec<_> = crate::tempo::pulse_train(bpm, 8000, 12)
                .iter()
                .map(|v| (*v * 32767.0) as i16)
                .collect();
            crate::test_support::wav_samples(&samples, 8000, 1)
        };
        let left = pulse(90.0);
        let right = pulse(140.0);
        let bad = TempFile::new("wav", b"invalid");
        let steady = crate::test_support::wav_samples(&vec![8192; 8000 * 12], 8000, 1);
        let paths = Arc::new(Mutex::new(VecDeque::from([
            Some(left.path().to_owned()),
            Some(right.path().to_owned()),
            None,
            Some(bad.path().to_owned()),
            Some(steady.path().to_owned()),
        ])));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(move || paths.lock().unwrap().pop_front().unwrap()),
            Arc::new(|| None),
        );
        assert_eq!(
            control(&ui, "DECK A use BPM suggestion").accessible_enabled(),
            Some(false)
        );
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        finish(&c);
        let suggestion = ui.get_left_suggested_bpm();
        assert!((suggestion - 90).abs() <= 1);
        assert_eq!(ui.get_left_grid_bpm(), 120); // Estimate never overwrites pending user input.
        assert!(!ui.get_left_grid_active() && !ui.get_left_playing());
        control(&ui, "DECK A use BPM suggestion").invoke_accessible_default_action();
        assert_eq!(ui.get_left_grid_bpm(), suggestion);
        assert!(engine.snapshot(DeckId::A).grid.is_none());
        control(&ui, "DECK A edit beatgrid").invoke_accessible_default_action();
        control(&ui, "DECK A save beatgrid").invoke_accessible_default_action();
        assert_eq!(
            engine.snapshot(DeckId::A).grid.unwrap().bpm(),
            f64::from(suggestion)
        );
        engine.play(DeckId::A);
        c.tick();
        control(&ui, "DECK B load track").invoke_accessible_default_action();
        finish(&c);
        let other = ui.get_right_suggested_bpm();
        assert!((other - 140).abs() <= 1);
        assert!(ui.get_right_bpm_caption().starts_with("Estimate BPM: ~"));
        ui.invoke_pitch_right(25.);
        c.tick();
        let caption = ui.get_right_bpm_caption();
        let values: Vec<f64> = caption
            .split('~')
            .skip(1)
            .map(|part| part.split_whitespace().next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(values.len(), 2);
        assert!((values[0] - 140.).abs() <= 1.);
        assert!((values[1] - 175.).abs() <= 1.5);
        assert!(ui.get_left_bpm_caption().starts_with("Manual BPM:"));
        ui.invoke_reset_pitch_right();
        c.tick();
        assert_eq!(engine.snapshot(DeckId::B).requested_speed, 1.);
        control(&ui, "DECK B use BPM suggestion").invoke_accessible_default_action();
        assert_eq!(ui.get_right_grid_bpm(), other);
        assert!(!ui.get_right_grid_active());
        assert!(ui.get_left_playing() && ui.get_left_grid_active());
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        for _ in 0..2 {
            // Cancellation and invalid data preserve old suggestions and grid.
            control(&ui, "DECK A load track").invoke_accessible_default_action();
            finish(&c);
            assert_eq!(ui.get_left_suggested_bpm(), suggestion);
            assert!(ui.get_left_grid_active());
            assert_eq!(ui.get_right_suggested_bpm(), other);
        }
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        finish(&c);
        assert_eq!(ui.get_left_suggested_bpm(), 0);
        assert_eq!(ui.get_left_bpm_caption(), "Source BPM unavailable");
        assert_eq!(
            control(&ui, "DECK A use BPM suggestion").accessible_enabled(),
            Some(false)
        );
        assert!(!ui.get_left_grid_active());
        assert_eq!(ui.get_right_suggested_bpm(), other);
        assert_eq!(ui.get_left_grid_bpm(), suggestion);
    }
    #[test]
    fn manual_bpm_grid_controls_overlay_geometry_and_track_lifetime_are_independent() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        assert_eq!(
            control(&ui, "DECK A edit beatgrid").accessible_enabled(),
            Some(false)
        );
        assert_eq!(ui.get_left_beats().row_count(), 0);
        let wav = wav_file(8192, 48_000 * 8);
        engine.load_a(wav.path()).unwrap();
        let other_wav = wav_file(4096, 48_000 * 8);
        engine.load_b(other_wav.path()).unwrap();
        engine.seek_percent(DeckId::A, 25.0).unwrap();
        c.tick();
        let peaks = ui.get_left_detail();
        control(&ui, "DECK A manual BPM").set_accessible_value("120");
        control(&ui, "DECK A edit beatgrid").invoke_accessible_default_action();
        control(&ui, "DECK A save beatgrid").invoke_accessible_default_action();
        assert!(ui.get_left_grid_active());
        assert!(ui.get_left_grid_caption().contains("120 BPM"));
        assert_eq!(ui.get_left_beats().row_count(), 17);
        let anchor = ui.get_left_beats().row_data(4).unwrap();
        assert_eq!(anchor.position, 25.0);
        assert!(anchor.downbeat);
        assert_eq!(ui.get_left_position(), 25.0);
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_right_beats().row_count(), 0);
        assert_eq!(ui.get_left_detail(), peaks);
        let grid_model = ui.get_left_beats();
        c.tick();
        assert_eq!(ui.get_left_beats(), grid_model);
        for width in [1200.0, 850.0] {
            ui.window().set_size(slint::LogicalSize::new(width, 740.0));
            let view = control(&ui, "DECK A detailed waveform");
            let lines: Vec<_> =
                ElementHandle::find_by_accessible_label(&ui, "DECK A beat line").collect();
            assert_eq!(lines.len(), 17);
            for (index, line) in lines.iter().enumerate() {
                let expected = view.absolute_position().x
                    + index as f32 / 16.0 * (view.size().width - line.size().width);
                assert!((line.absolute_position().x - expected).abs() < 0.1);
            }
        }
        control(&ui, "DECK B manual BPM").set_accessible_value("90");
        control(&ui, "DECK B edit beatgrid").invoke_accessible_default_action();
        control(&ui, "DECK B save beatgrid").invoke_accessible_default_action();
        let right = ui.get_right_beats();
        assert!(ui.get_right_grid_active());
        control(&ui, "DECK A zoom in").invoke_accessible_default_action();
        assert!(ui.get_left_beats().row_count() <= 9);
        assert_eq!(ui.get_right_beats(), right);
        control(&ui, "DECK A cue").invoke_accessible_default_action();
        assert!(ui.get_left_grid_active());
        control(&ui, "DECK A clear beatgrid").invoke_accessible_default_action();
        control(&ui, "DECK A save beatgrid").invoke_accessible_default_action();
        assert!(!ui.get_left_grid_active());
        assert_eq!(ui.get_left_beats().row_count(), 0);
        assert_eq!(ui.get_right_beats(), right);
        engine.load_b(wav.path()).unwrap();
        c.tick();
        assert!(!ui.get_right_grid_active());
        assert_eq!(ui.get_right_beats().row_count(), 0);
        ui.hide().unwrap();
    }
    #[test]
    fn library_folder_button_imports_nested_audio_in_background() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        let folder = tempfile::tempdir().unwrap();
        let nested = folder.path().join("set").join("disc");
        std::fs::create_dir_all(&nested).unwrap();
        let audio = nested.join("track.wav");
        let source = wav_file(8192, 48_000);
        std::fs::copy(source.path(), &audio).unwrap();
        std::fs::write(folder.path().join("readme.txt"), b"not audio").unwrap();
        let selected = folder.path().to_owned();
        c.set_library_folder_picker(Arc::new(move || Some(selected.clone())));

        ui.show().unwrap();
        control(&ui, "Import library folder").invoke_accessible_default_action();
        assert!(ui.get_library_busy());
        finish_import(&c);

        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert!(ui.get_status().contains("Added 1 tracks"));
        assert!(ui.get_status().contains("1 skipped"));
        assert!(audio.is_file());
        for width in [850.0, 1280.0] {
            ui.window().set_size(slint::LogicalSize::new(width, 740.0));
            let button = control(&ui, "Import library folder");
            let library = control(&ui, "Track library area");
            assert!(button.absolute_position().x >= library.absolute_position().x);
            assert!(
                button.absolute_position().x + button.size().width
                    <= library.absolute_position().x + library.size().width
            );
        }
    }
    #[test]
    fn missing_library_track_can_be_relocated_without_importing_a_duplicate() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let replacement = wav_file(8192, 48_000);
        let replacement_path = std::fs::canonicalize(replacement.path()).unwrap();
        let picker_path = replacement_path.clone();
        let selected_path = picker_path.clone();
        let missing_dir = tempfile::tempdir().unwrap();
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(move || Some(vec![selected_path.clone()])),
        );
        let old_path = missing_dir.path().join("missing-track.wav");
        *c.library.borrow_mut() = crate::library::Library::restore(crate::library::Catalog {
            version: 1,
            tracks: vec![crate::library::SavedTrack {
                id: 1,
                path: old_path,
                metadata: Default::default(),
                grid: None,
                hot_cues: Default::default(),
                saved_loops: Default::default(),
            }],
            crates: Default::default(),
            history: vec![],
        })
        .unwrap();
        c.library.borrow_mut().select(1);
        c.publish_library();
        finish_search(&c);
        ui.show().unwrap();

        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert!(ui.get_library_can_relocate_selected());
        for width in [850.0, 1280.0] {
            ui.window().set_size(slint::LogicalSize::new(width, 740.0));
            let button = control(&ui, "Relocate selected missing library track");
            let library_area = control(&ui, "Track library area");
            assert!(button.absolute_position().x >= library_area.absolute_position().x);
            assert!(
                button.absolute_position().x + button.size().width
                    <= library_area.absolute_position().x + library_area.size().width
            );
        }
        control(&ui, "Relocate selected missing library track").invoke_accessible_default_action();
        assert!(ui.get_library_busy());
        finish_import(&c);

        assert_eq!(ui.get_library_rows().row_count(), 1);
        assert_eq!(c.library.borrow().track_by_id(1).unwrap().path, picker_path);
        assert!(!ui.get_library_can_relocate_selected());
        assert!(ui.get_status().contains("Track relocated"));
    }
    #[test]
    fn page_edge_selection_updates_relocation_action_for_missing_track() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let c = AudioController::wire(
            &ui,
            Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000)),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        let dir = tempfile::tempdir().unwrap();
        let tracks = (1..=201)
            .map(|id| crate::library::SavedTrack {
                id,
                path: dir.path().join(format!("missing-{id}.wav")),
                metadata: Default::default(),
                grid: None,
                hot_cues: Default::default(),
                saved_loops: Default::default(),
            })
            .collect();
        *c.library.borrow_mut() = crate::library::Library::restore(crate::library::Catalog {
            version: 1,
            tracks,
            crates: Default::default(),
            history: vec![],
        })
        .unwrap();
        c.publish_library();
        finish_search(&c);
        ui.show().unwrap();
        assert!(ui.get_library_can_next_page());

        c.library.borrow_mut().select(200);
        c.publish_selected_library_track(&ui);
        ui.invoke_move_library_selection(1);
        finish_search(&c);

        assert_eq!(c.library.borrow().selected_id(), 201);
        assert_eq!(ui.get_selected_track(), 201);
        assert!(ui.get_library_can_relocate_selected());
    }
    #[test]
    fn keyboard_zoom_is_focus_scoped_bounded_and_preserves_audio_state() {
        use slint::platform::{Key, PointerEventButton, WindowEvent};
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        let click = |label: &str| {
            let target = control(&ui, label);
            let origin = target.absolute_position();
            let size = target.size();
            let position = slint::LogicalPosition::new(
                origin.x + size.width / 2.,
                origin.y + size.height / 2.,
            );
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
        };
        let key = |text: &str| {
            ui.window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
            ui.window()
                .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
        };
        click("DECK A detailed waveform");
        key("+");
        assert_eq!(ui.get_left_detail_window(), 8); // Unloaded detail cannot zoom.
        let file = wav_file(8192, 48_000 * 30);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        engine.play(DeckId::A);
        engine.set_speed(DeckId::A, 1.25).unwrap();
        engine.set_speed(DeckId::B, 0.75).unwrap();
        c.tick();
        let gains = engine.output_volumes();
        for (width, height) in [(850., 740.), (1280., 860.)] {
            ui.window().set_size(slint::LogicalSize::new(width, height));
            ui.set_left_detail_window(8);
            ui.set_right_detail_window(8);
            c.tick();
            click("DECK A detailed waveform");
            let before_a = engine.snapshot(DeckId::A);
            let before_b = engine.snapshot(DeckId::B);
            let span = ui.get_left_detail_end() - ui.get_left_detail_start();
            key("+");
            assert_eq!(ui.get_left_detail_window(), 4);
            assert_eq!(ui.get_right_detail_window(), 8);
            assert!(
                ((ui.get_left_detail_end() - ui.get_left_detail_start()) * 2. - span).abs() < 0.001
            );
            ui.window()
                .dispatch_event(WindowEvent::KeyPressRepeated { text: "+".into() });
            assert_eq!(ui.get_left_detail_window(), 2);
            key("+");
            assert_eq!(ui.get_left_detail_window(), 2);
            for _ in 0..5 {
                key("-");
            }
            assert_eq!(ui.get_left_detail_window(), 32);
            assert_eq!(ui.get_right_detail_window(), 8);
            for modifier in [Key::Control, Key::Alt, Key::Meta] {
                ui.window().dispatch_event(WindowEvent::KeyPressed {
                    text: modifier.into(),
                });
                key("+");
                ui.window().dispatch_event(WindowEvent::KeyReleased {
                    text: modifier.into(),
                });
                assert_eq!(ui.get_left_detail_window(), 32);
            }
            ui.window().dispatch_event(WindowEvent::KeyPressed {
                text: Key::Shift.into(),
            });
            key("+");
            ui.window().dispatch_event(WindowEvent::KeyReleased {
                text: Key::Shift.into(),
            });
            assert_eq!(ui.get_left_detail_window(), 16);
            key("x");
            assert_eq!(ui.get_left_detail_window(), 16);
            assert_eq!(engine.snapshot(DeckId::A).position, before_a.position);
            assert_eq!(engine.snapshot(DeckId::B).position, before_b.position);
            assert!(engine.snapshot(DeckId::A).playing);
            assert!(!engine.snapshot(DeckId::B).playing);
            assert_eq!(
                engine.snapshot(DeckId::A).requested_speed,
                before_a.requested_speed
            );
            assert_eq!(engine.output_volumes(), gains);
            click("DECK B detailed waveform");
            let b_position = engine.snapshot(DeckId::B).position;
            key("-");
            assert_eq!(ui.get_left_detail_window(), 16);
            assert_eq!(ui.get_right_detail_window(), 16);
            key("+");
            assert_eq!(ui.get_right_detail_window(), 8);
            assert_eq!(engine.snapshot(DeckId::B).position, b_position);
            ui.set_audio_enabled(false);
            key("+");
            assert_eq!(ui.get_right_detail_window(), 8);
            ui.set_audio_enabled(true);
        }
    }

    #[test]
    fn keyboard_zoom_does_not_consume_search_or_grid_text_input() {
        use slint::platform::{Key, PointerEventButton, WindowEvent};
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        let file = wav_file(8192, 48_000 * 3);
        engine.load_a(file.path()).unwrap();
        engine.load_b(file.path()).unwrap();
        c.tick();
        ui.show().unwrap();
        ui.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        let click = |label: &str| {
            let target = control(&ui, label);
            let origin = target.absolute_position();
            let size = target.size();
            let position = slint::LogicalPosition::new(
                origin.x + size.width / 2.,
                origin.y + size.height / 2.,
            );
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
        };
        let key = |text: &str| {
            ui.window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
            ui.window()
                .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
        };
        click("DECK A detailed waveform");
        key("+");
        assert_eq!(ui.get_left_detail_window(), 4);
        click("Library search");
        key("-");
        key("+");
        assert_eq!(ui.get_library_query(), "-+");
        assert_eq!(ui.get_left_detail_window(), 4);
        assert_eq!(ui.get_right_detail_window(), 8);
        control(&ui, "DECK A edit beatgrid").invoke_accessible_default_action();
        click("DECK A draft BPM");
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Control.into(),
        });
        key("a");
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Control.into(),
        });
        key("-");
        key("+");
        assert_eq!(ui.get_left_grid_draft_bpm(), "-+");
        assert_eq!(ui.get_left_detail_window(), 4);
        assert_eq!(ui.get_right_detail_window(), 8);
        click("DECK A detailed waveform");
        key("-");
        assert_eq!(ui.get_left_detail_window(), 8); // Zoom also works in grid-edit mode.
        assert_eq!(ui.get_left_grid_draft_bpm(), "-+");
    }

    #[test]
    fn detail_zoom_and_clicks_follow_local_time_without_changing_other_deck() {
        use slint::platform::{PointerEventButton, WindowEvent};
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap();
        assert_eq!(
            control(&ui, "DECK A zoom in").accessible_enabled(),
            Some(false)
        );
        let wav = wav_file(8192, 48_000 * 30);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(&fixture()).unwrap();
        c.tick();
        assert_eq!(
            ui.get_left_detail().row_count(),
            crate::waveform::OVERVIEW_BINS
        );
        let overview = ui.get_left_waveform();
        let right_model = ui.get_right_detail();
        for _ in 0..2 {
            control(&ui, "DECK A zoom in").invoke_accessible_default_action();
        }
        assert_eq!(ui.get_left_detail_window(), 2);
        assert_eq!(
            control(&ui, "DECK A zoom in").accessible_enabled(),
            Some(false)
        );
        for _ in 0..4 {
            control(&ui, "DECK A zoom out").invoke_accessible_default_action();
        }
        assert_eq!(ui.get_left_detail_window(), 32);
        assert_eq!(
            control(&ui, "DECK A zoom out").accessible_enabled(),
            Some(false)
        );
        for _ in 0..2 {
            control(&ui, "DECK A zoom in").invoke_accessible_default_action();
        }
        assert_eq!(ui.get_left_detail_window(), 8);
        engine.play(DeckId::A);
        for width in [1200.0, 850.0] {
            ui.window().set_size(slint::LogicalSize::new(width, 740.0));
            engine.seek_percent(DeckId::A, 50.0).unwrap();
            c.tick();
            assert!((ui.get_left_detail_start() - 100.0 * 11.0 / 30.0).abs() < 0.001);
            assert!((ui.get_left_detail_end() - 100.0 * 19.0 / 30.0).abs() < 0.001);
            let view = control(&ui, "DECK A detailed waveform");
            let origin = view.absolute_position();
            let size = view.size();
            let position = slint::LogicalPosition::new(
                origin.x + size.width * 0.25,
                origin.y + size.height / 2.0,
            );
            ui.window()
                .dispatch_event(WindowEvent::PointerMoved { position });
            ui.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            ui.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            // Quarter of the 11..19s view is 13s, not quarter of the whole track.
            assert!((engine.snapshot(DeckId::A).position.as_secs_f64() - 13.0).abs() < 0.001);
            assert!(ui.get_left_playing());
            assert!((ui.get_left_detail_marker() - 50.0).abs() < 0.01);
            let marker = control(&ui, "DECK A detail playhead");
            assert!(
                (marker.absolute_position().x
                    - origin.x
                    - (size.width - marker.size().width) / 2.0)
                    .abs()
                    < 0.1
            );
        }
        output.by_ref().take(4800).for_each(drop);
        c.tick();
        assert!(ui.get_left_detail_start() > 30.0);
        assert_eq!(ui.get_right_position(), 0.0);
        assert_eq!(ui.get_right_detail_window(), 8);
        assert_eq!(ui.get_right_detail(), right_model);
        engine.pause(DeckId::A);
        c.tick();
        let paused = ui.get_left_detail();
        let paused_colors = ui.get_left_detail_colors();
        c.tick();
        assert_eq!(ui.get_left_detail(), paused);
        assert_eq!(ui.get_left_detail_colors(), paused_colors);
        engine.seek_percent(DeckId::A, 100.0).unwrap();
        c.tick();
        assert!((ui.get_left_detail_marker() - 100.0).abs() < 0.001);
        assert!(!ui.get_left_playing());
        engine.cue(DeckId::A);
        c.tick();
        assert_eq!(ui.get_left_detail_marker(), 0.0);
        assert_eq!(ui.get_left_detail_start(), 0.0);
        assert_eq!(ui.get_left_waveform(), overview);
        ui.hide().unwrap();
    }
    #[test]
    fn headless_overview_clicks_map_to_track_time_and_playhead_at_different_widths() {
        use slint::platform::{PointerEventButton, WindowEvent};
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.show().unwrap(); // Testing backend only: no native window or visual acceptance.
        assert_eq!(
            control(&ui, "DECK A waveform overview").accessible_enabled(),
            Some(false)
        );
        assert_eq!(ui.get_left_waveform().row_count(), 0);
        let wav = wav_file(8192, 48_000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(&fixture()).unwrap();
        c.tick();
        let model = ui.get_left_waveform();
        assert_eq!(model.row_count(), crate::waveform::OVERVIEW_BINS);
        assert!((0..model.row_count()).all(|i| model.row_data(i) == Some(0.25)));
        let right_model = ui.get_right_waveform();
        for width in [1200.0, 850.0] {
            ui.window().set_size(slint::LogicalSize::new(width, 740.0));
            for (deck, fraction) in [("A", 0.25), ("B", 0.75), ("A", 0.5)] {
                let overview = control(&ui, &format!("DECK {deck} waveform overview"));
                let origin = overview.absolute_position();
                let size = overview.size();
                assert!(size.width > 0.0 && size.height > 0.0);
                let position = slint::LogicalPosition::new(
                    origin.x + size.width * fraction,
                    origin.y + size.height / 2.0,
                );
                ui.window()
                    .dispatch_event(WindowEvent::PointerMoved { position });
                ui.window().dispatch_event(WindowEvent::PointerPressed {
                    position,
                    button: PointerEventButton::Left,
                });
                ui.window().dispatch_event(WindowEvent::PointerReleased {
                    position,
                    button: PointerEventButton::Left,
                });
                let id = if deck == "A" { DeckId::A } else { DeckId::B };
                assert!((engine.snapshot(id).percent() - fraction * 100.0).abs() < 0.01);
                let marker = control(&ui, &format!("DECK {deck} waveform playhead"));
                let expected_x = origin.x + fraction * (size.width - marker.size().width);
                assert!((marker.absolute_position().x - expected_x).abs() < 0.1);
                assert!(!engine.snapshot(id).playing);
            }
        }
        engine.play(DeckId::A);
        c.tick();
        control(&ui, "DECK A waveform overview").set_accessible_value("60");
        assert!(ui.get_left_playing());
        assert!((ui.get_left_position() - 60.0).abs() < 0.01);
        let right = ui.get_right_position();
        for (value, fraction) in [("0", 0.0), ("100", 1.0)] {
            control(&ui, "DECK A waveform overview").set_accessible_value(value);
            let overview = control(&ui, "DECK A waveform overview");
            let marker = control(&ui, "DECK A waveform playhead");
            let expected = overview.absolute_position().x
                + fraction * (overview.size().width - marker.size().width);
            assert!((marker.absolute_position().x - expected).abs() < 0.1);
        }
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_right_position(), right);
        engine.cue(DeckId::A);
        c.tick();
        assert_eq!(ui.get_left_waveform(), model); // No model rebuild on timer, seek or Cue.
        assert_eq!(ui.get_right_waveform(), right_model);
        ui.hide().unwrap();
    }
    #[test]
    fn headless_position_sliders_seek_both_decks_without_changing_transport_or_other_deck() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let c = AudioController::wire(
            &ui,
            Rc::clone(&engine),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        assert_eq!(
            control(&ui, "DECK A position").accessible_enabled(),
            Some(false)
        );
        let wav = wav_file(8192, 48_000);
        engine.load_a(wav.path()).unwrap();
        engine.load_b(&fixture()).unwrap();
        c.tick();
        control(&ui, "DECK A position").set_accessible_value("25");
        c.tick();
        assert_eq!(ui.get_left_position(), 25.0);
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_right_position(), 0.0);
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        control(&ui, "DECK A position").set_accessible_value("75");
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 75.0);
        output.by_ref().take(2400).for_each(drop);
        c.tick();
        assert!(ui.get_left_position() > 75.0);
        let left = ui.get_left_position();
        control(&ui, "DECK B position").set_accessible_value("80");
        assert!((ui.get_right_position() - 80.0).abs() < 0.001);
        assert!(!ui.get_right_playing());
        assert_eq!(ui.get_left_position(), left);
        control(&ui, "DECK A position").set_accessible_value("100");
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 100.0);
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 0.0);
        control(&ui, "DECK A cue").invoke_accessible_default_action();
        assert!(!ui.get_left_playing());
        assert_eq!(ui.get_left_position(), 0.0);
        assert!((ui.get_right_position() - 80.0).abs() < 0.001);
    }
    #[test]
    fn headless_gui_load_transport_faders_errors_cancel_and_position() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer, 1, 48_000));
        let wav = wav_file(8192, 48_000);
        let bad = TempFile::new("mp3", b"bad music");
        let paths = Arc::new(Mutex::new(VecDeque::from([
            Some(wav.path().to_owned()),
            Some(fixture()),
            Some(bad.path().to_owned()),
            None,
            Some(wav.path().to_owned()),
        ])));
        let picker: TrackPicker = Arc::new(move || paths.lock().unwrap().pop_front().unwrap());
        let c = AudioController::wire(&ui, Rc::clone(&engine), picker, Arc::new(|| None));
        assert!(!ui.get_left_loaded());
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        assert!(ui.get_loading());
        c.request_load(DeckId::B); // A second request cannot overtake the first.
        finish(&c);
        assert!(ui.get_left_loaded());
        let left_waveform = ui.get_left_waveform();
        let left_detail = ui.get_left_detail();
        let left_colors = ui.get_left_waveform_colors();
        assert_eq!(left_colors.row_count(), left_waveform.row_count());
        let bass = left_colors.row_data(left_colors.row_count() - 1).unwrap();
        assert!(bass.red() > bass.green() && bass.red() > bass.blue());
        assert_eq!(
            ui.get_left_detail_colors().row_count(),
            left_detail.row_count()
        );
        assert!(!ui.get_right_loaded());
        assert!(!ui.get_left_playing());
        control(&ui, "DECK B load track").invoke_accessible_default_action();
        finish(&c);
        assert!(ui.get_right_loaded());
        let right_waveform = ui.get_right_waveform();
        let right_colors = ui.get_right_waveform_colors();
        assert_eq!(right_colors.row_count(), right_waveform.row_count());
        assert!((0..right_colors.row_count()).any(|i| {
            let color = right_colors.row_data(i).unwrap();
            color.green() > color.red() && color.green() > color.blue()
        }));
        let right_detail = ui.get_right_detail();
        assert!(ui.get_right_title().ends_with("tone-9s.mp3"));
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        assert!(engine.snapshot(DeckId::A).playing);
        assert!(!engine.snapshot(DeckId::B).playing);
        // Use actual slider accessibility actions, not just the public callback.
        control(&ui, "Channel A fader").set_accessible_value("0.4");
        control(&ui, "Channel B fader").set_accessible_value("0.8");
        control(&ui, "Crossfader").set_accessible_value("0.25");
        output.by_ref().take(2400).for_each(drop);
        c.tick();
        assert!(ui.get_left_position() > 0.0);
        assert_eq!(ui.get_right_position(), 0.0);
        let samples: Vec<_> = output.by_ref().take(2400).collect();
        assert!(samples.iter().all(|s| (s - 0.075).abs() < 1e-5));
        control(&ui, "DECK A play/pause").invoke_accessible_default_action();
        assert!(!engine.snapshot(DeckId::A).playing);
        control(&ui, "DECK B play/pause").invoke_accessible_default_action();
        control(&ui, "DECK A cue").invoke_accessible_default_action();
        assert_eq!(ui.get_left_position(), 0.0);
        assert!(ui.get_right_playing());
        let title = ui.get_left_title();
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        finish(&c);
        assert_eq!(ui.get_left_title(), title);
        assert_eq!(ui.get_left_waveform(), left_waveform);
        assert!(ui.get_status().starts_with("Deck A:"));
        control(&ui, "DECK A load track").invoke_accessible_default_action();
        finish(&c);
        assert_eq!(ui.get_left_title(), title);
        assert_eq!(ui.get_status(), "Selection cancelled");
        assert_eq!(ui.get_left_waveform_colors(), left_colors);
        assert_eq!(ui.get_right_waveform_colors(), right_colors);
        assert_eq!(ui.get_left_waveform(), left_waveform);
        assert_eq!(ui.get_right_waveform(), right_waveform);
        assert_eq!(ui.get_left_detail(), left_detail);
        assert_eq!(ui.get_right_detail(), right_detail);
        control(&ui, "DECK B play/pause").invoke_accessible_default_action();
        control(&ui, "DECK B load track").invoke_accessible_default_action();
        finish(&c);
        assert!(!ui.get_right_playing());
        assert_eq!(ui.get_right_position(), 0.0);
        assert_eq!(ui.get_left_title(), title);
        assert_ne!(ui.get_right_waveform(), right_waveform);
        assert_ne!(ui.get_right_waveform_colors(), right_colors);
        assert_eq!(ui.get_left_waveform_colors(), left_colors);
        assert_eq!(ui.get_left_waveform(), left_waveform);
        assert_ne!(ui.get_right_detail(), right_detail);
        assert_eq!(ui.get_left_detail(), left_detail);
    }
}
