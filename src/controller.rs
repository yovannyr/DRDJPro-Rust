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
struct PitchRequest {
    source: Arc<crate::waveform::DetailPeaks>,
    factor: f64,
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
    inflight_revision: Cell<Option<u64>>,
    save_failed: Cell<bool>,
    initial_paths: RefCell<Vec<PathBuf>>,
    crate_editor: Cell<Option<i32>>,
    crate_delete: Cell<Option<i32>>,
    crate_target: Cell<i32>,
    import_crate: Cell<i32>,
    pitch_ranges: Cell<[crate::pitch_control::PitchRange; 2]>,
    pending_pitch: RefCell<[Option<PitchRequest>; 2]>,
    nudges: RefCell<[Option<NudgeSession>; 2]>,
    cue_editors: RefCell<[Option<CueSession>; 2]>,
    grid_editors: RefCell<[Option<GridSession>; 2]>,
    pending_import: RefCell<Option<Receiver<ImportResult>>>,
    pending: RefCell<Option<(DeckId, Receiver<LoadResult>)>>,
    displayed_loops: RefCell<[Option<crate::saved_loops::SavedLoops>; 2]>,
    displayed_cues: RefCell<[Option<(crate::hot_cues::HotCues, std::time::Duration)>; 2]>,
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
            inflight_revision: Cell::new(None),
            save_failed: Cell::new(false),
            initial_paths: RefCell::new(Vec::new()),
            crate_editor: Cell::new(None),
            crate_delete: Cell::new(None),
            crate_target: Cell::new(0),
            import_crate: Cell::new(0),
            pitch_ranges: Cell::new([crate::pitch_control::PitchRange::default(); 2]),
            pending_pitch: RefCell::new([None, None]),
            nudges: RefCell::new([None, None]),
            cue_editors: RefCell::new([None, None]),
            grid_editors: RefCell::new([None, None]),
            pending_import: RefCell::new(None),
            pending: RefCell::new(None),
            displayed_loops: RefCell::new([None, None]),
            displayed_cues: RefCell::new([None, None]),
            displayed_overviews: RefCell::new([None, None]),
            displayed_details: RefCell::new([None, None]),
        });
        ui.set_audio_enabled(true);
        for id in [DeckId::A, DeckId::B] {
            let weak = Rc::downgrade(&controller);
            let toggle = move || {
                if let Some(c) = weak.upgrade() {
                    if c.engine.snapshot(id).playing {
                        c.cancel_nudge(id);
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
                    c.cancel_nudge(id);
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
        controller.wire_hot_cues(ui);
        controller.wire_loops(ui);
        controller.wire_pitch(ui);
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
        let snapshot = self.engine.snapshot(id);
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
                self.engine.restore_loop(id, value.start(), value.end())?;
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
    fn queue_pitch(&self, id: DeckId, percent: f32) -> Result<(), String> {
        self.pending_pitch.borrow_mut()[Self::grid_index(id)].take();
        if !self.can_control_deck(id) {
            return Err("Finish loading before changing pitch".into());
        }
        let source = self
            .engine
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
    fn fine_pitch(&self, id: DeckId, up: bool) -> Result<(), String> {
        if !self.can_control_deck(id) {
            return Err("Finish loading before changing pitch".into());
        }
        let snapshot = self.engine.snapshot(id);
        let source = snapshot.detail.ok_or("Load a track first")?;
        let factor = self.pending_pitch.borrow()[Self::grid_index(id)]
            .as_ref()
            .filter(|request| Arc::ptr_eq(&source, &request.source))
            .map_or(snapshot.requested_speed, |request| request.factor);
        let target = self.pitch_ranges.get()[Self::grid_index(id)].step(factor, up)?;
        self.queue_pitch(id, ((target - 1.) * 100.) as f32)
    }
    fn cancel_nudge(&self, id: DeckId) {
        self.nudges.borrow_mut()[Self::grid_index(id)] = None;
        // Release bypasses all busy guards and never starts a deck.
        let _ = self.engine.set_nudge(id, 0);
    }
    fn cancel_stale_nudges(&self) {
        for id in [DeckId::A, DeckId::B] {
            let snapshot = self.engine.snapshot(id);
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
        self.cancel_stale_nudges();
        let index = Self::grid_index(id);
        if held && !self.can_control_deck(id) {
            return Err("Nudge unavailable during loading".into());
        }
        let snapshot = self.engine.snapshot(id);
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
        if let Err(error) = self.engine.set_nudge(id, target) {
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
    fn apply_pending_pitch(&self) {
        for id in [DeckId::A, DeckId::B] {
            let Some(request) = self.pending_pitch.borrow_mut()[Self::grid_index(id)].take() else {
                continue;
            };
            let result = if !self.can_control_deck(id) {
                Err("Pending pitch discarded during loading".into())
            } else if self
                .engine
                .snapshot(id)
                .detail
                .as_ref()
                .is_none_or(|source| !Arc::ptr_eq(source, &request.source))
            {
                Err("Pending pitch discarded after track replacement".into())
            } else {
                self.engine.set_speed(id, request.factor)
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
            for command in 0..7 {
                let weak = Rc::downgrade(self);
                let action = move || {
                    if let Some(c) = weak.upgrade() {
                        let result = if !c.can_control_deck(id) {
                            Err("Finish loading before changing a loop".into())
                        } else {
                            let position = c.engine.snapshot(id).position;
                            match command {
                                0 => c.engine.set_loop_in(id, position),
                                1 => c.engine.set_loop_out(id, position),
                                2 => c.engine.activate_loop(id),
                                3 => c.engine.exit_loop(id),
                                4 => {
                                    let index =
                                        c.ui.upgrade()
                                            .map(|ui| match id {
                                                DeckId::A => ui.get_left_auto_loop_index(),
                                                DeckId::B => ui.get_right_auto_loop_index(),
                                            })
                                            .unwrap_or(-1);
                                    let snap = c.ui.upgrade().is_some_and(|ui| match id {
                                        DeckId::A => ui.get_left_auto_loop_snap(),
                                        DeckId::B => ui.get_right_auto_loop_snap(),
                                    });
                                    usize::try_from(index)
                                        .ok()
                                        .and_then(|i| {
                                            [1, 2, 4, 8, 16, 32, 64, 128, 256].get(i).copied()
                                        })
                                        .ok_or_else(|| "Choose a supported beat length".to_string())
                                        .and_then(|eighths| c.engine.auto_loop(id, eighths, snap))
                                }
                                5 => c.engine.resize_loop(id, false),
                                _ => c.engine.resize_loop(id, true),
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
                                            "Loop active",
                                            "Loop exited",
                                            "Auto loop active",
                                            "Loop halved",
                                            "Loop doubled"
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
                    (DeckId::A, _) => ui.on_double_loop_left(action),
                    (DeckId::B, _) => ui.on_double_loop_right(action),
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
        let snapshot = self.engine.snapshot(id);
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
        let snapshot = self.engine.snapshot(id);
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
        let source = self.engine.snapshot(id);
        let path = source.path.ok_or("Load a track first")?;
        let cues = self.library.borrow().hot_cues(&path);
        if !delete {
            if let Some(cue) = &cues.slots()[slot] {
                if cue.position() >= source.duration {
                    return Err("Saved cue is outside the loaded track; delete and reset it".into());
                }
                self.engine.seek_position(id, cue.position())?;
                self.cancel_nudge(id);
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
            const COLORS: [u32; 8] = [
                0xe74c3c, 0xf39c12, 0xf1c40f, 0x2ecc71, 0x1abc9c, 0x3498db, 0x9b59b6, 0xe84393,
            ];
            Some(crate::hot_cues::HotCue::new(
                source.position,
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
    fn wire_hot_cues(self: &Rc<Self>, ui: &AppWindow) {
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
        if !self.can_control_deck(id) {
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
        let inserted = self.library.borrow_mut().insert([path.to_path_buf()]);
        self.library.borrow_mut().set_grid(&path, value)?;
        self.engine.set_grid(id, value)?;
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
                    Ok(loaded) => {
                        let crate::library_store::LoadedLibrary {
                            library,
                            note,
                            backfill_error,
                        } = *loaded;
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
                let revision = self.inflight_revision.take();
                match result {
                    Ok(()) => {
                        if let Some(revision) = revision {
                            self.library.borrow_mut().acknowledge(revision);
                        }
                        self.dirty.set(self.library.borrow().has_changes());
                        self.save_failed.set(false);

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
            Some(Ok(crate::library_store::Event::CheckpointFailed(error))) => {
                // A backup warning is not completion of any queued save command.
                self.dirty.set(true);
                self.save_failed.set(true);
                self.storage_error(&format!("Backup checkpoint: {error}"), true);
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
        let delta = self.library.borrow().changes();
        let revision = delta.revision;
        match worker.save_delta(delta) {
            Ok(()) => {
                self.inflight_revision.set(Some(revision));
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
        for id in [DeckId::A, DeckId::B] {
            self.cancel_nudge(id);
        }
        if let Some(worker) = self.persistence.borrow_mut().take() {
            let final_delta = (self.storage_ready.get()
                && (self.dirty.get() || self.saving.get() || self.library.borrow().has_changes()))
            .then(|| self.library.borrow().changes());
            let revision = final_delta.as_ref().map(|d| d.revision);
            worker.close_delta(final_delta)?;
            if let Some(revision) = revision {
                self.library.borrow_mut().acknowledge(revision);
            }
            self.inflight_revision.set(None);
            self.saving.set(false);
            self.dirty.set(false);
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
        self.engine.collect_retired();
        self.cancel_stale_nudges();
        self.apply_pending_pitch();
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
        ui.set_left_busy(!self.can_control_deck(DeckId::A));
        ui.set_right_busy(!self.can_control_deck(DeckId::B));
        for id in [DeckId::A, DeckId::B] {
            let s = self.engine.snapshot(id);
            let index = Self::grid_index(id);
            let pitch_range = self.pitch_ranges.get()[index];
            let (pitch_min, pitch_max) = pitch_range.bounds();
            let pitch_outside = !pitch_range.contains(s.requested_speed);
            match id {
                DeckId::A => {
                    ui.set_left_pitch_range_index(pitch_range.index());
                    ui.set_left_pitch_min(pitch_min as f32);
                    ui.set_left_pitch_max(pitch_max as f32);
                    ui.set_left_pitch_outside(pitch_outside);
                    ui.set_left_pitch_can_down(pitch_range.step(s.requested_speed, false).is_ok());
                    ui.set_left_pitch_can_up(pitch_range.step(s.requested_speed, true).is_ok());
                }
                DeckId::B => {
                    ui.set_right_pitch_range_index(pitch_range.index());
                    ui.set_right_pitch_min(pitch_min as f32);
                    ui.set_right_pitch_max(pitch_max as f32);
                    ui.set_right_pitch_outside(pitch_outside);
                    ui.set_right_pitch_can_down(pitch_range.step(s.requested_speed, false).is_ok());
                    ui.set_right_pitch_can_up(pitch_range.step(s.requested_speed, true).is_ok());
                }
            }
            let pitch = format!(
                "Requested {:+.2}% · effective {:+.4}% · coupled",
                (s.requested_speed - 1.) * 100.,
                (s.effective_speed - 1.) * 100.
            );
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
                    ui.set_left_pitch_percent(((s.requested_speed - 1.) * 100.) as f32);
                    ui.set_left_pitch_caption(pitch.into());
                    ui.set_left_bpm_caption(bpm.into());
                }
                DeckId::B => {
                    ui.set_right_pitch_percent(((s.requested_speed - 1.) * 100.) as f32);
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
                    .map(|cue| crate::HotCueRow {
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
            control(&ui, &format!("{label} activate loop")).invoke_accessible_default_action();
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
                .send(Some(crate::library::inspect_paths(vec![file
                    .path()
                    .to_path_buf()])))
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
        control(&ui, "DECK A activate loop").invoke_accessible_default_action();
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
        control(&ui, "DECK A exit loop").invoke_accessible_default_action();
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
                for action in ["loop in", "loop out", "activate loop", "exit loop"] {
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
                for action in ["nudge down", "nudge up", "cancel nudge"] {
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
