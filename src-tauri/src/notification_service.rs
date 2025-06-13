use notify_rust::Notification;
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::sqlite::{get_db_connection, CalendarEvent};
use tauri::AppHandle;
use chrono::{DateTime, Utc, Duration, Datelike, Timelike};
use std::collections::HashMap;
use uuid::Uuid;

pub struct NotificationService {
    scheduler: JobScheduler,
    scheduled_jobs: HashMap<String, Uuid>,
}

impl NotificationService {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self {
            scheduler,
            scheduled_jobs: HashMap::new(),
        })
    }

    pub async fn start(&mut self, app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        self.scheduler.start().await?;
        
        // Schedule a job to check for upcoming events every minute
        let job = Job::new_async("0 * * * * *", move |_uuid, _l| {
            let app_handle = app_handle.clone();
            Box::pin(async move {
                if let Err(e) = check_and_schedule_notifications(&app_handle).await {
                    eprintln!("Error checking notifications: {}", e);
                }
            })
        })?;
        
        self.scheduler.add(job).await?;
        Ok(())
    }

    pub async fn schedule_event_notifications(&mut self, event: &CalendarEvent) -> Result<(), Box<dyn std::error::Error>> {
        if !event.alarm {
            return Ok(());
        }

        let event_time = event.time;
        let now = Utc::now();
        
        // Schedule 15-minute warning
        let warning_time = event_time - Duration::minutes(15);
        if warning_time > now {
            let warning_job = create_notification_job(
                &event.id,
                &event.description,
                warning_time,
                true
            )?;
            let job_id = self.scheduler.add(warning_job).await?;
            self.scheduled_jobs.insert(format!("{}_warning", event.id), job_id);
        }

        // Schedule event time notification
        if event_time > now {
            let event_job = create_notification_job(
                &event.id,
                &event.description,
                event_time,
                false
            )?;
            let job_id = self.scheduler.add(event_job).await?;
            self.scheduled_jobs.insert(format!("{}_event", event.id), job_id);
        }

        Ok(())
    }

    pub async fn remove_event_notifications(&mut self, event_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(warning_id) = self.scheduled_jobs.remove(&format!("{}_warning", event_id)) {
            self.scheduler.remove(&warning_id).await?;
        }
        if let Some(event_id) = self.scheduled_jobs.remove(&format!("{}_event", event_id)) {
            self.scheduler.remove(&event_id).await?;
        }
        Ok(())
    }
}

// Helper function to create a notification job
fn create_notification_job(
    event_id: &str,
    description: &str,
    notification_time: DateTime<Utc>,
    is_warning: bool
) -> Result<Job, Box<dyn std::error::Error>> {
    let cron_expression = format!(
        "{} {} {} {} {} *",
        notification_time.second(),
        notification_time.minute(),
        notification_time.hour(),
        notification_time.day(),
        notification_time.month()
    );

    let description = description.to_string();
    let event_id = event_id.to_string();
    
    let job = Job::new_async(cron_expression.as_str(), move |_uuid, _l| {
        let description = description.clone();
        let event_id = event_id.clone();
        Box::pin(async move {
            let title = if is_warning {
                "Event Reminder - 15 minutes"
            } else {
                "Event Now"
            };
            
            let body = if is_warning {
                format!("Upcoming event in 15 minutes: {}", description)
            } else {
                format!("Event now: {}", description)
            };

            if let Err(e) = Notification::new()
                .summary(title)
                .body(&body)
                .timeout(0) // Persistent notification
                .show()
            {
                eprintln!("Failed to show notification: {}", e);
            }
        })
    })?;

    Ok(job)
}

async fn check_and_schedule_notifications(app_handle: &AppHandle) -> Result<(), String> {
    let conn = get_db_connection(app_handle)
        .map_err(|e| e.to_string())?;
    
    let now = Utc::now();
    let next_24_hours = now + Duration::hours(24);
    
    let mut stmt = conn.prepare(
        "SELECT id, description, time, alarm, synced, deleted 
         FROM events 
         WHERE deleted = FALSE AND alarm = TRUE AND time BETWEEN ?1 AND ?2"
    ).map_err(|e| e.to_string())?;

    let _events = stmt.query_map([now.to_rfc3339(), next_24_hours.to_rfc3339()], |row| {
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

    // Here you would schedule notifications for these events
    // This requires access to the global notification service
    
    Ok(())
}