#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub mod window;
mod oauth;
mod login;
mod register;
mod token_utils;
mod theme_utils;
mod encription_key;
mod auto_login;

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

// Load theme command
#[tauri::command]
async fn load_theme(app_handle: tauri::AppHandle) -> Result<String, String> {
    theme_utils::load_theme(app_handle).await
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

pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      auto_login,
      login_user,
      register_user,
      logout_user,
      save_theme,
      load_theme,
      get_oauth_timeout,
      run_oauth2_flow
      ])
    .setup(|app| {
      // set window always on top
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
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}