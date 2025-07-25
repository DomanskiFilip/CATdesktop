use crate::user_utils::UserSettings;
use dotenvy::dotenv;
use std::env;
use std::fs;
#[cfg(not(target_os = "android"))]
use mac_address::get_mac_address;
use serde_json::Value;

pub struct AppConfig {
    pub lambda_base_url: String,
    pub enable_database_sync: bool,
    pub enable_google_sync: bool,
    pub notification_service: bool,
}

impl AppConfig {
    pub fn new() -> Result<Self, String> {
        dotenv().ok();
        
        let lambda_base_url = "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod".to_string();
        
        let enable_database_sync = true;
        let enable_google_sync = true;
        let notification_service = match fs::read_to_string("settings.json") {
            Ok(content) => {
                serde_json::from_str::<UserSettings>(&content)
                    .map(|s| s.notification_service)
                    .unwrap_or(true)
            },
            Err(_) => true, // default
        };

        Ok(Self {
            lambda_base_url,
            enable_database_sync,
            enable_google_sync,
            notification_service,
        })
    }
}

#[cfg(target_os = "android")]
fn get_android_device_id() -> String {
    // Placeholder: implement JNI call to get ANDROID_ID or use a static string for now
    "android-device-id-placeholder".to_string()
}

pub fn get_device_info() -> serde_json::Value {
    #[cfg(not(target_os = "android"))]
    let mac_address = get_mac_address()
        .ok()
        .flatten()
        .map(|mac| mac.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    #[cfg(target_os = "android")]
    let mac_address = get_android_device_id();

    serde_json::json!({
        "device_type": "desktop",
        "os": std::env::consts::OS,
        "mac_address": mac_address
    })
}