use serde::{ Deserialize, Serialize };
use chrono::{ DateTime, Utc, Duration, Local, TimeZone };
use tauri::{ AppHandle, Manager };
use uuid::Uuid;
use rand::Rng;
use base64::Engine;
use regex::Regex;
use crate::ConversationMessage;
use crate::database_utils::{ CalendarEvent, get_db_connection, save_event, get_events };
use crate::user_utils::get_current_user_id;
use crate::api_utils::AppConfig;
use crate::{ trigger_sync, UserLocation, get_weekly_weather };

#[derive(Deserialize)]
struct LambdaResponse {
    status_code: u16,
    body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMRequest {
    pub prompt: String,
    pub user_id: String,
    pub conversation_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMEventRequest {
    pub request_type: String,     // "create", "update", "delete", "query"
    pub description: Option<String>,
    pub date: Option<String>,     // ISO date format
    pub time: Option<String>,     // 24-hour format (e.g. "14:30")
    pub duration: Option<i64>,    // minutes
    pub alarm: Option<bool>,
    pub recurrence: Option<String>, // RRULE format
    pub event_id: Option<String>, // For update/delete operations
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMResponse {
    pub response_text: String,
    pub extracted_events: Option<Vec<ExtractedEvent>>,
    pub conversation_id: Option<String>,
    pub action_taken: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEvent {
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

      // Public function to process AI messages //
      pub async fn process_user_query(&self, query: String, app_handle: &AppHandle,  conversation_history: Option<Vec<ConversationMessage>>) -> Result<LLMResponse, String> {
        // log user query
        println!("📝 User Query: {}", query);

        // Check if we have a canned response for this query
        if let Some(response) = self.get_canned_response(&query) {
            println!("🤖 Using canned response");
            return Ok(response);
        }

        // Create prompt for the LLM
        let location_state = app_handle.state::<tokio::sync::Mutex<UserLocation>>();
        let prompt = self.create_prompt_with_history(&query, app_handle, conversation_history, location_state).await?;
       
        // Call Lambda endpoint and get parsed LLM response
        let mut llm_response = self.invoke_lambda_endpoint(prompt, app_handle).await?;

        Ok(llm_response)
    }

      // Method to get canned responses for common queries //
      fn get_canned_response(&self, query: &str) -> Option<LLMResponse> {
        let lowercase_query = query.to_lowercase();
        let normalized_query = lowercase_query.trim();
        
        // random number generator instance
        let mut rng = rand::thread_rng();

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
                    conversation_id: None,
                    action_taken: Some("none".to_string())
                })
            },
            "how are you" | "how are you?" | "how are you doing" | "how are you doing?" => {
                let responses = [
                    "I'm functioning well and ready to help organize your calendar! What can I do for you?",
                    "I'm good, thanks for asking! Would you like to check your schedule or create a new event?",
                    "All systems operational! I'm here to assist with your calendar needs. What's on your mind?"
                ];
                
                let index = rng.random_range(0..responses.len());
                let response = responses[index];
                
                Some(LLMResponse {
                    response_text: response.to_string(),
                    extracted_events: None,
                    conversation_id: None,
                    action_taken: Some("none".to_string())
                })
            },
            "what can you do" | "what can you do?" | "help" | "what are your features" => {
                Some(LLMResponse {
                    response_text: "I can help you manage your calendar by creating, updating, and finding events. Just ask me things like 'Schedule a meeting tomorrow at 2pm', 'When's my next appointment?', or 'Move my dentist appointment to Friday'.".to_string(),
                    extracted_events: None,
                    conversation_id: None,
                    action_taken: Some("none".to_string())
                })
            },
            "thanks" | "thank you" | "thanks!" | "thank you!" => {
                Some(LLMResponse {
                    response_text: "You're welcome! Let me know if you need any other help with your calendar.".to_string(),
                    extracted_events: None,
                    conversation_id: None,
                    action_taken: Some("none".to_string())
                })
            },
            "bye" | "goodbye" | "see you" | "bye bye" => {
                Some(LLMResponse {
                    response_text: "Goodbye! I'm here whenever you need help managing your calendar.".to_string(),
                    extracted_events: None,
                    conversation_id: None,
                    action_taken: Some("none".to_string())
                })
            },
            _ => None, // No canned response found
        }
    }
      
