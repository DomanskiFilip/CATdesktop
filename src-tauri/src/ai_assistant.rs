use crate::api_utils::{get_device_info, AppConfig};
use crate::auto_login::auto_login_lambda;
use crate::database_utils::{get_events, CalendarEvent};
use crate::logout_user;
use crate::token_utils::read_tokens_from_file;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use crate::ConversationMessage;
use crate::{get_weekly_weather, UserLocation};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use rand::Rng;
use serde::Deserializer;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Deserialize)]
pub struct LambdaResponse {
    pub status_code: u16,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMResponse {
    pub response_text: String,
    pub extracted_events: Option<Vec<ExtractedEvent>>,
    pub action_taken: Option<String>,
    pub confidence: Option<f64>,
    pub remaining_requests: Option<i32>, 
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEvent {
    pub target_event_id: Option<String>,
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_event_time")]
    pub time: Option<DateTime<Local>>,
    pub alarm: bool,
    pub recurrence: Option<String>,
}

// Custom deserializer for ExtractedEvent.time
fn deserialize_event_time<'de, D>(deserializer: D) -> Result<Option<DateTime<Local>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    if let Some(time_str) = s {
        // Try RFC3339 first
        if let Ok(dt) = DateTime::parse_from_rfc3339(&time_str) {
            return Ok(Some(dt.with_timezone(&Local)));
        }
        // Try naive local time (e.g., "2025-07-23T20:00:00")
        if let Ok(naive) = NaiveDateTime::parse_from_str(&time_str, "%Y-%m-%dT%H:%M:%S") {
            // Assume naive time is local
            return Ok(Some(Local.from_local_datetime(&naive).unwrap()));
        }
    }
    Ok(None)
}

pub struct AIAssistantService;

impl AIAssistantService {
    pub fn new() -> Self {
        Self
    }

    pub async fn process_user_query(&self, query: String, app_handle: &AppHandle, conversation_history: Option<Vec<ConversationMessage>>,) -> Result<LLMResponse, String> {
        println!("📝 User Query: {}", query);

        // Check for canned responses first
        if let Some(response) = self.get_canned_response(&query) {
            println!("🤖 Using canned response");
            return Ok(response);
        }

        // Create enhanced prompt with event IDs
        let location_state = app_handle.state::<tokio::sync::Mutex<UserLocation>>();
        let prompt = self
            .create_enhanced_prompt(&query, app_handle, conversation_history, location_state)
            .await?;

        let llm_response = self.invoke_lambda_endpoint(prompt, app_handle).await?;
        Ok(llm_response)
    }

        fn get_canned_response(&self, query: &str) -> Option<LLMResponse> {
        let lowercase_query = query.to_lowercase();
        let normalized_query = lowercase_query.trim();
        let mut rng = rand::rng();

        match normalized_query {
            q if (q == "hi" || q == "hello" || q == "hey" || q == "hi there" || q == "hi cat") => {
                let greetings = [
                    "Hi there! I'm CAT, your calendar assistant. How can I help with your schedule today?",
                    "Hello! I'm here to help manage your calendar. Need to schedule something?",
                    "Hey! I'm your calendar assistant. What can I do for you today?"
                ];
                
                let index = rng.random_range(0..greetings.len());
                let greeting = greetings[index];
                      
                Some(LLMResponse {
                    response_text: greeting.to_string(),
                    extracted_events: None,
                    action_taken: Some("none".to_string()),
                    confidence: Some(1.0),
                    remaining_requests: None,
                })
            },
            "what can you do" | "what can you do?" | "help" | "what are your features" => {
                Some(LLMResponse {
                    response_text: "I can help you manage your calendar by creating, updating, moving, and deleting events. I can also check your schedule, provide weather information, and set reminders. Just ask me things like 'Schedule a meeting tomorrow at 2pm', 'When's my next appointment?', 'Move my dentist appointment to Friday', or 'What's the weather like?'".to_string(),
                    extracted_events: None,
                    action_taken: Some("none".to_string()),
                    confidence: Some(1.0),
                    remaining_requests: None,
                })
            },
            _ => None,
        }
    }

