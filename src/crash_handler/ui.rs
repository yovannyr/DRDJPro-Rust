//! Slint composition. Callbacks only enqueue filesystem work and poll messages;
//! the UI owns upload cancellation directly, including while the FS actor waits.
use super::runtime_log;
use super::{
    manager::{Action, Event, Request, Snapshot, Worker},
    spawn_upload_queue, CrashSettings, CrashStartup, CrashStore, ReportStatus, UploadTask,
};
use crate::{AppWindow, CrashReportRow, CrashReportsWindow};
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::{cell::RefCell, io, rc::Rc, sync::mpsc::TryRecvError, time::Duration};

type Uploader = Rc<dyn Fn(CrashStore) -> io::Result<UploadTask>>;

#[derive(Clone, Copy)]
enum SelectionAction {
    Send,
    KeepLocal,
    Delete,
}

struct State {
    log_status: runtime_log::Status,
    app: slint::Weak<AppWindow>,
    view: Option<CrashReportsWindow>,
    store: Option<CrashStore>,
    worker: Option<Worker>,
    data: Option<Snapshot>,
    store_error: bool,
    pending: Option<u64>,
    next: u64,
    active: Option<UploadTask>,
    uploader: Uploader,
    want_upload: bool,
    after_upload: Option<String>,
    capture_notice: String,
    message: String,
    delivery_status: String,
    prompt: bool,
    dismissed: bool,
    closing: bool,
}

impl State {
    fn dispatch(&mut self, action: Action) {
        if self.closing || self.pending.is_some() {
            return;
        }
        if self.store_error && !matches!(action, Action::Refresh(_)) {
            return;
        }
        if matches!(
            action,
            Action::KeepLocal(_) | Action::Delete(_) | Action::Save(_)
        ) {
            if let Some(task) = &self.active {
                task.cancel();
            }
            self.want_upload = false;
        }
        let Some(worker) = &self.worker else {
            return;
        };
        let id = self.next;
        self.next += 1;
        match worker.send(Request { id, action }) {
            Ok(()) => {
                self.pending = Some(id);
                self.message = "Working on saved reports…".into();
            }
            Err(_) => self.message = "Report worker busy or unavailable; try again".into(),
        }
        self.render(false);
    }

    fn apply(&mut self, event: Event) -> bool {
        if self.closing {
            return false;
        }
        if self.pending != Some(event.id) {
            return false;
        }
        self.pending = None;
        self.store_error = event.snapshot.is_none();
        if self.store_error {
            if let Some(task) = &self.active {
                task.cancel();
            }
            self.want_upload = false;
        }
        if let Some(data) = event.snapshot {
            let old_id = self.selected_id();
            self.data = Some(data);
            if let Some(view) = &self.view {
                let index = old_id
                    .and_then(|id| self.data.as_ref()?.reports.iter().position(|r| r.id == id))
                    .or_else(|| {
                        self.data
                            .as_ref()?
                            .reports
                            .iter()
                            .position(|r| r.status == ReportStatus::Pending)
                    })
                    .or_else(|| (!self.data.as_ref()?.reports.is_empty()).then_some(0));
                view.set_selected_row(index.map_or(-1, |index| index as i32));
            }
        }
        self.message = event.message;
        self.prompt = self.data.as_ref().is_some_and(|data| {
            data.reports
                .iter()
                .any(|r| r.status == ReportStatus::Pending)
                && (!data.settings.auto_submit || !data.endpoint_ready)
        });
        self.want_upload |= event.upload;
        self.render(event.settings_changed);
        self.advance();
        event.prompt && !self.dismissed
    }

    fn advance(&mut self) {
        if self.closing || self.pending.is_some() || self.active.is_some() {
            return;
        }
        if self.want_upload {
            self.want_upload = false;
            if let Some(store) = &self.store {
                match (self.uploader)(store.clone()) {
                    Ok(task) => {
                        self.active = Some(task);
                        self.message = "Sending authorized text reports…".into();
                    }
                    Err(_) => {
                        self.message =
                            "Upload could not start; authorized reports remain queued".into()
                    }
                }
            }
        } else if let Some(message) = self.after_upload.take() {
            self.dispatch(Action::Refresh(Some(message)));
        }
        self.render(false);
    }

