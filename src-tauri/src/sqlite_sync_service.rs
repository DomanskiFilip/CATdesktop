use crate::sqlite::CalendarEvent;
use rusqlite::Connection;
use std::time::Duration;
use tokio::time;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

// Function to get the database platform-agnostic path
fn get_db_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle.path().app_data_dir() 
        .map(|path| path.join("calendar.db"))
        .map_err(|e| format!("Failed to get app data directory: {}", e))
}


pub async fn start_sync_service(app_handle: &AppHandle) {
    // Perform initial sync on app start
    if let Err(e) = sync_events(&app_handle).await {
        eprintln!("Initial sync failed: {}", e);
    }

    let mut interval = time::interval(Duration::from_secs(300)); // Sync every 5 minutes
    
    loop {
        interval.tick().await;
        if let Err(e) = sync_events(&app_handle).await {
            eprintln!("Sync failed: {}", e);
        }
    }
}

async fn sync_events(app_handle: &AppHandle) -> Result<(), String> {
    let db_path = get_db_path(app_handle)?;
    let conn = Connection::open(db_path)
        .map_err(|e| e.to_string())?;
    
    // get unsynced events
    let mut unsynced = conn.prepare(
        "SELECT * FROM events WHERE synced = FALSE AND deleted = FALSE"
    ).map_err(|e| e.to_string())?;
    
    let events: Vec<CalendarEvent> = unsynced.query_map([], |row| {
        Ok(CalendarEvent {
            id: row.get(0)?,
            description: row.get(1)?,
             time: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))?.with_timezone(&chrono::Utc),
            synced: row.get(3)?,
            deleted: row.get(4)?
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    // batch send unsynced events to DynamoDB
    if !events.is_empty() {
        match send_to_dynamodb(&events).await {
            Ok(_) => {
                let mut stmt = conn.prepare(
                    "UPDATE events SET synced = TRUE WHERE id = ?"
                ).map_err(|e| e.to_string())?;
                
                for event in events {
                    stmt.execute([event.id])
                        .map_err(|e| e.to_string())?;
                }
            }
            Err(e) => eprintln!("Failed to sync events: {}", e)
        }
    }

    Ok(())
}

async fn send_to_dynamodb(events: &[CalendarEvent]) -> Result<(), String> {
    // TODO: Implement DynamoDB sync
    // For now, just return Ok to test the flow
    Ok(())
}