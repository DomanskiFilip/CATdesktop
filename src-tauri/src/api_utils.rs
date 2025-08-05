use crate::user_utils::UserSettings;
use dotenvy::dotenv;
use serde_json::json;
use std::fs;
use tauri::AppHandle;
use tauri_plugin_machine_uid::MachineUidExt;

pub struct AppConfig {
    pub lambda_base_url: String,
    pub enable_database_sync: bool,
    pub enable_google_sync: bool,
    pub enable_outlook_sync: bool,
    pub notification_service: bool,
}

impl AppConfig {
    pub fn new() -> Result<Self, String> {
        dotenv().ok();

        let lambda_base_url = "https://dnal5hv7lj.execute-api.eu-west-2.amazonaws.com/prod".to_string();

        let enable_database_sync = false;
        let enable_google_sync = false;
        let enable_outlook_sync = false;
        let notification_service = match fs::read_to_string("settings.json") {
            Ok(content) => serde_json::from_str::<UserSettings>(&content)
                .map(|s| s.notification_service)
                .unwrap_or(true),
            Err(_) => true, // default
        };

        Ok(Self {
            lambda_base_url,
            enable_database_sync,
            enable_google_sync,
            enable_outlook_sync,
            notification_service,
        })
    }
}

pub fn get_device_info(app_handle: &AppHandle) -> serde_json::Value {
    let device_type = {
        #[cfg(target_os = "android")]
        {
            "android"
        }
        #[cfg(target_os = "windows")]
        {
            "windows"
        }
        #[cfg(target_os = "linux")]
        {
            "linux"
        }
        #[cfg(target_os = "macos")]
        {
            "macos"
        }
        #[cfg(target_os = "ios")]
        {
            "ios"
        }
        #[cfg(not(any(
            target_os = "android",
            target_os = "windows",
            target_os = "linux",
            target_os = "macos",
            target_os = "ios"
        )))]
        {
            "unknown"
        }
    };

    let id = app_handle
        .machine_uid()
        .get_machine_uid()
        .ok()
        .and_then(|info| info.id)
        .unwrap_or_else(|| "unknown".to_string());

    json!({
        "device_type": device_type,
        "os": device_type,
        "device_identifier": id
    })
}
