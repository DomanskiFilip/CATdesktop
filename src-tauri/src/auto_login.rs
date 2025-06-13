use reqwest::Client;
use serde::Deserialize;
use tauri::AppHandle;
use crate::token_utils::{read_tokens_from_file, save_tokens_to_file};
use crate::api_utils::{AppConfig, get_device_info};

// Structs (classes/objects) to deserialize the Lambda response
#[derive(Deserialize)]
struct LambdaResponse {
    status_code: u16,
    body: String,
}

#[derive(Deserialize)]
struct Body {
    access_token: Option<String>,
    message: Option<String>,
}

// Function to handle auto-login using AWS Lambda
pub async fn auto_login_lambda(app_handle: &AppHandle) -> Result<bool, String> {
    println!("Starting auto-login process...");
    let config = AppConfig::new()?;
    let device_info = get_device_info();

    // Read tokens from file
    let (access_token, refresh_token) = match read_tokens_from_file(&app_handle) {
        Ok(tokens) => tokens,
        Err(e) => {
            println!("No tokens found or failed to read tokens: {}", e);
            return Ok(false);
        }
    };

    let url = format!("{}/autologin", config.lambda_base_url);
    let client = Client::new();

    // Prepare the initial payload with access token and device info
    let mut payload = serde_json::json!({
        "body": serde_json::json!({
            "access_token": access_token,
            "deviceInfo": device_info
        }).to_string()
    });

    let mut response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &config.api_key)
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let text = response.text().await.map_err(|e| e.to_string())?;
    let lambda_resp: LambdaResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    match lambda_resp.status_code {
        200 => {
            // User is logged in
            println!("User is logged in successfully.");
            Ok(true)
        }
        201 => {
            // Access token expired, send refresh token
            println!("Access token expired. Server response: {}", lambda_resp.body);
            if let Some(refresh_token) = Some(refresh_token) {
                payload = serde_json::json!({
                    "body": serde_json::json!({
                        "refresh_token": refresh_token,
                        "deviceInfo": device_info
                    }).to_string()
                });

                response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .header("x-api-key", &config.api_key)
                    .body(payload.to_string())
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                let text = response.text().await.map_err(|e| e.to_string())?;
                let lambda_resp: LambdaResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;

                if lambda_resp.status_code == 300 {
                    // Save new access token
                    let body: Body = serde_json::from_str(&lambda_resp.body).map_err(|e| e.to_string())?;
                    if let Some(new_access_token) = body.access_token {
                        save_tokens_to_file(&app_handle, &new_access_token, &refresh_token)
                            .map_err(|e| format!("Failed to save tokens: {}", e))?;
                        println!("User is logged in successfully with refresh token.");
                        return Ok(true);
                    }
                }
                println!("Failed to refresh access token. Server response: {}", lambda_resp.body);
                return Ok(false);
            }
            println!("No refresh token available. Server response: {}", lambda_resp.body);
            Ok(false)
        }
        301 => {
            // Refresh token expired or device mismatch
            println!("Refresh token expired or device mismatch. Server response: {}", lambda_resp.body);
            Ok(false)
        }
        _ => {
            // Unexpected status code
            println!("Unexpected status code: {}. Server response: {}", lambda_resp.status_code, lambda_resp.body);
            Ok(false)
        }
    }
}