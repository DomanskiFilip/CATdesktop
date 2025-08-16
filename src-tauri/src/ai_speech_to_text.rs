use crate::api_utils::{get_device_info, AppConfig};
use crate::auto_login::auto_login_lambda;
use crate::logout_user;
use crate::token_utils::read_tokens_from_file;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[derive(Debug, Serialize, Deserialize)]
pub struct SpeechToTextRequest {
    pub audio_data: String, // Base64 encoded audio
    pub format: String,     // Audio format (webm, mp3, wav, etc.)
    pub user_id: String,
    pub access_token: String,
    #[serde(rename = "deviceInfo")]
    pub device_info: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpeechToTextResponse {
    pub transcription: String,
    pub confidence: f64,
    pub language: String,
    pub remaining_requests: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpeechErrorResponse {
    pub error: String,
    pub message: String,
    pub remaining_requests: Option<i32>,
}

#[tauri::command]
pub async fn transcribe_audio(app_handle: AppHandle, audio_data: Vec<u8>, format: String) -> Result<String, String> {
    // Get user ID and tokens
    let user_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match crate::user_utils::get_current_user_id(&app_handle) {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Err("You are not logged in. Please log in to use speech-to-text.".to_string());
                }
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            match crate::user_utils::get_current_user_id_mobile().await {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Err("You are not logged in. Please log in to use speech-to-text.".to_string());
                }
            }
        }
    };

    let device_info = get_device_info(&app_handle);
    let config = AppConfig::new()?;
    let url = format!("{}/speech-to-text", config.lambda_base_url);

    // Encode audio data to base64
    let audio_base64 = BASE64.encode(&audio_data);

    // Prepare request with tokens
    let mut request_payload = SpeechToTextRequest {
        audio_data: audio_base64,
        format,
        user_id: user_id.clone(),
        access_token: String::new(),
        device_info,
    };

    // Read tokens
    if let Ok((access_token, _, _)) = read_tokens_from_file(&app_handle).await {
        request_payload.access_token = access_token;
    } else {
        return Err("No valid authentication found. Please log in.".to_string());
    }

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request_payload)
        .send()
        .await
        .map_err(|e| format!("Failed to call speech-to-text endpoint: {}", e))?;

    let text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;
    println!("Raw Response: {}", text);

    // handle response
    if let Ok(lambda_wrapper) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(status_code) = lambda_wrapper.get("status_code").and_then(|s| s.as_u64()) {
            let body_str = lambda_wrapper.get("body")
                .and_then(|b| b.as_str())
                .unwrap_or("{}");
            
            match status_code {
                200 => {
                    // Success - parse the response
                    match serde_json::from_str::<SpeechToTextResponse>(body_str) {
                        Ok(speech_response) => {
                            return Ok(serde_json::to_string(&speech_response)
                                .map_err(|e| format!("Failed to serialize response: {}", e))?);
                        }
                        Err(_) => {
                            // Fallback parsing for the body
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(body_str) {
                                let transcription = v.get("transcription")
                                    .and_then(|t| t.as_str())
                                    .or_else(|| v.get("transcript").and_then(|t| t.as_str()))
                                    .or_else(|| v.get("text").and_then(|t| t.as_str()));
                                let confidence = v.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0);
                                let language = v.get("language").and_then(|l| l.as_str()).unwrap_or("en-US").to_string();
                                let remaining = v.get("remaining_requests").and_then(|r| r.as_i64()).map(|x| x as i32);

                                if let Some(text) = transcription {
                                    let fallback = SpeechToTextResponse {
                                        transcription: text.to_string(),
                                        confidence,
                                        language,
                                        remaining_requests: remaining,
                                    };
                                    return Ok(serde_json::to_string(&fallback)
                                        .map_err(|e| format!("Failed to serialize fallback response: {}", e))?);
                                }
                            }
                            return Err("Failed to parse success response body".to_string());
                        }
                    }
                }
                401 => {
                    // Unauthorized - try auto-login and retry
                    if auto_login_lambda(&app_handle).await.unwrap_or(false) {
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                        
                        if let Ok((access_token, _, _)) = read_tokens_from_file(&app_handle).await {
                            request_payload.access_token = access_token;
                            
                            let retry_response = client
                                .post(&url)
                                .header("Content-Type", "application/json")
                                .json(&request_payload)
                                .send()
                                .await
                                .map_err(|e| format!("Failed to retry speech-to-text request: {}", e))?;
                            
                            let retry_text = retry_response
                                .text()
                                .await
                                .map_err(|e| format!("Failed to read retry response: {}", e))?;
                            
                            if let Ok(retry_wrapper) = serde_json::from_str::<serde_json::Value>(&retry_text) {
                                if let Some(retry_status) = retry_wrapper.get("status_code").and_then(|s| s.as_u64()) {
                                    match retry_status {
                                        200 => {
                                            let retry_body = retry_wrapper.get("body")
                                                .and_then(|b| b.as_str())
                                                .unwrap_or("{}");
                                            
                                            let speech_response: SpeechToTextResponse = serde_json::from_str(retry_body)
                                                .map_err(|e| format!("Failed to parse retry response: {}", e))?;
                                            
                                            return Ok(serde_json::to_string(&speech_response)
                                                .map_err(|e| format!("Failed to serialize retry response: {}", e))?);
                                        }
                                        401 => {
                                            let _ = logout_user(app_handle).await;
                                            return Err("Session expired. Please log in again.".to_string());
                                        }
                                        429 => {
                                            let retry_body = retry_wrapper.get("body")
                                                .and_then(|b| b.as_str())
                                                .unwrap_or("{}");
                                            let error_response: SpeechErrorResponse = serde_json::from_str(retry_body)
                                                .map_err(|e| format!("Failed to parse rate limit response: {}", e))?;
                                            return Err(format!("RATE_LIMIT: {}", error_response.message));
                                        }
                                        _ => {
                                            // Other error status codes
                                            let error_response: SpeechErrorResponse = serde_json::from_str(body_str)
                                                .unwrap_or(SpeechErrorResponse {
                                                    error: "Unknown error".to_string(),
                                                    message: "An unexpected error occurred".to_string(),
                                                    remaining_requests: None,
                                                });
                                            return Err(error_response.message);
                                        }
                                    }
                                } else {
                                    // No status code found in response
                                    return Err("Invalid response format: missing status code".to_string());
                                }
                            } else {
                                // Failed to parse JSON response
                                return Err("Failed to parse response as JSON".to_string());
                            }
                        } else {
                            // Failed to read tokens after auto-login
                            let _ = logout_user(app_handle).await;
                            return Err("Failed to refresh authentication.".to_string());
                        }
                    } else {
                        // Auto-login failed
                        let _ = logout_user(app_handle).await;
                        return Err("Session expired. Please log in again.".to_string());
                    }
                }
                429 => {
                    // Rate limit exceeded
                    let error_response: SpeechErrorResponse = serde_json::from_str(body_str)
                        .map_err(|e| format!("Failed to parse rate limit response: {}", e))?;
                    return Err(format!("RATE_LIMIT: {}", error_response.message));
                }
                500 => {
                    // Server error - parse the error response
                    println!("Lambda returned 500 error: {}", body_str);
                    let error_response: SpeechErrorResponse = serde_json::from_str(body_str)
                        .unwrap_or(SpeechErrorResponse {
                            error: "server_error".to_string(),
                            message: "Speech-to-text service is temporarily unavailable. Please try again.".to_string(),
                            remaining_requests: None,
                        });
                    return Err(format!("SERVER_ERROR: {}", error_response.message));
                }
                _ => {
                    // Other error status codes
                    let error_response: SpeechErrorResponse = serde_json::from_str(body_str)
                        .unwrap_or(SpeechErrorResponse {
                            error: "Unknown error".to_string(),
                            message: "An unexpected error occurred".to_string(),
                            remaining_requests: None,
                        });
                    return Err(error_response.message);
                }
            }
        } else {
            // No status code found in response
            return Err("Invalid response format: missing status code".to_string());
        }
    } else {
        // Failed to parse JSON response
        return Err("Failed to parse response as JSON".to_string());
    }
}