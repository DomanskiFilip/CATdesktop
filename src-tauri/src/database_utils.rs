use crate::encryption_utils::{decrypt_user_data_base, encrypt_user_data_base};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use base64::{engine::general_purpose, Engine};
use chrono::{DateTime, Local, TimeZone};
use rusqlite::Error as SqliteError;
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub user_id: String,
    pub description: String,
    pub time: DateTime<Local>,
    pub alarm: bool,
    pub synced: bool,
    pub synced_google: bool,
    pub synced_outlook: bool,
    pub deleted: bool,
    pub recurrence: Option<String>,
    pub participants: Option<Vec<String>>,
}

impl CalendarEvent {
    fn time_to_string(&self) -> String {
        self.time.to_rfc3339()
    }

    pub fn from_json(json_str: &str) -> Result<Self, String> {
        // First parse as a generic JSON value to handle potential UTC timestamps
        let mut json_value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("JSON parse error: {}", e.to_string()))?;

        // Convert UTC timestamp to local timezone if needed
        if let Some(time_str) = json_value.get("time").and_then(|v| v.as_str()) {
            // Parse the timestamp (could be UTC or local)
            let parsed_time = if time_str.ends_with('Z')
                || time_str.contains('+')
                || time_str.rfind('-').map_or(false, |i| i > 10)
            {
                // It's already a timezone-aware timestamp, convert to local
                chrono::DateTime::parse_from_rfc3339(time_str)
                    .map_err(|e| format!("Time parse error: {}", e))?
                    .with_timezone(&Local)
            } else {
                // For naive timestamps like "2025-07-08T17:00:00", treat them as already being in local time
                let naive_dt =
                    chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S%.f")
                        .or_else(|_| {
                            chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S")
                        })
                        .map_err(|e| format!("Naive time parse error: {}", e))?;

                // Assume the naive datetime is already in local timezone
                Local
                    .from_local_datetime(&naive_dt)
                    .single()
                    .ok_or("Ambiguous local time")?
            };

            // Update the JSON value with local timezone
            json_value["time"] = serde_json::Value::String(parsed_time.to_rfc3339());
        }

        let event: CalendarEvent = serde_json::from_value(json_value)
            .map_err(|e| format!("JSON parse error: {}", e.to_string()))?;

        Ok(event)
    }

    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        let time_str: String = row.get(3)?;
        let time = chrono::DateTime::parse_from_rfc3339(&time_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Local);

        let participants_json: Option<String> = row.get(10)?;
        let participants = participants_json
            .and_then(|json| {
                match serde_json::from_str::<Vec<String>>(&json) {
                    Ok(vec) => Some(vec),
                    Err(_) => None,
                }
            });

        Ok(CalendarEvent {
            id: row.get(0)?,
            user_id: row.get(1)?,
            description: row.get(2)?,
            time,
            alarm: row.get(4)?,
            synced: row.get(5)?,
            synced_google: row.get(6)?,
            synced_outlook: row.get(7)?,
            deleted: row.get(8)?,
            recurrence: {
                let val: Option<String> = row.get(9)?;
                match val.as_deref() {
                    Some("none") => None,
                    Some("") => None,
                    other => other.map(|s| s.to_string()),
                }
            },
            participants,
        })
    }
}

// Helper function -> to get platform-agnostic app data dir //
fn get_app_data_dir(app_handle: &AppHandle) -> Result<PathBuf, SqliteError> {
    app_handle.path().app_data_dir().map_err(|e| {
        SqliteError::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some(format!("Could not get app dir: {}", e)),
        )
    })
}

// Helper function -> get database connection //
pub fn get_db_connection(app_handle: &AppHandle) -> Result<Connection, SqliteError> {
    let app_dir = get_app_data_dir(app_handle)?;
    let db_path = app_dir.join("calendar.db");

    Connection::open(&db_path).map_err(|e| e.into())
}

// Function to initialize the local database //
pub fn init_db(app_handle: &AppHandle) -> Result<(), SqliteError> {
    let app_dir = get_app_data_dir(app_handle)?;

    fs::create_dir_all(&app_dir).map_err(|e| {
        SqliteError::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string()))
    })?;

    let db_path = app_dir.join("calendar.db");
    let conn = Connection::open(&db_path)?;

    // Enable WAL mode for better cross-platform concurrency
    conn.execute_batch("PRAGMA journal_mode=WAL")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            description TEXT,
            time TEXT NOT NULL,
            alarm BOOLEAN DEFAULT FALSE,
            synced BOOLEAN DEFAULT FALSE,
            synced_google BOOLEAN DEFAULT FALSE,
            synced_outlook BOOLEAN DEFAULT FALSE,
            deleted BOOLEAN DEFAULT FALSE,
            recurrence TEXT DEFAULT NULL,
            participants TEXT DEFAULT NULL
        )",
    )?;

    Ok(())
}

