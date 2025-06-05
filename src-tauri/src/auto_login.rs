use reqwest::Client;
use std::env;
use dotenvy::dotenv;
use serde::Deserialize;
use mac_address::get_mac_address;
use crate::token_utils::{read_tokens_from_file, save_tokens_to_file};

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
pub async fn auto_login_lambda() -> Result<bool, String> {
    println!("Starting auto-login process...");
    dotenv().ok();

    // Retrieve MAC address
    let mac_address = get_mac_address()
        .ok()
        .flatten()
        .map(|mac| mac.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Create device info
    let device_info = serde_json::json!({
        "device_type": "desktop", 
        "os": env::consts::OS,
        "mac_address": mac_address
    });

    // Read tokens from file
    let (access_token, refresh_token) = read_tokens_from_file()
        .map_err(|e| format!("Failed to read tokens: {}", e))?;

    let api_key = env::var("API_KEY").map_err(|e| e.to_string())?;
    let url = "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod/autologin";
    let client = Client::new();

    // Prepare the initial payload with access token and device info
    let mut payload = serde_json::json!({
        "body": serde_json::json!({
            "access_token": access_token,
            "deviceInfo": device_info
        }).to_string()
    });

    let mut response = client
        .post(url)
        .header("x-api-key", api_key.clone())
        .header("Content-Type", "application-desktop/json")
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
                    .post(url)
                    .header("x-api-key", api_key.clone())
                    .header("Content-Type", "application-desktop/json")
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
                        save_tokens_to_file(&new_access_token, &refresh_token)
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