use crate::api_utils::{get_device_info, AppConfig};
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::read_tokens_from_cache;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::token_utils::save_tokens_to_file;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::token_utils::{read_tokens_from_file, save_tokens_to_file};
use crate::user_utils::save_current_user_id;
use reqwest::Client;
use serde::Deserialize;
use tauri::AppHandle;

// Structs (classes/objects) to deserialize the Lambda response
#[derive(Deserialize)]
struct LambdaResponse {
    status_code: u16,
    body: String,
}

#[derive(Deserialize)]
struct Body {
    access_token: Option<String>,
    _message: Option<String>,
}

// Function to handle auto-login using AWS Lambda //
pub async fn auto_login_lambda(app_handle: &AppHandle) -> Result<bool, String> {
    println!("Starting auto-login process...");
    let config = AppConfig::new()?;
    let device_info = get_device_info(&app_handle);

    // Read tokens from file
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let (access_token, refresh_token, database_token) = match crate::read_tokens_from_cache().await
    {
        Some(tokens) => tokens,
        None => {
            println!("No tokens found or failed to read tokens");
            return Ok(false);
        }
    };

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let (access_token, refresh_token, database_token) =
        match read_tokens_from_file(&app_handle).await {
            Ok(tokens) => tokens,
            Err(e) => {
                println!("No tokens found or failed to read tokens: {}", e);
                return Ok(false);
            }
        };

    if let Some(db_token) = database_token {
        let mut cache = crate::login::DATABASE_TOKEN.lock().unwrap();
        *cache = Some(db_token);
    }

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
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let text = response.text().await.map_err(|e| e.to_string())?;
    let lambda_resp: LambdaResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    match lambda_resp.status_code {
        200 => {
            // Access token is valid, save user_id
            let body_json: serde_json::Value = serde_json::from_str(&lambda_resp.body)
                .map_err(|e| format!("Failed to parse response body: {}", e))?;

            // Extract user_id from the body
            if let Some(user_id) = body_json["user_id"].as_str() {
                // Save the user_id
                save_current_user_id(app_handle, user_id)?;
            } else {
                return Err("Failed to extract user_id from response body".to_string());
            }

            // User is logged in
            println!("User is logged in successfully.");
            Ok(true)
        }
        201 => {
            // Access token expired, send refresh token
            println!("Access token expired. Server response: {}", lambda_resp.body);
            println!("Attempting to refresh access token... refresh_token: {}", refresh_token);
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
                    .body(payload.to_string())
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                let text = response.text().await.map_err(|e| e.to_string())?;
                let lambda_resp: LambdaResponse =
                    serde_json::from_str(&text).map_err(|e| e.to_string())?;

                if lambda_resp.status_code == 300 {
                    let body_json: serde_json::Value = serde_json::from_str(&lambda_resp.body)
                        .map_err(|e| format!("Failed to parse response body: {}", e))?;

                    // Extract user_id from the body
                    if let Some(user_id) = body_json["user_id"].as_str() {
                        // Save the user_id
                        save_current_user_id(app_handle, user_id)?;
                    } else {
                        return Err("Failed to extract user_id from response body".to_string());
                    }

                    // Save new access token
                    let body: Body =
                        serde_json::from_str(&lambda_resp.body).map_err(|e| e.to_string())?;
                    if let Some(new_access_token) = body.access_token {
                        save_tokens_to_file(
                            &app_handle,
                            &new_access_token,
                            &refresh_token,
                            database_token.as_ref(),
                        )
                        .await?;
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