    fn poll_upload(&mut self) {
        let outcome = match self.active.as_ref().map(UploadTask::try_result) {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => {
                Some(Err(io::Error::other("Upload worker exited")))
            }
            _ => None,
        };
        if let Some(outcome) = outcome {
            self.active.take();
            self.after_upload = Some(match outcome {
                Ok(summary) => format!(
                    "Server-confirmed: {}; retained for retry: {}; skipped: {}",
                    summary.submitted, summary.failed, summary.skipped
                ),
                Err(_) => {
                    "Upload unavailable; authorized reports remain queued for a later attempt"
                        .into()
                }
            });
            self.delivery_status = self.after_upload.clone().unwrap_or_default();
            self.advance();
        }
    }

    fn selected_id(&self) -> Option<String> {
        let index = usize::try_from(self.view.as_ref()?.get_selected_row()).ok()?;
        Some(self.data.as_ref()?.reports.get(index)?.id.clone())
    }

    fn selected_action(&mut self, action: SelectionAction) {
        if self.closing
            || self.store_error
            || self.pending.is_some()
            || (matches!(action, SelectionAction::Send) && self.active.is_some())
        {
            return;
        }
        let Some(id) = self.selected_id() else {
            return;
        };
        let submitted = self.data.as_ref().is_some_and(|d| {
            d.reports
                .iter()
                .any(|r| r.id == id && r.status == ReportStatus::Submitted)
        });
        if submitted && !matches!(action, SelectionAction::Delete) {
            return;
        }
        self.dispatch(match action {
            SelectionAction::Send => Action::Send(id),
            SelectionAction::KeepLocal => Action::KeepLocal(id),
            SelectionAction::Delete => Action::Delete(id),
        });
    }

    fn render(&self, settings_changed: bool) {
        if let Some(app) = self.app.upgrade() {
            let notice = if !self.capture_notice.is_empty() {
                self.capture_notice.as_str()
            } else if self.data.is_none() || self.store_error || self.worker.is_none() {
                "Crash report manager unavailable"
            } else if self
                .data
                .as_ref()
                .is_some_and(|d| !d.settings_issue.is_empty())
            {
                "Crash sharing settings need attention"
            } else if self.prompt {
                "Crash reports need your consent"
            } else {
                ""
            };
            app.set_crash_notice(notice.into());
        }
        let Some(view) = &self.view else {
            return;
        };
        view.set_busy(self.pending.is_some());
        view.set_available(
            self.data.is_some() && !self.store_error && self.worker.is_some() && !self.closing,
        );
        view.set_uploading(self.active.is_some());
        view.set_status(self.message.clone().into());
        view.set_delivery_status(self.delivery_status.clone().into());
        view.set_capture_notice(self.capture_notice.clone().into());
        view.set_logging_status(runtime_log::status().text().into());
        view.set_post_crash(self.prompt);
        if let Some(data) = &self.data {
            view.set_endpoint_ready(data.endpoint_ready);
            view.set_settings_notice(data.settings_issue.clone().into());
            view.set_saved_destination(
                if data.endpoint_ready {
                    data.settings.server_url.clone()
                } else {
                    "Not configured or invalid".into()
                }
                .into(),
            );
            if settings_changed {
                view.set_endpoint(data.settings.server_url.clone().into());
                view.set_automatic(data.settings.auto_submit);
            }
            self.render_details();
        }
    }

    fn render_rows(&self) {
        if let (Some(view), Some(data)) = (&self.view, &self.data) {
            let rows: Vec<_> = data
                .reports
                .iter()
                .map(|r| CrashReportRow {
                    time: chrono::DateTime::from_timestamp_millis(r.created_ms)
                        .map_or_else(
                            || "Unknown time".into(),
                            |t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                        )
                        .into(),
                    version: bounded(&r.version, 80).into(),
                    state: match r.status {
                        ReportStatus::Pending => "Needs consent",
                        ReportStatus::Declined => "Local only",
                        ReportStatus::Queued => "Authorized / queued",
                        ReportStatus::Submitted => "Server confirmed",
                    }
                    .into(),
                })
                .collect();
            view.set_rows(ModelRc::new(VecModel::from(rows)));
        }
    }

