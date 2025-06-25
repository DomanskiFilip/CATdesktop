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
      
      // Main method to process user query and interact with LLM //
      pub async fn process_user_query(&self, query: String, app_handle: &AppHandle) -> Result<LLMResponse, String> {
          // Create prompt for the LLM
          let prompt = self.create_prompt(&query, app_handle).await?;
          
          // Call Lambda endpoint (which forwards to SageMaker)
          let response = self.invoke_lambda_endpoint(prompt, app_handle).await?;
          
          // Parse LLM response
          let llm_response: LLMResponse = serde_json::from_str(&response)
              .map_err(|e| format!("Failed to parse LLM response: {}", e))?;
          
          // If events were extracted, save them
          if let Some(extracted_events) = &llm_response.extracted_events {
              for event in extracted_events {
                  self.save_extracted_event(event.clone(), app_handle).await?;
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
              "You are a Calendar AssistanT (CAT in short) that helps users manage their events.\n\
              Current date and time: {}\n\n\
              {}\n\n\
              User query: {}\n\n\
              Based on the user's query, determine if they want to create, update, or get information about calendar events. \
              If creating or updating an event, extract the event details including description, date, time, and whether \
              an alarm/notification is needed. If the user mentions recurring events, determine the recurrence pattern. \
              Respond conversationally and include any extracted event details in a structured format.\n\
              Return your response as valid JSON with the following structure:\n\
              {{\n\
                \"response_text\": \"Your conversational response to the user\",\n\
                \"extracted_events\": [{{\
                  \"description\": \"Event description\",\
                  \"time\": \"ISO8601 datetime\",\
                  \"alarm\": true,\
                  \"recurrence\": null\
                }}],\n\
                \"action_taken\": \"create_event\"\n\
              }}\n\
              Notes:\n\
              - If no specific time is mentioned, schedule for 1 hour from now\n\
              - Set alarm to true by default for new events\n\
              - Use null for recurrence if not specified\n\
              - action_taken should be one of: create_event, update_event, query_events, none",
              Utc::now().format("%Y-%m-%d %H:%M:%S"),
              events_context,
              query
          );
          
          Ok(prompt)
      }
      
      // Method to invoke the Lambda endpoint for LLM processing //
      async fn invoke_lambda_endpoint(&self, prompt: String, app_handle: &AppHandle) -> Result<String, String> {
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
                  "temperature": 0.7,
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

                println!("🔍 Raw Lambda response: {}", text);

          // Check for Lambda timeout error
          if text.contains("\"errorType\":\"Sandbox.Timedout\"") {
              eprintln!("Lambda timeout error: {}", text);
              return Err("Lambda function timed out. Please try again later.".to_string());
          }

          // Parse Lambda response
          let lambda_resp: LambdaResponse = serde_json::from_str(&text)
              .map_err(|e| format!("Failed to parse Lambda response: {}", e))?;
          
          // Check status code
          if lambda_resp.status_code != 200 {
              let error_body: serde_json::Value = serde_json::from_str(&lambda_resp.body)
                  .map_err(|e| format!("Failed to parse error body: {}", e))?;
              let error_message = error_body["message"].as_str().unwrap_or("Unknown error");
              eprintln!("Lambda error: {}", error_message);
              return Err(error_message.to_string());
          }

          // Extract LLM response from Lambda response
          let body_json: serde_json::Value = serde_json::from_str(&lambda_resp.body)
              .map_err(|e| format!("Failed to parse response body: {}", e))?;
          let llm_response_json = &body_json["llm_response"];
          
          println!("Lambda response: {}", lambda_resp.body);
          Ok(llm_response_json.to_string())
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

// Public function to process AI messages
pub async fn process_message(app_handle: &AppHandle, query: String) -> Result<String, String> {
    println!("🤖 AI Assistant received query: {}", query);
    let service = AIAssistantService::new();
    let llm_response = service.process_user_query(query, app_handle).await?;
    println!("🤖 AI Assistant response: {:?}", llm_response);
    serde_json::to_string(&llm_response).map_err(|e| e.to_string())
}