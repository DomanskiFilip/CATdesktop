use crate::database_utils::{ get_db_connection, CalendarEvent };
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use crate::user_utils::UserSettings;
use crate::encryption_utils::decrypt_user_data_base;
use base64::Engine;
use tauri_plugin_notification::NotificationExt;
use chrono::{ Duration, Local };
use rrule::{ RRuleSet, Tz };
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tauri::{ AppHandle, Manager, Emitter };
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;
use tokio::time::{ sleep, Duration as TokioDuration };

pub struct NotificationService {
    scheduled_tasks: HashMap<String, JoinHandle<()>>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            scheduled_tasks: HashMap::new(),
        }
    }

    // Stop service and cancel all scheduled tasks //
    pub async fn stop(&mut self) {
        println!("Stopping notification service and cancelling all scheduled tasks...");
        for (task_id, task) in self.scheduled_tasks.drain() {
            println!("Cancelling task: {}", task_id);
            task.abort();
        }
    }

    // Start the notification service //
    pub async fn start(&self, app_handle_arc: Arc<AppHandle>, user_logged_in: bool) {
        println!("Starting notification service...");

        // Schedule notifications for existing events immediately
        if let Err(e) =
            Self::check_and_schedule_all_notifications(&app_handle_arc, user_logged_in).await
        {
            eprintln!("Error scheduling existing notifications: {}", e);
        }

        // Start periodic checking
        let app_handle_ref1 = Arc::clone(&app_handle_arc);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(TokioDuration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;
                if let Err(e) =
                    Self::check_and_schedule_all_notifications(&app_handle_ref1, user_logged_in)
                        .await
                {
                    eprintln!("Error checking notifications: {}", e);
                }
            }
        });
    }

    // Helper method for schedule_event_notifications -> Remove notifications for an event //
    pub async fn remove_event_notifications(&mut self, event_id: &str,) -> Result<(), Box<dyn std::error::Error>> {
        // Cancel warning task
        if let Some(task) = self
            .scheduled_tasks
            .remove(&format!("{}_warning", event_id))
        {
            task.abort();
        }

        // Cancel event task
        if let Some(task) = self.scheduled_tasks.remove(&format!("{}_event", event_id)) {
            task.abort();
        }

        Ok(())
    }

    // Helper method for check_and_schedule_all_notifications -> schedule notifications for a single event //
    pub async fn schedule_event_notifications(&mut self, app_handle_arc: Arc<AppHandle>, event: &CalendarEvent,) -> Result<(), Box<dyn std::error::Error>> {
        println!("Scheduling notifications for event!");

        // Check if the event has an alarm set
        if !event.alarm {
            return Ok(());
        }

        // Remove existing notifications for this event first
        self.remove_event_notifications(&event.id).await?;

        // Handle recurrence if present
        if let Some(recurrence) = &event.recurrence {
            return Box::pin(self.schedule_recurring_event_notifications(app_handle_arc.clone(), event, recurrence)).await;
        }

        // Calculate delays
        let event_time = event.time;
        let now = chrono::Local::now();
        let lead_minutes = std::fs::read_to_string("settings.json")
            .ok()
            .and_then(|content| serde_json::from_str::<UserSettings>(&content).ok())
            .map(|s| s.notification_lead_minutes)
            .unwrap_or(15);

        let warning_delay = (event_time - Duration::minutes(lead_minutes as i64)) - now;
        let event_delay = event_time - now;

        // Check if the event is in the past
        if event_delay.num_seconds() <= 0 {
            return Ok(());
        }

        let event_id = event.id.clone();
        let description = event.description.clone();

        // Schedule 15-minute warning notification
        if warning_delay.num_seconds() > 0 {
            println!(
                "Scheduling {}-minute warning in {} minutes",
                lead_minutes,
                warning_delay.num_minutes()
            );

            let event_user_id = event.user_id.clone();
            let _event_id_clone = event_id.clone();
            let _description_clone = description.clone();

            let app_handle_arc = Arc::clone(&app_handle_arc);
            let warning_task = tokio::spawn(async move {
                sleep(TokioDuration::from_secs(warning_delay.num_seconds() as u64)).await;

                // Decrypt the description before showing the notification
                let decrypted_description = match base64::engine::general_purpose::STANDARD.decode(&_description_clone) {
                    Ok(decoded) => match decrypt_user_data_base(&app_handle_arc, &event_user_id, &decoded) {
                        Ok(decrypted) => String::from_utf8(decrypted).unwrap_or("[UNREADABLE EVENT]".to_string()),
                        Err(_) => "[UNREADABLE EVENT]".to_string(),
                    },
                    Err(_) => "[UNREADABLE EVENT]".to_string(),
                };


                if let Err(e) = app_handle_arc
                    .notification()
                    .builder()
                    .title("Calendar AssistanT - Event Reminder")
                    .body(&format!("Upcoming event in 15 minutes: {}", decrypted_description))
                    .show()
                {
                    eprintln!("Failed to show warning notification: {}", e);
                } else {
                    println!("✅ Warning notification shown successfully");
                    match app_handle_arc.emit("open-smartfeatures", serde_json::json!({ "eventId": _event_id_clone })) {
                        Ok(_) => println!("✅ open-smartfeatures event emitted"),
                        Err(e) => eprintln!("❌ Failed to emit open-smartfeatures: {}", e),
                    }
                }
            });

            self.scheduled_tasks
                .insert(format!("{}_warning", event_id), warning_task);
        }

        // Schedule event time notification
        if event_delay.num_seconds() > 0 {
            println!(
                "Scheduling event notification in {} minutes",
                event_delay.num_minutes()
            );

            let event_user_id = event.user_id.clone();
            let _event_id_clone = event_id.clone();
            let _description_clone = description.clone();

            let app_handle_arc = Arc::clone(&app_handle_arc);
            let event_task = tokio::spawn(async move {
                sleep(TokioDuration::from_secs(event_delay.num_seconds() as u64)).await;

                // Decrypt the description before showing the notification
                let decrypted_description = match base64::engine::general_purpose::STANDARD.decode(&_description_clone) {
                    Ok(decoded) => match decrypt_user_data_base(&app_handle_arc, &event_user_id, &decoded) {
                        Ok(decrypted) => String::from_utf8(decrypted).unwrap_or("[UNREADABLE EVENT]".to_string()),
                        Err(_) => "[UNREADABLE EVENT]".to_string(),
                    },
                    Err(_) => "[UNREADABLE EVENT]".to_string(),
                };

                if let Err(e) = app_handle_arc
                    .notification()
                    .builder()
                    .title("Calendar AssistanT - Event Now")
                    .body(&format!("Event now: {}", decrypted_description))
                    .show()
                {
                    eprintln!("Failed to show event notification: {}", e);
                } else {
                    println!("✅ Event notification shown successfully");
                    match app_handle_arc.emit("open-smartfeatures", serde_json::json!({ "eventId": _event_id_clone })) {
                        Ok(_) => println!("✅ open-smartfeatures event emitted"),
                        Err(e) => eprintln!("❌ Failed to emit open-smartfeatures: {}", e),
                    }
                }
            });

            self.scheduled_tasks
                .insert(format!("{}_event", event_id), event_task);
        }

        println!("Total scheduled tasks: {}", self.scheduled_tasks.len());
        Ok(())
    }

    // Method to check database and schedule notifications for all upcoming events //
    pub async fn check_and_schedule_all_notifications(app_handle_arc: &Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
        println!("Checking for upcoming events to schedule notifications...");

        // Verify user is actually logged in before proceeding
        if !user_logged_in {
            println!("User not logged in, skipping notification scheduling.");
            return Ok(());
        }

        // Get user ID
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

        // Get events using a blocking task to avoid Send issues
        let events = {
            let app_handle_ref1 = Arc::clone(app_handle_arc);
            tokio::task::spawn_blocking(move || -> Result<Vec<CalendarEvent>, String> {
                let conn = get_db_connection(&app_handle_ref1)
                    .map_err(|e| e.to_string())?;
                
                let now = chrono::Local::now();
                let next_24_hours = now + Duration::hours(24);
                
                let mut query = conn.prepare(
                    "SELECT id, user_id, description, time, alarm, synced, synced_google, deleted, recurrence, participants
                    FROM events 
                    WHERE deleted = FALSE AND alarm = TRUE AND time > ?1 AND time <= ?2 AND user_id = ?3"
                ).map_err(|e| e.to_string())?;

                let events: Vec<CalendarEvent> = query
                  .query_map([&now.to_rfc3339(), &next_24_hours.to_rfc3339(), &user_id], |row| CalendarEvent::from_row(row))
                  .map_err(|e| e.to_string())?
                  .collect::<Result<Vec<_>, _>>()
                  .map_err(|e| e.to_string())?;


                Ok(events)
            }).await.map_err(|e| e.to_string())?
        }?;

        println!("Found {} events with alarms in next 24 hours", events.len());

        // Access the notification service and schedule each event
        if let Some(service_state) =
            app_handle_arc.try_state::<Arc<TokioMutex<Option<NotificationService>>>>()
        {
            let lock_future = service_state.lock();
            // Use a timeout to avoid indefinite waiting
            let mut service_guard =
                match tokio::time::timeout(std::time::Duration::from_secs(5), lock_future).await {
                    Ok(guard) => guard,
                    Err(_) => {
                        println!(
                            "Timed out waiting for notification service lock - possible deadlock"
                        );
                        return Err("Timed out waiting for notification service lock".to_string());
                    }
                };

            if let Some(service) = service_guard.as_mut() {
                for (_index, event) in events.iter().enumerate() {
                    if let Err(e) = service.schedule_event_notifications(app_handle_arc.clone(), &event).await {
                        eprintln!(
                            "Failed to schedule notification for event {}: {}",
                            event.id, e
                        );
                    }
                }
                Ok(())
            } else {
                println!("Notification service is None!");
                Ok(())
            }
        } else {
            println!("Could not get notification service state!");
            Ok(())
        }
    }

    // Method to schedule notifications for recurring events //
    pub async fn schedule_recurring_event_notifications(&mut self, app_handle_arc: Arc<AppHandle>, event: &CalendarEvent, recurrence: &str,) -> Result<(), Box<dyn std::error::Error>> {
        println!("Scheduling notifications for recurring event with rule: {}", recurrence);

        // Initialize RRule parser
        let rrule_str = if recurrence.starts_with("RRULE:") {
            recurrence.to_string()
        } else {
            format!("RRULE:{}", recurrence)
        };

        // Parse the RRULE
        let rrule = match rrule::RRule::from_str(&rrule_str) {
            Ok(rule) => rule,
            Err(e) => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to parse RRule: {}", e),
                )))
            }
        };

        // Setup the RRULE with the event start time - use Local timezone
        let tz = Tz::LOCAL;
        let dt_start = event.time.with_timezone(&tz);

        // Create a new RRuleSet with the start time
        let rruleset = RRuleSet::new(dt_start);

        // Validate the rrule and add it to the set
        match rrule.validate(dt_start) {
            Ok(validated_rrule) => {
                let _ = rruleset.clone().rrule(validated_rrule);
            }
            Err(e) => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Invalid RRule: {:?}", e),
                )))
            }
        };

        // Calculate occurrences (limit to 5)
        let occurrences = rruleset.all(5);
        let now = chrono::Local::now();

        // Filter out past occurrences
        let future_occurrences: Vec<_> = occurrences
            .dates
            .iter()
            .filter(|date| *date > &now)
            .cloned()
            .collect();

        if future_occurrences.is_empty() {
            println!("No future occurrences found for recurring event");
            return Ok(());
        }

        println!("Found {} future occurrences", future_occurrences.len());

        // Schedule notifications for each occurrence
        for (i, occurrence_time) in future_occurrences.into_iter().enumerate() {
            // Create a single instance of the recurring event
            let instance_id = format!("{}_instance_{}", event.id, i);
            let instance_event = CalendarEvent {
                id: instance_id,
                user_id: event.user_id.clone(),
                description: event.description.clone(),
                time: occurrence_time.with_timezone(&Local),
                alarm: event.alarm,
                synced: event.synced,
                synced_google: event.synced_google,
                synced_outlook: event.synced_outlook,
                deleted: event.deleted,
                recurrence: None::<String>,
                participants: None,
            };

            // Schedule the notification for this instance
            if let Err(e) = self.schedule_event_notifications(app_handle_arc.clone(), &instance_event).await {
                eprintln!(
                    "Failed to schedule notification for recurring instance {}: {}",
                    instance_event.id, e
                );
            }
        }

        println!(
            "Total scheduled tasks after adding recurring event: {}",
            self.scheduled_tasks.len()
        );
        Ok(())
    }
}