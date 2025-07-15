use dotenvy::dotenv;
use std::env;
use mac_address::get_mac_address;
use serde_json::Value;
use std::sync::Arc;

pub struct AppConfig {
    pub api_key: String,
    pub lambda_base_url: String,
    pub enable_database_sync: bool,
    pub enable_google_sync: bool,
}

impl AppConfig {
    pub fn new() -> Result<Self, String> {
        dotenv().ok();
        
        let api_key = env::var("API_KEY").map_err(|e| e.to_string())?;
        let lambda_base_url = env::var("LAMBDA_BASE_URL")
            .unwrap_or_else(|_| "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod".to_string());
        
        let enable_database_sync = true;
        let enable_google_sync = true;

        Ok(Self {
            api_key,
            lambda_base_url,
            enable_database_sync,
            enable_google_sync,
        })
    }
}

pub fn get_device_info() -> Value {
    let mac_address = get_mac_address()
        .ok()
        .flatten()
        .map(|mac| mac.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    serde_json::json!({
        "device_type": "desktop", 
        "os": env::consts::OS,
        "mac_address": mac_address
    })
}

pub type SharedAppConfig = Arc<AppConfig>;