    fn render_details(&self) {
        let Some(view) = &self.view else {
            return;
        };
        let report = usize::try_from(view.get_selected_row())
            .ok()
            .and_then(|i| self.data.as_ref()?.reports.get(i));
        view.set_selected_submitted(report.is_some_and(|r| r.status == ReportStatus::Submitted));
        view.set_details(
            report
                .map_or_else(
                    || "No report selected".into(),
                    |r| {
                        format!(
                            "Version: {}\nOS: {} / {}\n\n{}",
                            bounded(&r.version, 80),
                            bounded(&r.os, 80),
                            bounded(&r.arch, 80),
                            bounded(&r.stack, 48_000)
                        )
                    },
                )
                .into(),
        );
    }

    fn shutdown(&mut self) {
        self.closing = true;
        self.want_upload = false;
        self.active.take(); // Cancel immediately, independently of filesystem work.
        self.worker.take();
        if let Some(view) = &self.view {
            let _ = view.hide();
        }
    }
}

fn bounded(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Owns window, actor and upload lifetimes. Dropping never joins worker threads.
pub struct CrashUiSession {
    state: Rc<RefCell<State>>,
    timer: Timer,
}
impl CrashUiSession {
    /// Main-window shutdown: hide the auxiliary window and cancel future sends.
    pub fn shutdown(&self) {
        self.timer.stop();
        self.state.borrow_mut().shutdown();
    }

    #[cfg(test)]
    pub(super) fn testing_tick(&self) {
        tick(&self.state);
    }
    #[cfg(test)]
    pub(super) fn testing_delivery_status(&self) -> String {
        self.state.borrow().delivery_status.clone()
    }
    #[cfg(test)]
    pub(super) fn testing_idle(&self) -> bool {
        let state = self.state.borrow();
        state.pending.is_none() && state.active.is_none()
    }
    #[cfg(test)]
    pub(super) fn testing_store_failed(&self) -> bool {
        self.state.borrow().store_error
    }
}
impl Drop for CrashUiSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Connect the normal startup context to Slint after AppWindow creation.
pub fn wire_ui(ui: &AppWindow, startup: &CrashStartup) -> CrashUiSession {
    let store = startup.store();
    let worker = store.as_ref().and_then(|s| Worker::spawn(s.clone()).ok());
    wire_with(
        ui,
        store,
        worker,
        startup.notice().to_owned(),
        Rc::new(spawn_upload_queue),
    )
}

fn wire_with(
    ui: &AppWindow,
    store: Option<CrashStore>,
    worker: Option<Worker>,
    notice: String,
    uploader: Uploader,
) -> CrashUiSession {
    let pending = worker.as_ref().map(|_| 0);
    let state = Rc::new(RefCell::new(State {
        log_status: runtime_log::status(),
        app: ui.as_weak(),
        view: None,
        store,
        worker,
        data: None,
        store_error: false,
        pending,
        next: 1,
        active: None,
        uploader,
        want_upload: false,
        after_upload: None,
        capture_notice: notice,
        message: "Loading saved crash reports…".into(),
        delivery_status: String::new(),
        prompt: false,
        dismissed: false,
        closing: false,
    }));
    if state.borrow().worker.is_none() {
        state.borrow_mut().message =
            "Crash report manager unavailable; capture/sharing may be disabled".into();
    }
    state.borrow().render(false);
    let opening = Rc::downgrade(&state);
    ui.on_show_crash_reports(move || {
        if let Some(state) = opening.upgrade() {
            show(&state, true);
        }
    });
    let closing = Rc::downgrade(&state);
    ui.on_close_crash_reports(move || {
        if let Some(state) = closing.upgrade() {
            state.borrow_mut().shutdown();
        }
    });
    let ticking = Rc::downgrade(&state);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        if let Some(state) = ticking.upgrade() {
            tick(&state);
        }
    });
    CrashUiSession { state, timer }
}

fn tick(state: &Rc<RefCell<State>>) {
    if state.borrow().closing {
        return;
    }
    let logging = runtime_log::status();
    if state.borrow().log_status != logging {
        let mut state = state.borrow_mut();
        state.log_status = logging;
        if let Some(view) = &state.view {
            view.set_logging_status(logging.text().into());
        }
    }
    for _ in 0..8 {
        let event = state.borrow().worker.as_ref().map(Worker::poll);
        match event {
            Some(Ok(event)) => {
                let prompt = state.borrow_mut().apply(event);
                state.borrow().render_rows();
                if prompt {
                    show(state, false);
                }
            }
            Some(Err(TryRecvError::Disconnected)) => {
                let mut state = state.borrow_mut();
                state.worker.take();
                state.pending = None;
                state.active.take();
                state.want_upload = false;
                state.after_upload = None;
                state.message =
                    "Crash report worker unavailable; reopen the application to retry".into();
                state.render(false);
                break;
            }
            _ => break,
        }
    }
    state.borrow_mut().poll_upload();
}

