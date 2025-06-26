use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use tauri::AppHandle;
use tauri::Emitter;
use uuid::Uuid;
use crate::database_utils::{CalendarEvent, get_db_connection};
use crate::user_utils::get_current_user_id;
use crate::api_utils::AppConfig;
use crate::trigger_sync;
use crate::schedule_notification;
use crate::save_event;

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
      pub async fn process_user_query(&self, query: String, app_handle: &AppHandle) -> Result<LLMResponse, String> {
        // log user query
        println!("📝 User Query: {}", query);

        // Create prompt for the LLM
        let prompt = self.create_prompt(&query, app_handle).await?;
        
        // Call Lambda endpoint and get parsed LLM response
        let llm_response = self.invoke_lambda_endpoint(prompt, app_handle).await?;

        // Handle actions based on `action_taken`
        match llm_response.action_taken.as_deref() {
            Some("create_event") => {
                if let Some(ref events) = llm_response.extracted_events {
                    for event in events {
                        self.save_extracted_event(event.clone(), app_handle).await?;
                    }
                }
            }
            Some("update_event") => {
                println!("Update event action received, but not implemented yet.");
                // Handle event update logic here
            }
            Some("query_events") => {
                println!("Query event action received, but not implemented yet.");
                // Handle event query logic here
            }
            Some("none") | None => {
                println!("No action taken.");
            }
            _ => {
                println!("Unknown action: {:?}", llm_response.action_taken);
            }
        }

        Ok(llm_response)
    }

      
      // Method to create a prompt for the LLM based on user query and recent events //
      async fn create_prompt(&self, query: &str, app_handle: &AppHandle) -> Result<String, String> {
          // Get recent user events for context
          let recent_events = self.get_recent_events(app_handle).await?;
          
          // Format events for the prompt
          let events_context = if recent_events.is_empty() {
              "You don't have any upcoming events scheduled.".to_string()
          } else {
              let events_formatted = recent_events.iter()
                  .map(|event| {
                      let time_str = event.time.format("%Y-%m-%d %H:%M").to_string();
                      format!("- {} at {}", event.description, time_str)
                  })
                  .collect::<Vec<String>>()
                  .join("\n");
              
              format!("Your upcoming events:\n{}", events_formatted)
          };
          
          // Create a comprehensive prompt with instructions
          let prompt = format!(
          "You are CAT (Calendar Assistant), an AI assistant built into a desktop calendar application.\n\n\
          
          CRITICAL FORMATTING INSTRUCTION:\n\
          - You must ONLY return a single JSON object\n\
          - Do not include ANY explanatory text, preamble, or conversation\n\
          - Your entire response must be parseable as JSON\n\
          - Never use phrases like \"Here's the JSON:\" or \"I'll create that for you\"\n\n\

          ABOUT THE APPLICATION:\n\
          - This is a personal calendar management app\n\
          - Users can create, update, delete, and query calendar events\n\
          - Each event has: description, date/time, alarm setting, and optional recurrence\n\
          - You have access to the user's current events and can modify their calendar\n\n\
          
          YOUR CAPABILITIES:\n\
          - Create new calendar events from natural language\n\
          - Update existing events\n\
          - Query and search through events\n\
          - Set alarms and recurring patterns\n\
          - Interpret relative time (\"in 2 hours\", \"next Monday\", \"tomorrow at 3pm\")\n\n\
          
          CURRENT CONTEXT:\n\
          - Current date and time: {}\n\
          - User's timezone: Local system timezone\n\
          - {}\n\n\

          SYSTEM ROLE: You are a JSON response generator only. You never engage in conversation.\n\n\
    
          EXAMPLE REQUEST: \"Schedule a meeting with John tomorrow at 3pm\"\n\n\
          
          EXAMPLE RESPONSE:\n\
          {{\"response_text\":\"Added meeting with John for tomorrow at 3:00 PM with alarm.\",\"extracted_events\":[{{\"description\":\"Meeting with John\",\"time\":\"2025-06-27T15:00:00\",\"alarm\":true,\"recurrence\":null}}],\"action_taken\":\"create_event\"}}\n\n\
          
          USER REQUEST: \"{}\"\n\n\
          
          RESPONSE TEMPLATE/FORMAT (FILL THIS IN):
          {{'response_text':'','extracted_events':[],'action_taken':'none'}}\n\n\
          
          IMPORTANT RULES YOU CANNOT BREAK WHEN RESPONDING:\n\
          - Your entire response must be ONLY the JSON object without any additional text, explanation or code\n\
          - Don't wrap the JSON in code blocks or quotation marks\n\
          - action_taken must be one of: \"create_event\", \"update_event\", \"query_events\", \"none\"\n\
          - For times without dates, assume today\n\
          - For times without specific time, suggest appropriate times\n\
          - Always set alarm to true for new events unless user specifies otherwise\n\
          - Use ISO 8601 format for timestamps (YYYY-MM-DDThh:mm:ss)\n\
          - If creating recurring events, use RRULE format for recurrence\n\
          - Be conversational but concise in response_text\n\
          - If query is unclear, ask for clarification\n\
          - You cannot use foul, disrespectful, or offensive language\n\
          - Do not include any code examples, comments, or explanations in your response\n\
          - Never include Python print statements or execution snippets or anything of sorts\n\
          - DO NOT add any decorations like backticks, triple quotes or markdown formatting\n\
          - Do not include any additional text, explanations, or comments\n\
          - Do not include any additional fields or metadata in the JSON\n\
          - Do not include any timestamps, IDs, or other metadata in the JSON\n\
          - Do not include any additional context or information outside the JSON object\n\
          - Do not include any additional instructions or guidelines in the JSON\n\
          - DO not repeat yourself or the instructions\n\
          - ONLY return the raw JSON object - your ENTIRE response must be a parseable JSON\n\
          YOUR RESPONSE IS USED FOR THE ACTUAL APP SO INCLUDE JUST ONE JSON OBJECT BASED ON RESPONSE FORMAT AS YOUR ENTIRE RESPONSE",
          Utc::now().format("%Y-%m-%d %H:%M:%S"),
          events_context,
          query
      );
          
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

        // Prepare request body for Lambda
        let inner_body = serde_json::json!({
            "inputs": prompt,
            "parameters": {
                "max_new_tokens": 1000,
                "temperature": 0.3,
                "top_p": 0.9,
                "return_full_text": false
            }
        });
        
        let request_body = serde_json::json!({
            "body": inner_body.to_string()
        });

        // Send POST request to Lambda
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("x-api-key", config.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Failed to call Lambda: {}", e))?;

        let text = resp.text().await
        .map_err(|e| format!("Failed to read Lambda response: {}", e))?;

        // Parse Lambda response
        let lambda_resp: LambdaResponse = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;
        
        // Validate status code
        if lambda_resp.status_code != 200 {
            return Err(format!("Lambda returned non-200 status: {}", lambda_resp.status_code));
        }

        // Parse the body for LLM response - Handle deeply nested JSON properly
        let body_json: serde_json::Value = serde_json::from_str(&lambda_resp.body)
            .map_err(|e| format!("Failed to parse response body: {}", e))?;
        
        let llm_response_str = body_json["llm_response"]
            .as_str()
            .ok_or_else(|| "llm_response is not a string".to_string())?;
        
        // Clean up the JSON string before parsing
        let cleaned_json = post_process_json(llm_response_str);
        
        let llm_response: LLMResponse = serde_json::from_str(&cleaned_json)
            .map_err(|e| format!("Failed to parse LLM response: {} - JSON was: {}", e, cleaned_json))?;

        Ok(llm_response)
    }

      
      // Method to save extracted event to the database and schedule notifications //
      async fn save_extracted_event(&self, event: ExtractedEvent, app_handle: &AppHandle) -> Result<(), String> {
          let user_id = get_current_user_id(app_handle)?;
          
          // Create a new calendar event
          let calendar_event = CalendarEvent {
              id: Uuid::new_v4().to_string(),
              user_id,
              description: event.description,
              time: event.time.unwrap_or_else(|| Utc::now() + Duration::hours(1)),
              alarm: event.alarm,
              synced: false,
              synced_google: false,
              deleted: false,
              recurrence: event.recurrence,
          };
          
          // Save the event to database
          save_event(
              app_handle.clone(), 
              serde_json::to_string(&calendar_event).map_err(|e| e.to_string())?
          ).await?;
          
          // Schedule notifications if alarm is enabled
          if calendar_event.alarm {
              let event_json = serde_json::to_string(&calendar_event)
                  .map_err(|e| format!("Failed to serialize event: {}", e))?;
              crate::schedule_notification(event_json, app_handle.clone()).await?;
          }
          
          // Trigger sync to DynamoDB and Google Calendar
          trigger_sync(app_handle.clone()).await?;
          
          Ok(())
      }
      
    // Helper method to get recent events for the user //
    async fn get_recent_events(&self, app_handle: &AppHandle) -> Result<Vec<CalendarEvent>, String> {
          let user_id = get_current_user_id(app_handle)?;
          let conn = get_db_connection(app_handle)
              .map_err(|e| e.to_string())?;
          
          let now = Utc::now();
          let next_week = now + Duration::days(7);
          
          let mut query = conn.prepare(
              "SELECT id, user_id, description, time, alarm, synced, synced_google, deleted, recurrence 
              FROM events 
              WHERE user_id = ? AND deleted = FALSE AND time >= ? AND time <= ?
              ORDER BY time ASC
              LIMIT 5"
          ).map_err(|e| e.to_string())?;
          
          let events = query.query_map(
              [&user_id, &now.to_rfc3339(), &next_week.to_rfc3339()],
              CalendarEvent::from_row
          ).map_err(|e| e.to_string())?
          .collect::<Result<Vec<_>, _>>()
          .map_err(|e| e.to_string())?;
          
          Ok(events)
      }
}

// Public function to process user query through the AI assistant service //
pub async fn process_user_query(app_handle: &AppHandle, query: String) -> Result<LLMResponse, String> {
    let service = AIAssistantService::new();
    service.process_user_query(query, app_handle).await
}

// Function to post-process the JSON response from the LLM //
fn post_process_json(json_str: &str) -> String {
    println!("🔍 Original LLM response: {}", json_str);

    // First, try to extract just the JSON object, ignoring any additional text
    if let Some(json_start) = json_str.find('{') {
        if let Some(json_end) = json_str.rfind('}') {
            if json_end > json_start {
                // Extract just the JSON object and nothing more
                let json_object = &json_str[json_start..=json_end];
                
                // Remove leading dots or spaces that might appear before JSON
                let clean_json = json_object.trim_start_matches(|c| c == '.' || c == ' ' || c == '\t');
                
                // Verify that this is a valid JSON before returning
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(clean_json) {
                    // If we have valid JSON with expected fields, return it directly
                    if parsed["response_text"].is_string() && parsed["action_taken"].is_string() {
                        println!("🔍 Valid JSON found and extracted");
                        return clean_json.to_string();
                    }
                }
            }
        }
    }
    
    // If direct extraction failed, use a stricter regex to find the first valid JSON
    // This regex looks for JSON that specifically contains our key fields
    let json_pattern = regex::Regex::new(
        r#"\{(?:[^{}]|"[^"]*"|(?:\{(?:[^{}]|"[^"]*")*\}))*"response_text"(?:[^{}]|"[^"]*"|(?:\{(?:[^{}]|"[^"]*")*\}))*"action_taken"(?:[^{}]|"[^"]*"|(?:\{(?:[^{}]|"[^"]*")*\}))*\}"#
    ).unwrap();
    
    // Try to find the first complete JSON object with our expected fields
    if let Some(captures) = json_pattern.find(json_str) {
        let json_content = captures.as_str().to_string();
        // If we have a match, clean it up and return it
        let clean_json = json_content.trim_start_matches(|c| c == '.' || c == ' ' || c == '\t');
        if let Ok(_) = serde_json::from_str::<serde_json::Value>(&clean_json) {
            println!("🔍 Valid JSON extracted via regex");
            return clean_json.to_string();
        }
    }
    
    // If regex extraction failed, fall back to the existing field extraction logic
    let response_text_regex = regex::Regex::new(r#""response_text":\s*"([^"]*)"#).unwrap();
    let action_taken_regex = regex::Regex::new(r#""action_taken":\s*"([^"]*)"#).unwrap();
    
    let response_text = match response_text_regex.captures(json_str)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str()) {
            Some(text) => text,
            None => "No response text found",
        };
        
    let action_taken = match action_taken_regex.captures(json_str)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str()) {
            Some(action) => action,
            None => "none",
        };
    
    // Construct a valid JSON manually
    let constructed_json = format!(
        r#"{{"response_text":"{}","extracted_events":[],"action_taken":"{}"}}"#, 
        response_text, action_taken
    );
    
    println!("🔍 Constructed JSON fallback: {}", constructed_json);
    constructed_json
}