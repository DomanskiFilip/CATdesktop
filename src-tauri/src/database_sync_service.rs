use crate::database::{CalendarEvent, get_db_connection};
use crate::api_utils::{AppConfig, get_device_info};
use std::time::Duration;
use tokio::time;
use tauri::AppHandle;
use reqwest::Client;
use serde_json::{json, Value};

pub struct DbSyncService {
    config: AppConfig,
    client: Client,
}

impl DbSyncService {
    pub fn new() -> Result<Self, String> {
        let config = AppConfig::new()?;
        let client = Client::new();
        
        Ok(Self {
            config,
            client,
        })
    }

    /// Starts the Db sync service //
    pub async fn start(&self, app_handle: &AppHandle, user_logged_in: bool) {
        println!("Starting Db sync service...");
        
        // Perform initial sync on app start
        if let Err(e) = self.sync_events(app_handle, user_logged_in).await {
            eprintln!("Initial sync failed: {}", e);
        }

        // Start periodic checking
        let mut interval = time::interval(Duration::from_secs(300)); // Sync every 5 minutes
        
        loop {
            interval.tick().await;
            if let Err(e) = self.sync_events(app_handle, user_logged_in).await {
                eprintln!("Sync failed: {}", e);
            }
        }
    }

    // Method to sync events to DynamoDB (upload changes) //
    async fn sync_events(&self, app_handle: &AppHandle) -> Result<(), String> {
        let conn = get_db_connection(app_handle)
            .map_err(|e| format!("Database connection failed: {}", e))?;
        
        // Get unsynced events using prepared statement
        let mut stmt = conn.prepare(
            "SELECT id, description, time, alarm, synced, deleted 
             FROM events 
             WHERE synced = FALSE AND deleted = FALSE"
        ).map_err(|e| e.to_string())?;
        
        let events: Vec<CalendarEvent> = stmt.query_map([], |row| {
            Ok(CalendarEvent {
                id: row.get(0)?,
                description: row.get(1)?,
                time: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?.with_timezone(&chrono::Utc),
                alarm: row.get(3)?,
                synced: row.get(4)?,
                deleted: row.get(5)?
            })
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        // Batch send unsynced events to DynamoDB
        if !events.is_empty() {
            match self.send_to_dynamodb(&events).await {
                Ok(_) => {
                    // Mark events as synced
                    self.mark_events_synced(&conn, &events)?;
                    println!("Successfully synced {} events", events.len());
                }
                Err(e) => eprintln!("Failed to sync events: {}", e)
            }
        }

        Ok(())
    }

    // Helper function -> send events to DynamoDB //
    async fn send_to_dynamodb(&self, events: &[CalendarEvent]) -> Result<(), String> {
        let device_info = get_device_info();
        
        // Prepare batch payload
        let payload = json!({
            "events": events.iter().map(|event| json!({
                "id": event.id,
                "description": event.description,
                "time": event.time.to_rfc3339(),
                "alarm": event.alarm,
                "device_info": device_info
            })).collect::<Vec<_>>()
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

        if response.status().is_success() {
            let response_body: Value = response.json().await
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            
            if response_body["statusCode"].as_u64() == Some(200) {
                Ok(())
            } else {
                Err(format!("Sync failed: {}", response_body["body"].as_str().unwrap_or("Unknown error")))
            }
        } else {
            Err(format!("HTTP error: {}", response.status()))
        }
    }

    fn mark_events_synced(&self, conn: &rusqlite::Connection, events: &[CalendarEvent]) -> Result<(), String> {
        let mut stmt = conn.prepare(
            "UPDATE events SET synced = TRUE WHERE id = ?"
        ).map_err(|e| e.to_string())?;
        
        for event in events {
            stmt.execute([&event.id])
                .map_err(|e| format!("Failed to mark event {} as synced: {}", event.id, e))?;
        }
        
        Ok(())
    }

    // function to sync from DynamoDB to local (download changes) //
    pub async fn sync_from_dynamodb(&self, app_handle: &AppHandle) -> Result<(), String> {
        let device_info = get_device_info();
        let sync_url = format!("{}/get-events", self.config.lambda_base_url);
        
        let response = self.client
            .post(&sync_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .json(&device_info)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            let response_body: Value = response.json().await
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            
            if let Some(events) = response_body["events"].as_array() {
                self.merge_remote_events(app_handle, events).await?;
            }
        }
        
        Ok(())
    }

    async fn merge_remote_events(&self, app_handle: &AppHandle, remote_events: &[Value]) -> Result<(), String> {
        let conn = get_db_connection(app_handle)
            .map_err(|e| format!("Database connection failed: {}", e))?;
        
        for event_data in remote_events {
            if let Some(event_json) = event_data.as_str() {
                crate::database_utils::save_event(app_handle, event_json.to_string())?;
            }
        }
        
        Ok(())
    }
}