fn show(state: &Rc<RefCell<State>>, user: bool) {
    if state.borrow().closing {
        return;
    }
    if state.borrow().view.is_none() {
        let view = match CrashReportsWindow::new() {
            Ok(view) => view,
            Err(_) => {
                state.borrow_mut().message = "Crash report window unavailable".into();
                return;
            }
        };
        for action in [
            SelectionAction::Send,
            SelectionAction::KeepLocal,
            SelectionAction::Delete,
        ] {
            let weak = Rc::downgrade(state);
            let callback = move || {
                if let Some(state) = weak.upgrade() {
                    state.borrow_mut().selected_action(action);
                }
            };
            match action {
                SelectionAction::Send => view.on_send_selected(callback),
                SelectionAction::KeepLocal => view.on_keep_local(callback),
                SelectionAction::Delete => view.on_delete_selected(callback),
            }
        }
        let weak = Rc::downgrade(state);
        view.on_selected(move || {
            if let Some(state) = weak.upgrade() {
                state.borrow().render_details();
            }
        });
        let weak = Rc::downgrade(state);
        view.on_refresh(move || {
            if let Some(state) = weak.upgrade() {
                state.borrow_mut().dispatch(Action::Refresh(None));
            }
        });
        let weak = Rc::downgrade(state);
        view.on_retry(move || {
            if let Some(state) = weak.upgrade() {
                let mut state = state.borrow_mut();
                if state.active.is_none() {
                    state.dispatch(Action::Retry);
                }
            }
        });
        let weak = Rc::downgrade(state);
        view.on_save_settings(move || {
            if let Some(state) = weak.upgrade() {
                let settings = state.borrow().view.as_ref().map(|v| CrashSettings {
                    auto_submit: v.get_automatic(),
                    server_url: v.get_endpoint().to_string(),
                });
                if let Some(settings) = settings {
                    state.borrow_mut().dispatch(Action::Save(settings));
                }
            }
        });
        let weak = Rc::downgrade(state);
        view.on_dismiss(move || {
            if let Some(state) = weak.upgrade() {
                dismiss(&state);
            }
        });
        let weak = Rc::downgrade(state);
        view.window().on_close_requested(move || {
            if let Some(state) = weak.upgrade() {
                dismiss(&state);
            }
            slint::CloseRequestResponse::HideWindow
        });
        state.borrow_mut().view = Some(view);
        if let Some(data) = &state.borrow().data {
            let index = data
                .reports
                .iter()
                .position(|r| r.status == ReportStatus::Pending)
                .or_else(|| (!data.reports.is_empty()).then_some(0));
            state
                .borrow()
                .view
                .as_ref()
                .unwrap()
                .set_selected_row(index.map_or(-1, |i| i as i32));
        }
        state.borrow().render_rows();
        state.borrow().render(true);
    }
    if let Some(view) = &state.borrow().view {
        let _ = view.show();
    }
    if user {
        state.borrow_mut().dispatch(Action::Refresh(None));
    }
}

fn dismiss(state: &Rc<RefCell<State>>) {
    let mut state = state.borrow_mut();
    state.dismissed = true;
    if let Some(view) = &state.view {
        let _ = view.hide();
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn testing_wire(
    ui: &AppWindow,
    store: CrashStore,
    uploader: Uploader,
) -> CrashUiSession {
    let worker = Worker::spawn(store.clone()).unwrap();
    wire_with(ui, Some(store), Some(worker), String::new(), uploader)
}

#[cfg(test)]
pub(super) fn testing_own_upload(
    ui: &AppWindow,
    store: CrashStore,
    task: UploadTask,
) -> CrashUiSession {
    let session = testing_wire(ui, store, Rc::new(spawn_upload_queue));
    session.state.borrow_mut().active = Some(task);
    session
}
