use crate::api_utils::{get_device_info, AppConfig};
use crate::auto_login::auto_login_lambda;
use crate::database_utils::{get_db_connection, save_event, CalendarEvent};
use crate::logout_user;
use crate::token_utils::read_tokens_from_file;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use base64::engine::general_purpose;
use base64::Engine;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::task::JoinHandle;
use tokio::time;

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
        if let Err(e) = self
            .sync_from_dynamodb(&app_handle_arc, user_logged_in)
            .await
        {
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
            let sync_interval = Duration::from_secs(240); // 4 minutes
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
                let user_logged_in = {
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    {
                        match get_current_user_id(&app_handle_arc) {
                            Ok(_) => true,
                            Err(_) => false,
                        }
                    }
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    {
                        match get_current_user_id_mobile().await {
                            Ok(_) => true,
                            Err(_) => false,
                        }
                    }
                };

                println!("Running periodic sync to DynamoDB...");
                if let Err(e) = temp_service
                    .sync_to_dynamodb(&app_handle_ref, user_logged_in)
                    .await
                {
                    eprintln!("Sync to DynamoDB failed: {}", e);
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                println!("Running periodic sync from DynamoDB...");
                if let Err(e) = temp_service
                    .sync_from_dynamodb(&app_handle_ref, user_logged_in)
                    .await
                {
                    eprintln!("Sync from DynamoDB failed: {}", e);
                }
            }

            println!("DB sync background task completed");
        }));
    }

    // Method to sync events to DynamoDB (upload changes) //
    pub async fn sync_to_dynamodb(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
        // Verify user is actually logged in before proceeding
        if !user_logged_in {
            println!("User not logged in, skipping notification scheduling.");
            return Ok(());
        }

        // Get user ID
        let user_id: String = {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                match get_current_user_id(app_handle_arc) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(());
                    }
                }
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                match get_current_user_id_mobile().await {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(());
                    }
                }
            }
        };

        let events = {
            let conn = get_db_connection(app_handle_arc)
                .map_err(|e| format!("Database connection failed: {}", e))?;

            let now = chrono::Utc::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let mut unsynced = conn.prepare(
                "SELECT id, user_id, description, time, alarm, synced, synced_google, synced_outlook, event_id_google, event_id_outlook, deleted, recurrence, participants FROM events 
                WHERE user_id = ? AND ((synced = 0) OR (deleted = 1 AND synced = 0)) AND time >= ?"
            ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

            let events_result = unsynced
                .query_map((&user_id, &now.to_string()), CalendarEvent::from_row)
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
        match self.send_to_dynamodb(app_handle_arc, &events).await {
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
    pub async fn sync_from_dynamodb(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
        if !user_logged_in {
            println!("User not logged in, skipping sync from DynamoDB.");
            return Ok(());
        }

        let sync_url = format!("{}/get-events", self.config.lambda_base_url);

        // Get user ID
        let user_id: String = {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                match get_current_user_id(app_handle_arc) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(());
                    }
                }
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                match get_current_user_id_mobile().await {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(());
                    }
                }
            }
        };
        let device_info = get_device_info(&app_handle_arc);

        let mut payload = json!({
            "body": json!({
                "user_id": user_id
            }).to_string(),
            "deviceInfo": device_info,
            "user_id": user_id
        });
        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle_arc).await {
            payload["access_token"] = serde_json::json!(access_token);
        }
        let response = self
            .client
            .post(&sync_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let mut status = response.status();
        let mut text = response.text().await.map_err(|e| e.to_string())?;
        let mut lambda_response: Option<serde_json::Value> = serde_json::from_str(&text).ok();

        // If 401, try auto-login and retry once
        if let Some(ref resp) = lambda_response {
            if resp.get("status_code").and_then(|v| v.as_u64()) == Some(401) {
                if auto_login_lambda(app_handle_arc).await.unwrap_or(false) {
                    // Wait briefly to ensure token file is written
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    // Retry with new token (always read from file)
                    if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle_arc).await {
                        payload["access_token"] = serde_json::json!(access_token);
                        payload["deviceInfo"] = device_info;
                        payload["user_id"] = serde_json::json!(user_id);
                        let retry_response = self
                            .client
                            .post(&sync_url)
                            .header("Content-Type", "application/json")
                            .json(&payload)
                            .send()
                            .await
                            .map_err(|e| format!("Request failed after auto-login: {}", e))?;
                        status = retry_response.status();
                        text = retry_response.text().await.map_err(|e| e.to_string())?;
                        lambda_response = serde_json::from_str::<serde_json::Value>(&text).ok();
                        // If still 401, force logout
                        if let Some(ref resp2) = lambda_response {
                            if resp2.get("status_code").and_then(|v| v.as_u64()) == Some(401) {
                                let _ = logout_user(app_handle_arc.as_ref().clone()).await;
                                return Err("Session expired. Please log in again.".to_string());
                            }
                        }
                    } else {
                        // Could not read tokens after auto-login
                        let _ = logout_user(app_handle_arc.as_ref().clone()).await;
                        return Err("Could not read tokens after auto-login".to_string());
                    }
                } else {
                    let _ = logout_user(app_handle_arc.as_ref().clone()).await;
                    return Err("Session expired. Please log in again.".to_string());
                }
            }
        }

        if let Some(lambda_response) = lambda_response {
            if let Some(status_code) = lambda_response.get("status_code").and_then(|v| v.as_u64()) {
                if status_code != 200 {
                    if let Some(body) = lambda_response.get("body").and_then(|v| v.as_str()) {
                        return Err(format!("Failed to get events: {}", body));
                    } else {
                        return Err(format!("Failed to get events: status code {}", status_code));
                    }
                }
                if let Some(body) = lambda_response.get("body").and_then(|v| v.as_str()) {
                    if let Ok(body_json) = serde_json::from_str::<serde_json::Value>(body) {
                        if let Some(events_array) = body_json.get("events").and_then(|v| v.as_array()) {
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
    async fn send_to_dynamodb(&self, app_handle_arc: &Arc<AppHandle>, events: &[CalendarEvent],) -> Result<(), String> {
        // Get user ID
        let user_id: String = {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                match get_current_user_id(app_handle_arc) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(());
                    }
                }
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                match get_current_user_id_mobile().await {
                    Ok(id) => id,
                    Err(e) => {
                        println!("Failed to get user ID: {}", e);
                        return Ok(());
                    }
                }
            }
        };
        let device_info = get_device_info(&app_handle_arc);

        // Prepare batch payload
        let mut payload = json!({
            "body": json!({
                "events": events.iter().map(|event| json!({
                    "id": event.id,
                    "user_id": event.user_id,
                    "description": event.description,
                    "time": event.time.to_rfc3339(),
                    "alarm": event.alarm,
                    "synced_google": event.synced_google,
                    "synced_outlook": event.synced_outlook,
                    "event_id_google": event.event_id_google,
                    "event_id_outlook": event.event_id_outlook,
                    "deleted": event.deleted,
                    "recurrence": event.recurrence.clone().unwrap_or_else(|| "none".to_string()),
                    "participants": event.participants.clone().unwrap_or_default()
                })).collect::<Vec<_>>()
            }).to_string(),
            "deviceInfo": device_info,
            "user_id": user_id
        });
        if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle_arc).await {
            payload["access_token"] = serde_json::json!(access_token);
        }
        // Use lambda endpoint from config
        let sync_url = format!("{}/sync-events", self.config.lambda_base_url);

        let response = self
            .client
            .post(&sync_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let mut status = response.status();
        let mut text = response.text().await.map_err(|e| e.to_string())?;
        let mut lambda_response: Option<serde_json::Value> = serde_json::from_str(&text).ok();

        // If 401, try auto-login and retry once
        if let Some(ref resp) = lambda_response {
            if resp.get("status_code").and_then(|v| v.as_u64()) == Some(401) {
                if auto_login_lambda(app_handle_arc).await.unwrap_or(false) {
                    // Wait briefly to ensure token file is written
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    // Retry with new token (always read from file)
                    if let Ok((access_token, _, _)) = read_tokens_from_file(app_handle_arc).await {
                        payload["access_token"] = serde_json::json!(access_token);
                        payload["deviceInfo"] = device_info;
                        payload["user_id"] = serde_json::json!(user_id);
                        let retry_response = self
                            .client
                            .post(&sync_url)
                            .header("Content-Type", "application/json")
                            .json(&payload)
                            .send()
                            .await
                            .map_err(|e| format!("Request failed after auto-login: {}", e))?;
                        status = retry_response.status();
                        text = retry_response.text().await.map_err(|e| e.to_string())?;
                        lambda_response = serde_json::from_str::<serde_json::Value>(&text).ok();
                        // If still 401, force logout
                        if let Some(ref resp2) = lambda_response {
                            if resp2.get("status_code").and_then(|v| v.as_u64()) == Some(401) {
                                let _ = logout_user(app_handle_arc.as_ref().clone()).await;
                                return Err("Session expired. Please log in again.".to_string());
                            }
                        }
                    } else {
                        // Could not read tokens after auto-login
                        let _ = logout_user(app_handle_arc.as_ref().clone()).await;
                        return Err("Could not read tokens after auto-login".to_string());
                    }
                } else {
                    let _ = logout_user(app_handle_arc.as_ref().clone()).await;
                    return Err("Session expired. Please log in again.".to_string());
                }
            }
        }

        if let Some(lambda_response) = lambda_response {
            if let Some(status_code) = lambda_response.get("status_code").and_then(|v| v.as_u64()) {
                if status_code != 200 {
                    if let Some(body) = lambda_response.get("body").and_then(|v| v.as_str()) {
                        return Err(format!("Failed to sync events: {}", body));
                    } else {
                        return Err(format!(
                            "Failed to sync events: status code {}",
                            status_code
                        ));
                    }
                }
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

        if !status.is_success() {
            return Err(format!("Failed to sync events: {}", text));
        }
        Ok(())
    }

    // Helper method -> mark events synced //
    fn mark_events_synced(&self, conn: &rusqlite::Connection, events: &[CalendarEvent],) -> Result<(), String> {
        let mut synced = conn
            .prepare("UPDATE events SET synced = TRUE WHERE id = ?")
            .map_err(|e| e.to_string())?;

        for event in events {
            synced
                .execute([&event.id])
                .map_err(|e| format!("Failed to mark event {} as synced: {}", event.id, e))?;
        }

        Ok(())
    }

    // Helper method -> merge remote events into local database //
    async fn merge_remote_events(&self, app_handle: &AppHandle, remote_events: &[Value],) -> Result<(), String> {
        for event_data in remote_events.iter().cloned() {
            let event_id = event_data["id"].as_str().unwrap_or("").to_string();
            let user_id = event_data["user_id"].as_str().unwrap_or("").to_string();
            let event_id_for_log = event_id.clone();
            let mut description_plain = String::new();
            if let Some(enc_desc) = event_data.get("description").and_then(|v| v.as_str()) {
                if let Ok(decoded) = general_purpose::STANDARD.decode(enc_desc) {
                    match crate::encryption_utils::decrypt_user_data_base(
                        app_handle, &user_id, &decoded,
                    ) {
                        Ok(decrypted) => {
                            if let Ok(desc_str) = String::from_utf8(decrypted) {
                                description_plain = desc_str;
                            } else {
                                description_plain = "[UNREADABLE EVENT]".to_string();
                            }
                        }
                        Err(_) => {
                            description_plain = "[UNREADABLE EVENT]".to_string();
                        }
                    }
                } else {
                    description_plain = enc_desc.to_string();
                }
            }

            let incoming_deleted = event_data
                .get("deleted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Handle participants
             let participants_from_dynamo = event_data.get("participants")
                .and_then(|v| {
                    if v.is_array() {
                        Some(
                            v.as_array()
                                .unwrap()
                                .iter()
                                .filter_map(|p| p.as_str().map(|s| s.to_string()))
                                .collect::<Vec<String>>()
                        )
                    } else {
                        None
                    }
                });

            let participants_for_json = participants_from_dynamo.clone();

            let event_json = json!({
                "id": event_data["id"],
                "user_id": event_data["user_id"],
                "description": description_plain,
                "time": event_data["time"],
                "alarm": event_data["alarm"],
                "deleted": incoming_deleted,
                "synced": event_data.get("synced").and_then(|v| v.as_bool()).unwrap_or(false),
                // USE REMOTE VALUES DIRECTLY
                "synced_google": event_data.get("synced_google").and_then(|v| v.as_bool()).unwrap_or(false),
                "synced_outlook": event_data.get("synced_outlook").and_then(|v| v.as_bool()).unwrap_or(false),
                "event_id_google": event_data.get("event_id_google").and_then(|v| v.as_str()).map(String::from),
                "event_id_outlook": event_data.get("event_id_outlook").and_then(|v| v.as_str()).map(String::from),
                "recurrence": event_data.get("recurrence").and_then(|v| v.as_str()).map(String::from),
                "participants": participants_for_json.unwrap_or_default(),
            });

            let participants_for_db = participants_from_dynamo.clone();

            let app_handle = app_handle.clone();
            let event_json_string = event_json.to_string();

            let result = tokio::task::spawn_blocking(move || {
                let conn = get_db_connection(&app_handle)
                    .map_err(|e| format!("Database connection failed: {}", e))?;

                let mut existing_query = conn
                    .prepare("SELECT deleted, participants FROM events WHERE id = ?1 AND user_id = ?2")
                    .map_err(|e| format!("Failed to prepare existing check statement: {}", e))?;
                
                let existing_data: Option<(bool, String)> = existing_query
                    .query_row([event_id.as_str(), user_id.as_str()], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })
                    .ok();

                let should_save = if let Some((existing_deleted, existing_participants)) = existing_data {
                    // Only update if deleted status OR participants are different
                    let incoming_participants_json = serde_json::to_string(&participants_for_db).unwrap_or("[]".to_string());
                    let participants_changed = existing_participants != incoming_participants_json;
                    existing_deleted != incoming_deleted || participants_changed
                } else {
                    true // New event, always save
                };

                if should_save {
                    futures::executor::block_on(save_event(&app_handle, event_json_string))
                } else {
                    Ok(())
                }
            })
            .await
            .map_err(|e| format!("Join error: {}", e))?;

            if let Err(e) = result {
                println!("❌ Failed to save event {}: {}", event_id_for_log, e);
            } else {
                println!("✅ Event saved successfully");
            }
        }
        Ok(())
    }
}
