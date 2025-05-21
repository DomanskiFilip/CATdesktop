use tauri::{AppHandle, Manager};

pub fn set_always_on_top(app: &AppHandle, always_on_top: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(always_on_top);
    }
}
