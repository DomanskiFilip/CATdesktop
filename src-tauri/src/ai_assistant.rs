use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use tauri::AppHandle;
use tauri::Emitter;
use uuid::Uuid;
use rand::Rng;
use crate::ConversationMessage;
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

        // Log the raw LLM response for debugging
        println!("🔍 Raw LLM Response: {}", llm_response.response_text);

        // Handle actions based on `action_taken`
        match llm_response.action_taken.as_deref() {
            Some("create_event") => {
                if let Some(ref events) = llm_response.extracted_events {
                    let mut valid_events = false;
                    
                    for event in events {
                        // Validate event fields before proceeding
                        if event.description.is_empty() {
                            println!("⚠️ Skipping event with empty description");
                            continue;
                        }
                        
                        // Try to save the event, but handle any errors more gracefully
                        match self.save_extracted_event(event.clone(), app_handle).await {
                            Ok(_) => {
                                println!("✅ Successfully saved event: {}", event.description);
                                valid_events = true;
                            },
                            Err(e) => println!("❌ Failed to save event: {} - Error: {}", event.description, e),
                        }
                    }
                    
                    // If all events were invalid, update the response text
                    if !valid_events && llm_response.extracted_events.as_ref().unwrap().iter().all(|e| e.description.is_empty()) {
                        llm_response.response_text = "I need more details to create an event. Please provide a description.".to_string();
                        llm_response.action_taken = Some("none".to_string());
                    }
                } else {
                    println!("⚠️ create_event action specified but no events provided");
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

        // Method to get canned responses for common queries //
        fn get_canned_response(&self, query: &str) -> Option<LLMResponse> {
          // Convert query to lowercase for case-insensitive matching
          let lowercase_query = query.to_lowercase();
          let normalized_query = lowercase_query.trim();
          
          // Create a random number generator instance
          let mut rng = rand::thread_rng();

          // Define patterns for common greetings and questions
          match normalized_query {
              q if (q == "hi" || q == "hello" || q == "hey" || q == "hi there" || q == "hi cat") => {
                  // Return one of 3 random greetings
                  let greetings = [
                      "Hi there! I'm CAT, your calendar assistant. How can I help with your schedule today?",
                      "Hello! I'm here to help manage your calendar. Need to schedule something?",
                      "Hey! I'm your calendar assistant. What can I do for you today?"
                  ];
                  
                  let index = rng.gen_range(0..greetings.len());
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
                  
                  let index = rng.gen_range(0..responses.len());
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
                    // Don't skip any messages, include all history
                    let history_formatted = history.iter()
                        .map(|msg| {
                            // Enhance formatting based on role
                            match msg.role.as_str() {
                                "user" => format!("User: {}", msg.content),
                                "assistant" => format!("Assistant: {}", msg.content),
                                _ => format!("{}: {}", msg.role, msg.content)
                            }
                        })
                        .collect::<Vec<String>>()
                        .join("\n");
                    
                    history_formatted
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
            - Do not include ANY explanatory text, preamble, or conversation\n\
            - Your entire response must be parseable as JSON\n\
            - Never use phrases like \"Here's the JSON:\" or \"I'll create that for you\"\n\n\

            ABOUT THE APPLICATION:\n\
            - This is a personal calendar management app\n\
            - Users can create, update, delete, and query calendar events\n\
            - Each event has: description, date/time, alarm setting, and optional recurrence\n\
            - You have access to the user's current events and can modify their calendar\n\n\
            - You can include polite and helpful responses, but they must be included within 'response_text':'(here your polite and helpful responses)'\n\
            
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
pub async fn process_user_query(app_handle: &AppHandle, query: String, conversation_history: Option<Vec<ConversationMessage>>) -> Result<LLMResponse, String> {
    let service = AIAssistantService::new();
    service.process_user_query(query, app_handle, conversation_history).await
}

// Function to post-process the JSON response from the LLM //
fn post_process_json(json_str: &str) -> String {
    println!("🔍 Original LLM response: {}", json_str);

    // Handle code blocks with json or backticks - check for multiple code blocks
    let code_block_pattern = regex::Regex::new(r"```(?:json)?\s*([\s\S]*?)```").unwrap();
    
    // Process code blocks if they exist
    for captures in code_block_pattern.captures_iter(json_str) {
        if let Some(code_content) = captures.get(1) {
            let inner_content = code_content.as_str().trim();
            
            // Handle escaped characters and convert single quotes properly
            let fixed_json = fix_json_formatting(inner_content);
            
            // Check if it's valid JSON
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&fixed_json) {
                if parsed["response_text"].is_string() {
                    println!("🔍 Valid JSON extracted from code block");
                    return fixed_json;
                }
            }
        }
    }

    // Try to find standalone JSON objects in the response
    let json_pattern = regex::Regex::new(r"\{[^{]*'response_text'[^}]*\}").unwrap();
    
    for json_match in json_pattern.find_iter(json_str) {
        let potential_json = json_match.as_str();
        
        // Fix escaped characters and convert single quotes properly
        let fixed_json = fix_json_formatting(potential_json);
        
        // Check if it's valid JSON after conversion
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&fixed_json) {
            if parsed["response_text"].is_string() {
                println!("🔍 Valid JSON extracted from response: {}", fixed_json);
                return fixed_json;
            }
        }
    }

    // If direct detection fails, try a more comprehensive approach
    // Find the first opening brace that looks like it could be the start of a JSON object
    for (i, c) in json_str.chars().enumerate() {
        if c == '{' {
            // Try to extract balanced JSON
            let mut depth = 1;
            let mut end_pos = i;
            
            for (j, next_char) in json_str[i+1..].chars().enumerate() {
                if next_char == '{' {
                    depth += 1;
                } else if next_char == '}' {
                    depth -= 1;
                    if depth == 0 {
                        end_pos = i + j + 1;
                        break;
                    }
                }
            }
            
            if depth == 0 {
                let json_candidate = &json_str[i..=end_pos];
                
                // Fix escaped characters and convert single quotes properly
                let fixed_json = fix_json_formatting(json_candidate);
                
                // Try to parse
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&fixed_json) {
                    if parsed["response_text"].is_string() {
                        println!("🔍 Valid JSON extracted using balanced braces");
                        return fixed_json;
                    }
                }
            }
        }
    }

    // If all else fails, extract the values directly and construct a new JSON
    let mut response_text = String::from("I'm your calendar assistant. How can I help you?");
    let mut action_taken = String::from("none");
    
    // Try to extract response_text using a more comprehensive approach
    let response_pattern = regex::Regex::new(r"'response_text':\s*'((?:[^'\\]|\\.)*?)'").unwrap();
    if let Some(caps) = response_pattern.captures(json_str) {
        if let Some(text_match) = caps.get(1) {
            let text = text_match.as_str();
            // Replace escaped apostrophes with regular ones for the final output
            response_text = text.replace("\\'", "'");
        }
    }
    
    // Extract action_taken
    let action_pattern = regex::Regex::new(r"'action_taken':\s*'([^']*)'").unwrap();
    if let Some(caps) = action_pattern.captures(json_str) {
        if let Some(action_match) = caps.get(1) {
            action_taken = action_match.as_str().to_string();
        }
    }
    
    // Construct a valid JSON manually with proper escaping
    let escaped_response = response_text.replace("\"", "\\\"");
    let constructed_json = format!(
        r#"{{"response_text":"{}","extracted_events":[],"action_taken":"{}"}}"#, 
        escaped_response, action_taken
    );
    
    println!("🔍 Constructed JSON fallback: {}", constructed_json);
    constructed_json
}

// Helper function to fix JSON formatting issues
fn fix_json_formatting(json_text: &str) -> String {
    let mut result = json_text.to_string();
    
    // First handle escaped characters that need to be preserved
    let mut temp = String::new();
    let mut i = 0;
    let chars: Vec<char> = result.chars().collect();
    
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '\'' {
            // Replace escaped single quote with a temporary placeholder
            temp.push_str("__ESCAPED_QUOTE__");
            i += 2; // Skip the backslash and quote
        } else {
            temp.push(chars[i]);
            i += 1;
        }
    }
    
    // Now do the regular replacements
    result = temp
        .replace("{'", "{\"")
        .replace("':", "\":")
        .replace("': ", "\": ")
        .replace(", '", ", \"")
        .replace("','", "\",\"")
        .replace("'}", "\"}")
        .replace("']", "\"]")
        .replace("['", "[\"")
        // Handle more complex nested cases
        .replace("',{", "\",{")
        .replace("},'", "},\"")
        .replace("':", "\":")
        .replace(": '", ": \"")
        .replace("',", "\",")
        .replace(",'", ",\"");
    
    // Finally, restore the escaped quotes properly as regular quotes in the final JSON
    result = result.replace("__ESCAPED_QUOTE__", "'");
    
    // Filter out non-printable characters and control characters that might corrupt JSON
    result = result.chars()
        .filter(|&c| c >= ' ' || c == '\n' || c == '\t') // Keep only printable chars and some whitespace
        .collect();
    
    // Check if the JSON is properly balanced
    let mut brace_count = 0;
    let mut bracket_count = 0;
    
    for c in result.chars() {
        if c == '{' {
            brace_count += 1;
        } else if c == '}' {
            brace_count -= 1;
        } else if c == '[' {
            bracket_count += 1;
        } else if c == ']' {
            bracket_count -= 1;
        }
    }
    
    // Add missing closing braces/brackets if needed
    while brace_count > 0 {
        result.push('}');
        brace_count -= 1;
    }
    
    while bracket_count > 0 {
        result.push(']');
        bracket_count -= 1;
    }
    
    result
}