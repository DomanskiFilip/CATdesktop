use crate::api_utils::AppConfig;
use base64::Engine;
use log::warn;
use reqwest::Client;
use rsa::{pkcs8::DecodePublicKey, rand_core::OsRng, Oaep, RsaPublicKey};
use serde::Deserialize;
use sha2::Sha256;

// Structs (classes/objects) to deserialize the Lambda response
#[derive(Deserialize)]
struct LambdaResponse {
    status_code: u16,
    body: String,
}

#[derive(Deserialize)]
struct Body {
    _status: String,
    _message: String,
}

// Function to register a user using AWS Lambda //
pub async fn register_user_lambda(email: String, password: String) -> Result<String, String> {
    let config = AppConfig::new()?;

    let url = format!("{}/register", config.lambda_base_url);
    let client = Client::new();
    let encrypted_password = encrypt_password_for_transport(&client, &config.lambda_base_url, &password).await?;
    let user_data = serde_json::json!({
        "email": &email,
        "password": encrypted_password
    });
    let payload = serde_json::json!({
        "body": user_data.to_string()
    });
    // send and handle register query
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = response.text().await.map_err(|e| e.to_string())?;

    // Check for Sandbox.Timedout error in the raw response
    if text.contains("\"errorType\":\"Sandbox.Timedout\"") {
        let frontend_response = serde_json::json!({
            "status": "error",
            "message": "Server timeout, please try again.",
        });
        return Err(frontend_response.to_string());
    }

    // Check for API Gateway errors (like Forbidden)
    if text.contains("\"message\":\"Forbidden\"") || text == "{\"message\":\"Forbidden\"}" {
        let frontend_response = serde_json::json!({
            "status": "error",
            "message": "Access denied!",
        });
        return Err(frontend_response.to_string());
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

    // Pass status and message to frontend as JSON string
    let frontend_response = serde_json::json!({
        "status": "ok",
        "message": &lambda_resp.body,
    });

    Ok(frontend_response.to_string())
}

async fn encrypt_password_for_transport(client: &Client, base_url: &str, password: &str) -> Result<String, String> {
    match try_encrypt_with_server_key(client, base_url, password).await? {
        Some(cipher) => Ok(cipher),
        None => {
            warn!("Falling back to plaintext password payload (could not encrypt)");
            Ok(password.to_string())
        }
    }
}

async fn try_encrypt_with_server_key(
    client: &Client,
    base_url: &str,
    password: &str,
) -> Result<Option<String>, String> {
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
