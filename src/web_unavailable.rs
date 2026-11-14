//! Explicit capability boundary until an OS-native browser adapter is supplied.
use crate::AppWindow;
use slint::{ComponentHandle, Timer};
/// Register the same app actions without pretending that a browser was opened.
pub fn wire(ui: &AppWindow) -> Timer {
    let weak = ui.as_weak();
    ui.on_show_browser(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.set_status("Embedded browser is currently available on Windows only".into());
        }
    });
    ui.on_close_web_browsers(|| {});
    Timer::default()
}
