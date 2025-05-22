use reqwest::Client;
use std::env;
use dotenvy::dotenv;
use serde::Deserialize;

#[derive(Deserialize)]
struct LambdaResponse {
    statusCode: u16,
    body: String,
}

#[derive(Deserialize)]
struct Body {
    status: String,
    message: String,
}

pub async fn register_user_lambda(email: String, password: String) -> Result<String, String> {
    if email.trim().is_empty() || password.trim().is_empty() {
        return Err("Email and password must not be empty".to_string());
    }
    dotenv().ok();
    let api_key = env::var("API_KEY").map_err(|e| e.to_string())?;
    let url = "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod/register";
    let client = Client::new();
    let user_data = serde_json::json!({
        "email": email,
        "password": password
    });
    let payload = serde_json::json!({
        "body": user_data.to_string()
    });
    let response = client
        .post(url)
        .header("x-api-key", api_key)
        .header("Content-Type", "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    println!("Lambda response: {}", text);

    // Parse Lambda response
    let lambda_resp: LambdaResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let body: Body = serde_json::from_str(&lambda_resp.body).map_err(|e| e.to_string())?;

    // Pass status and message to frontend as JSON string
    let frontend_response = serde_json::json!({
        "status": body.status,
        "message": body.message,
        "statusCode": lambda_resp.statusCode
    });

    if body.status == "error" {
        return Err(frontend_response.to_string());
    }
    Ok(frontend_response.to_string())
}