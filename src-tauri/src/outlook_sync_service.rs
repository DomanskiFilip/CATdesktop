pub struct OutlookSyncService {
    client: reqwest::Client,
    config: AppConfig,
}

impl OutlookSyncService {
    // Sync local events to Outlook
    pub async fn sync_to_outlook(&self, app_handle: &Arc<AppHandle>) -> Result<(), String>
    
    // Sync Outlook events to local database
    pub async fn sync_from_outlook(&self, app_handle: &Arc<AppHandle>) -> Result<(), String>
    
    // Handle recurring events (Outlook uses different recurrence format)
    fn convert_recurrence_to_outlook_format(&self, recurrence: &str) -> String
    
    // Convert Outlook events to local format
    fn convert_outlook_event_to_local(&self, outlook_event: &OutlookEvent) -> CalendarEvent

    fn mark_events_synced_outlook(conn: &rusqlite::Connection, events: &[CalendarEvent]) -> Result<(), String> {
        let mut synced = conn
            .prepare("UPDATE events SET synced_outlook = TRUE WHERE id = ?")
            .map_err(|e| e.to_string())?;

        for event in events {
            synced.execute([&event.id]).map_err(|e| {
                format!("Failed to mark event {} as synced_outlook: {}", event.id, e)
            })?;
        }

        Ok(())
    }
}