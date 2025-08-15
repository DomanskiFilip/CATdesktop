use crate::database_utils::{get_db_connection, save_event, CalendarEvent};
use crate::encryption_utils::decrypt_user_data_base;
use crate::credential_utils::fetch_google_credentials;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use base64::Engine;
use chrono::Timelike;
use reqwest::Client;
use serde_json::json;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tauri::{AppHandle, Manager};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};

pub struct GoogleSyncService {
    config: Arc<crate::api_utils::AppConfig>,
    client: Client,
    running: Arc<AtomicBool>,
    task_handle: Option<JoinHandle<()>>,
}

impl GoogleSyncService {
    pub fn new(config: Arc<crate::api_utils::AppConfig>) -> Self {
        Self {
            config,
            client: Client::new(),
            running: Arc::new(AtomicBool::new(false)),
            task_handle: None,
        }
    }

    pub async fn stop(&mut self) {
        println!("Stopping Google sync service...");
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            println!("Google sync background task aborted");
        }
    }

    // Starts the Google sync service //
    pub async fn start(&mut self, app_handle_arc: Arc<AppHandle>, user_logged_in: bool) {
        println!("Starting Google sync service...");

        // Initial syncs
        if let Err(e) = self.sync_to_google(&app_handle_arc, user_logged_in).await {
            eprintln!("Initial sync to Google failed: {}", e);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Err(e) = self.sync_from_google(&app_handle_arc, user_logged_in).await {
            eprintln!("Initial sync from Google failed: {}", e);
        }

        self.running.store(true, Ordering::SeqCst);

        let running = Arc::clone(&self.running);
        let config = Arc::clone(&self.config);
        let client = self.client.clone();
        let app_handle_ref = Arc::clone(&app_handle_arc);

        self.task_handle = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(20)).await; // Wait 20s after initial sync

            let sync_interval = Duration::from_secs(240); // 4 minutes
            let mut interval = time::interval(sync_interval);

            let temp_service = GoogleSyncService {
                config,
                client,
                running: Arc::new(AtomicBool::new(true)),
                task_handle: None,
            };

            while running.load(Ordering::SeqCst) {
                interval.tick().await;

                #[allow(unused_variables)]
                let user_id = {
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

                println!("Running periodic sync to Google...");
                if let Err(e) = temp_service
                    .sync_to_google(&app_handle_ref, user_logged_in)
                    .await
                {
                    eprintln!("Sync to Google failed: {}", e);
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                println!("Running periodic sync from Google...");
                if let Err(e) = temp_service
                    .sync_from_google(&app_handle_ref, user_logged_in)
                    .await
                {
                    eprintln!("Sync from Google failed: {}", e);
                }
            }
            println!("Google sync background task completed");
        }));
    }

    // Method to sync local events to Google Calendar (push unsynced_google events) //
    pub async fn sync_to_google(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
        if !user_logged_in {
            println!("User not logged in, skipping sync to Google.");
            return Ok(());
        }

        #[allow(unused_variables)]
        let user_id = {
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

        let app_data_dir = app_handle_arc
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?;
        let token_path = app_data_dir.join(format!("google_tokens_{}.json", user_id));
        let token_json = std::fs::read_to_string(&token_path)
            .map_err(|e| format!("Failed to read token: {}", e))?;
        let token_data: Value = serde_json::from_str(&token_json)
            .map_err(|e| format!("Failed to parse token JSON: {}", e))?;
        let mut access_token = token_data
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("No access_token in token file")?
            .to_string();

        let refresh_token = token_data
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Fetch client credentials from Lambda instead of local file
        let google_client_data = fetch_google_credentials(app_handle_arc).await
            .map_err(|e| format!("Failed to fetch Google credentials: {}", e))?;
        let installed = google_client_data
            .get("installed")
            .ok_or("No 'installed' section in Google credentials")?;
        let client_id = installed
            .get("client_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let client_secret = installed
            .get("client_secret")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Get unsynced_google events
        let events = {
            let conn = get_db_connection(app_handle_arc)
                .map_err(|e| format!("Database connection failed: {}", e))?;

            let now = chrono::Utc::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let mut unsynced = conn.prepare(
                "SELECT id, user_id, description, time, alarm, synced, synced_google, synced_outlook, event_id_google, event_id_outlook, deleted, recurrence, participants FROM events 
                WHERE user_id = ? AND ((synced_google = 0) OR (deleted = 1 AND synced_google = 0)) AND time >= ?"
            ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

            let events_result = unsynced
                .query_map((&user_id, &now.to_string()), CalendarEvent::from_row)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to collect events: {}", e))?;

            events_result
        };

        if events.is_empty() {
            println!("No unsynced_google events found, skipping sync to Google.");
            return Ok(());
        }

        // Send and process each event to Google Calendar
        for event in &events {
            // Skip events that already have a Google ID (already originated from Google or already synced)
            if let Some(ref google_id) = event.event_id_google {
                if !google_id.is_empty() {
                    println!("Skipping event {} - already has Google ID: {}", event.id, google_id);
                    continue;
                }
            }

            // Skip events that are already synced to Google
            if event.synced_google {
                println!("Skipping event {} - already synced to Google", event.id);
                continue;
            }

            // Skip deleted events unless they need to be deleted from Google
            if event.deleted && !event.synced_google {
                println!("Skipping deleted event {} - not synced to Google yet", event.id);
                continue;
            }

            // Skip events that are marked as deleted
            if event.deleted == true && event.synced_google == true {
                let google_id_to_delete = if let Some(ref google_id) = event.event_id_google {
                    if !google_id.is_empty() {
                        // This was a locally-created event synced to Google
                        google_id
                    } else {
                        // Empty Google ID, use the event ID
                        &event.id
                    }
                } else {
                    // No Google ID stored, use the event ID (this was an event pulled from Google)
                    &event.id
                };
                
                println!("Deleting Google event {}", google_id_to_delete);
                let delete_url = format!("https://www.googleapis.com/calendar/v3/calendars/primary/events/{}", google_id_to_delete);
                
                let delete_resp = tokio::time::timeout(
                    Duration::from_secs(15),
                    self.client
                        .delete(&delete_url)
                        .bearer_auth(access_token.trim())
                        .send(),
                )
                .await;

                match delete_resp {
                    Ok(Ok(resp)) => {
                        if resp.status() == reqwest::StatusCode::UNAUTHORIZED && !refresh_token.is_empty() {
                            // Retry with refreshed token
                            if let Ok(new_token) = self
                                .refresh_google_access_token(refresh_token, client_id, client_secret)
                                .await
                            {
                                self.update_access_token_file(&token_path, &new_token)?;
                                access_token = new_token;
                                
                                let retry_resp = tokio::time::timeout(
                                    Duration::from_secs(15),
                                    self.client
                                        .delete(&delete_url)
                                        .bearer_auth(access_token.trim())
                                        .send(),
                                )
                                .await;
                                
                                if let Ok(Ok(retry_resp)) = retry_resp {
                                    if retry_resp.status().is_success() || retry_resp.status() == reqwest::StatusCode::NOT_FOUND {
                                        println!("Successfully deleted Google event {}", google_id_to_delete);
                                    } else {
                                        eprintln!("Failed to delete Google event {}: {}", google_id_to_delete, retry_resp.status());
                                    }
                                }
                            }
                        } else if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND {
                            println!("Successfully deleted Google event {}", google_id_to_delete);
                        } else {
                            eprintln!("Failed to delete Google event {}: {}", google_id_to_delete, resp.status());
                        }
                    }
                    Ok(Err(e)) => eprintln!("Request error deleting Google event: {}", e),
                    Err(_) => eprintln!("Timeout deleting Google event {}", google_id_to_delete),
                }
                continue; // Skip to next event
            }

            // CREATE new Google event (no Google ID or empty Google ID)
            println!("Creating new Google event for local event {}", event.id);

            // Decrypt the event description
            let decrypted_description = if !event.description.is_empty() {
                match decrypt_user_data_base(
                    app_handle_arc,
                    &event.user_id,
                    &base64::engine::general_purpose::STANDARD
                        .decode(&event.description)
                        .unwrap_or_default(),
                ) {
                    Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
                    Err(e) => {
                        eprintln!("Failed to decrypt event description for Google sync: {}", e);
                        String::from("[UNREADABLE EVENT]")
                    }
                }
            } else {
                String::new()
            };

            let start_time = event.time;
            let end_time = event.time + chrono::Duration::hours(1);

            // Calculate the hour range for this event
            let event_hour_start = start_time.with_minute(0).unwrap().with_second(0).unwrap();
            let event_hour_end = start_time.with_minute(59).unwrap().with_second(59).unwrap();

            // First, fetch events from Google Calendar for this hour
            let events_url = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
            let mut events_response = tokio::time::timeout(
                Duration::from_secs(15),
                self.client
                    .get(events_url)
                    .bearer_auth(access_token.trim())
                    .query(&[
                        ("timeMin", event_hour_start.to_rfc3339().as_str()),
                        ("timeMax", event_hour_end.to_rfc3339().as_str()),
                        ("singleEvents", "true"),
                    ])
                    .send(),
            )
            .await
            .map_err(|e| format!("Request timed out: {}", e))?
            .map_err(|e| format!("Failed to get events for hour: {}", e))?;

            // If unauthorized, refresh token and retry once
            if events_response.status() == reqwest::StatusCode::UNAUTHORIZED
                && !refresh_token.is_empty()
            {
                if let Ok(new_token) = self
                    .refresh_google_access_token(refresh_token, client_id, client_secret)
                    .await
                {
                    self.update_access_token_file(&token_path, &new_token)?;
                    access_token = new_token;
                    events_response = tokio::time::timeout(
                        Duration::from_secs(15),
                        self.client
                            .get(events_url)
                            .bearer_auth(access_token.trim())
                            .query(&[
                                ("timeMin", event_hour_start.to_rfc3339().as_str()),
                                ("timeMax", event_hour_end.to_rfc3339().as_str()),
                                ("singleEvents", "true"),
                            ])
                            .send(),
                    )
                    .await
                    .map_err(|e| format!("Request timed out: {}", e))?
                    .map_err(|e| format!("Failed to get events for hour: {}", e))?;
                }
            }

            if events_response.status().is_success() {
                let json: serde_json::Value =
                    events_response.json().await.map_err(|e| e.to_string())?;

                // // Delete any existing events in this hour
                // if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
                //     for item in items {
                //         if let Some(google_id) = item.get("id").and_then(|v| v.as_str()) {
                //             println!(
                //                 "Deleting existing Google event {} in the same hour",
                //                 google_id
                //             );

                //             let delete_url = format!("https://www.googleapis.com/calendar/v3/calendars/primary/events/{}", google_id);
                //             let delete_resp = self
                //                 .client
                //                 .delete(&delete_url)
                //                 .bearer_auth(access_token.trim())
                //                 .send()
                //                 .await;

                //             if let Err(e) = delete_resp {
                //                 eprintln!("Failed to delete existing event: {}", e);
                //             } else if !delete_resp.unwrap().status().is_success() {
                //                 eprintln!("Google API error when deleting event");
                //             } else {
                //                 println!("Successfully deleted Google event in same hour");
                //             }
                //         }
                //     }
                // }
            }



            let attendees: Vec<serde_json::Value> = event.participants
                .as_ref()
                .map(|participants| {
                    participants.iter()
                        .map(|email| {
                            json!({
                                "email": email,
                                "responseStatus": "needsAction"
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let body = json!({
                "summary": decrypted_description,
                "start": { "dateTime": start_time.to_rfc3339() },
                "end": { "dateTime": end_time.to_rfc3339() },
                "attendees": attendees
            });

            let url = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
            let resp = tokio::time::timeout(
                Duration::from_secs(15),
                self.client
                    .post(url)
                    .bearer_auth(access_token.trim())
                    .json(&body)
                    .send(),
            )
            .await
            .map_err(|e| format!("Request timed out: {}", e))?
            .map_err(|e| format!("Failed to send event to Google: {}", e))?;

            if resp.status().is_success() {
                // Extract the Google event ID from the response
                if let Ok(response_json) = resp.json::<serde_json::Value>().await {
                    if let Some(google_id) = response_json.get("id").and_then(|v| v.as_str()) {
                        // Update the local event with the Google ID
                        let conn = get_db_connection(app_handle_arc)
                            .map_err(|e| format!("Database connection failed: {}", e))?;
                        
                        conn.execute(
                            "UPDATE events SET event_id_google = ? WHERE id = ? AND user_id = ?",
                            (google_id, &event.id, &event.user_id),
                        )
                        .map_err(|e| format!("Failed to store Google event ID: {}", e))?;
                        
                        println!("Successfully created Google event with ID: {}", google_id);
                    }
                } else {
                    println!("Successfully created new Google event (could not parse response)");
                }
            } else {
                eprintln!("Google API error: {}", resp.status());
            }
        }

        // Mark events as synced_google
        let conn = get_db_connection(app_handle_arc)
            .map_err(|e| format!("Database connection failed: {}", e))?;
        Self::mark_events_synced_google(&conn, &events)?;

        // TRIGGER IMMEDIATE DYNAMODB SYNC TO PERSIST SYNC FLAGS
        if let Err(e) = crate::trigger_sync(app_handle_arc.as_ref().clone()).await {
            eprintln!("Failed to trigger DynamoDB sync after Google sync: {}", e);
        } else {
            println!("Successfully triggered DynamoDB sync after Google sync");
        }

        Ok(())
    }

    // Method to sync from Google Calendar to local DB (pull events) //
    pub async fn sync_from_google(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
        if !user_logged_in {
            println!("User not logged in, skipping sync from Google.");
            return Ok(());
        }

        #[allow(unused_variables)]
        let user_id = {
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

        let app_data_dir = app_handle_arc
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?;
        let token_path = app_data_dir.join(format!("google_tokens_{}.json", user_id));
        let token_json = std::fs::read_to_string(&token_path)
            .map_err(|e| format!("Failed to read token: {}", e))?;
        let token_data: Value = serde_json::from_str(&token_json)
            .map_err(|e| format!("Failed to parse token JSON: {}", e))?;
        let access_token = token_data
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("No access_token in token file")?;

        let url = "https://www.googleapis.com/calendar/v3/calendars/primary/events";

        // Get current time and time 1 year from now in RFC3339 format
        let now = chrono::Utc::now();
        let one_year_from_now = now + chrono::Duration::days(365);
        let min_time = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let max_time = one_year_from_now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        println!("Sending request to Google Calendar API...");

        // Build the request with query parameters
        let response = tokio::time::timeout(
            Duration::from_secs(15),
            self.client
                .get(url)
                .bearer_auth(access_token.trim())
                .query(&[
                    ("timeMin", min_time.as_str()),
                    ("timeMax", max_time.as_str()),
                    ("maxResults", "100"),
                    ("showDeleted", "false"),
                    ("singleEvents", "true"),
                    ("timeZone", "UTC"),
                    ("orderBy", "startTime"),
                ])
                .send(),
        )
        .await
        .map_err(|e| format!("Request timed out: {}", e))?
        .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Could not read error response".to_string());

            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_body) {
                if let Some(error) = error_json.get("error") {
                    let message = error
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    let reason = error
                        .get("errors")
                        .and_then(|e| e.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|e| e.get("reason"))
                        .and_then(|r| r.as_str())
                        .unwrap_or("unknown");

                    eprintln!("Google Calendar API error ({}): {}", reason, message);
                    return Err(format!(
                        "Google Calendar API error: {} ({})",
                        message, reason
                    ));
                }
            }
            eprintln!("Google API error response: {}", error_body);
            return Err(format!("Google API error: {} - {}", status, error_body));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
            for item in items {
                if let (Some(google_id), Some(summary), Some(start)) = (
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("summary").and_then(|v| v.as_str()),
                    item.get("start")
                        .and_then(|v| v.get("dateTime").or_else(|| v.get("date")))
                        .and_then(|v| v.as_str()),
                ) {
                    // Extract participants from Google Calendar attendees
                    let participants: Option<Vec<String>> = item.get("attendees")
                        .and_then(|attendees| attendees.as_array())
                        .map(|attendees_array| {
                            attendees_array.iter()
                                .filter_map(|attendee| {
                                    attendee.get("email").and_then(|email| email.as_str())
                                })
                                .map(String::from)
                                .collect()
                        });

                    if participants.is_some() {
                        println!("Processing Google event: {}", summary);
                    } else {
                        println!("Processing Google event without participants: {}", summary);
                    }

                    // Parse event time as chrono::DateTime
                    if let Ok(event_time) = chrono::DateTime::parse_from_rfc3339(start) {
                        let today = chrono::Utc::now()
                            .date_naive()
                            .and_hms_opt(0, 0, 0)
                            .unwrap();
                        if event_time.naive_utc() < today {
                            continue; // Skip past events
                        }

                        // Check for duplicates in a separate scope
                        let duplicate_info = {
                            let conn = get_db_connection(app_handle_arc)
                                .map_err(|e| format!("Database connection failed: {}", e))?;

                            // 1. Check if this Google event already exists by ID OR by event_id_google
                            let mut id_query = conn
                                .prepare("SELECT COUNT(*) FROM events WHERE (id = ?1 OR event_id_google = ?1) AND user_id = ?2")
                                .map_err(|e| format!("Failed to prepare ID check statement: {}", e))?;
                            let id_exists: i64 = id_query
                                .query_row([google_id, &user_id], |row| row.get(0))
                                .map_err(|e| format!("Failed to check for existing event by ID: {}", e))?;

                            if id_exists > 0 {
                                true
                            } else {
                                // 2. Check for events with same time and description (within 30-minute window)
                                let event_start = event_time.with_timezone(&chrono::Utc);
                                let window_start = event_start - chrono::Duration::minutes(30);
                                let window_end = event_start + chrono::Duration::minutes(30);

                                let mut time_query = conn.prepare(
                                    "SELECT COUNT(*) FROM events WHERE user_id = ?1 AND description = ?2 AND time BETWEEN ?3 AND ?4 AND deleted = 0"
                                ).map_err(|e| format!("Failed to prepare time check statement: {}", e))?;
                                
                                let time_exists: i64 = time_query.query_row([
                                    &user_id,
                                    summary,
                                    &window_start.to_rfc3339(),
                                    &window_end.to_rfc3339()
                                ], |row| row.get(0))
                                .map_err(|e| format!("Failed to check for existing event by time: {}", e))?;

                                time_exists > 0
                            }
                        };

                        if should_skip {
                            println!("Skipping Google event '{}' - duplicate found", summary);
                            continue;
                        }

                        // Create the event JSON
                        let event_json = json!({
                            "id": google_id,
                            "user_id": user_id,
                            "description": summary,
                            "time": start,
                            "alarm": false,
                            "synced": false,
                            "synced_google": true,
                            "synced_outlook": false,
                            "event_id_google": google_id,
                            "event_id_outlook": None::<String>,
                            "deleted": false,
                            "recurrence": None::<String>,
                            "participants": participants.unwrap_or_default(),
                        });

                        println!("Saving Google event '{}' to local database...", google_id);
                        let save_result = save_event(app_handle_arc, event_json.to_string()).await;
                        if let Err(e) = save_result {
                            eprintln!("Failed to save Google event '{}': {}", google_id, e);
                        }
                    }
                }
            }
        }
        let _ = app_handle_arc.emit("google_sync_complete", ());
        Ok(())
    }

    fn mark_events_synced_google(conn: &rusqlite::Connection, events: &[CalendarEvent]) -> Result<(), String> {
        for event in events {
            conn.execute(
                "UPDATE events SET synced_google = 1 WHERE id = ? AND user_id = ?",
                (&event.id, &event.user_id),
            )
            .map_err(|e| format!("Failed to mark event as synced to Google: {}", e))?;
        }
        Ok(())
    }

    /// Refreshes the Google access token using the refresh token. //
    pub async fn refresh_google_access_token(&self, refresh_token: &str, client_id: &str, client_secret: &str,) -> Result<String, String> {
        let url = "https://oauth2.googleapis.com/token";
        let params = [
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let resp = self
            .client
            .post(url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to send refresh request: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Google token refresh failed: {} - {}",
                status, body
            ));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("No access_token in response")?;
        Ok(access_token.to_string())
    }

    /// Updates the token file with the new access token. //
    pub fn update_access_token_file(&self, token_path: &std::path::Path, new_access_token: &str,) -> Result<(), String> {
        let mut token_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(token_path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;

        token_json["access_token"] = serde_json::Value::String(new_access_token.to_string());
        std::fs::write(token_path, token_json.to_string()).map_err(|e| e.to_string())
    }
}