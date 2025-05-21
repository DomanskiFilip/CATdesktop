#[cfg_attr(mobile, tauri::mobile_entry_point)]
use tauri::command;
use std::env;
use dotenvy::dotenv;
pub mod window;

#[command]
async fn fetch_lambda_endpoint() -> Result<String, String> {
    dotenv().ok();
    let api_key = env::var("API_KEY").map_err(|e| e.to_string())?;
    let url = "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod/data";
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("x-api-key", api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body = response.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}

pub fn run() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![fetch_lambda_endpoint])
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