use std::fs;
use tauri::{AppHandle, Manager};

pub async fn save_theme(app_handle: AppHandle, theme: String) -> Result<(), String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get data directory: {}", e))?;

    fs::create_dir_all(&app_dir).map_err(|e| format!("Failed to create data directory: {}", e))?;

    let theme_path = app_dir.join("theme.txt");
    fs::write(theme_path, theme).map_err(|e| format!("Failed to save theme: {}", e))?;
    Ok(())
}

pub async fn load_theme(app_handle: AppHandle) -> Result<String, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get data directory: {}", e))?;
    let theme_path = app_dir.join("theme.txt");

    if theme_path.exists() {
        fs::read_to_string(theme_path).map_err(|e| format!("Failed to read theme: {}", e))
    } else {
        Ok("".to_string())
    }
}
