use rusqlite::{Connection, Result};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use std::fs;
use chrono::{DateTime, Utc};
use rusqlite::Error as SqliteError;
use crate::user_utils::get_current_user_id;
use crate::encription_key::{encrypt_user_data, decrypt_user_data};
use base64::{ engine::general_purpose, Engine };

#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub user_id: String, 
    pub description: String,
    pub time: DateTime<Utc>,
    pub alarm: bool,
    pub synced: bool,
    pub deleted: bool,
}

impl CalendarEvent {
    fn time_to_string(&self) -> String {
        self.time.to_rfc3339()
    }

    pub fn from_json(json_str: &str) -> Result<Self, String> {
        let event: CalendarEvent = serde_json::from_str(json_str)
            .map_err(|e| format!("JSON parse error: {}", e.to_string()))?;
        
        Ok(event)
    }
}

// Helper function -> to get platform-agnostic app data dir //
fn get_app_data_dir(app_handle: &AppHandle) -> Result<PathBuf, SqliteError> {
    app_handle.path().app_data_dir()
        .map_err(|e| SqliteError::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some(format!("Could not get app dir: {}", e))
        ))
}

// Helper function -> get database connection //
pub fn get_db_connection(app_handle: &AppHandle) -> Result<Connection, SqliteError> {
    let app_dir = get_app_data_dir(app_handle)?;
    let db_path = app_dir.join("calendar.db");
    
    Connection::open(&db_path)
        .map_err(|e| e.into())
}

// Function to initialize the local database //
pub fn init_db(app_handle: &AppHandle) -> Result<(), SqliteError> {
    let app_dir = get_app_data_dir(app_handle)?;
    
    fs::create_dir_all(&app_dir).map_err(|e| SqliteError::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some(e.to_string())
    ))?;
    
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
            deleted BOOLEAN DEFAULT FALSE
        )"
    )?;

    Ok(())
}

// Function to save events //
pub fn save_event(app_handle: &AppHandle, event_json: String) -> Result<(), String> {
    let mut event = CalendarEvent::from_json(&event_json)?;

    // Get current user ID and assign to event
    let user_id = match get_current_user_id(app_handle) {
        Ok(id) => id,
        Err(e) => return Err(format!("Failed to get current user: {}", e))
    };

    event.user_id = user_id.clone();

    // Encrypt the event description before saving
    let encrypted_description = if !event.description.is_empty() {
        let encryption_result = encrypt_user_data(
            app_handle, 
            &user_id, 
            event.description.as_bytes()
        );
        
        match encryption_result {
            Ok(encrypted) => general_purpose::STANDARD.encode(encrypted),
            Err(e) => return Err(format!("Failed to encrypt event data: {}", e))
        }
    } else {
        String::new()
    };

    let mut conn = get_db_connection(app_handle)
        .map_err(|e| format!("Connection error: {}", e.to_string()))?;

    let tx = conn.transaction()
        .map_err(|e| format!("Transaction error: {}", e.to_string()))?;

    tx.execute(
        "INSERT OR REPLACE INTO events (id, user_id, description, time, alarm, synced, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            &event.id,
            &event.user_id,
            &encrypted_description,
            &event.time_to_string(),
            &event.alarm,
            false,
            false
        ),
    ).map_err(|e| format!("Execute error: {}", e.to_string()))?;

    tx.commit().map_err(|e| format!("Commit error: {}", e.to_string()))?;
    Ok(())
}

// Function to get all events //
pub fn get_events(app_handle: &AppHandle) -> Result<Vec<String>, SqliteError> {
  let conn = match get_db_connection(app_handle) {
    Ok(conn) => conn,
    Err(e) => {
      eprintln!("Database connection error: {}", e);
      return Ok(Vec::new()); // Return empty list on connection error
    }
  };

  // Get current user ID
    let user_id = match get_current_user_id(app_handle) {
        Ok(id) => id,
        Err(_) => return Ok(Vec::new()),  // Return empty list if no user is logged in
    };
  
    let mut query = conn.prepare(
        "SELECT id, user_id, description, time, alarm, synced, deleted 
         FROM events 
         WHERE deleted = FALSE
         AND user_id = ?1
         ORDER BY time ASC"
    )?;

    let rows = query.query_map([&user_id], |row| {
        Ok(CalendarEvent {
            id: row.get(0)?,
            user_id: row.get(1)?,
            description: row.get(2)?,
            time: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))?.with_timezone(&Utc),
            alarm: row.get(4)?,
            synced: row.get(5)?,
            deleted: row.get(6)?
        })
    })?;

    let mut events = Vec::new();
    for row_result in rows {
        // Get the event from the result
        let mut event = row_result?;
        
        // Skip deleted events (if this check is needed)
        if event.deleted {
            continue;
        }
        
        // Decrypt the description
        if !event.description.is_empty() {
            let decoded = match general_purpose::STANDARD.decode(&event.description) {
                Ok(decoded) => decoded,
                Err(_) => {
                    // If base64 decoding fails, assume it's not encrypted (legacy data)
                    event.description.as_bytes().to_vec()
                }
            };
            
            match decrypt_user_data(app_handle, &event.user_id, &decoded) {
                Ok(decrypted) => {
                    if let Ok(s) = String::from_utf8(decrypted) {
                        event.description = s;
                    }
                },
                Err(e) => {
                    // If decryption fails, log the error but keep the encrypted data
                    eprintln!("Failed to decrypt event data: {}", e);
                }
            }
        }

        events.push(event);
    }

    // Convert events to JSON strings
    let events_json: Vec<String> = events.into_iter()
        .map(|event| serde_json::to_string(&event)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(e),
            )))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(events_json)
}

// Function to delete an event //
pub fn delete_event(app_handle: &AppHandle, id: String) -> Result<(), SqliteError> {
    let conn = get_db_connection(app_handle)?;

    // Get current user ID
    let user_id = match get_current_user_id(app_handle) {
        Ok(id) => id,
        Err(_) => return Ok(()),
    };

    conn.execute(
        "UPDATE events SET deleted = TRUE WHERE id = ? AND user_id = ?",
        [id, user_id]
    )?;
    Ok(())
}

// Function to clean old events //
pub fn clean_old_events(app_handle: &AppHandle) -> Result<(), SqliteError> {
    let conn = get_db_connection(app_handle)?;
    let now = chrono::Utc::now();
    
    // Get current user ID
    let user_id = match get_current_user_id(app_handle) {
        Ok(id) => id,
        Err(_) => return Ok(()),
    };

    conn.execute(
        "UPDATE events SET deleted = TRUE WHERE time < ? AND user_id = ?",
        [now.to_rfc3339(), user_id]
    )?;
    Ok(())
}