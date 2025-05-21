#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub mod window;
mod oauth;

use tauri::command;
use std::env;
use dotenvy::dotenv;
use crate::oauth::oauth2_flow;

const TIMEOUT: u64 = 120; // 2 minutes

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
    TIMEOUT // or read from config if you want
}

#[command]
async fn fetch_lambda_endpoint() -> Result<String, String> {
    use std::fs;
    dotenv().ok();
    let api_key = env::var("API_KEY").map_err(|e| e.to_string())?;
    let url = "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod/data";
    // gert access token from the file
    let data_dir = dirs::data_local_dir().ok_or("Could not get app data dir")?;
    let token_path = data_dir.join("CalendarAssistantApp").join("access_token.txt");
    let access_token = fs::read_to_string(token_path).map_err(|e| e.to_string())?;
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("x-api-key", api_key)
        .bearer_auth(access_token.trim())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body = response.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}

pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      get_oauth_timeout,
      run_oauth2_flow,
      fetch_lambda_endpoint
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