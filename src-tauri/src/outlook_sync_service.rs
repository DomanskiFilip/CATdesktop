use crate::database_utils::{get_db_connection, save_event, CalendarEvent};
use crate::encryption_utils::decrypt_user_data_base;
use crate::outlook_oauth::refresh_outlook_token;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use base64::Engine;
use reqwest::Client;
use serde_json::json;
use serde_json::Value;
use std::sync::atomic::{ AtomicBool, Ordering };
use std::sync::Arc;
use tauri::{ AppHandle, Manager };
use tokio::task::JoinHandle;
use tokio::time::{ self, Duration };
use chrono::TimeZone;

pub struct OutlookSyncService {
    config: Arc<crate::api_utils::AppConfig>,
    client: Client,
    running: Arc<AtomicBool>,
    task_handle: Option<JoinHandle<()>>,
}

impl OutlookSyncService {
    pub fn new(config: Arc<crate::api_utils::AppConfig>) -> Self {
        Self {
            config,
            client: Client::new(),
            running: Arc::new(AtomicBool::new(false)),
            task_handle: None,
        }
    }

    pub async fn stop(&mut self) {
        println!("Stopping Outlook sync service...");
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            println!("Outlook sync background task aborted");
        }
    }

    pub async fn start(&mut self, app_handle_arc: Arc<AppHandle>, user_logged_in: bool) {
        println!("Starting Outlook sync service...");

        // Initial syncs
        if let Err(e) = self.sync_to_outlook(&app_handle_arc, user_logged_in).await {
            eprintln!("Initial sync to Outlook failed: {}", e);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Err(e) = self.sync_from_outlook(&app_handle_arc, user_logged_in).await {
            eprintln!("Initial sync from Outlook failed: {}", e);
        }

        self.running.store(true, Ordering::SeqCst);

        let running = Arc::clone(&self.running);
        let config = Arc::clone(&self.config);
        let client = self.client.clone();
        let app_handle_ref = Arc::clone(&app_handle_arc);

        self.task_handle = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(20)).await;

            let sync_interval = Duration::from_secs(240); // 4 minutes
            let mut interval = time::interval(sync_interval);

            let temp_service = OutlookSyncService {
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
                        match get_current_user_id(&app_handle_ref) {
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

                println!("Running periodic sync to Outlook...");
                if let Err(e) = temp_service
                    .sync_to_outlook(&app_handle_ref, user_logged_in)
                    .await
                {
                    eprintln!("Sync to Outlook failed: {}", e);
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                println!("Running periodic sync from Outlook...");
                if let Err(e) = temp_service
                    .sync_from_outlook(&app_handle_ref, user_logged_in)
                    .await
                {
                    eprintln!("Sync from Outlook failed: {}", e);
                }
            }
            println!("Outlook sync background task completed");
        }));
    }

    // Sync local events to Outlook
    pub async fn sync_to_outlook(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
        if !user_logged_in {
            println!("User not logged in, skipping sync to Outlook.");
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
        let token_path = app_data_dir.join(format!("outlook_tokens_{}.json", user_id));
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

        // Get unsynced_outlook events
        let events = {
            let conn = get_db_connection(app_handle_arc)
                .map_err(|e| format!("Database connection failed: {}", e))?;

            let now = chrono::Utc::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let mut unsynced = conn.prepare(
                "SELECT id, user_id, description, time, alarm, synced, synced_google, synced_outlook, event_id_google, event_id_outlook, deleted, recurrence, participants FROM events
                WHERE user_id = ? AND ((synced_outlook = 0) OR (deleted = 1 AND synced_outlook = 0)) AND time >= ?"
            ).map_err(|e| format!("Failed to prepare statement: {}", e))?;

            let events_result = unsynced
                .query_map((&user_id, &now.to_string()), CalendarEvent::from_row)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Failed to collect events: {}", e))?;

            events_result
        };

        if events.is_empty() {
            println!("No unsynced_outlook events found, skipping sync to Outlook.");
            return Ok(());
        }

        // Send and process each event to Outlook Calendar
        for event in &events {
            let is_local_change = !event.synced;

            // Skip deleted events unless they need to be deleted from Outlook
            if event.deleted && event.synced_outlook {
                println!("Skipping deleted event {} - not synced to Outlook yet", event.id);
                continue;
            }

            // Handle deletion of synced events
            if event.deleted == true && event.synced_outlook == false {
                let outlook_id_to_delete = if let Some(ref outlook_id) = event.event_id_outlook {
                    if !outlook_id.is_empty() {
                        outlook_id
                    } else {
                        &event.id
                    }
                } else {
                    &event.id
                };
                
                println!("Deleting Outlook event {}", outlook_id_to_delete);
                let delete_url = format!("https://graph.microsoft.com/v1.0/me/events/{}", outlook_id_to_delete);
                
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
                            if let Ok(new_token) = self.refresh_outlook_access_token(&refresh_token, app_handle_arc).await {
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
                                        println!("Successfully deleted Outlook event {}", outlook_id_to_delete);
                                    }
                                }
                            }
                        } else if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND {
                            println!("Successfully deleted Outlook event {}", outlook_id_to_delete);
                        } else {
                            eprintln!("Failed to delete Outlook event {}: {}", outlook_id_to_delete, resp.status());
                        }
                    }
                    Ok(Err(e)) => eprintln!("Request error deleting Outlook event: {}", e),
                    Err(_) => eprintln!("Timeout deleting Outlook event {}", outlook_id_to_delete),
                }
                continue;
            }

            // Check if this event has an Outlook ID (existing Outlook event to update)
            if let Some(ref outlook_id) = event.event_id_outlook {
                if !outlook_id.is_empty() && is_local_change {
                    // UPDATE existing Outlook event with local changes
                    println!("Updating existing Outlook event {} with local changes", outlook_id);
                    
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
                                eprintln!("Failed to decrypt event description for Outlook sync: {}", e);
                                String::from("[UNREADABLE EVENT]")
                            }
                        }
                    } else {
                        String::new()
                    };

                    let start_time = event.time;
                    let end_time = event.time + chrono::Duration::hours(1);

                    let attendees: Vec<serde_json::Value> = event.participants
                        .as_ref()
                        .map(|participants| {
                            participants.iter()
                                .map(|email| {
                                    json!({
                                        "emailAddress": {
                                            "address": email,
                                            "name": email
                                        }
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let body = json!({
                        "subject": decrypted_description,
                        "start": {
                            "dateTime": start_time.to_rfc3339(),
                            "timeZone": "UTC"
                        },
                        "end": {
                            "dateTime": end_time.to_rfc3339(),
                            "timeZone": "UTC"
                        },
                        "attendees": attendees
                    });

                    // UPDATE existing Outlook event
                    let update_url = format!("https://graph.microsoft.com/v1.0/me/events/{}", outlook_id);
                    let resp = tokio::time::timeout(
                        Duration::from_secs(15),
                        self.client
                            .patch(&update_url)
                            .bearer_auth(access_token.trim())
                            .json(&body)
                            .send(),
                    )
                    .await
                    .map_err(|e| format!("Request timed out: {}", e))?
                    .map_err(|e| format!("Failed to update event in Outlook: {}", e))?;

                    if resp.status().is_success() {
                        println!("✅ Successfully updated Outlook event with ID: {}", outlook_id);
                    } else if resp.status() == reqwest::StatusCode::UNAUTHORIZED && !refresh_token.is_empty() {
                        // Try refreshing token and retry
                        if let Ok(new_token) = self.refresh_outlook_access_token(&refresh_token, app_handle_arc).await {
                            self.update_access_token_file(&token_path, &new_token)?;
                            access_token = new_token;
                            
                            let retry_resp = tokio::time::timeout(
                                Duration::from_secs(15),
                                self.client
                                    .patch(&update_url)
                                    .bearer_auth(access_token.trim())
                                    .json(&body)
                                    .send(),
                            )
                            .await;
                            
                            if let Ok(Ok(retry_resp)) = retry_resp {
                                if retry_resp.status().is_success() {
                                    println!("✅ Successfully updated Outlook event with ID: {} (after token refresh)", outlook_id);
                                } else {
                                    eprintln!("Failed to update Outlook event {}: {}", outlook_id, retry_resp.status());
                                }
                            }
                        }
                    } else {
                        eprintln!("Outlook API error updating event: {}", resp.status());
                    }
                    continue; // Skip to next event - we're done with this one
                } else if !outlook_id.is_empty() && !is_local_change {
                    // Event already has Outlook ID and no local changes - skip
                    println!("Skipping event {} - already has Outlook ID and no local changes: {}", event.id, outlook_id);
                    continue;
                }
            }

            // Skip events that are already synced to Outlook (no local changes)
            if event.synced_outlook && !is_local_change {
                println!("Skipping event {} - already synced to Outlook", event.id);
                continue;
            }

            // CREATE new Outlook event (no Outlook ID or empty Outlook ID)
            println!("Creating new Outlook event for local event {}", event.id);

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
                        eprintln!("Failed to decrypt event description for Outlook sync: {}", e);
                        String::from("[UNREADABLE EVENT]")
                    }
                }
            } else {
                String::new()
            };

            let start_time = event.time;
            let end_time = event.time + chrono::Duration::hours(1);

            let attendees: Vec<serde_json::Value> = event.participants
                .as_ref()
                .map(|participants| {
                    participants.iter()
                        .map(|email| {
                            json!({
                                "emailAddress": {
                                    "address": email,
                                    "name": email
                                }
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let body = json!({
                "subject": decrypted_description,
                "start": {
                    "dateTime": start_time.to_rfc3339(),
                    "timeZone": "UTC"
                },
                "end": {
                    "dateTime": end_time.to_rfc3339(),
                    "timeZone": "UTC"
                },
                "attendees": attendees
            });

            let url = "https://graph.microsoft.com/v1.0/me/events";
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
            .map_err(|e| format!("Failed to send event to Outlook: {}", e))?;

        if resp.status().is_success() {
                // IMPORTANT: Capture the Outlook ID and store it
                if let Ok(response_json) = resp.json::<serde_json::Value>().await {
                    if let Some(outlook_id) = response_json.get("id").and_then(|v| v.as_str()) {
                        let conn = get_db_connection(app_handle_arc)
                            .map_err(|e| format!("Database connection failed: {}", e))?;
                        
                        conn.execute(
                            "UPDATE events SET event_id_outlook = ?, synced_outlook = 1 WHERE id = ? AND user_id = ?",
                            (outlook_id, &event.id, &event.user_id),
                        )
                        .map_err(|e| format!("Failed to store Outlook event ID: {}", e))?;
                        
                        println!("✅ Successfully created Outlook event with ID: {} for local event: {}", outlook_id, event.id);
                    }
                } else {
                    eprintln!("Failed to parse Outlook response for event: {}", event.id);
                }
            } else if resp.status() == reqwest::StatusCode::UNAUTHORIZED && !refresh_token.is_empty() {
                // Try refreshing token and retry once for creation
                if let Ok(new_token) = self.refresh_outlook_access_token(&refresh_token, app_handle_arc).await {
                    self.update_access_token_file(&token_path, &new_token)?;
                    access_token = new_token;
                    
                    let retry_resp = tokio::time::timeout(
                        Duration::from_secs(15),
                        self.client
                            .post(url)
                            .bearer_auth(access_token.trim())
                            .json(&body)
                            .send(),
                    )
                    .await;
                    
                    if let Ok(Ok(retry_resp)) = retry_resp {
                        if retry_resp.status().is_success() {
                            if let Ok(response_json) = retry_resp.json::<serde_json::Value>().await {
                                if let Some(outlook_id) = response_json.get("id").and_then(|v| v.as_str()) {
                                    let conn = get_db_connection(app_handle_arc)
                                        .map_err(|e| format!("Database connection failed: {}", e))?;
                                    
                                    conn.execute(
                                        "UPDATE events SET event_id_outlook = ?, synced_outlook = 1 WHERE id = ? AND user_id = ?",
                                        (outlook_id, &event.id, &event.user_id),
                                    )
                                    .map_err(|e| format!("Failed to store Outlook event ID: {}", e))?;
                                    
                                    println!("✅ Successfully created Outlook event with ID: {} for local event: {} (after token refresh)", outlook_id, event.id);
                                }
                            }
                        }
                    }
                }
            } else {
                eprintln!("Outlook API error creating event: {}", resp.status());
            }
        }

        // Mark events as synced_outlook
        let conn = get_db_connection(app_handle_arc)
            .map_err(|e| format!("Database connection failed: {}", e))?;
        Self::mark_events_synced_outlook(&conn, &events)?;

        Ok(())
    }

    // Sync Outlook events to local database
    pub async fn sync_from_outlook(&self, app_handle_arc: &Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
        if !user_logged_in {
            println!("User not logged in, skipping sync from Outlook.");
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
        let token_path = app_data_dir.join(format!("outlook_tokens_{}.json", user_id));
        let token_json = std::fs::read_to_string(&token_path)
            .map_err(|e| format!("Failed to read token: {}", e))?;
        let token_data: Value = serde_json::from_str(&token_json)
            .map_err(|e| format!("Failed to parse token JSON: {}", e))?;
        let access_token = token_data
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("No access_token in token file")?;

        let url = "https://graph.microsoft.com/v1.0/me/events";

        let now = chrono::Utc::now();
        let one_year_from_now = now + chrono::Duration::days(365);
        let filter = format!(
            "start/dateTime ge '{}' and start/dateTime le '{}'",
            now.format("%Y-%m-%dT%H:%M:%SZ"),
            one_year_from_now.format("%Y-%m-%dT%H:%M:%SZ")
        );

        println!("Sending request to Outlook Calendar API...");

        let response = tokio::time::timeout(
            Duration::from_secs(15),
            self.client
                .get(url)
                .bearer_auth(access_token.trim())
                .query(&[
                    ("$filter", filter.as_str()),
                    ("$top", "100"),
                    ("$orderby", "start/dateTime"),
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
            return Err(format!("Outlook API error: {} - {}", status, error_body));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        
        if let Some(items) = json.get("value").and_then(|v| v.as_array()) {
            for item in items {
                if let (Some(outlook_id), Some(subject), Some(start)) = (
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("subject").and_then(|v| v.as_str()),
                    item.get("start")
                        .and_then(|v| v.get("dateTime"))
                        .and_then(|v| v.as_str()),
                ) {
                    // Extract participants from Outlook attendees
                    let participants: Option<Vec<String>> = item.get("attendees")
                        .and_then(|attendees| attendees.as_array())
                        .map(|attendees_array| {
                            attendees_array.iter()
                                .filter_map(|attendee| {
                                    attendee.get("emailAddress")
                                        .and_then(|email_obj| email_obj.get("address"))
                                        .and_then(|email| email.as_str())
                                })
                                .map(String::from)
                                .collect()
                        });

                    // Parse Microsoft's datetime format
                    let event_time = {
                        // First try standard RFC3339
                        if let Ok(time) = chrono::DateTime::parse_from_rfc3339(start) {
                            time
                        } else {
                            // Handle Microsoft's format: 2025-08-05T21:00:00.0000000
                            // Remove extra precision and add timezone
                            let cleaned_start = if start.contains('.') {
                                // Remove microseconds beyond 3 digits and add Z for UTC
                                let parts: Vec<&str> = start.split('.').collect();
                                if parts.len() == 2 {
                                    let base = parts[0];
                                    let fraction = &parts[1];
                                    // Take only first 3 digits of fraction (milliseconds)
                                    let short_fraction = if fraction.len() > 3 {
                                        &fraction[..3]
                                    } else {
                                        fraction
                                    };
                                    format!("{}.{}Z", base, short_fraction)
                                } else {
                                    format!("{}Z", start)
                                }
                            } else {
                                format!("{}Z", start)
                            };
                            
                            match chrono::DateTime::parse_from_rfc3339(&cleaned_start) {
                                Ok(time) => time,
                                Err(e) => {
                                    eprintln!("Failed to parse cleaned event time '{}': {}", cleaned_start, e);
                                    
                                    // Last resort: try parsing as naive datetime and assume UTC
                                    match chrono::NaiveDateTime::parse_from_str(start, "%Y-%m-%dT%H:%M:%S%.f") {
                                        Ok(naive_time) => {
                                            let utc_time = chrono::Utc.from_utc_datetime(&naive_time);
                                            utc_time.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap())
                                        }
                                        Err(_) => {
                                            // If all parsing fails, skip this event
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                    };

                    let today = chrono::Utc::now()
                        .date_naive()
                        .and_hms_opt(0, 0, 0)
                        .unwrap();
                    if event_time.naive_utc() < today {
                        // Skip past events
                        continue; 
                    }

                    // Check for duplicates
                    let duplicate_info = {
                        let conn = get_db_connection(app_handle_arc)
                            .map_err(|e| format!("Database connection failed: {}", e))?;
                        
                        let mut id_query = conn
                            .prepare("SELECT COUNT(*), description FROM events WHERE (id = ?1 OR event_id_outlook = ?1) AND user_id = ?2")
                            .map_err(|e| format!("Failed to prepare ID check statement: {}", e))?;

                        if let Ok(row) = id_query.query_row([outlook_id, &user_id], |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                        }) {
                            if row.0 > 0 {
                                // Event exists, decrypt and compare descriptions
                                let existing_description = if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&row.1) {
                                    match crate::encryption_utils::decrypt_user_data_base(app_handle_arc, &user_id, &decoded) {
                                        Ok(decrypted) => String::from_utf8(decrypted).unwrap_or_default(),
                                        Err(_) => row.1.clone(), // Fall back to encrypted string if decryption fails
                                    }
                                } else {
                                    row.1.clone() // Not base64, assume plain text
                                };
                                
                                let needs_update = existing_description != subject;
                                (true, needs_update, existing_description)
                            } else {
                                // Check for events with same time and description (within 30-minute window)
                                let event_start = event_time.with_timezone(&chrono::Utc);
                                let window_start = event_start - chrono::Duration::minutes(30);
                                let window_end = event_start + chrono::Duration::minutes(30);

                                let mut time_query = conn.prepare(
                                    "SELECT COUNT(*), description FROM events WHERE user_id = ?1 AND description = ?2 AND time BETWEEN ?3 AND ?4 AND deleted = 0"
                                ).map_err(|e| format!("Failed to prepare time check statement: {}", e))?;
                                
                                if let Ok(time_row) = time_query.query_row([
                                    &user_id,
                                    subject,
                                    &window_start.to_rfc3339(),
                                    &window_end.to_rfc3339()
                                ], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))) {
                                    let needs_update = time_row.0 > 0 && time_row.1 != subject;
                                    (time_row.0 > 0, needs_update, time_row.1)
                                } else {
                                    (false, false, String::new())
                                }
                            }
                        } else {
                            (false, false, String::new())
                        }
                    };

                    if duplicate_info.0 && !duplicate_info.1 {
                        println!("Skipping Outlook event '{}' - duplicate found with same description", outlook_id);
                        continue;
                    } else if duplicate_info.0 && duplicate_info.1 {
                        println!("Updating Outlook event '{}'", outlook_id);

                        // Update the existing event
                        let conn = get_db_connection(app_handle_arc)
                            .map_err(|e| format!("Database connection failed: {}", e))?;

                        // Encrypt the new description before updating
                        let encrypted_description = match crate::encryption_utils::encrypt_user_data_base(
                            app_handle_arc,
                            &user_id,
                            subject.as_bytes(),
                        ) {
                            Ok(encrypted) => base64::engine::general_purpose::STANDARD.encode(&encrypted),
                            Err(e) => {
                                eprintln!("Failed to encrypt description for Outlook update: {}", e);
                                subject.to_string() // Fall back to plain text if encryption fails
                            }
                        };

                        conn.execute(
                            "UPDATE events SET description = ?, participants = ? WHERE (id = ? OR event_id_outlook = ?) AND user_id = ?",
                            (encrypted_description, serde_json::to_string(&participants.unwrap_or_default()).unwrap_or("[]".to_string()), outlook_id, outlook_id, &user_id),
                        ).map_err(|e| format!("Failed to update event: {}", e))?;
                                                
                        println!("✅ Successfully updated Outlook event '{}'", outlook_id);
                        continue;
                    }

                    // Convert the event time to the format expected by the database
                    let event_time_str = event_time.with_timezone(&chrono::Utc).to_rfc3339();
                    
                    let event_json = json!({
                        "id": outlook_id,
                        "user_id": user_id,
                        "description": subject,
                        "time": event_time_str,
                        "alarm": false,
                        "synced": false,
                        "synced_google": false,
                        "synced_outlook": true,
                        "event_id_google": None::<String>,
                        "event_id_outlook": outlook_id,
                        "deleted": false,
                        "recurrence": None::<String>,
                        "participants": participants.unwrap_or_default(),
                    });
                    
                    let _ = save_event(app_handle_arc, event_json.to_string()).await;
                } else {
                    println!("Skipping malformed event - missing required fields");
                    if let Some(debug_id) = item.get("id") {
                        println!("Event ID: {:?}", debug_id);
                    }
                    if let Some(debug_subject) = item.get("subject") {
                        println!("Event subject: {:?}", debug_subject);
                    }
                    if let Some(debug_start) = item.get("start") {
                        println!("Event start: {:?}", debug_start);
                    }
                }
            }
        } else {
            println!("No events found in Outlook API response");
        }
        
        Ok(())
    }

    fn mark_events_synced_outlook(conn: &rusqlite::Connection, events: &[CalendarEvent]) -> Result<(), String> {
        for event in events {
            conn.execute(
                "UPDATE events SET synced_outlook = 1 WHERE id = ? AND user_id = ?",
                (&event.id, &event.user_id),
            )
            .map_err(|e| format!("Failed to mark event as synced to Outlook: {}", e))?;
        }
        Ok(())
    }

    pub async fn refresh_outlook_access_token(&self, _refresh_token: &str, app_handle: &AppHandle) -> Result<String, String> {
        // Use the refresh function that doesn't require client secret
        refresh_outlook_token(app_handle).await
    }

    pub fn update_access_token_file(&self, token_path: &std::path::Path, new_access_token: &str) -> Result<(), String> {
        let mut token_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(token_path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;

        token_json["access_token"] = serde_json::Value::String(new_access_token.to_string());
        std::fs::write(token_path, token_json.to_string()).map_err(|e| e.to_string())
    }
}