use rusqlite::{Connection, Result};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::fs;
use chrono::{DateTime, Utc};
use rusqlite::Error as SqliteError;

#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub description: String,
    pub time: DateTime<Utc>,
    pub synced: bool,
    pub last_modified: i64,
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

// function to initialize the local database
pub fn init_db() -> Result<(), SqliteError> {
    let app_dir = dirs::data_local_dir()
        .ok_or_else(|| SqliteError::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("Could not get app dir".to_string())
        ))?
        .join("CalendarAssistantApp");
    
    fs::create_dir_all(&app_dir).map_err(|e| SqliteError::SqliteFailure(
        rusqlite::ffi::Error::new(1),
        Some(e.to_string())
    ))?;
    
    let db_path = app_dir.join("calendar.db");
    let conn = Connection::open(&db_path)?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            time TEXT NOT NULL,
            synced BOOLEAN DEFAULT FALSE,
            last_modified INTEGER NOT NULL,
            deleted BOOLEAN DEFAULT FALSE
        )",
        [],
    )?;

    Ok(())
}

// function to get database connection
pub fn get_db_connection() -> Result<Connection, SqliteError> {
    let app_dir = dirs::data_local_dir()
        .ok_or_else(|| SqliteError::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some("Could not get app dir".to_string())
        ))?
        .join("CalendarAssistantApp");
    
    Connection::open(app_dir.join("calendar.db"))
        .map_err(|e| e.into())
}

// function to save events
pub fn save_event(event_json: String) -> Result<(), String> {
    let event = CalendarEvent::from_json(&event_json)?;
    let mut conn = get_db_connection()
        .map_err(|e| format!("Connection error: {}", e.to_string()))?;

    let tx = conn.transaction()
        .map_err(|e| format!("Transaction error: {}", e.to_string()))?;

    tx.execute(
        "INSERT OR REPLACE INTO events (id, title, description, time, synced, last_modified, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            &event.id,
            &event.title,
            &event.description,
            &event.time_to_string(),
            false,
            chrono::Utc::now().timestamp(),
            false
        ),
    ).map_err(|e| format!("Execute error: {}", e.to_string()))?;

    tx.commit().map_err(|e| format!("Commit error: {}", e.to_string()))?;
    println!("Successfully saved event with id: {}", event.id);
    Ok(())
}

// function to get all events
pub fn get_events() -> Result<Vec<String>, SqliteError> {
    let conn = get_db_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, description, time, synced, last_modified, deleted 
         FROM events 
         WHERE deleted = FALSE
         ORDER BY time ASC"
    )?;

    let events = stmt.query_map([], |row| {
        Ok(CalendarEvent {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            time: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))?.with_timezone(&Utc),
            synced: row.get(4)?,
            last_modified: row.get(5)?,
            deleted: row.get(6)?
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
pub fn delete_event(id: String) -> Result<(), SqliteError> {
    let conn = get_db_connection()?;
    conn.execute(
        "DELETE FROM events WHERE id = ?",
        [id]
    )?;
    Ok(())
}

// function to clean old events
pub fn clean_old_events() -> Result<(), SqliteError> {
    let conn = get_db_connection()?;
    let now = chrono::Utc::now();
    let today = now.date_naive();
    
    conn.execute(
        "DELETE FROM events WHERE date(time) < date(?)",
        [today.to_string()]
    )?;
    Ok(())
}