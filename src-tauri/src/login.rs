use reqwest::Client;
use serde::Deserialize;
use tauri::AppHandle;
use crate::token_utils::save_tokens_to_file;
use crate::api_utils::{AppConfig, get_device_info};


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
pub async fn login_user_lambda(app_handle: &AppHandle, email: String, password: String) -> Result<String, String> {
    let config = AppConfig::new()?;
    let device_info = get_device_info();

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
        .header("x-api-key", config.api_key)
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
        let error_body: serde_json::Value = serde_json::from_str(&lambda_resp.body).map_err(|e| e.to_string())?;
        let error_message = error_body["message"].as_str().unwrap_or("Unknown error");

        let frontend_response = serde_json::json!({
            "status": "error",
            "message": error_message,
        });
        return Err(frontend_response.to_string());
    }

    let body: Body = serde_json::from_str(&lambda_resp.body).map_err(|e| e.to_string())?;

    // Save tokens to an encrypted file
    save_tokens_to_file(&app_handle, &body.access_token, &body.refresh_token,).map_err(|e| format!("Failed to save tokens: {}", e))?;

    // Pass status to frontend
    let frontend_response = serde_json::json!({
        "status": "ok",
    });
    
    Ok(frontend_response.to_string())
}