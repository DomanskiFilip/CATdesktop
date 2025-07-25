use crate::ai_assistant::LambdaResponse;
use crate::database_utils::CalendarEvent;
use crate::user_utils::get_current_user_id;
use crate::api_utils::{ AppConfig, get_device_info };
use crate::token_utils::read_tokens_from_file;
use crate::get_weekly_weather;
use tauri::{ AppHandle, Manager };
use serde::{ Deserialize, Serialize };
use chrono::{ Utc };

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

    async fn get_context(&self, app_handle: &AppHandle, event: &CalendarEvent,) -> Result<(String, serde_json::Value, String, String, String, String), String> {
        let user_id = get_current_user_id(app_handle)?;
        let device_info = get_device_info();
        let config = AppConfig::new()?;
        let api_key = config.api_key.clone();
        let current_time = Utc::now().to_rfc3339();

        // fetch weather for event date
        let weather = {
            let event_date = event.time.date_naive();
            let user_location_state = app_handle.state::<tokio::sync::Mutex<crate::UserLocation>>();
            let loc = user_location_state.lock().await;
            let weather_map = get_weekly_weather(app_handle.clone(), loc.latitude, loc.longitude).await
                .map_err(|e| format!("Failed to fetch weather: {}", e))?;
            let key = event_date.format("%Y-%m-%d").to_string();
            weather_map.get(&key)
                .map(|w| w.weather.clone())
                .unwrap_or_else(|| "No weather data available.".to_string())
        };

        Ok((user_id, device_info, config.lambda_base_url, api_key, current_time, weather))
    }

    async fn send_lambda_request(&self, url: &str, api_key: &str, payload: &serde_json::Value,) -> Result<EnrichmentResponse, String> {
        let client = reqwest::Client::new();
        let resp = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("x-api-key", api_key)
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("Failed to call Lambda: {}", e))?;

        let text = resp.text().await
            .map_err(|e| format!("Failed to read Lambda response: {}", e))?;

        let lambda_resp: LambdaResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;

        let enrichment: EnrichmentResponse = serde_json::from_str(&lambda_resp.body)
            .map_err(|e| format!("Failed to parse enrichment response: {}", e))?;

        println!("Enrichment response: {:?}", &enrichment);

        Ok(enrichment)
    }

    pub async fn enrich_event(&self, app_handle: &AppHandle, event: CalendarEvent) -> Result<EnrichmentResponse, String> {
        let (user_id, device_info, lambda_base_url, api_key, current_time, weather) =
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

        if let Ok((access_token, _)) = read_tokens_from_file(app_handle) {
            payload["access_token"] = serde_json::json!(access_token);
        }

        let url = format!("{}/llm", lambda_base_url);
        self.send_lambda_request(&url, &api_key, &payload).await
    }

    pub async fn enrichment_followup(&self, app_handle: &AppHandle, event: CalendarEvent, user_additional_info: String, clarification_history: Option<String>,) -> Result<EnrichmentResponse, String> {
        let (user_id, device_info, lambda_base_url, api_key, current_time, weather) =
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

        if let Ok((access_token, _)) = read_tokens_from_file(app_handle) {
            payload["access_token"] = serde_json::json!(access_token);
        }

        let url = format!("{}/llm", lambda_base_url);
        self.send_lambda_request(&url, &api_key, &payload).await
    }
}