    async fn create_enhanced_prompt(&self, query: &str, app_handle: &AppHandle, conversation_history: Option<Vec<ConversationMessage>>,  location_state: tauri::State<'_, tokio::sync::Mutex<UserLocation>>,) -> Result<serde_json::Value, String> {
        let recent_events = self.get_recent_events(app_handle).await?;

        // Format events with IDs for AI context
        let events_context = if recent_events.is_empty() {
            "You don't have any upcoming events scheduled.".to_string()
        } else {
            let events_formatted = recent_events
                .iter()
                .map(|event| {
                    let time_str = event.time.format("%Y-%m-%d %H:%M").to_string();
                    format!(
                        "- ID: {} | description: {} | time: {} | alarm: {}",
                        event.id, event.description, time_str, event.alarm
                    )
                })
                .collect::<Vec<String>>()
                .join("\n");

            format!("Your upcoming events:\n{}", events_formatted)
        };

        let conversation_context = if let Some(history) = conversation_history {
            if !history.is_empty() {
                history
                    .iter()
                    .map(|msg| format!("{}: {}", msg.sender, msg.content))
                    .collect::<Vec<String>>()
                    .join("\n")
            } else {
                "No previous conversation.".to_string()
            }
        } else {
            "No previous conversation.".to_string()
        };

        let loc = location_state.lock().await;
        let latitude = loc.latitude;
        let longitude = loc.longitude;

        let weather_map = get_weekly_weather(app_handle.clone(), latitude, longitude)
            .await
            .map_err(|e| format!("Failed to fetch weather: {}", e))?;

        let weather_forecast = if weather_map.is_empty() {
            "No weather data available.".to_string()
        } else {
            weather_map
                .iter()
                .map(|(date, daily)| {
                    format!(
                        "{}: {}, max temp: {:.1}°C, max wind: {:.1} km/h",
                        date, daily.weather, daily.temperature_2m_max, daily.wind_speed_10m_max
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let prompt_json = serde_json::json!({
            "weather_forecast": weather_forecast,
            "current_time": Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            "conversation_history": conversation_context,
            "events_context": events_context,
            "user_query": query,
        });

        Ok(prompt_json)
    }

    async fn invoke_lambda_endpoint(&self, prompt: serde_json::Value, app_handle: &AppHandle,) -> Result<LLMResponse, String> {
        // Get user ID
        let user_id: String = {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                match get_current_user_id(&app_handle) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(LLMResponse {
                            response_text:
                                "You are not logged in. Please log in to use the assistant."
                                    .to_string(),
                            extracted_events: None,
                            action_taken: None,
                            confidence: None,
                            remaining_requests: None,
                        });
                    }
                }
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                match get_current_user_id_mobile().await {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(LLMResponse {
                            response_text:
                                "You are not logged in. Please log in to use the assistant."
                                    .to_string(),
                            extracted_events: None,
                            action_taken: Some("none".to_string()),
                            confidence: None,
                            remaining_requests: None,
                        });
                    }
                }
            }
        };
        let device_info = get_device_info(&app_handle);

        let config = AppConfig::new()?;
        let url = format!("{}/llm", config.lambda_base_url);

        let client = reqwest::Client::new();
        let mut prompt_with_token = prompt.clone();
        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle).await {
            prompt_with_token["access_token"] = serde_json::json!(access_token);
            prompt_with_token["deviceInfo"] = device_info;
            prompt_with_token["email"] = serde_json::json!(user_id);
        }
        
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&prompt_with_token)
            .send()
            .await
            .map_err(|e| format!("Failed to call Lambda: {}", e))?;

        let text = resp.text().await.map_err(|e| format!("Failed to read Lambda response: {}", e))?;

        // Parse Lambda response for status_code
        let mut lambda_resp: LambdaResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;

        if lambda_resp.status_code == 429 {
            // Handle rate limiting
            let rate_limit_error: serde_json::Value = serde_json::from_str(&lambda_resp.body)
                .map_err(|e| format!("Failed to parse rate limit response: {}", e))?;
            
            let error_message = rate_limit_error["message"]
                .as_str()
                .unwrap_or("Daily AI request limit exceeded. You can make 25 requests per day. Please try again tomorrow. :3 😽");
            
            let remaining_requests = rate_limit_error["remaining_requests"]
                .as_i64()
                .unwrap_or(0) as i32;

            return Ok(LLMResponse {
                response_text: format!("🚫 {}", error_message),
                extracted_events: None,
                action_taken: Some("none".to_string()),
                confidence: Some(1.0),
                remaining_requests: Some(remaining_requests),
            });
        }
        
        // If access token is rejected (status_code 401), try auto-login
        if lambda_resp.status_code == 401 {
            // Try auto-login to refresh tokens
            if auto_login_lambda(app_handle).await.unwrap_or(false) {
                // Wait briefly to ensure token file is written
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                // Retry with new tokens
                if let Ok((access_token, _refresh_token, _)) =
                    read_tokens_from_file(app_handle).await
                {
                    prompt_with_token["access_token"] = serde_json::json!(access_token);
                    let retry_resp = client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .json(&prompt_with_token)
                        .send()
                        .await
                        .map_err(|e| format!("Failed to call Lambda after auto-login: {}", e))?;
                    
                    let retry_status = retry_resp.status();
                    let retry_text = retry_resp.text().await.map_err(|e| {
                        format!("Failed to read Lambda response after auto-login: {}", e)
                    })?;

                    // Check for rate limiting on retry as well
                    if retry_status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        let rate_limit_error: serde_json::Value = serde_json::from_str(&retry_text)
                            .map_err(|e| format!("Failed to parse rate limit response on retry: {}", e))?;
                        
                        let error_message = rate_limit_error["message"]
                            .as_str()
                            .unwrap_or("Daily AI request limit exceeded. You can make 25 requests per day. Please try again tomorrow. :3 😽");
                        
                        let remaining_requests = rate_limit_error["remaining_requests"]
                            .as_i64()
                            .unwrap_or(0) as i32;

                        return Ok(LLMResponse {
                            response_text: format!("🚫 {}", error_message),
                            extracted_events: None,
                            action_taken: Some("none".to_string()),
                            confidence: Some(1.0),
                            remaining_requests: Some(remaining_requests),
                        });
                    }

                    lambda_resp = serde_json::from_str(&retry_text).map_err(|e| {
                        format!("Failed to parse Lambda response after auto-login: {}", e)
                    })?;
                    // If still unauthorized, force logout
                    if lambda_resp.status_code == 401 {
                        let _ = logout_user(app_handle.clone()).await;
                        return Err("Session expired. Please log in again.".to_string());
                    }
                } else {
                    // Could not read tokens after auto-login, force logout
                    let _ = logout_user(app_handle.clone()).await;
                    return Err("Could not read tokens after auto-login, force logout".to_string());
                }
            } else {
                // Auto-login failed, force logout
                let _ = logout_user(app_handle.clone()).await;
                return Err("Session expired. Please log in again.".to_string());
            }
        }

        if lambda_resp.status_code == 500 {
            println!("Lambda returned error: {}", lambda_resp.body);
        }

        println!("🔍 Raw Lambda response: {}", lambda_resp.body);

        let sanitized_body = lambda_resp
            .body
            .replace('\n', "")
            .replace('\r', "")
            .replace('\t', "")
            .replace('\u{a0}', "")
            .trim()
            .to_string();

        if !sanitized_body.ends_with('}') {
            println!("❌ Lambda response appears truncated: {}", sanitized_body);
            return Err("Received incomplete response from AI. Please try again.".to_string());
        }

        println!("🔍 Patched Lambda response for parsing: {}", sanitized_body);

        // If body is a JSON string literal, parse it first
        let mut llm_response: LLMResponse = serde_json::from_str(&sanitized_body).map_err(|e| {
            println!(
                "❌ Failed to parse LLM response: {} - JSON was: {}",
                e, sanitized_body
            );
            format!("Failed to parse LLM response: {}", e)
        })?;

        // Patch: Ensure every extracted event has a non-empty description
        if let Some(events) = &mut llm_response.extracted_events {
            for event in events.iter_mut() {
                if event
                    .description
                    .as_ref()
                    .map(|d| d.trim().is_empty())
                    .unwrap_or(true)
                {
                    event.description = Some("Untitled Event".to_string());
                }
            }
        }

        Ok(llm_response)
    }

    async fn get_recent_events(&self, app_handle: &AppHandle,) -> Result<Vec<CalendarEvent>, String> {
        let events_json = get_events(app_handle)
            .await
            .map_err(|e| format!("Failed to get events: {}", e))?;

        let events: Result<Vec<CalendarEvent>, _> = events_json
            .into_iter()
            .map(|json_str| {
                serde_json::from_str(&json_str)
                    .map_err(|e| format!("Failed to parse event JSON: {}", e))
            })
            .collect();

        let mut events = events?;

        let now = chrono::Local::now();
        let recent_cutoff = now - chrono::Duration::hours(24);
        let future_cutoff = now + chrono::Duration::days(30);

        events.retain(|event| {
            !event.deleted && event.time >= recent_cutoff && event.time <= future_cutoff
        });

        events.sort_by(|a, b| a.time.cmp(&b.time));
        events.truncate(20);

        Ok(events)
    }
}

pub async fn process_user_query(app_handle: &AppHandle, query: String, conversation_history: Option<Vec<ConversationMessage>>,) -> Result<LLMResponse, String> {
    let ai_service = AIAssistantService::new();
    ai_service
        .process_user_query(query, app_handle, conversation_history)
        .await
}