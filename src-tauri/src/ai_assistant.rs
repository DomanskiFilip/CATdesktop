use serde::{Deserialize, Serialize};
use reqwest::Client;
use aws_sdk_sagemakerruntime::{Client as SageMakerClient };
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_sdk_sagemakerruntime::primitives::Blob;
use aws_sdk_sagemakerruntime::config::endpoint::Endpoint;
use chrono::{DateTime, Utc, NaiveDateTime, Duration};
use std::time::Duration as StdDuration;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri::Emitter;
use crate::database_utils::{CalendarEvent, get_db_connection};
use crate::user_utils::get_current_user_id;

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

pub struct SageMakerService {
    client: SageMakerClient,
    endpoint_name: String,
}

impl SageMakerService {
    pub async fn new() -> Result<Self, String> {
        let region_provider = RegionProviderChain::default_provider().or_else("eu-west-2");
        let config = aws_config::defaults(BehaviorVersion::v2023_11_09())
            .region(region_provider)
            .load()
            .await;
        
        let client = SageMakerClient::new(&config);
        
        Ok(Self {
            client,
            endpoint_name: std::env::var("SAGEMAKER_ENDPOINT_NAME")
                .unwrap_or_else(|_| "calendar-assistant-llm".to_string()),
        })
    }
    
    pub async fn process_user_query(&self, query: String, app_handle: &AppHandle) -> Result<LLMResponse, String> {
        let user_id = get_current_user_id(app_handle)?;
        
        // Create prompt for the LLM
        let prompt = self.create_prompt(&query, app_handle).await?;
        
        // Call SageMaker endpoint
        let response = self.invoke_endpoint(prompt).await?;
        
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
            "You are a calendar assistant that helps users manage their events.\n\
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
              \"extracted_events\": [{{ \
                \"description\": \"Event description\", \
                \"time\": \"ISO8601 datetime\", \
                \"alarm\": true|false, \
                \"recurrence\": \"RRULE:FREQ=DAILY\" (optional)\n\
              }}],\n\
              \"action_taken\": \"create_event|update_event|query_events|none\"\n\
            }}",
            Utc::now().format("%Y-%m-%d %H:%M:%S"),
            events_context,
            query
        );
        
        Ok(prompt)
    }
    
    async fn invoke_endpoint(&self, prompt: String) -> Result<String, String> {
        // Prepare the request body
        let request_body = serde_json::json!({
            "inputs": prompt,
            "parameters": {
                "max_new_tokens": 1000,
                "temperature": 0.7,
                "top_p": 0.9,
                "return_full_text": false
            }
        });
        
        let blob = Blob::new(serde_json::to_vec(&request_body).unwrap());
        
        // Call the SageMaker endpoint
        let response = self.client.invoke_endpoint()
            .endpoint_name(&self.endpoint_name)
            .body(blob)
            .content_type("application/json")
            .send()
            .await
            .map_err(|e| format!("SageMaker API error: {}", e))?;
        
        // Extract the response body
        let response_body = response.body
            .as_ref()
            .ok_or_else(|| "Empty response from SageMaker".to_string())?;
        
        let response_str = String::from_utf8(response_body.as_ref().to_vec())
          .map_err(|e| format!("Failed to parse response as UTF-8: {}", e))?;
        
        Ok(response_str)
    }
    
    async fn save_extracted_event(&self, event: ExtractedEvent, app_handle: &AppHandle) -> Result<(), String> {
        let user_id = get_current_user_id(app_handle)?;
        
        // Create a new calendar event
        let calendar_event = CalendarEvent {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            description: event.description,
            time: event.time.unwrap_or_else(|| Utc::now() + Duration::hours(1)),
            alarm: event.alarm,
            synced: false,
            synced_google: false,
            deleted: false,
            recurrence: event.recurrence,
        };
        
        // Save the event
        crate::database_utils::save_event(
            app_handle, 
            serde_json::to_string(&calendar_event).map_err(|e| e.to_string())?
        )?;
        
        // Schedule notifications if alarm is on
        if calendar_event.alarm {
            invoke_schedule_notification(app_handle, &calendar_event).await?;
        }
        
        // Trigger sync to DynamoDB and Google Calendar
        invoke_sync(app_handle).await?;
        
        Ok(())
    }
    
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
            |row| {
                Ok(CalendarEvent {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    description: row.get(2)?,
                    time: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?.with_timezone(&Utc),
                    alarm: row.get(4)?,
                    synced: row.get(5)?,
                    synced_google: row.get(6)?,
                    deleted: row.get(7)?,
                    recurrence: row.get::<_, Option<String>>(8)?,
                })
            },
        ).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
        
        Ok(events)
    }
}

// Helper to invoke the schedule_event_notification command
async fn invoke_schedule_notification(app_handle: &AppHandle, event: &CalendarEvent) -> Result<(), String> {
    let event_json = serde_json::to_string(event)
        .map_err(|e| format!("Failed to serialize event: {}", e))?;
    
    app_handle.emit("invoke-handler", serde_json::json!({
        "command": "schedule_event_notification",
        "payload": {
            "event_json": event_json,
        }
    })).map_err(|e| format!("Failed to emit event: {}", e))?;
    
    Ok(())
}

// Helper to invoke the trigger_sync command
async fn invoke_sync(app_handle: &AppHandle) -> Result<(), String> {
    app_handle.emit("invoke-handler", serde_json::json!({
        "command": "trigger_sync",
        "payload": {}
    })).map_err(|e| format!("Failed to emit sync event: {}", e))?;
    
    Ok(())
}

pub async fn process_message(app_handle: &AppHandle, query: String) -> Result<String, String> {
    // You may want to pass AppHandle if needed, here is a simple version:
    let service = SageMakerService::new().await?;
    // You may need to pass an AppHandle from the caller, adjust as needed
    // For now, just return an error if not available
    Err("AppHandle required for process_message".to_string())
}