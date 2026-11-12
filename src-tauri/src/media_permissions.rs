// Microphone access for the webview.
//
// On Linux the webview is WebKitGTK. Tauri/wry leave media capture disabled
// there and never answer WebKit's permission requests, so getUserMedia() fails
// (or navigator.mediaDevices is missing entirely). We enable media streams and
// answer microphone requests ourselves, gated by an in-app "allow access" flag
// the frontend sets via `set_microphone_permission`.
//
// On Windows/macOS the native webview shows its own prompt, so the flag is
// only informational there.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;

static MICROPHONE_ALLOWED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn set_microphone_permission(allowed: bool) {
    MICROPHONE_ALLOWED.store(allowed, Ordering::SeqCst);
}

#[cfg(target_os = "linux")]
pub fn configure_webview_media(app: &AppHandle) {
    use tauri::Manager;
    use webkit2gtk::glib::prelude::*;
    use webkit2gtk::{
        DeviceInfoPermissionRequest, PermissionRequestExt, SettingsExt,
        UserMediaPermissionRequest, UserMediaPermissionRequestExt, WebViewExt,
    };

    let Some(window) = app.get_webview_window("main") else {
        eprintln!("configure_webview_media: main window not found");
        return;
    };

    let result = window.with_webview(|webview| {
        let wv = webview.inner();

        if let Some(settings) = WebViewExt::settings(&wv) {
            let was_enabled = settings.enables_media_stream();
            settings.set_enable_media_stream(true);
            // navigator.mediaDevices is only exposed to documents created after
            // the setting is on; reload if the page already finished loading.
            if !was_enabled && !wv.is_loading() && wv.uri().is_some() {
                wv.reload();
            }
        }

        wv.connect_permission_request(|_, request| {
            if let Some(media) = request.downcast_ref::<UserMediaPermissionRequest>() {
                // Microphone only; never hand out the camera.
                if media.is_for_audio_device()
                    && !media.is_for_video_device()
                    && MICROPHONE_ALLOWED.load(Ordering::SeqCst)
                {
                    request.allow();
                } else {
                    request.deny();
                }
                return true;
            }
            // Needed for device labels/ids in enumerateDevices()
            if request.is::<DeviceInfoPermissionRequest>() {
                if MICROPHONE_ALLOWED.load(Ordering::SeqCst) {
                    request.allow();
                } else {
                    request.deny();
                }
                return true;
            }
            false // let WebKit handle anything else
        });
    });

    if let Err(e) = result {
        eprintln!("configure_webview_media: failed to access webview: {}", e);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn configure_webview_media(_app: &AppHandle) {}
