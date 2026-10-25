//! GUI control plane with injectable file selection and one bounded load worker.
use crate::{
    playback::{DeckId, DualDeckEngine, MixerLevels, PreparedTrack},
    AppWindow,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
};

type LoadResult =
    Option<Result<(PathBuf, PreparedTrack, crate::track_metadata::TrackMetadata), String>>;
type ImportResult = Option<crate::library::ImportBatch>;
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
    Clear,
}
struct DetailCache {
    source: Arc<crate::waveform::DetailPeaks>,
    range: crate::waveform::DetailRange,
    grid: Option<crate::beatgrid::BeatGrid>,
}
pub type TrackPicker = Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>;
pub type LibraryPicker = Arc<dyn Fn() -> Option<Vec<PathBuf>> + Send + Sync>;
pub struct AudioController {
    ui: slint::Weak<AppWindow>,
    engine: Rc<DualDeckEngine>,
    picker: TrackPicker,
    library_picker: LibraryPicker,
    library: RefCell<crate::library::Library>,
    persistence: RefCell<Option<crate::library_store::Worker>>,
    storage_ready: Cell<bool>,
    dirty: Cell<bool>,
    saving: Cell<bool>,
    save_failed: Cell<bool>,
    initial_paths: RefCell<Vec<PathBuf>>,
    crate_editor: Cell<Option<i32>>,
    crate_delete: Cell<Option<i32>>,
    crate_target: Cell<i32>,
    import_crate: Cell<i32>,
    grid_editors: RefCell<[Option<GridSession>; 2]>,
    pending_import: RefCell<Option<Receiver<ImportResult>>>,
    pending: RefCell<Option<(DeckId, Receiver<LoadResult>)>>,
    displayed_overviews: RefCell<[Option<Arc<[f32]>>; 2]>,
    displayed_details: RefCell<[Option<DetailCache>; 2]>,
}
impl AudioController {
    pub fn wire(
        ui: &AppWindow,
        engine: Rc<DualDeckEngine>,
        picker: TrackPicker,
        library_picker: LibraryPicker,
    ) -> Rc<Self> {
        let controller = Rc::new(Self {
            ui: ui.as_weak(),
            engine,
            picker,
            library_picker,
            library: RefCell::new(crate::library::Library::default()),
            persistence: RefCell::new(None),
            storage_ready: Cell::new(true),
            dirty: Cell::new(false),
            saving: Cell::new(false),
            save_failed: Cell::new(false),
            initial_paths: RefCell::new(Vec::new()),
            crate_editor: Cell::new(None),
            crate_delete: Cell::new(None),
            crate_target: Cell::new(0),
            import_crate: Cell::new(0),
            grid_editors: RefCell::new([None, None]),
            pending_import: RefCell::new(None),
            pending: RefCell::new(None),
            displayed_overviews: RefCell::new([None, None]),
            displayed_details: RefCell::new([None, None]),
        });
        ui.set_audio_enabled(true);
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(&controller);
            let toggle = move || {
                if let Some(c) = weak.upgrade() {
                    if c.engine.snapshot(id).playing {
                        c.engine.pause(id);
                    } else {
                        c.engine.play(id);
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
                    c.engine.cue(id);
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
                    if let Err(err) = c.engine.seek_percent(id, percent) {
                        if let Some(ui) = c.ui.upgrade() {
                            ui.set_status(format!("Deck {id:?}: {err}").into());
                        }
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
        ui.on_import_library(move || {
            if let Some(c) = weak.upgrade() {
                c.start_import(Arc::clone(&c.library_picker));
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_library_search(move || {
            if let Some(c) = weak.upgrade() {
                if let Some(ui) = c.ui.upgrade() {
                    c.library
                        .borrow_mut()
                        .set_query(ui.get_library_query().to_string());
                    c.publish_library();
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_select_library(move |id| {
            if let Some(c) = weak.upgrade() {
                c.library.borrow_mut().select(id);
                if let Some(ui) = c.ui.upgrade() {
                    ui.set_selected_track(c.library.borrow().selected_id());
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_move_library_selection(move |delta| {
            if let Some(c) = weak.upgrade() {
                c.library.borrow_mut().move_selection(delta);
                if let Some(ui) = c.ui.upgrade() {
                    ui.set_selected_track(c.library.borrow().selected_id());
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        ui.on_sort_library(move |field| {
            if let Some(c) = weak.upgrade() {
                c.library.borrow_mut().sort(field);
                c.publish_library();
            }
        });
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(&controller);
            let load = move || {
                if let Some(c) = weak.upgrade() {
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
        controller.refresh();
        controller
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
                    .get(index.max(0) as usize)
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
                    let name = if rename {
                        let Some(entry) = library.crates().entries.iter().find(|e| e.id == id)
                        else {
                            return;
                        };
                        entry.name.clone()
                    } else {
                        String::new()
                    };
                    c.crate_editor.set(Some(id));
                    if let Some(ui) = c.ui.upgrade() {
                        ui.set_crate_edit_name(name.into());
                        ui.set_crate_dialog_title(
                            if rename { "Rename crate" } else { "New crate" }.into(),
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
        ui.on_save_crate_name(move || {
            if let Some(c) = weak.upgrade() {
                if !c.can_change_crates() {
                    return;
                }
                let Some(id) = c.crate_editor.get() else {
                    return;
                };
                let Some(ui) = c.ui.upgrade() else {
                    return;
                };
                let name = ui.get_crate_edit_name().to_string();
                let result = if id == 0 {
                    let mut library = c.library.borrow_mut();
                    library.create_crate(&name).and_then(|id| {
                        c.crate_target.set(id);
                        library.select_crate(id)
                    })
                } else {
                    c.library.borrow_mut().rename_crate(id, &name)
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
    fn begin_grid_editor(&self, id: DeckId) -> Result<(), String> {
        if !self.can_change_crates() {
            return Err("Finish library loading before editing its grid".into());
        }
        if self.grid_editors.borrow()[Self::grid_index(id)].is_some() {
            return Err("Save or cancel the existing grid draft first".into());
        }
        let snapshot = self.engine.snapshot(id);
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
        self.set_grid_editing(id, true);
        Ok(())
    }
    fn change_grid_draft(&self, id: DeckId, action: GridAction) -> Result<(), String> {
        let ui = self.ui.upgrade().ok_or("Window closed")?;
        let mut editors = self.grid_editors.borrow_mut();
        let editor = editors[Self::grid_index(id)]
            .as_mut()
            .ok_or("Open Edit Grid first")?;
        if !matches!(action, GridAction::Clear) {
            editor.draft.set_bpm(&Self::grid_bpm_text(&ui, id))?;
        }
        match action {
            GridAction::Tempo => Ok(()),
            GridAction::Anchor => editor.draft.anchor_here(self.engine.snapshot(id).position),
            GridAction::Nudge(ms) => editor.draft.nudge(ms),
            GridAction::Clear => {
                editor.draft.clear();
                Ok(())
            }
        }
    }
    fn apply_saved_grid(&self, id: DeckId) -> Result<(), String> {
        if let Some(path) = self.engine.snapshot(id).path {
            self.engine
                .set_grid(id, self.library.borrow().grid(&path))?;
        }
        Ok(())
    }
    fn save_grid_editor(&self, id: DeckId) -> Result<(), String> {
        if !self.can_change_crates() {
            return Err("Finish library loading before saving its grid".into());
        }
        let snapshot = self.engine.snapshot(id);
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
            editor.draft.set_bpm(&Self::grid_bpm_text(
                &self.ui.upgrade().ok_or("Window closed")?,
                id,
            ))?;
            (Arc::clone(&editor.path), editor.draft.value())
        };
        self.library.borrow_mut().insert([path.to_path_buf()]);
        self.library.borrow_mut().set_grid(&path, value)?;
        self.engine.set_grid(id, value)?;
        self.library_changed();
        self.cancel_grid_editor(id);
        self.publish_library();
        let other = if id == DeckId::A {
            DeckId::B
        } else {
            DeckId::A
        };
        if self
            .engine
            .snapshot(other)
            .path
            .as_ref()
            .is_some_and(|p| crate::library::identity(p) == crate::library::identity(&path))
        {
            self.engine
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
        let result = path.and_then(crate::library_store::Worker::start);
        match result {
            Ok(worker) => {
                *self.persistence.borrow_mut() = Some(worker);
                self.storage_ready.set(false);
                *self.initial_paths.borrow_mut() = initial_paths;
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_busy(true);
                    ui.set_library_storage("Restoring library…".into());
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
    fn poll_storage(&self) {
        let event = self
            .persistence
            .borrow()
            .as_ref()
            .map(|w| w.events.try_recv());
        match event {
            Some(Ok(crate::library_store::Event::Loaded(result))) => {
                self.storage_ready.set(true);
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_busy(false);
                }
                match result {
                    Ok((library, note)) => {
                        *self.library.borrow_mut() = library;
                        self.publish_library();
                        for id in [DeckId::A, DeckId::B] {
                            if let Err(error) = self.apply_saved_grid(id) {
                                self.grid_feedback(Err(error), "");
                            }
                        }
                        if let Some(ui) = self.ui.upgrade() {
                            ui.set_library_storage(note.into());
                        }
                    }
                    Err(error) => {
                        self.persistence.borrow_mut().take();
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
                match result {
                    Ok(()) => {
                        if let Some(ui) = self.ui.upgrade() {
                            ui.set_library_storage("Library saved".into());
                            ui.set_library_save_failed(false);
                        }
                    }
                    Err(error) => {
                        self.dirty.set(true);
                        self.save_failed.set(true);
                        self.storage_error(&error, true);
                    }
                }
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.persistence.borrow_mut().take();
                self.storage_ready.set(true);
                self.saving.set(false);
                if let Some(ui) = self.ui.upgrade() {
                    ui.set_library_busy(false);
                }
                self.storage_error("Storage worker stopped; session only", false);
            }
            _ => {}
        }
    }
    fn save_library(&self) {
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
        match worker.save(self.library.borrow().catalog()) {
            Ok(()) => {
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
        if let Some(worker) = self.persistence.borrow_mut().take() {
            let final_catalog = (self.storage_ready.get()
                && (self.dirty.get() || self.saving.get()))
            .then(|| self.library.borrow().catalog());
            worker.close(final_catalog)?;
        }
        Ok(())
    }
    fn apply_levels(&self) {
        if let Some(ui) = self.ui.upgrade() {
            self.engine.set_levels(MixerLevels {
                crossfader: ui.get_crossfader(),
                channel_a: ui.get_channel_a(),
                channel_b: ui.get_channel_b(),
            });
        }
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
        if self.engine.snapshot(id).playing {
            ui.set_status(format!("Pause deck {id:?} before replacing its track").into());
            return;
        }
        ui.set_loading(true);
        ui.set_status("Select MP3/WAV; decoding runs in the background".into());
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("track-loader".into())
            .spawn(move || {
                let result = picker().map(|path| {
                    let path = std::fs::canonicalize(path).map_err(|err| err.to_string())?;
                    let track = PreparedTrack::open(&path)?;
                    let mut metadata = crate::track_metadata::read(&path);
                    metadata.duration_ms = Some(track.duration().as_millis() as u64);
                    metadata.duration_exact = true;
                    Ok((path, track, metadata))
                });
                let _ = tx.send(result);
            }) {
            Ok(_) => {
                *self.pending.borrow_mut() = Some((id, rx));
            }
            Err(err) => {
                ui.set_loading(false);
                ui.set_status(format!("Cannot start loader: {err}").into());
            }
        }
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
                if self.engine.snapshot(id).playing {
                    ui.set_status(format!("Load discarded: deck {id:?} started playing").into());
                } else {
                    self.engine.install(id, track);
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
    pub fn tick(&self) {
        self.poll_storage();
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
            self.pending_import.borrow_mut().take();
            if let Some(ui) = self.ui.upgrade() {
                ui.set_library_busy(false);
                match result {
                    Ok(Some(batch)) => {
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
                    Ok(None) => ui.set_status("Library import cancelled".into()),
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
            self.pending.borrow_mut().take();
            self.complete_load(id, result);
        }
        self.save_library();
        self.refresh();
    }
    pub fn import_paths(&self, paths: Vec<PathBuf>) {
        self.start_import(Arc::new(move || Some(paths.clone())));
    }
    fn start_import(&self, picker: LibraryPicker) {
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
        self.import_crate.set(self.library.borrow().active_crate());
        ui.set_status("Importing local MP3/WAV files…".into());
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("library-import".into())
            .spawn(move || {
                let _ = tx.send(picker().map(crate::library::inspect_paths));
            }) {
            Ok(_) => *self.pending_import.borrow_mut() = Some(rx),
            Err(err) => {
                ui.set_library_busy(false);
                ui.set_status(format!("Cannot start library worker: {err}").into());
            }
        }
    }
    fn publish_library(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.library
            .borrow_mut()
            .set_query(ui.get_library_query().to_string());
        let library = self.library.borrow();
        let crates = library.crates().ordered();
        let target = crates
            .iter()
            .position(|entry| entry.id == self.crate_target.get())
            .or_else(|| (!crates.is_empty()).then_some(0));
        self.crate_target
            .set(target.map_or(0, |index| crates[index].id));
        ui.set_target_crate_index(target.map_or(-1, |index| index as i32));
        ui.set_crate_names(ModelRc::new(Rc::new(VecModel::from(
            crates
                .iter()
                .map(|c| slint::SharedString::from(c.name.as_str()))
                .collect::<Vec<_>>(),
        ))));
        ui.set_crate_rows(ModelRc::new(Rc::new(VecModel::from(
            crates
                .iter()
                .map(|c| crate::CrateRow {
                    id: c.id,
                    name: c.name.clone().into(),
                    count: c.tracks.len() as i32,
                })
                .collect::<Vec<_>>(),
        ))));
        ui.set_selected_crate(library.active_crate());
        let visible = library.visible();
        let rows: Vec<_> = visible
            .iter()
            .map(|track| crate::LibraryRow {
                id: track.id,
                name: track.display_title().into(),
                artist: track.metadata.artist.clone().into(),
                album: track.metadata.album.clone().into(),
                duration: track.metadata.duration_text().into(),
                metadata_error: track.metadata.error.clone().unwrap_or_default().into(),
                format: track.format.clone().into(),
                path: track.path.to_string_lossy().into_owned().into(),
                available: track.available,
            })
            .collect();
        ui.set_library_count(
            format!(
                "{} / {} tracks ({} missing)",
                visible.len(),
                library.scope_len(),
                library.missing_count()
            )
            .into(),
        );
        ui.set_library_empty(
            if library.len() == 0 {
                "Add MP3/WAV files to your library"
            } else if visible.is_empty()
                && library.active_crate() != 0
                && ui.get_library_query().is_empty()
            {
                "This crate is empty. Add tracks or import files."
            } else if visible.is_empty() {
                "No tracks match your search"
            } else {
                ""
            }
            .into(),
        );
        ui.set_library_rows(ModelRc::new(Rc::new(VecModel::from(rows))));
        ui.set_selected_track(library.selected_id());
    }
    fn refresh(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        for id in [DeckId::A, DeckId::B] {
            let s = self.engine.snapshot(id);
            let index = Self::grid_index(id);
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
            let time = format!("{} / {}", clock(s.position), clock(s.duration));
            let caption = preview.map_or_else(
                || "GRID OFF".into(),
                |grid| format!("{} BPM · MANUAL 4/4", grid.bpm()),
            );
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
fn clock(time: std::time::Duration) -> String {
    let secs = time.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
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
    fn control(ui: &AppWindow, label: &str) -> ElementHandle {
        let controls: Vec<_> = ElementHandle::find_by_accessible_label(ui, label).collect();
        assert_eq!(controls.len(), 1, "accessible control {label}");
        controls.into_iter().next().unwrap()
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
    }
    fn finish_import(c: &AudioController) {
        assert!(c.pending_import.borrow().is_some());
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while c.pending_import.borrow().is_some() {
            assert!(std::time::Instant::now() < deadline);
            c.tick();
            std::thread::sleep(Duration::from_millis(1));
        }
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
                id: 73,
                path: fixture(),
                metadata: Default::default(),
                grid: None,
            }],
        };
        let original = serde_json::to_vec(&catalog).unwrap();
        std::fs::write(&legacy, &original).unwrap();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        assert_eq!(ui.get_library_rows().row_count(), 1);
        ui.invoke_select_library(1);
        control(&ui, "Clear library search").invoke_accessible_default_action();
        control(&ui, "Sort library by duration").invoke_accessible_default_action();
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
            Rc::new(DualDeckEngine::from_mixer(mixer)),
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
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 2);
        ui.invoke_select_library(2);
        ui.invoke_load_library_right();
        finish(&c);
        assert!(ui.get_right_title().ends_with(".wav"));
        assert!(!ui.get_right_playing());
        c.close_library().unwrap();
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        assert_eq!(ui.get_selected_crate(), 1);
        assert_eq!(ui.get_crate_rows().row_count(), 1);
        assert_eq!(ui.get_library_rows().row_count(), 0);
        assert!(ui.get_library_empty().contains("crate is empty"));
        control(&ui, "All library tracks").invoke_accessible_default_action();
        control(&ui, "Library track 1").invoke_accessible_default_action();
        control(&ui, "Add selected track to crate").invoke_accessible_default_action();
        control(&ui, "Add selected track to crate").invoke_accessible_default_action();
        assert_eq!(ui.get_crate_rows().row_data(0).unwrap().count, 1);
        control(&ui, "Crate 1").invoke_accessible_default_action();
        assert_eq!(ui.get_library_rows().row_count(), 1);
        control(&ui, "Library search").set_accessible_value("no matching member");
        assert_eq!(ui.get_library_rows().row_count(), 0);
        control(&ui, "Clear library search").invoke_accessible_default_action();
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
        assert_eq!(ui.get_selected_crate(), 2);
        // An import belongs to the captured crate, even if a later callback changes the filter.
        c.import_paths(vec![third.path().to_owned()]);
        ui.invoke_select_crate(1);
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
        ui.invoke_select_library(1);
        ui.set_target_crate_index(1);
        ui.invoke_target_crate(1);
        control(&ui, "Add selected track to crate").invoke_accessible_default_action();
        assert_eq!(c.crate_target.get(), 2);
        ui.invoke_select_crate(1);
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
        ui.invoke_confirm_delete_crate();
        assert_eq!(ui.get_crate_rows().row_count(), 2);
        ui.invoke_select_crate(1);
        control(&ui, "Delete crate").invoke_accessible_default_action();
        control(&ui, "Confirm crate deletion").invoke_accessible_default_action();
        assert_eq!(ui.get_crate_rows().row_count(), 1);
        assert_eq!(ui.get_selected_crate(), 0);
        assert_eq!(ui.get_library_rows().row_count(), 3);
        assert!(ui.get_left_playing());
        assert_eq!(ui.get_left_title(), title);
        assert!(fixture().is_file() && wav.path().is_file() && third.path().is_file());
        ui.invoke_select_crate(2);
        ui.invoke_select_library(1);
        control(&ui, "Remove selected track from crate").invoke_accessible_default_action();
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
            Rc::new(DualDeckEngine::from_mixer(mixer)),
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
        assert_eq!(ui.get_library_rows().row_data(0).unwrap().id, 3);
        ui.invoke_select_library(3);
        ui.invoke_load_library_right();
        finish(&c);
        assert!(ui.get_right_title().ends_with(".wav"));
        assert!(!ui.get_right_playing());
        c.close_library().unwrap();
    }
    #[test]
    fn grid_editor_drafts_invalid_input_cancel_and_clear_preserve_transport() {
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
    fn durable_library_reports_write_failure_retry_and_damaged_startup_without_data_loss() {
        i_slint_backend_testing::init_no_event_loop();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        let ui = AppWindow::new().unwrap();
        let (mixer, _output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        assert_eq!(ui.get_selected_track(), 2);
        control(&ui, "Load selected into deck A").invoke_accessible_default_action();
        finish(&c);
        assert!(ui.get_left_title().ends_with("tone-9s.mp3"));
        assert!(!ui.get_right_loaded());
        control(&ui, "Library search").set_accessible_value(".WAV");
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
        assert_eq!(ui.get_library_rows().row_count(), 0);
        assert!(ui.get_library_empty().contains("No tracks match"));
        control(&ui, "Clear library search").invoke_accessible_default_action();
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
            let shared = control(&ui, "Shared waveform area");
            let library = control(&ui, "Track library area");
            let status = control(&ui, "Application status");
            let left = control(&ui, "DECK A detailed waveform");
            let right = control(&ui, "DECK B detailed waveform");
            let overview = control(&ui, "DECK A waveform overview");
            assert!(overview.absolute_position().y < shared.absolute_position().y);
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
    fn detail_zoom_and_clicks_follow_local_time_without_changing_other_deck() {
        use slint::platform::{PointerEventButton, WindowEvent};
        use slint::Model;
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
        let engine = Rc::new(DualDeckEngine::from_mixer(mixer));
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
