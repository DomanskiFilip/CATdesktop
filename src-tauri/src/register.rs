use reqwest::Client;
use serde::Deserialize;
use crate::config::AppConfig;

// Structs (classes/objects) to deserialize the Lambda response
#[derive(Deserialize)]
struct LambdaResponse {
    status_code: u16,
    body: String,
}

#[derive(Deserialize)]
struct Body {
    status: String,
    message: String,
}

// Function to register a user using AWS Lambda
pub async fn register_user_lambda(email: String, password: String) -> Result<String, String> {
    let config = AppConfig::new()?;
    
    let url = format!("{}/register", config.lambda_base_url);
    let client = Client::new();
    let user_data = serde_json::json!({
        "email": email,
        "password": password
    });
    let payload = serde_json::json!({
        "body": user_data.to_string()
    });
    // send and handle register query
    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("x-api-key", config.api_key)
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;

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
        "message": body.message,
    });
    
    Ok(frontend_response.to_string())
}