    // Method to create a prompt for the LLM based on user query and recent events //
    async fn create_prompt_with_history(&self, query: &str, app_handle: &AppHandle, conversation_history: Option<Vec<ConversationMessage>>, location_state: tauri::State<'_, tokio::sync::Mutex<UserLocation>>,) -> Result<String, String> {
        // Get recent user events for context (existing logic)
        let recent_events = self.get_recent_events(app_handle).await?;
        
        // Format events for the prompt (existing logic)
        let events_context = if recent_events.is_empty() {
            "You don't have any upcoming events scheduled.".to_string()
        } else {
            let events_formatted = recent_events.iter()
                .map(|event| {
                    let time_str = event.time.format("%Y-%m-%d %H:%M").to_string();
                    format!("- description: {} ; time: {}", event.description, time_str)
                })
                .collect::<Vec<String>>()
                .join("\n");
            
            format!("Your upcoming events:\n{}", events_formatted)
        };
        
        // Format conversation history for context
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

        // Fetch weather using coordinates
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

        let prompt = serde_json::json!({
            "weather_forecast": weather_forecast,
            "current_time": Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            "conversation_history": conversation_context,
            "events_context": events_context,
            "user_query": query,
        });

        println!("📝 Generated Prompt: {}", prompt);
        
        Ok(prompt)
    }
      
    // Method to invoke the Lambda endpoint for LLM processing //
    async fn invoke_lambda_endpoint(&self, prompt: String, app_handle: &AppHandle) -> Result<LLMResponse, String> {
      // Check if user is logged in
      let _user_id = get_current_user_id(app_handle)
          .map_err(|_| "User is not logged in.".to_string())?;

      // Get API config
      let config = AppConfig::new()?;
      let url = format!("{}/llm", config.lambda_base_url);


      // Send POST request to Lambda
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

      // Parse Lambda response
      let lambda_resp: LambdaResponse = serde_json::from_str(&text)
          .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;
      
      // Validate status code
      if lambda_resp.status_code == 500 {
        println!("Lambda returned error: {}", lambda_resp.body);
      }

      // Parse the body for LLM response
      let body_json: serde_json::Value = serde_json::from_str(&lambda_resp.body)
          .map_err(|e| format!("Failed to parse response body: {}", e))?;
      
      let llm_response: LLMResponse = match serde_json::from_str(&body_json.to_string()) {
          Ok(response) => response,
          Err(e) => {
              println!("❌ Failed to parse as LLMResponse: {} - JSON was: {}", e, body_json);
              return Err(format!("Failed to parse LLM response: {} - JSON was: {}", e, body_json));
          }
      };

      Ok(llm_response)
  }

  // Helper method to get recent events for the user //
  async fn get_recent_events(&self, app_handle: &AppHandle) -> Result<Vec<CalendarEvent>, String> {
      // Use the existing get_events function which handles decryption
      let events_json = get_events(app_handle)
          .map_err(|e| format!("Failed to get events: {}", e))?;
      
      // Parse the JSON strings back to CalendarEvent structs
      let events: Result<Vec<CalendarEvent>, _> = events_json
          .into_iter()
          .map(|json_str| {
              serde_json::from_str(&json_str)
                  .map_err(|e| format!("Failed to parse event JSON: {}", e))
          })
          .collect();
      
      let mut events = events?;
      
      // Filter for recent/upcoming events (within the last 24 hours to next 30 days)
      let now = chrono::Local::now();
      let recent_cutoff = now - chrono::Duration::hours(24);
      let future_cutoff = now + chrono::Duration::days(30);
      
      events.retain(|event| {
          !event.deleted && 
          event.time >= recent_cutoff && 
          event.time <= future_cutoff
      });
      
      // Sort by time
      events.sort_by(|a, b| a.time.cmp(&b.time));
      
      // Limit to reasonable number for AI context
      events.truncate(20);
      
      Ok(events)
  }
}

pub async fn process_user_query(app_handle: &AppHandle, query: String, conversation_history: Option<Vec<ConversationMessage>>) -> Result<LLMResponse, String> {
    let ai_service = AIAssistantService::new();
    ai_service.process_user_query(query, app_handle, conversation_history).await
}