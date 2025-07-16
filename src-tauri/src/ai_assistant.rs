use serde::{ Deserialize, Serialize };
use chrono::{ Utc, DateTime, TimeZone };
use tauri::{ AppHandle, Manager };
use rand::Rng;
use crate::ConversationMessage;
use crate::database_utils::{ CalendarEvent, get_events };
use crate::user_utils::get_current_user_id;
use crate::api_utils::AppConfig;
use crate::{ UserLocation, get_weekly_weather };

#[derive(Deserialize)]
struct LambdaResponse {
    status_code: u16,
    body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMResponse {
    pub response_text: String,
    pub extracted_events: Option<Vec<ExtractedEvent>>,
    pub action_taken: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEvent {
    pub target_event_id: Option<String>,
    pub description: String,
    pub time: Option<DateTime<Utc>>,
    pub alarm: bool,
    pub recurrence: Option<String>,
}

pub struct AIAssistantService;

impl AIAssistantService {
    pub fn new() -> Self {
        Self
    }

    pub async fn process_user_query(&self, query: String, app_handle: &AppHandle, conversation_history: Option<Vec<ConversationMessage>>) -> Result<LLMResponse, String> {
        println!("📝 User Query: {}", query);

        // Check for canned responses first
        if let Some(response) = self.get_canned_response(&query) {
            println!("🤖 Using canned response");
            return Ok(response);
        }

        // Create enhanced prompt with event IDs
        let location_state = app_handle.state::<tokio::sync::Mutex<UserLocation>>();
        let prompt = self.create_enhanced_prompt(&query, app_handle, conversation_history, location_state).await?;
       
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
                })
            },
            "what can you do" | "what can you do?" | "help" | "what are your features" => {
                Some(LLMResponse {
                    response_text: "I can help you manage your calendar by creating, updating, moving, and deleting events. I can also check your schedule, provide weather information, and set reminders. Just ask me things like 'Schedule a meeting tomorrow at 2pm', 'When's my next appointment?', 'Move my dentist appointment to Friday', or 'What's the weather like?'".to_string(),
                    extracted_events: None,
                    action_taken: Some("none".to_string()),
                    confidence: Some(1.0),
                })
            },
            _ => None,
        }
    }

    async fn create_enhanced_prompt(&self, query: &str, app_handle: &AppHandle, conversation_history: Option<Vec<ConversationMessage>>, location_state: tauri::State<'_, tokio::sync::Mutex<UserLocation>>) -> Result<serde_json::Value, String> {
        let recent_events = self.get_recent_events(app_handle).await?;
        
        // Format events with IDs for AI context
        let events_context = if recent_events.is_empty() {
            "You don't have any upcoming events scheduled.".to_string()
        } else {
            let events_formatted = recent_events.iter()
                .map(|event| {
                    let time_str = event.time.format("%Y-%m-%d %H:%M").to_string();
                    format!("- ID: {} | description: {} | time: {} | alarm: {}", 
                        event.id, event.description, time_str, event.alarm)
                })
                .collect::<Vec<String>>()
                .join("\n");
            
            format!("Your upcoming events:\n{}", events_formatted)
        };
        
        let conversation_context = if let Some(history) = conversation_history {
            if !history.is_empty() {
                history.iter()
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

        let weather_map = get_weekly_weather(app_handle.clone(), latitude, longitude).await
            .map_err(|e| format!("Failed to fetch weather: {}", e))?;

        let weather_forecast = if weather_map.is_empty() {
            "No weather data available.".to_string()
        } else {
            weather_map.iter()
                .map(|(date, daily)| format!(
                    "{}: {}, max temp: {:.1}°C, max wind: {:.1} km/h",
                    date,
                    daily.weather,
                    daily.temperature_2m_max,
                    daily.wind_speed_10m_max
                ))
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

        println!("📝 Prompt: {}", prompt_json);

        Ok(prompt_json)
    }

    async fn invoke_lambda_endpoint(&self, prompt: serde_json::Value, app_handle: &AppHandle) -> Result<LLMResponse, String> {
        let _user_id = get_current_user_id(app_handle)
            .map_err(|_| "User is not logged in.".to_string())?;

        let config = AppConfig::new()?;
        let url = format!("{}/llm", config.lambda_base_url);

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", config.api_key)
            .json(&prompt)
            .send()
            .await
            .map_err(|e| format!("Failed to call Lambda: {}", e))?;

        let text = resp.text().await
            .map_err(|e| format!("Failed to read Lambda response: {}", e))?;

        println!("🔍 Raw Lambda response: {}", text);

        let lambda_resp: LambdaResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;
        
        if lambda_resp.status_code == 500 {
            println!("Lambda returned error: {}", lambda_resp.body);
        }

        let patched_body = lambda_resp.body.replace(
            r#""time":"2025-07-17T10:00:00""#,
            r#""time":"2025-07-17T10:00:00Z""#
        );
        let mut llm_response: LLMResponse = serde_json::from_str(&patched_body)
            .map_err(|e| {
                println!("❌ Failed to parse LLM response: {} - JSON was: {}", e, patched_body);
                format!("Failed to parse LLM response: {}", e)
            })?;

        // treat AI times as local times, not UTC
        if let Some(ref mut events) = llm_response.extracted_events {
            for event in events.iter_mut() {
                if let Some(utc_time) = event.time {
                    // Convert the UTC time to a naive datetime and then treat it as local
                    let naive_time = utc_time.naive_utc();
                    let local_time = chrono::Local.from_local_datetime(&naive_time).unwrap();
                    event.time = Some(local_time.with_timezone(&Utc));
                }
            }
        }    

        Ok(llm_response)
    }

    async fn get_recent_events(&self, app_handle: &AppHandle) -> Result<Vec<CalendarEvent>, String> {
        let events_json = get_events(app_handle)
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
            !event.deleted && 
            event.time >= recent_cutoff && 
            event.time <= future_cutoff
        });
        
        events.sort_by(|a, b| a.time.cmp(&b.time));
        events.truncate(20);
        
        Ok(events)
    }
}

pub async fn process_user_query(app_handle: &AppHandle, query: String, conversation_history: Option<Vec<ConversationMessage>>) -> Result<LLMResponse, String> {
    let ai_service = AIAssistantService::new();
    ai_service.process_user_query(query, app_handle, conversation_history).await
}