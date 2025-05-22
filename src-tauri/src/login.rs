use reqwest::Client;
use std::env;
use dotenvy::dotenv;

pub async fn login_user_lambda(email: String, password: String) -> Result<String, String> {
    dotenv().ok();
    let api_key = env::var("API_KEY").map_err(|e| e.to_string())?;
    let url = "https://ywaixwivt3.execute-api.eu-west-2.amazonaws.com/prod/login";
    let client = Client::new();
    let body = serde_json::json!({ "email": email, "password": password });
    let response = client
        .post(url)
        .header("x-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = response.text().await.map_err(|e| e.to_string())?;
    Ok(text)
}