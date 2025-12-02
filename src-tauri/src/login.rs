use crate::api_utils::{get_device_info, AppConfig};
use crate::token_utils::save_tokens_to_file;
use crate::user_utils::{save_current_user_id};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::encryption_utils::initialize_encryption_key;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use base64::Engine;
use log::warn;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::Deserialize;
use rsa::{pkcs8::DecodePublicKey, rand_core::OsRng, Oaep, RsaPublicKey};
use sha2::Sha256;
use std::sync::Mutex;
use tauri::AppHandle;

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
    user_id: String,
}

// Function to log in a user using AWS Lambda //
pub async fn login_user_lambda(app_handle: &AppHandle, email: String, password: String,) -> Result<String, String> {
    let config = AppConfig::new()?;
    let device_info = get_device_info(&app_handle);

    // IMPORTANT: Initialize encryption key BEFORE any token operations
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if let Err(e) = initialize_encryption_key(&app_handle) {
            return Err(format!("Failed to initialize encryption key: {}", e));
        }
    }

    let url = format!("{}/login", config.lambda_base_url);
    let client = Client::new();
    let encrypted_password = encrypt_password_for_transport(&client, &config.lambda_base_url, &password).await?;

    let user_data = serde_json::json!({
        "email": &email,
        "password": encrypted_password,
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
    let user_id = body.user_id;
    // Derive and cache the database token for this session
    let db_token = derive_database_token(&user_id, &password);
    {
        let mut cache = DATABASE_TOKEN.lock().unwrap();
        *cache = Some(db_token);
    }

    // Desktop: Save tokens to an encrypted file
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    save_tokens_to_file(
        &app_handle,
        &body.access_token,
        &body.refresh_token,
        Some(&db_token),
    )
    .await
    .map_err(|e| format!("Failed to save tokens: {}", e))?;

    // Save user_id instead of email
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let _ = save_current_user_id(&app_handle, &user_id)?;

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
        frontend_response["user_id"] = serde_json::json!(user_id);
        frontend_response["database_token"] =
            serde_json::json!(base64::engine::general_purpose::STANDARD.encode(db_token));
    }

    Ok(frontend_response.to_string())
}

// Static cache for the database token
pub static DATABASE_TOKEN: Lazy<Mutex<Option<[u8; 32]>>> = Lazy::new(|| Mutex::new(None));

fn derive_database_token(username: &str, password: &str) -> [u8; 32] {
    let salt = SaltString::encode_b64(username.as_bytes()).unwrap(); // Use username as salt for determinism
    let argon2 = Argon2::default();
    let password = format!("{}:{}", username, password);
    let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();
    let hash_value = hash.hash.unwrap();
    let hash_bytes = hash_value.as_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash_bytes[..32]);
    key
}

// Encrypt the password using the server's exposed public key
async fn encrypt_password_for_transport(client: &Client, base_url: &str, password: &str) -> Result<String, String> {
    match try_encrypt_with_server_key(client, base_url, password).await? {
        Some(cipher) => Ok(cipher),
        None => {
            // Fallback to plaintext if encryption fails !! insecure whatch our for this log
            warn!("❗❗❗Falling back to plaintext password payload (could not encrypt❗❗❗)");
            Ok(password.to_string())
        }
    }
}

// Attempt to encrypt the password with the server's public key
async fn try_encrypt_with_server_key(client: &Client, base_url: &str, password: &str,) -> Result<Option<String>, String> {
    let key_response = match client
        .get(format!("{}/login", base_url))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            warn!("Failed to request encryption key: {}", e);
            return Ok(None);
        }
    };

    if !key_response.status().is_success() {
        warn!("Encryption key endpoint returned status {}", key_response.status());
        return Ok(None);
    }

    let key_body = match key_response.text().await {
        Ok(body) => body,
        Err(e) => {
            warn!("Failed to read encryption key response: {}", e);
            return Ok(None);
        }
    };

    let key_json: serde_json::Value = match serde_json::from_str(&key_body) {
        Ok(json) => json,
        Err(e) => {
            warn!("Invalid key response JSON: {}", e);
            return Ok(None);
        }
    };

    let public_key_pem = match key_json.get("publicKey").and_then(|k| k.as_str()) {
        Some(value) => value,
        None => {
            warn!("Public key missing in key response");
            return Ok(None);
        }
    };

    let rsa_key = match RsaPublicKey::from_public_key_pem(public_key_pem) {
        Ok(key) => key,
        Err(e) => {
            warn!("Invalid public key: {}", e);
            return Ok(None);
        }
    };

    let encrypted = match rsa_key.encrypt(&mut OsRng, Oaep::new::<Sha256>(), password.as_bytes()) {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!("Password encryption failed: {}", e);
            return Ok(None);
        }
    };

    Ok(Some(base64::engine::general_purpose::STANDARD.encode(encrypted)))
}
