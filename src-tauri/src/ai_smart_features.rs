use crate::ai_assistant::LambdaResponse;
use crate::api_utils::{get_device_info, AppConfig};
use crate::database_utils::CalendarEvent;
use crate::get_weekly_weather;
use crate::token_utils::read_tokens_from_file;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize)]
pub struct AISmartFeaturesResponse {
    pub response_text: Option<String>,
    pub event_type: Option<String>,
    pub location: Option<String>,
    pub time: Option<String>,
    pub info_needed: Option<Vec<String>>,
    pub email_subject: Option<String>,
    pub participants: Option<Vec<String>>,
    pub confidence: Option<f64>,
    pub remaining_requests: Option<i32>,
}

pub struct AISmartFeaturesService;

impl AISmartFeaturesService {
    pub fn new() -> Self {
        Self
    }

    async fn get_context(&self, app_handle: &AppHandle, event: &CalendarEvent,) -> Result<(String, serde_json::Value, String, String, String), String> {
        // Get user ID
        let user_id: String = {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                match get_current_user_id(&app_handle) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok((
                            String::new(),
                            serde_json::Value::Null,
                            String::new(),
                            String::new(),
                            String::new(),
                        ));
                    }
                }
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                match get_current_user_id_mobile().await {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok((
                            String::new(),
                            serde_json::Value::Null,
                            String::new(),
                            String::new(),
                            String::new(),
                        ));
                    }
                }
            }
        };
        let device_info = get_device_info(&app_handle);
        let config = AppConfig::new()?;
        let current_time = Utc::now().to_rfc3339();

        // fetch weather for event date
        let weather = {
            let event_date = event.time.date_naive();
            let user_location_state = app_handle.state::<tokio::sync::Mutex<crate::UserLocation>>();
            let loc = user_location_state.lock().await;
            let weather_map = get_weekly_weather(app_handle.clone(), loc.latitude, loc.longitude)
                .await
                .map_err(|e| format!("Failed to fetch weather: {}", e))?;
            let key = event_date.format("%Y-%m-%d").to_string();
            weather_map
                .get(&key)
                .map(|w| w.weather.clone())
                .unwrap_or_else(|| "No weather data available.".to_string())
        };

        Ok((
            user_id,
            device_info,
            config.lambda_base_url,
            current_time,
            weather,
        ))
    }

    async fn send_lambda_request(&self, url: &str, payload: &serde_json::Value,) -> Result<AISmartFeaturesResponse, String> {
        let client = reqwest::Client::new();
        let resp = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("Failed to call Lambda: {}", e))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read Lambda response: {}", e))?;

        // Handle rate limiting (429 status code)
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            // Parse the rate limit error response
            let rate_limit_error: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| format!("Failed to parse rate limit response: {}", e))?;
            
            let error_message = rate_limit_error["message"]
                .as_str()
                .unwrap_or("Daily AI request limit exceeded. Please try again tomorrow :3. 😽");
            
            let remaining_requests = rate_limit_error["remaining_requests"]
                .as_i64()
                .unwrap_or(0) as i32;

            return Ok(AISmartFeaturesResponse {
                response_text: Some(format!("🚫 {}", error_message)),
                event_type: None,
                location: None,
                time: None,
                info_needed: None,
                email_subject: None,
                participants: None,
                confidence: Some(1.0),
                remaining_requests: Some(remaining_requests),
            });
        }

        // Handle other HTTP errors
        if !status.is_success() {
            return Err(format!("Lambda request failed with status {}: {}", status, text));
        }

        let lambda_resp: LambdaResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;

        // Check if the Lambda response contains a rate limit error (status_code: 429 in body)
        if let Ok(lambda_body) = serde_json::from_str::<serde_json::Value>(&lambda_resp.body) {
            if let Some(status_code) = lambda_body.get("status_code").and_then(|v| v.as_u64()) {
                if status_code == 429 {
                    let error_message = lambda_body["message"]
                        .as_str()
                        .unwrap_or("Daily AI request limit exceeded. Please try again tomorrow :3. 😽");
                    
                    let remaining_requests = lambda_body["remaining_requests"]
                        .as_i64()
                        .unwrap_or(0) as i32;

                    return Ok(AISmartFeaturesResponse {
                        response_text: Some(format!("🚫 {}", error_message)),
                        event_type: None,
                        location: None,
                        time: None,
                        info_needed: None,
                        email_subject: None,
                        participants: None,
                        confidence: Some(1.0),
                        remaining_requests: Some(remaining_requests),
                    });
                }
            }
        }

        let mut enrichment: AISmartFeaturesResponse = serde_json::from_str(&lambda_resp.body)
            .map_err(|e| format!("Failed to parse smart features response: {}", e))?;

        println!("Smart features response: {:?}", &enrichment);

        // If all fields are None/empty and remaining_requests is 0, it's likely a rate limit
        if enrichment.response_text.is_none() && 
          enrichment.event_type.is_none() && 
          enrichment.location.is_none() && 
          enrichment.time.is_none() && 
          enrichment.info_needed.is_none() && 
          enrichment.email_subject.is_none() && 
          enrichment.participants.is_none() && 
          enrichment.confidence.is_none() && 
          enrichment.remaining_requests == Some(0) {
            
            enrichment.response_text = Some("🚫 Daily AI request limit exceeded. You can make 25 requests per day. Please try again tomorrow. :3 😽".to_string());
            enrichment.confidence = Some(1.0);
        }

        Ok(enrichment)
    }

    pub async fn generate_email(&self, app_handle: &AppHandle, event: CalendarEvent, email_topic: String, participants: Vec<String>) -> Result<AISmartFeaturesResponse, String> {
        let (user_id, device_info, lambda_base_url, current_time, weather) = self.get_context(app_handle, &event).await?;

        let mut payload = serde_json::json!({
            "request_type": "email_agent",
            "event": event,
            "current_time": current_time,
            "weather_forecast": weather,
            "email_topic": email_topic,
            "participants": participants,
            "access_token": "",
            "deviceInfo": device_info,
            "user_id": user_id,
        });

        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle).await {
            payload["access_token"] = serde_json::json!(access_token);
        }

        let url = format!("{}/llm", lambda_base_url);
        self.send_lambda_request(&url, &payload).await
    }

    pub async fn enrich_event(&self, app_handle: &AppHandle, event: CalendarEvent,) -> Result<AISmartFeaturesResponse, String> {
        let (user_id, device_info, lambda_base_url, current_time, weather) = self.get_context(app_handle, &event).await?;

        let mut payload = serde_json::json!({
            "request_type": "event_enrichment",
            "event": event,
            "current_time": current_time,
            "weather_forecast": weather,
            "access_token": "",
            "deviceInfo": device_info,
            "user_id": user_id,
        });

        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle).await {
            payload["access_token"] = serde_json::json!(access_token);
        }

        let url = format!("{}/llm", lambda_base_url);
        self.send_lambda_request(&url, &payload).await
    }

    pub async fn enrichment_followup(&self, app_handle: &AppHandle, event: CalendarEvent, user_additional_info: String, clarification_history: Option<String>,) -> Result<AISmartFeaturesResponse, String> {
        let (user_id, device_info, lambda_base_url, current_time, weather) = self.get_context(app_handle, &event).await?;

        let mut payload = serde_json::json!({
            "request_type": "enrichment_followup",
            "event": event,
            "current_time": current_time,
            "weather_forecast": weather,
            "user_additional_info": user_additional_info,
            "clarification_history": clarification_history,
            "access_token": "",
            "deviceInfo": device_info,
            "user_id": user_id,
        });

        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle).await {
            payload["access_token"] = serde_json::json!(access_token);
        }

        let url = format!("{}/llm", lambda_base_url);
        self.send_lambda_request(&url, &payload).await
    }
}
