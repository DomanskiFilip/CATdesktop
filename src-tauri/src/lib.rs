#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub mod window;
mod oauth;
mod login;
mod register;

use tauri::command;
use std::env;
use dotenvy::dotenv;
use crate::oauth::oauth2_flow;


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
      get_oauth_timeout,
      run_oauth2_flow,
      login_user,
      register_user
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