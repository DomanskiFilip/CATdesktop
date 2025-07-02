use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration, Local, TimeZone};
use tauri::AppHandle;
use uuid::Uuid;
use rand::Rng;
use crate::ConversationMessage;
use crate::database_utils::{CalendarEvent, get_db_connection, save_event};
use crate::user_utils::get_current_user_id;
use crate::api_utils::AppConfig;
use crate::trigger_sync;
use crate::schedule_event_notification;

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
        let prompt = self.create_prompt_with_history(&query, app_handle, conversation_history).await?;
       
        // Call Lambda endpoint and get parsed LLM response
        let mut llm_response = self.invoke_lambda_endpoint(prompt, app_handle).await?;

        // Validate for empty descriptions in created events
        if let Some(events) = &mut llm_response.extracted_events {
            if llm_response.action_taken.as_deref() == Some("create_event") {
                for event in events.iter_mut() {
                    if event.description.trim().is_empty() {
                        event.description = "Untitled Event".to_string();
                    }
                }
                if llm_response.response_text.trim().is_empty() {
                    llm_response.response_text = "I've created an event for you. It's called 'Untitled Event'. Would you like to add any details or change the time?".to_string();
                }
            }
        }

        // Handle actions based on `action_taken`
        match llm_response.action_taken.as_deref() {
            Some("create_event") => {
                println!("✅ Suggesting event creation to the user.");
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

        // Method to get canned responses for common queries //
        fn get_canned_response(&self, query: &str) -> Option<LLMResponse> {
          // Convert query to lowercase for case-insensitive matching
          let lowercase_query = query.to_lowercase();
          let normalized_query = lowercase_query.trim();
          
          // Create a random number generator instance
          let mut rng = rand::rng();

          // Define patterns for common greetings and questions
          match normalized_query {
              q if (q == "hi" || q == "hello" || q == "hey" || q == "hi there" || q == "hi cat") => {
                  // Return one of 3 random greetings
                  let greetings = [
                      "Hi there! I'm CAT, your calendar assistant. How can I help with your schedule today?",
                      "Hello! I'm here to help manage your calendar. Need to schedule something?",
                      "Hey! I'm your calendar assistant. What can I do for you today?"
                  ];
                  
                  let index = rng.random_range(0..greetings.len()); // Update deprecated `gen_range` to `random_range`
                  let greeting = greetings[index];
                        
                  Some(LLMResponse {
                      response_text: greeting.to_string(),
                      extracted_events: None,
                      conversation_id: None,
                      action_taken: Some("none".to_string())
                  })
              },
              "how are you" | "how are you?" | "how are you doing" | "how are you doing?" => {
                  // Return one of 3 random responses for "how are you"
                  let responses = [
                      "I'm functioning well and ready to help organize your calendar! What can I do for you?",
                      "I'm good, thanks for asking! Would you like to check your schedule or create a new event?",
                      "All systems operational! I'm here to assist with your calendar needs. What's on your mind?"
                  ];
                  
                  let index = rng.random_range(0..responses.len()); // Update deprecated `gen_range` to `random_range`
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
      async fn create_prompt_with_history(&self, query: &str, app_handle: &AppHandle, conversation_history: Option<Vec<ConversationMessage>>) -> Result<String, String> {
            // Get recent user events for context (existing logic)
            let recent_events = self.get_recent_events(app_handle).await?;
            
            // Format events for the prompt (existing logic)
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
            
            // Format conversation history for context
            let conversation_context = if let Some(history) = conversation_history {
                if !history.is_empty() {
                    history.iter()
                        .map(|msg| format!("{}: {}", msg.role, msg.content))
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    "No previous conversation.".to_string()
                }
            } else {
                "No previous conversation.".to_string()
            };

            println!("📝 conversation history: {}", conversation_context);
            
            // Create a comprehensive prompt with instructions and history
            let prompt = format!(
            "<system>
            You are a JSON-only response generator. Avoid any helper text. Always return ONLY the requested JSON.
            
            You are CAT (Calendar Assistant), an AI assistant built into a desktop calendar application.\n\n\
            
            CRITICAL FORMATTING INSTRUCTION:\n\
            - You must ONLY return a single JSON object\n\
            - Use double quotes (`\"`) for all JSON keys and values\n\
            - Do not include ANY explanatory text, preamble, or conversation\n\
            - Your entire response must be parseable as JSON\n\
            - Never use phrases like \"Here's the JSON:\" or \"I'll create that for you\"\n\n\

            ABOUT THE APPLICATION:\n\
            - This is a personal calendar management app\n\
            - Users can create, update, delete, and query calendar events\n\
            - Each event has: description, date/time, alarm setting, and optional recurrence\n\
            - You have access to the user's current events and can modify their calendar\n\
            - You can include polite and helpful responses, but they must be included within 'response_text':'(here your polite and helpful responses)'\n\
            
            ADDITIONAL INFORMATION FOR CASUAL INTERACTION WITH THE USER:\n\
            - You are friendly, helpful, and concise\n\
            - You can use emojis to enhance user experience, but only in the 'response_text' field\n\
            - You can use a casual tone, but always be professional and respectful\n\
            - You are a calendar assistant, not a general-purpose AI\n\
            - You should not answer general knowledge questions or engage in small talk\n\
            - You should not provide explanations or reasoning for your responses\n\
            - When asked about what cat are you are a rare yellow Maine Coon cat with a fluffy tail and a friendly demeanor\n\
            - When asked about weather ask about the city or country that the user whants the weather information for, then check the conversation history if the user answeared and then provide the weather for that location\n\n\

            YOUR CAPABILITIES:\n\
            - Create new calendar events from natural language\n\
            - Update existing events\n\
            - Query and search through events\n\
            - Set alarms and recurring patterns\n\
            - Interpret relative time (\"in 2 hours\", \"next Monday\", \"tomorrow at 3pm\")\n\n\
            
            CONVERSATION HISTORY:\n\
            {}\n\n\
            
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
            
            CRITICAL INSTRUCTION:\n\
            - Always consider the full conversation history to understand the user's intent.\n\
            - User responses may depend on prior context. Ensure your response aligns with the broader context of the conversation.\n\
            - If the user's input is unclear or incomplete, refer to the conversation history to infer their intent.\n\
            - After the assistants previous message asking for clarification, always assume the user has provided the necessary information in their next message.\n\n\
           
            IMPORTANT RULES YOU CANNOT BREAK WHEN RESPONDING:\n\
            - Your entire response must be ONLY the JSON object without any additional text, explanation or code\n\
            - Don't wrap the JSON in code blocks or quotation marks\n\
            - action_taken must be one of: \"create_event\", \"update_event\", \"query_events\", \"none\"\n\
            - Every event must have a non-empty description. If the description is missing, ask the user for clarification.\n\
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
            - you CANNOT allow yourself to break the rules, if the rule is not allowing you to respond then inform the user that you are not allowed to answear\n\
            YOUR RESPONSE IS USED FOR THE ACTUAL APP SO INCLUDE JUST ONE JSON OBJECT BASED ON RESPONSE FORMAT AS YOUR ENTIRE RESPONSE
            </system>",
            conversation_context,
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
                "max_new_tokens": 150,
                "temperature": 0.05,
                "top_p": 0.85,
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
        
        // Validate JSON before parsing
        if let Err(e) = serde_json::from_str::<serde_json::Value>(&cleaned_json) {
            println!("❌ Invalid JSON after post-processing: {}", e);
            return Err(format!("Failed to process LLM response: {}", e));
        }

        let llm_response: LLMResponse = match serde_json::from_str(&cleaned_json) {
            Ok(response) => response,
            Err(e) => {
                println!("❌ Failed to parse as LLMResponse: {} - JSON was: {}", e, cleaned_json);
                return Err(format!("Failed to parse LLM response: {} - JSON was: {}", e, cleaned_json));
            }
        };

        Ok(llm_response)
    }

      
      // Method to save extracted event to the database and schedule notifications //
      async fn save_extracted_event(&self, event: ExtractedEvent, app_handle: &AppHandle) -> Result<(), String> {
          println!("📝 Attempting to save event: {:?}", event);
  
          let user_id = get_current_user_id(app_handle)?;
          
          // Create a new calendar event
          let calendar_event = CalendarEvent {
              id: Uuid::new_v4().to_string(),
              user_id,
              description: event.description.clone(),
              time: event.time.unwrap_or_else(|| Utc::now() + Duration::hours(1)),
              alarm: event.alarm,
              synced: false,
              synced_google: false,
              deleted: false,
              recurrence: event.recurrence.clone(),
          };
          
          // Save the event to database
          println!("📝 Saving event: {:?}", calendar_event);

          match save_event(app_handle, serde_json::to_string(&calendar_event).unwrap()) {
              Ok(_) => println!("✅ Successfully saved event: {}", event.description),
              Err(e) => println!("❌ Failed to save event: {} - Error: {}", event.description, e),
          }
          
          // Schedule notifications if alarm is enabled
          if calendar_event.alarm {
              let event_json = serde_json::to_string(&calendar_event)
                  .map_err(|e| format!("Failed to serialize event: {}", e))?;
              schedule_event_notification(event_json, app_handle.clone()).await?;
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
fn post_process_json(json_str: &str) -> String {
    println!("🔍 Original LLM response: {}", json_str);

   let extracted_str = if let Some(captures) = regex::Regex::new(r"```(?:json)?\s*(\{[\s\S]*?\})\s*```").unwrap().captures(json_str) {
        captures.get(1).map_or("", |m| m.as_str()).trim()
    } else if let Some(start) = json_str.find('{') {
        if let Some(end) = json_str.rfind('}') {
            &json_str[start..=end]
        } else {
            json_str
        }
    } else {
        json_str
    };

    let cleaned_json = fix_json_formatting(extracted_str);
    println!("🔍 Cleaned JSON for parsing: {}", cleaned_json);

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&cleaned_json) {
        let response_text = value["response_text"].as_str().unwrap_or("I'm having trouble processing your request. Could you try rephrasing?").to_string();
        let action_taken = value["action_taken"].as_str().unwrap_or("none").to_string();

        let extracted_events = value["extracted_events"].as_array().map(|arr| {
            arr.iter().filter_map(|event_val| {
                let description = event_val["description"].as_str()?.to_string();
                let alarm = event_val["alarm"].as_bool().unwrap_or(true);
                let recurrence = event_val["recurrence"].as_str().map(String::from);

                let time_str = event_val["time"].as_str()?;
                let time = DateTime::parse_from_rfc3339(time_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
                    .or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S")
                            .ok()
                            .and_then(|ndt| Local.from_local_datetime(&ndt).single())
                            .map(|local_dt| local_dt.with_timezone(&Utc))
                    });

                Some(ExtractedEvent {
                    description,
                    time,
                    alarm,
                    recurrence,
                })
            }).collect::<Vec<ExtractedEvent>>()
        }).unwrap_or_default();

        let llm_response = LLMResponse {
            response_text,
            extracted_events: if extracted_events.is_empty() { None } else { Some(extracted_events) },
            conversation_id: value["conversation_id"].as_str().map(String::from),
            action_taken: Some(action_taken),
        };

        if let Ok(final_json) = serde_json::to_string(&llm_response) {
            println!("✅ Successfully reconstructed and serialized JSON: {}", final_json);
            return final_json;
        }
    }

    println!("❌ Failed to parse and reconstruct JSON, returning emergency fallback.");
    r#"{"response_text":"I'm having trouble processing your request. Could you try rephrasing?","extracted_events":[],"action_taken":"none"}"#.to_string()
}

// Helper function to fix JSON formatting issues
fn fix_json_formatting(json_text: &str) -> String {
    json_text
        .trim()
        .replace('\'', "\"") // Replace single quotes with double quotes
        .replace("True", "true")
        .replace("False", "false")
        .replace("None", "null")
        .replace(",\n}", "\n}") // Remove trailing commas in objects
        .replace(",\n]", "\n]") // Remove trailing commas in arrays
}

pub async fn process_user_query(app_handle: &AppHandle, query: String, conversation_history: Option<Vec<ConversationMessage>>) -> Result<LLMResponse, String> {
    let ai_service = AIAssistantService::new();
    ai_service.process_user_query(query, app_handle, conversation_history).await
}