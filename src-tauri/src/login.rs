use crate::api_utils::{ get_device_info, AppConfig };
use crate::token_utils::save_tokens_to_file;
use reqwest::Client;
use serde::Deserialize;
use tauri::AppHandle;
use argon2::{ Argon2, PasswordHasher };
use argon2::password_hash::{ SaltString };
use once_cell::sync::Lazy;
use std::sync::Mutex;
#[cfg(any(target_os = "android", target_os = "ios"))]
use base64::Engine;

// Structs (classes/objects) to deserialize the Lambda response
#[derive(Deserialize)]
struct LambdaResponse {
    status_code: u16,
    body: String,
}

#[derive(Deserialize)]
struct Body {
    access_token: String,
    refresh_token: String,
}

// Function to log in a user using AWS Lambda //
pub async fn login_user_lambda(app_handle: &AppHandle, email: String, password: String,) -> Result<String, String> {
    let config = AppConfig::new()?;
    let device_info = get_device_info(&app_handle);

    let url = format!("{}/login", config.lambda_base_url);
    let client = Client::new();

    let user_data = serde_json::json!({
        "email": email,
        "password": password,
        "deviceInfo": device_info
    });

    let payload = serde_json::json!({
        "body": user_data.to_string()
    });
    // send and handle login query
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let _status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;

    // Check for Sandbox.Timedout error in the raw response
    if text.contains("\"errorType\":\"Sandbox.Timedout\"") {
        let frontend_response = serde_json::json!({
            "status": "error",
            "message": "Server timeout, please try again.",
        });
        return Err(frontend_response.to_string());
    }

    // try to parse as a direct error message
    if let Ok(error_obj) = serde_json::from_str::<serde_json::Value>(&text) {
        // If we have a direct error message format like {"message":"Forbidden"}
        if let Some(error_msg) = error_obj.get("message").and_then(|m| m.as_str()) {
            let frontend_response = serde_json::json!({
                "status": "error",
                "message": error_msg,
            });
            return Err(frontend_response.to_string());
        }
    }

    // Parse Lambda response
    let lambda_resp: LambdaResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    // Check status_code
    if lambda_resp.status_code != 200 {
        // Parse the error message from the Lambda response body
        let error_body: serde_json::Value =
            serde_json::from_str(&lambda_resp.body).map_err(|e| e.to_string())?;
        let error_message = error_body["message"].as_str().unwrap_or("Unknown error");

        let frontend_response = serde_json::json!({
            "status": "error",
            "message": error_message,
        });
        return Err(frontend_response.to_string());
    }

    let body: Body = serde_json::from_str(&lambda_resp.body).map_err(|e| e.to_string())?;

    // Derive and cache the database token for this session
    let db_token = derive_database_token(&email, &password);
    {
        let mut cache = DATABASE_TOKEN.lock().unwrap();
        *cache = Some(db_token);
    }

    // Desktop: Save tokens to an encrypted file
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    save_tokens_to_file(&app_handle, &body.access_token, &body.refresh_token, Some(&db_token)).await
        .map_err(|e| format!("Failed to save tokens: {}", e))?;

    // Build frontend response for all platforms
    #[allow(unused_mut)] // silence unused mut warning on desktop platforms
    let mut frontend_response = serde_json::json!({
        "status": "ok",
    });

    // On Android/iOS, return tokens and user_id to frontend for keystore storage
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        frontend_response["tokens"] = serde_json::json!({
            "access_token": body.access_token,
            "refresh_token": body.refresh_token,
        });
        frontend_response["user_id"] = serde_json::json!(email);
        frontend_response["database_token"] = serde_json::json!(base64::engine::general_purpose::STANDARD.encode(db_token));
    }

    Ok(frontend_response.to_string())
}

// Static cache for the database token
pub static DATABASE_TOKEN: Lazy<Mutex<Option<[u8; 32]>>> = Lazy::new(|| Mutex::new(None));

fn derive_database_token(email: &str, password: &str) -> [u8; 32] {
    let salt = SaltString::encode_b64(email.as_bytes()).unwrap(); // Use email as salt for determinism
    let argon2 = Argon2::default();
    let password = format!("{}:{}", email, password);
    let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();
    let hash_value = hash.hash.unwrap();
    let hash_bytes = hash_value.as_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash_bytes[..32]);
    key
}