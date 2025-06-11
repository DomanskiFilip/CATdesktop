use rusqlite::{Connection, Result};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use std::fs;
use chrono::{DateTime, Utc};
use rusqlite::Error as SqliteError;


#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub description: String,
    pub time: DateTime<Utc>,
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

// function to get platform-agnostic app data dir
fn get_app_data_dir(app_handle: &AppHandle) -> Result<PathBuf, SqliteError> {
    app_handle.path().app_data_dir()
        .map_err(|e| SqliteError::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some(format!("Could not get app dir: {}", e))
        ))
}

// function to initialize the local database
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
            description TEXT,
            time TEXT NOT NULL,
            synced BOOLEAN DEFAULT FALSE,
            deleted BOOLEAN DEFAULT FALSE
        )"
    )?;

    Ok(())
}

// function to get database connection
pub fn get_db_connection(app_handle: &AppHandle) -> Result<Connection, SqliteError> {
    let app_dir = get_app_data_dir(app_handle)?;
    let db_path = app_dir.join("calendar.db");
    
    Connection::open(&db_path)
        .map_err(|e| e.into())
}

// function to save events
pub fn save_event(app_handle: &AppHandle, event_json: String) -> Result<(), String> {
    let event = CalendarEvent::from_json(&event_json)?;
    let mut conn = get_db_connection(app_handle)
        .map_err(|e| format!("Connection error: {}", e.to_string()))?;

    let tx = conn.transaction()
        .map_err(|e| format!("Transaction error: {}", e.to_string()))?;

    tx.execute(
        "INSERT OR REPLACE INTO events (id, description, time, synced, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            &event.id,
            &event.description,
            &event.time_to_string(),
            false,
            false
        ),
    ).map_err(|e| format!("Execute error: {}", e.to_string()))?;

    tx.commit().map_err(|e| format!("Commit error: {}", e.to_string()))?;
    Ok(())
}

// function to get all events
pub fn get_events(app_handle: &AppHandle) -> Result<Vec<String>, SqliteError> {
    let conn = get_db_connection(app_handle)?;
    let mut stmt = conn.prepare(
        "SELECT id, description, time, synced, deleted 
         FROM events 
         WHERE deleted = FALSE
         ORDER BY time ASC"
    )?;

    let events = stmt.query_map([], |row| {
        Ok(CalendarEvent {
            id: row.get(0)?,
            description: row.get(1)?,
            time: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))?.with_timezone(&Utc),
            synced: row.get(3)?,
            deleted: row.get(4)?
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

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

// function to delete an event
pub fn delete_event(app_handle: &AppHandle, id: String) -> Result<(), SqliteError> {
    let conn = get_db_connection(app_handle)?;
    conn.execute(
        "UPDATE events SET deleted = TRUE WHERE id = ?",
        [id]
    )?;
    Ok(())
}

// function to clean old events
pub fn clean_old_events(app_handle: &AppHandle) -> Result<(), SqliteError> {
    let conn = get_db_connection(app_handle)?;
    let now = chrono::Utc::now();
    let today = now.date_naive();
    
    conn.execute(
        "UPDATE events SET deleted = TRUE WHERE date(time) < date(?)",
        [today.to_string()]
    )?;
    Ok(())
}