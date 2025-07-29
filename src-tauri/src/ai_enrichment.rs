use crate::ai_assistant::LambdaResponse;
use crate::api_utils::{ get_device_info, AppConfig };
use crate::database_utils::CalendarEvent;
use crate::get_weekly_weather;
use crate::token_utils::read_tokens_from_file;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::{ get_current_user_id };
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::{ get_current_user_id_mobile };
use chrono::Utc;
use serde::{ Deserialize, Serialize };
use tauri::{ AppHandle, Manager };

#[derive(Debug, Serialize, Deserialize)]
pub struct EnrichmentResponse {
    pub response_text: String,
    pub event_type: Option<String>,
    pub location: Option<String>,
    pub time: Option<String>,
    pub info_needed: Option<Vec<String>>,
    pub confidence: Option<f64>,
}

pub struct AIEnrichmentService;

impl AIEnrichmentService {
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

    async fn send_lambda_request(&self, url: &str, payload: &serde_json::Value,) -> Result<EnrichmentResponse, String> {
        let client = reqwest::Client::new();
        let resp = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("Failed to call Lambda: {}", e))?;

        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read Lambda response: {}", e))?;

        let lambda_resp: LambdaResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;

        let enrichment: EnrichmentResponse = serde_json::from_str(&lambda_resp.body)
            .map_err(|e| format!("Failed to parse enrichment response: {}", e))?;

        println!("Enrichment response: {:?}", &enrichment);

        Ok(enrichment)
    }

    pub async fn enrich_event(&self, app_handle: &AppHandle, event: CalendarEvent,) -> Result<EnrichmentResponse, String> {
        let (user_id, device_info, lambda_base_url, current_time, weather) =
            self.get_context(app_handle, &event).await?;

        let mut payload = serde_json::json!({
            "request_type": "event_enrichment",
            "event": event,
            "current_time": current_time,
            "weather_forecast": weather,
            "access_token": "",
            "deviceInfo": device_info,
            "email": user_id,
        });

        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle).await {
            payload["access_token"] = serde_json::json!(access_token);
        }

        let url = format!("{}/llm", lambda_base_url);
        self.send_lambda_request(&url, &payload).await
    }

    pub async fn enrichment_followup(&self, app_handle: &AppHandle, event: CalendarEvent, user_additional_info: String, clarification_history: Option<String>,) -> Result<EnrichmentResponse, String> {
        let (user_id, device_info, lambda_base_url, current_time, weather) =
            self.get_context(app_handle, &event).await?;

        let mut payload = serde_json::json!({
            "request_type": "enrichment_followup",
            "event": event,
            "current_time": current_time,
            "weather_forecast": weather,
            "user_additional_info": user_additional_info,
            "clarification_history": clarification_history,
            "access_token": "",
            "deviceInfo": device_info,
            "email": user_id,
        });

        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle).await {
            payload["access_token"] = serde_json::json!(access_token);
        }

        let url = format!("{}/llm", lambda_base_url);
        self.send_lambda_request(&url, &payload).await
    }
}
