use reqwest::Client;
use serde_json::{ Value, json };
use tauri::{ AppHandle, Manager };
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use crate::api_utils::{ get_device_info, AppConfig };
use crate::token_utils::read_tokens_from_file;

pub async fn fetch_google_credentials(app_handle: &AppHandle) -> Result<Value, String> {  
    // Load config
    let config = AppConfig::new().map_err(|e| format!("Failed to load config: {}", e))?;
    
    let google_cred_url = format!("{}/google_creds", config.lambda_base_url);
    // Get user ID
    let user_id: String = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(app_handle) {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Err(format!("Failed to get user ID: {}", e));
                }
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            match get_current_user_id_mobile().await {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Err(format!("Failed to get user ID: {}", e));
                }
            }
        }
    };
    
    let device_info = get_device_info(app_handle);

    let access_token = match read_tokens_from_file(app_handle).await {
        Ok((access_token, _, _)) => access_token,
        Err(e) => {
            println!("Failed to read tokens: {}", e);
            return Err(format!("Authentication required. Please log in first: {}", e));
        }
    };

    // Construct the payload
    let mut payload = json!({
        "accessToken": access_token,
        "deviceInfo": device_info,
        "email": user_id
    });

    let client = Client::new();
    let response = client
        .post(&google_cred_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    let text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;
    
    let response_json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
    
    // Parse the nested body JSON string
    let body_str = response_json
        .get("body")
        .and_then(|v| v.as_str())
        .ok_or("No body in response".to_string())?;
    
    let body_json: Value = serde_json::from_str(body_str)
        .map_err(|e| format!("Failed to parse body JSON: {}", e))?;
    
    body_json
        .get("credentials")
        .cloned()
        .ok_or("No credentials in response".to_string())
}

pub async fn fetch_outlook_credentials(app_handle: &AppHandle) -> Result<Value, String> {
    // Load config
    let config = AppConfig::new().map_err(|e| format!("Failed to load config: {}", e))?;
    
    let google_cred_url = format!("{}/outlook_creds", config.lambda_base_url);
    // Get user ID
    let user_id: String = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(app_handle) {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Err(format!("Failed to get user ID: {}", e));
                }
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            match get_current_user_id_mobile().await {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Err(format!("Failed to get user ID: {}", e));
                }
            }
        }
    };
    
    let device_info = get_device_info(app_handle);

    let access_token = match read_tokens_from_file(app_handle).await {
        Ok((access_token, _, _)) => access_token,
        Err(e) => {
            println!("Failed to read tokens: {}", e);
            return Err(format!("Authentication required. Please log in first: {}", e));
        }
    };

    // Construct the payload
    let mut payload = json!({
        "accessToken": access_token,
        "deviceInfo": device_info,
        "email": user_id
    });

    let client = Client::new();
    let response = client
        .post(&google_cred_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    let text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;
    
    let response_json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
    
    // Parse the nested body JSON string
    let body_str = response_json
        .get("body")
        .and_then(|v| v.as_str())
        .ok_or("No body in response".to_string())?;
    
    let body_json: Value = serde_json::from_str(body_str)
        .map_err(|e| format!("Failed to parse body JSON: {}", e))?;
    
    body_json
        .get("credentials")
        .cloned()
        .ok_or("No credentials in response".to_string())
}