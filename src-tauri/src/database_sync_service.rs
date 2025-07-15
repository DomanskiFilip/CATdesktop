use crate::database_utils::{ CalendarEvent, get_db_connection, save_event };
use crate::api_utils::{ AppConfig };
use crate::user_utils::get_current_user_id;
use std::time::Duration;
use tokio::time;
use tauri::AppHandle;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::task::JoinHandle;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct DbSyncService {
    config: Arc<AppConfig>,
    client: Client,
    running: Arc<AtomicBool>,
    task_handle: Option<JoinHandle<()>>,
}

impl DbSyncService {
    pub fn new(config: Arc<AppConfig>) -> Result<Self, String> {
        let client = Client::new();
        Ok(Self {
            config,
            client,
            running: Arc::new(AtomicBool::new(false)),
            task_handle: None,
        })
    }

    /// Stops the Db sync service and cancels all scheduled tasks
    pub async fn stop(&mut self) {
        println!("Stopping Db sync service...");
        self.running.store(false, Ordering::SeqCst);
        
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            println!("DB sync background task aborted");
        }
    }

    // Starts the Db sync service //
    pub async fn start(&mut self, app_handle_arc: Arc<AppHandle>, user_logged_in: bool) {
        println!("Starting Db sync service...");
        
        // Perform initial sync on app start
        println!("Performing initial sync to DynamoDB...");
        if let Err(e) = self.sync_to_dynamodb(&app_handle_arc, user_logged_in).await {
            eprintln!("Initial sync to DynamoDB failed: {}", e);
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        println!("Performing initial sync from DynamoDB...");
        if let Err(e) = self.sync_from_dynamodb(&app_handle_arc, user_logged_in).await {
            eprintln!("Initial sync from DynamoDB failed: {}", e);
        }

        // Set running to true
        self.running.store(true, Ordering::SeqCst);
        
        // Create clones for the background task
        let running = Arc::clone(&self.running);
        let config = Arc::clone(&self.config);
        let client = self.client.clone();
        let app_handle_ref = Arc::clone(&app_handle_arc);
        
        // Start periodic checking in a separate task
        self.task_handle = Some(tokio::spawn(async move {
            let sync_interval = Duration::from_secs(300); // 5 minutes
            let mut interval = time::interval(sync_interval);
            
            // Create a temporary service instance for the background task
            let temp_service = DbSyncService {
                config,
                client,
                running: Arc::new(AtomicBool::new(true)),
                task_handle: None,
            };

            while running.load(Ordering::SeqCst) {
                interval.tick().await;
                
                // Check if the user is logged in
                let user_logged_in = match get_current_user_id(&app_handle_ref) {
                    Ok(_) => true,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        false
                    }
                };
                
                println!("Running periodic sync to DynamoDB...");
                if let Err(e) = temp_service.sync_to_dynamodb(&app_handle_ref, user_logged_in).await {
                  eprintln!("Sync to DynamoDB failed: {}", e);
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                println!("Running periodic sync from DynamoDB...");
                if let Err(e) = temp_service.sync_from_dynamodb(&app_handle_ref, user_logged_in).await {
                  eprintln!("Sync from DynamoDB failed: {}", e); 
                }

                // Sleep for 5 minutes before next sync
                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
            }
            
            println!("DB sync background task completed");
        }));
    }

    // Method to sync events to DynamoDB (upload changes) //
    pub async fn sync_to_dynamodb(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
        // Verify user is actually logged in before proceeding
        if !user_logged_in {
            println!("User not logged in, skipping notification scheduling.");
            return Ok(());
        }

          // Get user ID
          let user_id = match get_current_user_id(&app_handle_arc) {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to get user ID: {}", e);
                return Ok(());
            }
          };

        let events = {
            let conn = get_db_connection(app_handle_arc)
                .map_err(|e| format!("Database connection failed: {}", e))?;
            
            let now = chrono::Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
            let mut unsynced = conn.prepare(
                "SELECT id, user_id, description, time, alarm, synced, synced_google, deleted, recurrence FROM events 
                WHERE user_id = ? AND ((synced = 0) OR (deleted = 1 AND synced = 0)) AND time >= ?"
            ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

            let events_result = unsynced.query_map((&user_id, &now.to_string()), CalendarEvent::from_row)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to collect events: {}", e))?;

            // Return collected events
            events_result
        };

        // No events to sync
        if events.is_empty() {
            println!("No unsynced events found, skipping sync to DynamoDB.");
            return Ok(());
        }

        // Batch send unsynced events to DynamoDB
        match self.send_to_dynamodb(&events).await {
            Ok(_) => {
                // Mark events as synced in a new connection
                let conn = get_db_connection(app_handle_arc)
                    .map_err(|e| format!("Database connection failed: {}", e))?;
                self.mark_events_synced(&conn, &events)?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    // Method to sync from DynamoDB to local (download changes) //
    pub async fn sync_from_dynamodb(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
        if !user_logged_in {
            println!("User not logged in, skipping sync from DynamoDB.");
            return Ok(());
        }

        let sync_url = format!("{}/get-events", self.config.lambda_base_url);
        
        // Get user ID
        let user_id = match get_current_user_id(&app_handle_arc) {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to get user ID: {}", e);
                return Ok(());
            }
        };

        let payload = json!({
          "body": json!({
            "email": user_id
            }).to_string()
        });

        let response = self.client
            .post(&sync_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let text = response.text().await.map_err(|e| e.to_string())?;
        
        // Check for Sandbox.Timedout error in the raw response
        if text.contains("\"errorType\":\"Sandbox.Timedout\"") {
              println!("Server timeout, please try again.");
              return Ok(());
          }

        // Parse the Lambda response to get status_code and body
        if let Ok(lambda_response) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(status_code) = lambda_response.get("status_code").and_then(|v| v.as_u64()) {
                // Check Lambda status code, not just HTTP status
                if status_code != 200 {
                    if let Some(body) = lambda_response.get("body").and_then(|v| v.as_str()) {
                        return Err(format!("Failed to get events: {}", body));
                    } else {
                        return Err(format!("Failed to get events: status code {}", status_code));
                    }
                }
                
                // Successfully synced events
                if let Some(body) = lambda_response.get("body").and_then(|v| v.as_str()) {
                    if let Ok(body_json) = serde_json::from_str::<serde_json::Value>(body) {
                        if let Some(events_array) = body_json.get("events").and_then(|v| v.as_array()) {
                            println!("Retrieved events: {}", events_array.len());
                            self.merge_remote_events(app_handle_arc, events_array)
                                .await
                                .map_err(|e| format!("Failed to merge remote events: {}", e))?;
                        } else {
                            println!("No events found in response or invalid format");
                        }
                    }
                }
                return Ok(());
            }
        }
        
        if !status.is_success() {
            return Err(format!("Failed to get events: {}", text));
        }
        Ok(())
    }


    // Helper method -> send events to DynamoDB //
    async fn send_to_dynamodb(&self, events: &[CalendarEvent]) -> Result<(), String> {
    // Prepare batch payload
    let payload = json!({
        "body": json!({
          "events": events.iter().map(|event| json!({
              "id": event.id,
              "email": event.user_id,
              "description": event.description,
              "time": event.time.to_rfc3339(),
              "alarm": event.alarm,
              "synced_google": event.synced_google,
              "deleted": event.deleted,
              "recurrence": event.recurrence.clone().unwrap_or_else(|| "none".to_string()) 
          })).collect::<Vec<_>>()
        }).to_string()
    });

    // Use lambda endpoint from config
    let sync_url = format!("{}/sync-events", self.config.lambda_base_url);
    
    let response = self.client
        .post(&sync_url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &self.config.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;
    
    if text.contains("\"errorType\":\"Sandbox.Timedout\"") {
            println!("Server timeout, please try again.");
            return Ok(());
        }

    // Parse the Lambda response to get status_code and body
    if let Ok(lambda_response) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(status_code) = lambda_response.get("status_code").and_then(|v| v.as_u64()) {
            // Check Lambda status code, not just HTTP status
            if status_code != 200 {
                if let Some(body) = lambda_response.get("body").and_then(|v| v.as_str()) {
                    return Err(format!("Failed to sync events: {}", body));
                } else {
                    return Err(format!("Failed to sync events: status code {}", status_code));
                }
            }
            
            // Successfully synced events
            if let Some(body) = lambda_response.get("body").and_then(|v| v.as_str()) {
                if let Ok(body_json) = serde_json::from_str::<serde_json::Value>(body) {
                    if let Some(msg) = body_json.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
            }
            return Ok(());
        }
    }
    
    // If we couldn't parse the Lambda response format, fall back to HTTP status check
    if !status.is_success() {
        return Err(format!("Failed to sync events: {}", text));
    }
    Ok(())
  }

    // Helper method -> mark events synced //
    fn mark_events_synced(&self, conn: &rusqlite::Connection, events: &[CalendarEvent]) -> Result<(), String> {
        let mut synced = conn.prepare(
            "UPDATE events SET synced = TRUE WHERE id = ?"
        ).map_err(|e| e.to_string())?;
        
        for event in events {
            synced.execute([&event.id])
                .map_err(|e| format!("Failed to mark event {} as synced: {}", event.id, e))?;
        }
        
        Ok(())
    }

    // Helper method -> merge remote events into local database //
    async fn merge_remote_events(&self, app_handle: &AppHandle, remote_events: &[Value]) -> Result<(), String> {
        let _conn = get_db_connection(app_handle)
            .map_err(|e| format!("Database connection failed: {}", e))?;
        
        // Get today's date at midnight
        let today = chrono::Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();

        for event_data in remote_events {
            // Parse event time as chrono::DateTime
            if let Some(time_str) = event_data["time"].as_str() {
                if let Ok(event_time) = chrono::DateTime::parse_from_rfc3339(time_str) {
                    if event_time.naive_utc() < today {
                        continue; // Skip past events
                    }
                }
            }
            let event_json = json!({
            "id": event_data["id"],
            "user_id": event_data["email"],
            "description": event_data["description"],
            "time": event_data["time"],
            "alarm": event_data["alarm"],
            "deleted": event_data["deleted"],
            "synced": true,  // Mark as synced since it came from the server
            "synced_google": event_data["synced_google"],
            "recurrence": event_data.get("recurrence").and_then(|v| v.as_str()).map(String::from)
          });
          let _ = save_event(app_handle, event_json.to_string());
        }
        
        Ok(())
    }
}
