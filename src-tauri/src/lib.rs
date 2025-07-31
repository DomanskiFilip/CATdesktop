mod ai_assistant;
mod ai_smart_features;
mod api_utils;
mod auto_login;
mod database_sync_service;
mod database_utils;
mod encryption_utils;
mod google_oauth;
mod google_sync_service;
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
use crate::notification_service::NotificationService;
use crate::weather_service::get_weekly_weather;
#[cfg(not(target_os = "android"))]
use auto_launch::AutoLaunchBuilder;
use base64::Engine;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
#[cfg(not(target_os = "android"))]
use tauri::menu::{Menu, MenuItem};
#[cfg(not(target_os = "android"))]
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

pub type AppConfigState = Arc<crate::api_utils::AppConfig>;
pub type NotificationServiceState = Arc<Mutex<Option<NotificationService>>>;
pub type DbSyncServiceState = Arc<Mutex<Option<DbSyncService>>>;
pub type GoogleSyncServiceState = Arc<Mutex<Option<GoogleSyncService>>>;

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
        cache.access_token = tokens.access_token;
        cache.refresh_token = tokens.refresh_token;
        cache.user_id = tokens.user_id;
        cache.database_token = tokens.database_token.clone();

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
    }
    println!("Tokens set for autologin: {:?}", tokens_json);
}

#[cfg(target_os = "android")]
async fn read_tokens_from_cache() -> Option<(String, String, Option<[u8; 32]>)> {
    let cache = TOKEN_CACHE.lock().await;
    let access_token = cache.access_token.clone().unwrap_or_default();
    let refresh_token = cache.refresh_token.clone().unwrap_or_default();
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
    println!(
        "Tokens loaded from cache: access_token='{}', refresh_token='{}', database_token='{:?}'",
        access_token, refresh_token, database_token
    );
    Some((access_token, refresh_token, database_token))
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
    let login_result =
        crate::login::login_user_lambda(&app_handle_arc, email.clone(), password).await?;

    // If login was successful, store the email as user ID and start notification service
    if login_result.contains("\"status\":\"ok\"") {
        // Store the email as user ID (desktop only)
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        user_utils::save_current_user_id(&app_handle_arc, &email)?;

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
async fn register_user(email: String, password: String) -> Result<String, String> {
    crate::register::register_user_lambda(email, password).await
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
fn get_oauth_timeout() -> u64 {
    TIMEOUT
}

// Setup auto-launch command
#[cfg(not(target_os = "android"))]
#[tauri::command]
#[allow(dead_code)]
async fn setup_auto_launch() -> Result<(), String> {
    // Only enable auto-launch in release builds
    if cfg!(debug_assertions) {
        return Ok(());
    }

    let app_path =
        std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

    let auto = AutoLaunchBuilder::new()
        .set_app_name("Calendar Assistant")
        .set_app_path(&app_path.to_string_lossy())
        .build()
        .map_err(|e| e.to_string())?;

    auto.enable().map_err(|e| e.to_string())?;
    Ok(())
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
                Err(e) => Err(format!("Failed to serialize response: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to process AI message: {}", e)),
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
#[cfg(not(target_os = "android"))]
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
            schedule_event_notification,
            process_ai_message,
            generate_ai_email,
            enrich_event,
            enrichment_followup,
            save_event_from_ai,
            reject_event_suggestion,
            delete_all_events,
            get_weekly_weather,
            set_user_coordinates,
            set_notification_service,
            set_notification_lead_time,
        ])
        .setup(|app| {
            // Initialize database on app startup
            database_utils::init_db(&app.handle()).map_err(|e| e.to_string())?;

            // Create system tray
            #[cfg(not(target_os = "android"))]
            create_system_tray(&app.handle())?;

            // Initialize notification service state
            app.manage(
                Arc::new(Mutex::new(None::<NotificationService>)) as NotificationServiceState
            );

            // Initialize database sync service state
            app.manage(Arc::new(Mutex::new(None::<DbSyncService>)) as DbSyncServiceState);

            // Initialize google sync service state
            app.manage(Arc::new(Mutex::new(None::<GoogleSyncService>)) as GoogleSyncServiceState);
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|app_handle, event| match event {
            tauri::RunEvent::Ready => {
                let app_handle_owned = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let app_handle_arc = Arc::new(app_handle_owned);
                    // Start auto-login
                    let login_success = start_auto_login(Arc::clone(&app_handle_arc))
                        .await
                        .unwrap_or(false);

                    // Start notification service
                    if let Err(e) =
                        start_notification_service(Arc::clone(&app_handle_arc), login_success).await
                    {
                        eprintln!("Failed to start notification service: {}", e);
                    }

                    // Start database sync service using a connection pool or other thread-safe approach
                    if let Err(e) =
                        start_database_sync_service(Arc::clone(&app_handle_arc), login_success)
                            .await
                    {
                        eprintln!("Failed to start database sync service: {}", e);
                    }

                    // Start google sync service using a connection pool or other thread-safe approach
                    if let Err(e) =
                        start_google_sync_service(Arc::clone(&app_handle_arc), login_success).await
                    {
                        eprintln!("Failed to start google sync service: {}", e);
                    }
                });
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                #[cfg(not(target_os = "android"))]
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
