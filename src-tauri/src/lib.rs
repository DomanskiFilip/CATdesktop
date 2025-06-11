#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub mod window;
mod oauth;
mod login;
mod register;
mod token_utils;
mod theme_utils;
mod encription_key;
mod auto_login;
mod sqlite;
mod sqlite_sync_service;

use tauri::command;
use std::env;
use dotenvy::dotenv;
use crate::oauth::oauth2_flow;

// auto-login command
#[tauri::command]
async fn auto_login() -> Result<bool, String> {
    crate::auto_login::auto_login_lambda().await
}

// login user command
#[tauri::command]
async fn login_user(email: String, password: String) -> Result<String, String> {
    crate::login::login_user_lambda(email, password).await
}

// register user command
#[tauri::command]
async fn register_user(email: String, password: String) -> Result<String, String> {
    crate::register::register_user_lambda(email, password).await
}

// logout user command
#[tauri::command]
async fn logout_user() -> Result<bool, String> {
    crate::token_utils::clear_tokens().map(|_| true)
}

// save and load theme commands
#[tauri::command]
async fn save_theme(app_handle: tauri::AppHandle, theme: String) -> Result<(), String> {
    theme_utils::save_theme(app_handle, theme).await
}

#[tauri::command]
async fn load_theme(app_handle: tauri::AppHandle) -> Result<String, String> {
    theme_utils::load_theme(app_handle).await
}

// save, load and delete event commands
#[tauri::command]
async fn save_event(event: String) -> Result<(), String> {
    sqlite::save_event(event)
}

#[tauri::command]
async fn get_events() -> Result<Vec<String>, String> {
    sqlite::get_events().map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_event(id: String) -> Result<(), String> {
    sqlite::delete_event(id).map_err(|e| e.to_string())
}

// clean old events comand
#[tauri::command]
async fn clean_old_events() -> Result<(), String> {
    sqlite::clean_old_events().map_err(|e| e.to_string())
}

// google oauth2 functionalities
const TIMEOUT: u64 = 120;

#[command]
async fn run_oauth2_flow() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(crate::oauth::oauth2_flow(TIMEOUT))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_oauth_timeout() -> u64 {
    TIMEOUT
}


pub fn run() -> Result<(), Box<dyn std::error::Error>> {
  // tokio::spawn(sqlite_sync_service::start_sync_service());
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            auto_login,
            login_user,
            register_user,
            logout_user,
            save_theme,
            load_theme,
            save_event,
            delete_event,
            get_events,
            clean_old_events,
            get_oauth_timeout,
            run_oauth2_flow
        ])
        .setup(|app| {
            // Initialize database on app startup
            sqlite::init_db().map_err(|e| e.to_string())?;
            crate::window::set_always_on_top(&app.handle(), true);
            
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}