mod ai_assistant;
mod ai_smart_features;
mod ai_speech_to_text;
mod api_utils;
mod auto_login;
mod database_sync_service;
mod database_utils;
mod encryption_utils;
mod credential_utils;
mod google_oauth;
mod outlook_oauth;
mod google_sync_service;
mod outlook_sync_service;
mod login;
mod notification_service;
mod register;
mod theme_utils;
mod token_utils;
mod user_utils;
mod weather_service;


use crate::ai_smart_features::AISmartFeaturesService;
use crate::database_sync_service::DbSyncService;
use crate::database_utils::CalendarEvent;
use crate::google_sync_service::GoogleSyncService;
use crate::outlook_sync_service::OutlookSyncService;
use crate::notification_service::NotificationService;
use crate::weather_service::get_weekly_weather;
use ai_speech_to_text::transcribe_audio;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use auto_launch::AutoLaunchBuilder;
use base64::Engine;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::menu::{Menu, MenuItem};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

pub type AppConfigState = Arc<crate::api_utils::AppConfig>;
pub type NotificationServiceState = Arc<Mutex<Option<NotificationService>>>;
pub type DbSyncServiceState = Arc<Mutex<Option<DbSyncService>>>;
pub type GoogleSyncServiceState = Arc<Mutex<Option<GoogleSyncService>>>;
pub type OutlookSyncServiceState = Arc<Mutex<Option<OutlookSyncService>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub sender: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Default)]
pub struct UserLocation {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBundle {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub user_id: Option<String>,
    pub database_token: Option<String>,
}

static TOKEN_CACHE: once_cell::sync::Lazy<Mutex<TokenBundle>> = once_cell::sync::Lazy::new(|| {
    Mutex::new(TokenBundle {
        access_token: None,
        refresh_token: None,
        user_id: None,
        database_token: None,
    })
});

#[tauri::command]
async fn set_tokens_for_autologin(tokens_json: String) {
    let parsed: Result<TokenBundle, _> = serde_json::from_str(&tokens_json);
    if let Ok(tokens) = parsed {
        let mut cache = TOKEN_CACHE.lock().await;
        cache.access_token = tokens.access_token.clone();
        cache.refresh_token = tokens.refresh_token.clone();
        cache.user_id = tokens.user_id.clone();
        cache.database_token = tokens.database_token.clone();

        // Validate tokens are not empty
        if let (Some(access), Some(refresh)) = (&tokens.access_token, &tokens.refresh_token) {
            if !access.is_empty() && !refresh.is_empty() {
                println!("Valid tokens cached successfully");
            } else {
                println!("Warning: Empty tokens being cached");
            }
        }

        // Optionally decode and cache database_token as bytes if needed
        if let Some(db_token_b64) = tokens.database_token {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(db_token_b64) {
                if bytes.len() == 32 {
                    let mut db_token = [0u8; 32];
                    db_token.copy_from_slice(&bytes);
                    let mut db_cache = crate::login::DATABASE_TOKEN.lock().unwrap();
                    *db_cache = Some(db_token);
                }
            }
        }
    } else {
        println!("Failed to parse tokens JSON: {:?}", parsed.err());
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
async fn read_tokens_from_cache() -> Option<(String, String, Option<[u8; 32]>)> {
    let cache = TOKEN_CACHE.lock().await;

    // Validate tokens exist and are not empty
    match (&cache.access_token, &cache.refresh_token) {
        (Some(access), Some(refresh)) if !access.is_empty() && !refresh.is_empty() => {
            let database_token = cache.database_token.as_ref().and_then(|b64| {
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .ok()
                    .and_then(|bytes| {
                        if bytes.len() == 32 {
                            let mut arr = [0u8; 32];
                            arr.copy_from_slice(&bytes);
                            Some(arr)
                        } else {
                            None
                        }
                    })
            });
            Some((access.clone(), refresh.clone(), database_token))
        }
        _ => {
            println!("No valid tokens found in cache");
            None
        }
    }
}

static USER_ID_CACHE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

#[tauri::command]
async fn set_user_id_for_backend(user_id: String) {
    let mut cache = USER_ID_CACHE.lock().await;
    *cache = Some(user_id);
}

// Helper for Android/iOS to get user_id from cache
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn get_current_user_id_from_cache() -> Result<String, String> {
    let cache = USER_ID_CACHE.lock().await;
    cache
        .clone()
        .ok_or("User ID not set in backend cache".to_string())
}

// Check login status command
#[tauri::command]
async fn check_login_status(app_handle: tauri::AppHandle) -> Result<bool, String> {
    match crate::auto_login::auto_login_lambda(&app_handle).await {
        Ok(is_logged_in) => Ok(is_logged_in),
        Err(_) => Ok(false),
    }
}

// login user command
#[tauri::command]
async fn login_user(app_handle: tauri::AppHandle, email: String, password: String,) -> Result<String, String> {
    // Wrap in Arc for the async tasks
    let app_handle_arc = Arc::new(app_handle);

    // Attempt login
    let login_result = crate::login::login_user_lambda(&app_handle_arc, email.clone(), password).await?;

    // If login was successful, store the email as user ID and start notification service
    if login_result.contains("\"status\":\"ok\"") {

        // initialize database
        if let Err(e) = database_utils::init_db(&app_handle_arc) {
            eprintln!("Failed to initialize database after login: {}", e);
        }

        let notifications_on = app_handle_arc
            .state::<AppConfigState>()
            .notification_service;
        if notifications_on == true {
            // Start notification service and database sync service asynchronously
            let app_handle_ref1 = Arc::clone(&app_handle_arc);
            tauri::async_runtime::spawn(async move {
                if let Err(e) = start_notification_service(app_handle_ref1, true).await {
                    eprintln!("Failed to start notification service after login: {}", e);
                } else {
                    println!("Notification service started successfully after login.");
                }
            });
        }

        let app_handle_ref2 = Arc::clone(&app_handle_arc);
        tauri::async_runtime::spawn(async move {
            if let Err(e) = start_database_sync_service(app_handle_ref2, true).await {
                eprintln!("Failed to start database sync service after login: {}", e);
            } else {
                println!("Database sync service started successfully after login.");
            }
        });
    }

    Ok(login_result)
}

// register user command
#[tauri::command]
async fn register_user(username: String, password: String) -> Result<String, String> {
    crate::register::register_user_lambda(username, password).await
}

// logout user command
#[tauri::command]
async fn logout_user(app_handle: tauri::AppHandle) -> Result<bool, String> {
    // Clear tokens first
    crate::token_utils::clear_tokens(&app_handle)?;

    // Clear current user ID
    user_utils::clear_current_user_id(&app_handle)?;

    // Wrap in Arc for the async tasks
    let app_handle_arc = Arc::new(app_handle);

    // Stop notification service asynchronously
    let app_handle_ref1 = Arc::clone(&app_handle_arc);
    tokio::spawn(async move {
        let notification_state = app_handle_ref1.state::<NotificationServiceState>();
        let mut service_guard = notification_state.lock().await;

        if let Some(mut existing_service) = service_guard.take() {
            println!("Stopping existing notification service...");
            existing_service.stop().await;
        }
    });
    // Stop database sync service asynchronously
    let app_handle_ref2 = Arc::clone(&app_handle_arc);
    tokio::spawn(async move {
        let db_state = app_handle_ref2.state::<DbSyncServiceState>();
        let mut service_guard = db_state.lock().await;

        if let Some(mut existing_service) = service_guard.take() {
            println!("Stopping existing database sync service...");
            existing_service.stop().await;
        }
    });

    // emit ("auto-login-completed", false) to notify the frontend that auto-login is no longer valid
    let _ = app_handle_arc.emit("auto-login-completed", false);
    Ok(true)
}

// save and load theme commands
#[tauri::command]
async fn save_theme(app_handle: tauri::AppHandle, theme: String) -> Result<(), String> {
    theme_utils::save_theme(app_handle, theme).await
}

#[tauri::command]
async fn load_theme(app_handle: tauri::AppHandle) -> Result<String, String> {
    theme_utils::load_theme(app_handle).await
}

// save, load and delete event commands
#[tauri::command]
async fn save_event(app_handle: tauri::AppHandle, event: String) -> Result<(), String> {
    database_utils::save_event(&app_handle, event).await
}

#[tauri::command]
async fn get_events(app_handle: tauri::AppHandle) -> Result<Vec<String>, String> {
    database_utils::get_events(&app_handle)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_event(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    database_utils::delete_event(&app_handle, id)
        .await
        .map_err(|e| e.to_string())
}

// clean old events comand
#[tauri::command]
async fn clean_old_events(app_handle: tauri::AppHandle) -> Result<(), String> {
    database_utils::clean_old_events(&app_handle)
        .await
        .map_err(|e| e.to_string())
}

// google oauth2 functionalities
const TIMEOUT: u64 = 120;

#[tauri::command]
async fn run_oauth2_flow(app_handle: tauri::AppHandle) -> Result<String, String> {
    crate::google_oauth::oauth2_flow(&app_handle, TIMEOUT).await
}

#[tauri::command]
async fn run_outlook_oauth2_flow(app_handle: tauri::AppHandle) -> Result<String, String> {
    crate::outlook_oauth::outlook_oauth2_flow(&app_handle, TIMEOUT).await
}

#[tauri::command]
fn get_oauth_timeout() -> u64 {
    TIMEOUT
}

// Setup auto-launch command
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
async fn setup_auto_launch() -> Result<(), String> {
    // Enable auto-launch in both debug and release builds for testing
    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;

    let auto = AutoLaunchBuilder::new()
        .set_app_name("CAT - Calendar Assistant")
        .set_app_path(&app_path.to_string_lossy())
        .set_use_launch_agent(false) // Use registry on Windows instead of launch agent
        .build()
        .map_err(|e| format!("Failed to build auto-launch: {}", e))?;

    // Check if already enabled to avoid errors
    if auto.is_enabled().map_err(|e| e.to_string())? {
        return Ok(());
    }

    auto.enable().map_err(|e| format!("Failed to enable auto-launch: {}", e))?;
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
async fn disable_auto_launch() -> Result<(), String> {
    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;

    let auto = AutoLaunchBuilder::new()
        .set_app_name("CAT - Calendar Assistant")
        .set_app_path(&app_path.to_string_lossy())
        .build()
        .map_err(|e| e.to_string())?;

    auto.disable().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
async fn check_auto_launch_status() -> Result<bool, String> {
    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;

    let auto = AutoLaunchBuilder::new()
        .set_app_name("CAT - Calendar Assistant")
        .set_app_path(&app_path.to_string_lossy())
        .build()
        .map_err(|e| e.to_string())?;

    auto.is_enabled().map_err(|e| e.to_string())
}

// save user coordinates command //
#[tauri::command]
async fn set_user_coordinates(state: State<'_, Mutex<UserLocation>>, latitude: f64, longitude: f64,) -> Result<(), String> {
    let mut loc = state.lock().await;
    loc.latitude = latitude;
    loc.longitude = longitude;
    Ok(())
}

#[tauri::command]
async fn set_notification_service(app_handle: tauri::AppHandle, enabled: bool, lead_minutes: Option<u32>,) -> Result<(), String> {
    user_utils::set_notification_service(app_handle, enabled, lead_minutes)
        .await
        .map_err(|e| format!("Failed to set notification service: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn set_notification_lead_time(app_handle: tauri::AppHandle, lead_minutes: u32,) -> Result<(), String> {
    user_utils::set_notification_lead_time(app_handle, lead_minutes)
        .await
        .map_err(|e| format!("Failed to set notification lead time: {}", e))
}

// ai assistant comands //
#[tauri::command]
async fn process_ai_message(app_handle: AppHandle, query: String, conversation_history: String,) -> Result<String, String> {
    let parsed_history: Option<Vec<ConversationMessage>> = if conversation_history.is_empty() {
        None
    } else {
        match serde_json::from_str(&conversation_history) {
            Ok(history) => Some(history),
            Err(e) => {
                println!(
                    "Failed to parse conversation_history: {} | Raw: {}",
                    e, conversation_history
                );
                None
            }
        }
    };

    // Call the AI processing logic
    match crate::ai_assistant::process_user_query(&app_handle, query, parsed_history).await {
        Ok(response) => {
            // Ensure we're returning valid, clean JSON
            match serde_json::to_string(&response) {
                Ok(json_string) => Ok(json_string),
                Err(e) => {
                    // If serialization fails, return a fallback response
                    let fallback_response = crate::ai_assistant::LLMResponse {
                        response_text: format!("Failed to serialize response: {}", e),
                        extracted_events: None,
                        action_taken: Some("none".to_string()),
                        confidence: Some(0.0),
                        remaining_requests: None,
                    };
                    match serde_json::to_string(&fallback_response) {
                        Ok(fallback_json) => Ok(fallback_json),
                        Err(_) => Err(format!("Critical serialization error: {}", e)),
                    }
                }
            }
        }
        Err(e) => {
            // Check if this is a rate limit error and create a proper LLMResponse
            if e.contains("Daily AI request limit exceeded") || e.contains("rate limit") {
                let rate_limit_response = crate::ai_assistant::LLMResponse {
                    response_text: format!("🚫 {}", e),
                    extracted_events: None,
                    action_taken: Some("none".to_string()),
                    confidence: Some(1.0),
                    remaining_requests: Some(0),
                };
                match serde_json::to_string(&rate_limit_response) {
                    Ok(json_string) => Ok(json_string),
                    Err(serialize_err) => Err(format!("Failed to serialize rate limit response: {}", serialize_err)),
                }
            } else {
                // For other errors, create a generic error response
                let error_response = crate::ai_assistant::LLMResponse {
                    response_text: "I'm sorry, I encountered an error processing your request.".to_string(),
                    extracted_events: None,
                    action_taken: Some("none".to_string()),
                    confidence: Some(0.0),
                    remaining_requests: None,
                };
                match serde_json::to_string(&error_response) {
                    Ok(json_string) => Ok(json_string),
                    Err(serialize_err) => Err(format!("Failed to serialize error response: {}", serialize_err)),
                }
            }
        }
    }
}

// Tauri command to generate AI email //
#[tauri::command]
async fn generate_ai_email(app_handle: tauri::AppHandle, event_json: String, email_topic: String, participants: Vec<String>) -> Result<String, String> {
    let event: CalendarEvent = serde_json::from_str(&event_json).map_err(|e| format!("Failed to parse event: {}", e))?;
    let service = AISmartFeaturesService::new();
    let response = service.generate_email(&app_handle, event, email_topic, participants).await?;
    serde_json::to_string(&response).map_err(|e| format!("Failed to serialize response: {}", e))
}

// Tauri command to enrich calendar event //
#[tauri::command]
async fn enrich_event(app_handle: AppHandle, event_json: String) -> Result<String, String> {
    let event: CalendarEvent = serde_json::from_str(&event_json).map_err(|e| format!("Failed to parse event: {}", e))?;
    let service = AISmartFeaturesService::new();
    let response = service.enrich_event(&app_handle, event).await?;
    serde_json::to_string(&response).map_err(|e| format!("Failed to serialize response: {}", e))
}

// Tauri command to handle AI enrichment follow-up //
#[tauri::command]
async fn enrichment_followup(app_handle: AppHandle, event_json: String, user_additional_info: String, clarification_history: Option<String>,) -> Result<String, String> {
    let event: CalendarEvent = serde_json::from_str(&event_json).map_err(|e| format!("Failed to parse event: {}", e))?;
    let service = AISmartFeaturesService::new();
    let response = service.enrichment_followup(&app_handle, event, user_additional_info, clarification_history,).await?;
    serde_json::to_string(&response).map_err(|e| format!("Failed to serialize response: {}", e))
}

#[tauri::command]
async fn save_event_from_ai(event_json: String, app_handle: AppHandle) -> Result<(), String> {
    let event: crate::database_utils::CalendarEvent = serde_json::from_str(&event_json).map_err(|e| format!("Failed to parse event: {}", e))?;

    database_utils::save_event(&app_handle, serde_json::to_string(&event).unwrap()).await.map_err(|e| format!("Failed to save event: {}", e))
}

#[tauri::command]
async fn reject_event_suggestion(event_id: String) -> Result<(), String> {
    println!("Event suggestion with ID {} rejected.", event_id);
    Ok(())
}

#[tauri::command]
async fn record_rejection(app_handle: AppHandle, event_suggestion: String, user_query: String, rejection_reason: Option<String>) -> Result<(), String> {
    let user_id = match crate::user_utils::get_current_user_id(&app_handle) {
        Ok(id) => id,
        Err(e) => return Err(format!("Failed to get user ID: {}", e))
    };
    
    let config = crate::api_utils::AppConfig::new()
        .map_err(|e| format!("Failed to get config: {}", e))?;
    let url = format!("{}/llm", config.lambda_base_url);
    
    let mut payload = serde_json::json!({
        "request_type": "record_rejection",
        "user_id": user_id,
        "event_suggestion": event_suggestion,
        "user_query": user_query,
        "rejection_reason": rejection_reason,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "deviceInfo": crate::api_utils::get_device_info(&app_handle)
    });

    // Add access token if available
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if let Ok((access_token, _, _)) = crate::token_utils::read_tokens_from_file(&app_handle).await {
            payload["access_token"] = serde_json::json!(access_token);
        }
    }
    
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        if let Some((access_token, _, _)) = read_tokens_from_cache().await {
            payload["access_token"] = serde_json::json!(access_token);
        }
    }

    let client = reqwest::Client::new();
    let _response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to record rejection: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn delete_all_events(app_handle: tauri::AppHandle) -> Result<usize, String> {
    let conn = match database_utils::get_db_connection(&app_handle) {
        Ok(conn) => conn,
        Err(e) => return Err(e.to_string()),
    };

    // Get current user ID
    let user_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match user_utils::get_current_user_id(&app_handle) {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Ok(0);
                }
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            match user_utils::get_current_user_id_mobile().await {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Ok(0);
                }
            }
        }
    };

    // Mark all events as deleted
    let result = conn.execute("UPDATE events SET deleted = TRUE WHERE user_id = ?", [&user_id],).map_err(|e| e.to_string())?;
    Ok(result as usize)
}

// Start auto-login process //
async fn start_auto_login(app_handle_arc: Arc<AppHandle>) -> Result<bool, String> {
    let login_success = match crate::auto_login::auto_login_lambda(&app_handle_arc).await {
        Ok(result) => result,
        Err(_e) => false,
    };

    // Emit login status to frontend
    app_handle_arc.emit("auto-login-completed", login_success).ok();
    Ok(login_success)
}

// Start notification service //
async fn start_notification_service(app_handle_arc: Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
    // Skip on iOS if there are issues
    #[cfg(target_os = "ios")]
    {
        if !user_logged_in {
            println!("iOS: Skipping notification service - user not logged in");
            return Ok(());
        }
    }

    let notification_state = app_handle_arc.state::<NotificationServiceState>();
    let mut service_guard = notification_state.lock().await;

    // Stop existing service if it exists
    if let Some(mut existing_service) = service_guard.take() {
        existing_service.stop().await;
    }

    // Always create and start a new service
    let service = NotificationService::new();
    service.start(Arc::clone(&app_handle_arc), user_logged_in).await;
    *service_guard = Some(service);
    Ok(())
}

// Start database sync service //
async fn start_database_sync_service(app_handle_arc: Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
    let config_state = app_handle_arc.state::<AppConfigState>();
    if !config_state.enable_database_sync {
        println!("Database sync service is disabled via configuration");
        return Ok(());
    }

    let db_state = app_handle_arc.state::<DbSyncServiceState>();
    let mut service_guard = db_state.lock().await;

    // Stop existing service if it exists
    if let Some(mut existing_service) = service_guard.take() {
        existing_service.stop().await;
    }

    // Always create and start a new service
    match DbSyncService::new(Arc::clone(&config_state)) {
        Ok(mut service) => {
            service
                .start(Arc::clone(&app_handle_arc), user_logged_in)
                .await;
            *service_guard = Some(service);
            println!("Database sync service started successfully");
            Ok(())
        }
        Err(e) => Err(format!("Failed to create database sync service: {}", e)),
    }
}

async fn start_google_sync_service(app_handle_arc: Arc<AppHandle>, user_logged_in: bool,) -> Result<(), String> {
    let config_state = app_handle_arc.state::<AppConfigState>();
    if !config_state.enable_google_sync {
        println!("Google sync service is disabled via configuration");
        return Ok(());
    }

    let db_state = app_handle_arc.state::<GoogleSyncServiceState>();
    let mut service_guard = db_state.lock().await;

    // Stop existing service if it exists
    if let Some(mut existing_service) = service_guard.take() {
        existing_service.stop().await;
    }

    // Always create and start a new service
    let mut service = GoogleSyncService::new(Arc::clone(&config_state));
    service
        .start(Arc::clone(&app_handle_arc), user_logged_in)
        .await;
    *service_guard = Some(service);
    println!("Google sync service started successfully");
    Ok(())
}

async fn start_outlook_sync_service(app_handle_arc: Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
    let config_state = app_handle_arc.state::<AppConfigState>();
    if !config_state.enable_outlook_sync {
        println!("Outlook sync service is disabled via configuration");
        return Ok(());
    }

    let outlook_state = app_handle_arc.state::<OutlookSyncServiceState>();
    let mut service_guard = outlook_state.lock().await;

    // Stop existing service if it exists
    if let Some(mut existing_service) = service_guard.take() {
        existing_service.stop().await;
    }

    // Always create and start a new service
    let mut service = OutlookSyncService::new(Arc::clone(&config_state));
    service
        .start(Arc::clone(&app_handle_arc), user_logged_in)
        .await;
    *service_guard = Some(service);
    println!("Outlook sync service started successfully");
    Ok(())
}

// Schedule event notification command //
#[tauri::command]
async fn schedule_event_notification(event_json: String, app_handle: AppHandle) -> Result<String, String> {
    let app_handle_arc = Arc::new(app_handle);
    let event: crate::database_utils::CalendarEvent =
        serde_json::from_str(&event_json).map_err(|e| format!("Failed to parse event: {}", e))?;

    let notification_state = app_handle_arc.state::<NotificationServiceState>();
    let mut service_guard = notification_state.lock().await;

    if let Some(service) = service_guard.as_mut() {
        service
            .schedule_event_notifications(app_handle_arc.clone(), &event)
            .await
            .map_err(|e| format!("Failed to schedule notification: {}", e))?;
        Ok("Notification scheduled successfully".to_string())
    } else {
        Err("Notification service not available".to_string())
    }
}

// Trigger immediate sync command //
#[tauri::command]
async fn trigger_sync(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_handle_arc = Arc::new(app_handle);
    let config_state = app_handle_arc.state::<AppConfigState>();

    let user_logged_in = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match crate::user_utils::get_current_user_id(&app_handle_arc) {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            match crate::user_utils::get_current_user_id_mobile().await {
                Ok(_) => true,
                Err(_) => false,
            }
        }
    };

    if !user_logged_in {
        return Err("User not logged in".to_string());
    }

    // SEQUENTIAL SYNC: Google first, then Outlook, then DynamoDB
    
    // 1. Sync to Google Calendar first
    if config_state.enable_google_sync {
        let google_state = app_handle_arc.state::<GoogleSyncServiceState>();
        let service_guard = google_state.lock().await;

        if let Some(service) = service_guard.as_ref() {
            if let Err(e) = service.sync_to_google(&app_handle_arc, true).await {
                eprintln!("Failed to sync to Google Calendar: {}", e);
            } else {
                println!("Immediate sync to Google Calendar completed");
            }
        }
    } else {
        println!("Google sync is disabled, skipping Google Calendar sync");
    }

    // Wait 3 seconds before next sync
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 2. Sync from Google Calendar to get any IDs
    if config_state.enable_google_sync {
        let google_state = app_handle_arc.state::<GoogleSyncServiceState>();
        let service_guard = google_state.lock().await;

        if let Some(service) = service_guard.as_ref() {
            if let Err(e) = service.sync_from_google(&app_handle_arc, true).await {
                eprintln!("Failed to sync from Google Calendar: {}", e);
            } else {
                println!("Immediate sync from Google Calendar completed");
            }
        }
    }

    // Wait 3 seconds before Outlook sync
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 3. Sync to Outlook Calendar
    if config_state.enable_outlook_sync {
        let outlook_state = app_handle_arc.state::<OutlookSyncServiceState>();
        let service_guard = outlook_state.lock().await;

        if let Some(service) = service_guard.as_ref() {
            if let Err(e) = service.sync_to_outlook(&app_handle_arc, true).await {
                eprintln!("Failed to sync to Outlook Calendar: {}", e);
            } else {
                println!("Immediate sync to Outlook Calendar completed");
            }
        }
    } else {
        println!("Outlook sync is disabled, skipping Outlook Calendar sync");
    }

    // Wait 3 seconds before sync from Outlook
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 4. Sync from Outlook Calendar
    if config_state.enable_outlook_sync {
        let outlook_state = app_handle_arc.state::<OutlookSyncServiceState>();
        let service_guard = outlook_state.lock().await;

        if let Some(service) = service_guard.as_ref() {
            if let Err(e) = service.sync_from_outlook(&app_handle_arc, true).await {
                eprintln!("Failed to sync from Outlook Calendar: {}", e);
            } else {
                println!("Immediate sync from Outlook Calendar completed");
            }
        }
    }

    // Wait 3 seconds before DynamoDB sync
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // 5. Sync to DynamoDB (all events should have external IDs now)
    if config_state.enable_database_sync {
        let db_state = app_handle_arc.state::<DbSyncServiceState>();
        let service_guard = db_state.lock().await;

        if let Some(service) = service_guard.as_ref() {
            service.sync_to_dynamodb(&app_handle_arc, true).await?;
            println!("Immediate sync to DynamoDB completed");
        }
    } else {
        println!("Database sync is disabled, skipping DynamoDB sync");
    }

    Ok(())
}

// Create system tray
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn create_system_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&quit])?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |_app, event| match event.id().as_ref() {
            "quit" => {
                std::process::exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|_tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button == tauri::tray::MouseButton::Left
                    && button_state == tauri::tray::MouseButtonState::Up
                {
                    if let Some(app) = _tray.app_handle().get_webview_window("main") {
                        if app.is_visible().unwrap_or(false) {
                            let _ = app.hide();
                        } else {
                            let _ = app.show();
                            let _ = app.set_focus();
                        }
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

// Main function to run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = run_impl() {
        eprintln!("Application error: {}", e);
    }
}

pub fn run_impl() -> Result<(), Box<dyn std::error::Error>> {
    let app_config = Arc::new(crate::api_utils::AppConfig::new()?);
    #[allow(unused_mut)] // silence unused mut warning on desktop platforms
    let mut builder = tauri::Builder::default();

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        builder = builder.plugin(tauri_plugin_biometric::init());
    }

    builder
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_machine_uid::init())
        .plugin(tauri_plugin_keystore::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_config.clone() as AppConfigState)
        .manage(tokio::sync::Mutex::new(UserLocation::default()))
        .manage(Arc::new(Mutex::new(None::<NotificationService>)) as NotificationServiceState)
        .manage(Arc::new(Mutex::new(None::<DbSyncService>)) as DbSyncServiceState)
        .manage(Arc::new(Mutex::new(None::<GoogleSyncService>)) as GoogleSyncServiceState)
        .manage(Arc::new(Mutex::new(None::<OutlookSyncService>)) as OutlookSyncServiceState)
        .invoke_handler(tauri::generate_handler![
            check_login_status,
            login_user,
            register_user,
            logout_user,
            save_theme,
            set_tokens_for_autologin,
            set_user_id_for_backend,
            load_theme,
            save_event,
            delete_event,
            get_events,
            clean_old_events,
            trigger_sync,
            get_oauth_timeout,
            run_oauth2_flow,
            run_outlook_oauth2_flow,
            schedule_event_notification,
            process_ai_message,
            generate_ai_email,
            enrich_event,
            enrichment_followup,
            save_event_from_ai,
            reject_event_suggestion,
            record_rejection,
            delete_all_events,
            get_weekly_weather,
            set_user_coordinates,
            set_notification_service,
            set_notification_lead_time,
            transcribe_audio,
            setup_auto_launch,
            disable_auto_launch,
            check_auto_launch_status,
        ])
        .setup(|app| {
             // Initialize the encryption key FIRST, before any other operations
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                if let Err(e) = encryption_utils::initialize_encryption_key(&app.handle()) {
                    eprintln!("Failed to initialize encryption key: {}", e);
                    // Don't fail the app, but log the error
                }
            }

            // Initialize database with iOS-specific error handling
            #[cfg(target_os = "ios")]
            {
                match database_utils::init_db(&app.handle()) {
                    Ok(_) => println!("iOS: Database initialized successfully"),
                    Err(e) => {
                        eprintln!("iOS: Database initialization failed (non-critical): {}", e);
                        // Don't fail the app
                    }
                }
            }
            
            #[cfg(not(target_os = "ios"))]
            {
                match database_utils::init_db(&app.handle()) {
                    Ok(_) => println!("Database initialized successfully"),
                    Err(e) => {
                        eprintln!("Database initialization failed: {}", e);
                        // Don't return error immediately, let the app continue
                    }
                }
            }

            // iOS-specific setup
            #[cfg(target_os = "ios")]
            {
                std::env::set_var("RUST_BACKTRACE", "1");
                std::env::set_var("RUST_LOG", "debug");
            }
            
            // Create system tray (desktop only)
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            create_system_tray(&app.handle())?;

             // Create system tray (desktop only)
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            create_system_tray(&app.handle())?;

            // Setup auto-launch on first run (desktop only)
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Check if running in dev mode
                    let exe_path = std::env::current_exe().unwrap_or_default();
                    let is_dev = exe_path.to_string_lossy().contains("target\\debug") || exe_path.to_string_lossy().contains("target/debug");
                    if is_dev {
                        println!("Skipping auto-launch setup in dev mode.");
                        // Optionally, disable auto-launch if it was previously enabled by dev
                        let _ = disable_auto_launch().await;
                        return;
                    }

                    // Check if this is first run by looking for a flag file
                    let app_data_dir = match app_handle.path().app_data_dir() {
                        Ok(dir) => dir,
                        Err(_) => return,
                    };
                    let first_run_flag = app_data_dir.join("auto_launch_setup.flag");
                    if !first_run_flag.exists() {
                        // First run - set up auto-launch
                        match setup_auto_launch().await {
                            Ok(_) => {
                                println!("Auto-launch configured successfully");
                                if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
                                    eprintln!("Failed to create app data dir: {}", e);
                                } else if let Err(e) = std::fs::write(&first_run_flag, "1") {
                                    eprintln!("Failed to create first run flag: {}", e);
                                }
                            }
                            Err(e) => eprintln!("Failed to setup auto-launch: {}", e),
                        }
                    }
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|app_handle, event| match event {
            tauri::RunEvent::Ready => {
                let app_handle_owned = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let app_handle_arc = Arc::new(app_handle_owned);
                    
                    // Start auto-login with timeout
                    let login_success = {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        {
                            match tokio::time::timeout(
                                std::time::Duration::from_secs(5), // Reduce timeout
                                start_auto_login(Arc::clone(&app_handle_arc))
                            ).await {
                                Ok(Ok(success)) => success,
                                Ok(Err(e)) => {
                                    eprintln!("Auto-login failed: {}", e);
                                    false
                                }
                                Err(_) => {
                                    eprintln!("Auto-login timeout");
                                    false
                                }
                            }
                        }
                        #[cfg(any(target_os = "android", target_os = "ios"))]
                        {
                            // Completely skip auto-login on iOS and Android to prevent crashes
                            println!("iOS/Android: Skipping auto-login entirely");
                            let _ = app_handle_arc.emit("auto-login-completed", false);
                            false
                        }
                    };

                    // Add delays between service starts
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; 

                    // Start notification service with error handling
                    match start_notification_service(Arc::clone(&app_handle_arc), login_success).await {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to start notification service: {}", e),
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    // Start database sync service with error handling
                    match start_database_sync_service(Arc::clone(&app_handle_arc), login_success).await {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to start database sync service: {}", e),
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    // Start google sync service with error handling
                    match start_google_sync_service(Arc::clone(&app_handle_arc), login_success).await {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to start google sync service: {}", e),
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    // Start outlook sync service with error handling
                    match start_outlook_sync_service(Arc::clone(&app_handle_arc), login_success).await {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to start outlook sync service: {}", e),
                    }
                });
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Hide the window instead of closing it
                        if let Some(window) = app_handle.get_webview_window(&label) {
                            let _ = window.hide();
                        }
                        api.prevent_close();
                    }
                }
            }
            _ => {}
        });
    Ok(())
}
