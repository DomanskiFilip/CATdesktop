use notify_rust::Notification;
use tauri::{AppHandle, Manager};
use chrono::{Duration, DateTime, Utc};
use std::collections::HashMap;
use tokio::time::{sleep, Duration as TokioDuration};
use tokio::task::JoinHandle;
use crate::database_utils::CalendarEvent;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NotificationService {
    scheduled_tasks: HashMap<String, JoinHandle<()>>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            scheduled_tasks: HashMap::new(),
        }
    }

    pub async fn start(&mut self, app_handle: AppHandle) {
        println!("Starting notification service...");
        
        // Start periodic checking
        let app_handle_clone = app_handle.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(TokioDuration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                if let Err(e) = Self::check_and_schedule_all_notifications(&app_handle_clone).await {
                    eprintln!("Error checking notifications: {}", e);
                }
            }
        });
        
        println!("Notification service started successfully");
    }

    pub async fn schedule_event_notifications(&mut self, event: &CalendarEvent) -> Result<(), Box<dyn std::error::Error>> {
        println!("Scheduling notifications for event: {} (alarm: {})", event.description, event.alarm);
        
        if !event.alarm {
            println!("Event has no alarm set, skipping notification scheduling");
            return Ok(());
        }

        // Remove existing notifications for this event first
        self.remove_event_notifications(&event.id).await?;

        let event_time = event.time;
        let now = chrono::Utc::now();
        
        println!("Event time: {}, Current time: {}", event_time, now);
        
        // Calculate delays
        let warning_delay = (event_time - Duration::minutes(15)) - now;
        let event_delay = event_time - now;
        
        // Check if the event is in the past
        if event_delay.num_seconds() <= 0 {
            println!("Event '{}' is in the past, skipping notification scheduling", event.description);
            return Ok(());
        }
        
        let event_id = event.id.clone();
        let description = event.description.clone();
        
        // Schedule 15-minute warning notification
        if warning_delay.num_seconds() > 0 {
            println!("Scheduling 15-minute warning in {} seconds", warning_delay.num_seconds());
            
            let event_id_clone = event_id.clone();
            let description_clone = description.clone();
            
            let warning_task = tokio::spawn(async move {
                sleep(TokioDuration::from_secs(warning_delay.num_seconds() as u64)).await;
                
                println!("🔔 Showing 15-minute warning for: {}", event_id_clone);
                
                // Use notify-rust with proper app branding
                if let Err(e) = Notification::new()
                    .summary("Calendar AssistanT - Event Reminder")
                    .body(&format!("Upcoming event in 15 minutes: {}", description_clone))
                    .appname("Calendar AssistanT")
                    .icon("icons/icon.png")
                    .timeout(0)
                    .show()
                {
                    eprintln!("Failed to show warning notification: {}", e);
                } else {
                    println!("✅ Warning notification shown successfully");
                }
            });
            
            self.scheduled_tasks.insert(format!("{}_warning", event_id), warning_task);
        }
        
        // Schedule event time notification
        if event_delay.num_seconds() > 0 {
            println!("Scheduling event notification in {} seconds", event_delay.num_seconds());
            
            let event_id_clone = event_id.clone();
            let description_clone = description.clone();
            
            let event_task = tokio::spawn(async move {
                sleep(TokioDuration::from_secs(event_delay.num_seconds() as u64)).await;
                
                println!("🔔 Showing event notification for: {}", event_id_clone);
                
                if let Err(e) = Notification::new()
                    .summary("Calendar AssistanT - Event Now")
                    .body(&format!("Event now: {}", description_clone))
                    .appname("Calendar AssistanT")
                    .icon("icons/icon.png")
                    .timeout(0)
                    .show()
                {
                    eprintln!("Failed to show event notification: {}", e);
                } else {
                    println!("✅ Event notification shown successfully");
                }
            });
            
            self.scheduled_tasks.insert(format!("{}_event", event_id), event_task);
        }
        
        println!("Total scheduled tasks: {}", self.scheduled_tasks.len());
        Ok(())
    }

    // Remove notifications for an event
    pub async fn remove_event_notifications(&mut self, event_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("Removing notifications for event: {}", event_id);
        
        // Cancel warning task
        if let Some(task) = self.scheduled_tasks.remove(&format!("{}_warning", event_id)) {
            task.abort();
            println!("Cancelled warning task for {}", event_id);
        }
        
        // Cancel event task
        if let Some(task) = self.scheduled_tasks.remove(&format!("{}_event", event_id)) {
            task.abort();
            println!("Cancelled event task for {}", event_id);
        }
        
        Ok(())
    }

    // Check database and schedule notifications for all upcoming events
    pub async fn check_and_schedule_all_notifications(app_handle: &AppHandle) -> Result<(), String> {
        println!("Checking for upcoming events to schedule notifications...");
        
        // Get events using a blocking task to avoid Send issues
        let events = {
            let app_handle_clone = app_handle.clone();
            tokio::task::spawn_blocking(move || -> Result<Vec<CalendarEvent>, String> {
                let conn = crate::database_utils::get_db_connection(&app_handle_clone)
                    .map_err(|e| e.to_string())?;
                
                let now = Utc::now();
                let next_24_hours = now + Duration::hours(24);
                
                let mut stmt = conn.prepare(
                    "SELECT id, description, time, alarm, synced, deleted 
                     FROM events 
                     WHERE deleted = FALSE AND alarm = TRUE AND time > ?1 AND time <= ?2"
                ).map_err(|e| e.to_string())?;

                let events: Vec<CalendarEvent> = stmt.query_map([now.to_rfc3339(), next_24_hours.to_rfc3339()], |row| {
                    Ok(CalendarEvent {
                        id: row.get(0)?,
                        description: row.get(1)?,
                        time: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            ))?.with_timezone(&Utc),
                        alarm: row.get(3)?,
                        synced: row.get(4)?,
                        deleted: row.get(5)?
                    })
                }).map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;

                Ok(events)
            }).await.map_err(|e| e.to_string())?
        }?;

        println!("Found {} events with alarms in next 24 hours", events.len());

        // Access the notification service and schedule each event
        if let Some(service_state) = app_handle.try_state::<Arc<Mutex<Option<NotificationService>>>>() {
            println!("Got notification service state, acquiring lock...");
            let mut service_guard = service_state.lock().await;
            
            if let Some(service) = service_guard.as_mut() {
                println!("Got notification service, starting to process {} events", events.len());
                for (index, event) in events.iter().enumerate() {
                    println!("Processing event {}/{}: {}", index + 1, events.len(), event.id);
                    if let Err(e) = service.schedule_event_notifications(&event).await {
                        eprintln!("Failed to schedule notification for event {}: {}", event.id, e);
                    }
                }
                println!("Finished processing all events");
            } else {
                println!("Notification service is None!");
            }
        } else {
            println!("Could not get notification service state!");
        }

        // Access the notification service and schedule each event
        if let Some(service_state) = app_handle.try_state::<Arc<Mutex<Option<NotificationService>>>>() {
            let mut service_guard = service_state.lock().await;
            
            if let Some(service) = service_guard.as_mut() {
                for event in events {
                    if let Err(e) = service.schedule_event_notifications(&event).await {
                        eprintln!("Failed to schedule notification for event {}: {}", event.id, e);
                    }
                }
            }
        }
        
        Ok(())
    }
}