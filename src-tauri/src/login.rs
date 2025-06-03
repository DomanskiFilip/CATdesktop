use reqwest::Client;
use std::env;
use dotenvy::dotenv;
use serde::Deserialize;
use mac_address::get_mac_address;

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
    access_token_expires_in: u64,
    refresh_token_expires_in: u64,
}

// Function to log in a user using AWS Lambda
pub async fn login_user_lambda(email: String, password: String) -> Result<String, String> {
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

    // create login query
    let api_key = env::var("API_KEY").map_err(|e| e.to_string())?;
    let url = "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod/login";
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
        .header("x-api-key", api_key)
        .header("Content-Type", "application-desktop/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    println!("Raw response text: {}", text);
    
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

    // Pass status and message to frontend as JSON string
    let frontend_response = serde_json::json!({
        "status": "ok",
        "access_token": body.access_token,
        "access_token_expires_in": body.access_token_expires_in, 
        "refresh_token_expires_in": body.refresh_token_expires_in,
    });
    
    Ok(frontend_response.to_string())
}