// Function to save events //
pub async fn save_event(app_handle: &AppHandle, event_json: String) -> Result<(), String> {

    println!("Saving event: {}", event_json);
    let mut event = CalendarEvent::from_json(&event_json)?;
    // Get current user ID and assign to event
    let user_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(&app_handle) {
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

    event.user_id = user_id.clone();

    // Extract synced and deleted status from original JSON to determine if they were explicitly provided
    let json_value: serde_json::Value = serde_json::from_str(&event_json)
        .map_err(|_| "Failed to re-parse event JSON".to_string())?;

    let mut conn = get_db_connection(app_handle)
        .map_err(|e| format!("Connection error: {}", e.to_string()))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Transaction error: {}", e.to_string()))?;

    // Check if this is an existing event being updated
    let is_existing_event = {
        let existing_event_query = tx.prepare("SELECT id FROM events WHERE id = ? AND user_id = ?");
        match existing_event_query {
            Ok(mut stmt) => stmt.exists([&event.id, &event.user_id]).unwrap_or(false),
            Err(_) => false,
        }
    };

    // Use the values from JSON if present, otherwise determine based on whether it's a new or existing event
    let synced = if json_value.get("synced").is_some() {
        json_value
            .get("synced")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    } else {
        !is_existing_event
    };

    let synced_google = if json_value.get("synced_google").is_some() {
        json_value
            .get("synced_google")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    } else {
        !is_existing_event
    };

    let synced_outlook = if json_value.get("synced_outlook").is_some() {
        json_value
            .get("synced_outlook")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    } else {
        !is_existing_event
    };

    let deleted = json_value
        .get("deleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let encrypted_description =
        match encrypt_user_data_base(app_handle, &user_id, event.description.as_bytes()) {
            Ok(encrypted) => general_purpose::STANDARD.encode(encrypted),
            Err(e) => return Err(format!("Failed to encrypt event data: {}", e)),
        };

    tx.execute(
        "INSERT OR REPLACE INTO events (id, user_id, description, time, alarm, synced, synced_google, synced_outlook, deleted, recurrence, participants)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        (
            &event.id,
            &event.user_id,
            &encrypted_description,
            &event.time_to_string(),
            &event.alarm,
            synced,
            synced_google,
            synced_outlook,
            deleted,
            event.recurrence.as_deref(),
            Some(match &event.participants { Some(p) => serde_json::to_string(p).unwrap_or("[]".to_string()), None => "[]".to_string(),}),
        ),
    ).map_err(|e| format!("Execute error: {}", e.to_string()))?;

    tx.commit()
        .map_err(|e| format!("Commit error: {}", e.to_string()))?;
    println!("✅ Event saved successfully");
    Ok(())
}

// Function to get all events //
pub async fn get_events(app_handle: &AppHandle) -> Result<Vec<String>, SqliteError> {
    let conn = match get_db_connection(app_handle) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Database connection error: {}", e);
            return Ok(Vec::new()); // Return empty list on connection error
        }
    };

    // Get current user ID
    let user_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(&app_handle) {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Ok(Vec::new());
                }
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            match get_current_user_id_mobile().await {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Ok(Vec::new());
                }
            }
        }
    };

    let mut query = conn.prepare(
        "SELECT id, user_id, description, time, alarm, synced, synced_google, synced_outlook, deleted, recurrence, participants
        FROM events 
        WHERE deleted = FALSE
        AND user_id = ?1
        ORDER BY time ASC",
    )?;

    let rows = query.query_map([&user_id], CalendarEvent::from_row)?;

    let mut events = Vec::new();
    for row_result in rows {
        // Get the event from the result
        let mut event = row_result?;

        // Skip deleted events (if this check is needed)
        if event.deleted {
            continue;
        }

        let decoded = match general_purpose::STANDARD.decode(&event.description) {
            Ok(decoded) => decoded,
            Err(_) => {
                // If base64 decoding fails, assume it's not encrypted (legacy data)
                event.description.as_bytes().to_vec()
            }
        };

        match decrypt_user_data_base(app_handle, &event.user_id, &decoded) {
            Ok(decrypted) => {
                if let Ok(s) = String::from_utf8(decrypted) {
                    event.description = s;
                }
            }
            Err(e) => {
                // If decryption fails, log the error but keep the encrypted data
                eprintln!("Failed to decrypt event data: {}", e);
            }
        }

        events.push(event);
    }

    // Convert events to JSON strings
    let events_json: Vec<String> = events
        .into_iter()
        .map(|event| {
            serde_json::to_string(&event).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(events_json)
}

// Function to delete an event //
pub async fn delete_event(app_handle: &AppHandle, id: String) -> Result<(), SqliteError> {
    let conn = get_db_connection(app_handle)?;

    // Get current user ID
    let user_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(&app_handle) {
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

    conn.execute(
        "UPDATE events SET deleted = TRUE WHERE id = ? AND user_id = ?",
        [id, user_id],
    )?;
    Ok(())
}

// Function to clean old events //
pub async fn clean_old_events(app_handle: &AppHandle) -> Result<(), SqliteError> {
    let conn = get_db_connection(app_handle)?;
    let now = chrono::Local::now();

    // Get current user ID
    let user_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(&app_handle) {
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

    conn.execute(
        "UPDATE events SET deleted = TRUE WHERE time < ? AND user_id = ?",
        [now.to_rfc3339(), user_id],
    )?;
    Ok(())
}
