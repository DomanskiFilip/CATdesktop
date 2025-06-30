#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub mod window;
mod api_utils;
mod token_utils;
mod theme_utils;
mod database_utils;
mod user_utils;
mod google_oauth;
mod login;
mod register;
mod notification_service;
mod database_sync_service;
mod google_sync_service;
mod encryption_utils;
mod auto_login;
mod ai_assistant;


use tauri::{AppHandle, Manager, Emitter};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem};
use std::sync::Arc;
use tokio::sync::Mutex;
use auto_launch::AutoLaunchBuilder;
use std::env;
use serde::{Serialize, Deserialize};
use crate::notification_service::NotificationService;
use crate::database_sync_service::DbSyncService;
use crate::google_sync_service::GoogleSyncService;

pub type NotificationServiceState = Arc<Mutex<Option<NotificationService>>>;
pub type DbSyncServiceState = Arc<Mutex<Option<DbSyncService>>>;
pub type GoogleSyncServiceState = Arc<Mutex<Option<GoogleSyncService>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

// Check login status command
#[tauri::command]
async fn check_login_status(app_handle: tauri::AppHandle) -> Result<bool, String> {
    match crate::auto_login::auto_login_lambda(&app_handle).await {
        Ok(is_logged_in) => Ok(is_logged_in),
        Err(_) => Ok(false)
    }
}

// login user command
#[tauri::command]
async fn login_user(app_handle: tauri::AppHandle, email: String, password: String) -> Result<String, String> {
// Wrap in Arc for the async tasks
    let app_handle_arc = Arc::new(app_handle);
    
    // Attempt login
    let login_result = crate::login::login_user_lambda(&app_handle_arc, email.clone(), password).await?;

    // If login was successful, store the email as user ID and start notification service
    if login_result.contains("\"status\":\"ok\"") {
        // Store the email as user ID
        user_utils::save_current_user_id(&app_handle_arc, &email)?;

        // Create or load the user's encryption key
        match crate::encryption_utils::create_user_encryption_key(&app_handle_arc, &email) {
            Ok(_) => println!("User encryption key created/loaded successfully"),
            Err(e) => eprintln!("Failed to create/load user encryption key: {}", e),
        }

        // initialize database
        if let Err(e) = database_utils::init_db(&app_handle_arc) {
            eprintln!("Failed to initialize database after login: {}", e);
        }
        
        // Start notification service and database sync service asynchronously
        let app_handle_ref1 = Arc::clone(&app_handle_arc);
        tauri::async_runtime::spawn(async move {
            if let Err(e) = start_notification_service(app_handle_ref1, true).await {
                eprintln!("Failed to start notification service after login: {}", e);
            } else {
                println!("Notification service started successfully after login.");
            }
        });
        
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
    database_utils::save_event(&app_handle, event)
}

#[tauri::command]
async fn get_events(app_handle: tauri::AppHandle) -> Result<Vec<String>, String> {
    database_utils::get_events(&app_handle).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_event(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    database_utils::delete_event(&app_handle, id).map_err(|e| e.to_string())
}

// clean old events comand
#[tauri::command]
async fn clean_old_events(app_handle: tauri::AppHandle) -> Result<(), String> {
    database_utils::clean_old_events(&app_handle).map_err(|e| e.to_string())
}

// google oauth2 functionalities
const TIMEOUT: u64 = 120;

#[tauri::command]
async fn run_oauth2_flow(app_handle: tauri::AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(crate::google_oauth::oauth2_flow(&app_handle, TIMEOUT))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_oauth_timeout() -> u64 {
    TIMEOUT
}

// Setup auto-launch command
#[tauri::command]
async fn setup_auto_launch() -> Result<(), String> {
    // Only enable auto-launch in release builds
    if cfg!(debug_assertions) {
        return Ok(());
    }

    let app_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;
    
    let auto = AutoLaunchBuilder::new()
        .set_app_name("Calendar Assistant")
        .set_app_path(&app_path.to_string_lossy())
        .build()
        .map_err(|e| e.to_string())?;
    
    auto.enable().map_err(|e| e.to_string())?;
    Ok(())
}

// ai assistant comands //
#[tauri::command]
async fn process_ai_message(app_handle: AppHandle, query: String, conversation_history: String) -> Result<String, String> {
    println!("Received conversation_history: {}", conversation_history); 
      let parsed_history: Option<Vec<ConversationMessage>> = if conversation_history.is_empty() {
        None
    } else {
        match serde_json::from_str(&conversation_history) {
            Ok(history) => Some(history),
            Err(e) => {
                println!("Failed to parse conversation_history: {} | Raw: {}", e, conversation_history);
                None
            }
        }
    };

    println!("Received conversation history: {:?}", parsed_history);  
  // Call the AI processing logic
    match crate::ai_assistant::process_user_query(&app_handle, query, parsed_history).await {
        Ok(response) => {
            // Ensure we're returning valid, clean JSON
            match serde_json::to_string(&response) {
                Ok(json_string) => Ok(json_string),
                Err(e) => Err(format!("Failed to serialize response: {}", e)),
            }
        },
        Err(e) => Err(format!("Failed to process AI message: {}", e)),
    }
}

#[tauri::command]
async fn save_event_from_ai(event_json: String, app_handle: AppHandle) -> Result<(), String> {
    let event: crate::database_utils::CalendarEvent = serde_json::from_str(&event_json)
        .map_err(|e| format!("Failed to parse event: {}", e))?;
    
    database_utils::save_event(&app_handle, serde_json::to_string(&event).unwrap())
        .map_err(|e| format!("Failed to save event: {}", e))
}

#[tauri::command]
async fn reject_event_suggestion(event_id: String) -> Result<(), String> {
    println!("Event suggestion with ID {} rejected.", event_id);
    Ok(())
}

#[tauri::command]
async fn trigger_immediate_sync(app_handle: AppHandle) -> Result<(), String> {
    trigger_sync(app_handle).await
}

#[tauri::command]
async fn get_events_for_ai(app_handle: AppHandle) -> Result<Vec<String>, String> {
    get_events(app_handle).await
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
async fn start_notification_service(app_handle_arc: Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
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
async fn start_database_sync_service(app_handle_arc: Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
    let config = crate::api_utils::AppConfig::new()?;
    
    if !config.enable_database_sync {
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
    match DbSyncService::new() {
        Ok(mut service) => {
            service.start(Arc::clone(&app_handle_arc), user_logged_in).await;
            *service_guard = Some(service);
            println!("Database sync service started successfully");
            Ok(())
        },
        Err(e) => Err(format!("Failed to create database sync service: {}", e))
    }
}

async fn start_google_sync_service(app_handle_arc: Arc<AppHandle>, user_logged_in: bool) -> Result<(), String> {
    let config = crate::api_utils::AppConfig::new()?;
    
    if !config.enable_google_sync {
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
    let mut service = GoogleSyncService::new();
    service.start(Arc::clone(&app_handle_arc), user_logged_in).await;
    *service_guard = Some(service);
    println!("Google sync service started successfully");
    Ok(())
}

// Schedule event notification command //
#[tauri::command]
async fn schedule_event_notification( event_json: String, app_handle: AppHandle) -> Result<String, String> {
    let event: crate::database_utils::CalendarEvent = serde_json::from_str(&event_json)
        .map_err(|e| format!("Failed to parse event: {}", e))?;
    
    let notification_state = app_handle.state::<NotificationServiceState>();
    let mut service_guard = notification_state.lock().await;
    
    if let Some(service) = service_guard.as_mut() {
        service.schedule_event_notifications(&event).await
            .map_err(|e| format!("Failed to schedule notification: {}", e))?;
        Ok("Notification scheduled successfully".to_string())
    } else {
        Err("Notification service not available".to_string())
    }
}

// Trigger immediate sync command //
#[tauri::command]
async fn trigger_sync(app_handle: tauri::AppHandle) -> Result<(), String> {
    let config = crate::api_utils::AppConfig::new()?;
    let app_handle_arc = Arc::new(app_handle);
    
    // Check if user is logged in
    let user_logged_in = match crate::user_utils::get_current_user_id(&app_handle_arc) {
        Ok(_) => true,
        Err(_) => false,
    };
    
    if !user_logged_in {
        return Err("User not logged in".to_string());
    }
    
    // Trigger immediate sync to DynamoDB (if enabled)
    if config.enable_database_sync {
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
fn create_system_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&quit])?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |_app, event| {
            match event.id().as_ref() {
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|_tray, event| {
            if let TrayIconEvent::Click { button, button_state, .. } = event {
                if button == tauri::tray::MouseButton::Left && button_state == tauri::tray::MouseButtonState::Up {
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
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            check_login_status,
            login_user,
            register_user,
            logout_user,
            save_theme,
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
            save_event_from_ai,
            reject_event_suggestion,
            trigger_immediate_sync,
            get_events_for_ai,
        ])
        .setup(|app| {
          // Request notification permissions on macOS
            #[cfg(target_os = "macos")]
            {
                tauri::api::notification::request_permission();
            }

            // Initialize database on app startup
            database_utils::init_db(&app.handle()).map_err(|e| e.to_string())?;

            // Create system tray
            create_system_tray(&app.handle())?;

            //put window always on top
            crate::window::set_always_on_top(&app.handle(), true);
            
            // Initialize notification service state
            app.manage(Arc::new(Mutex::new(None::<NotificationService>)) as NotificationServiceState);
            
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
                  let login_success = start_auto_login(Arc::clone(&app_handle_arc)).await
                      .unwrap_or(false);
                      
                  // Start notification service
                  if let Err(e) = start_notification_service(Arc::clone(&app_handle_arc), login_success).await {
                      eprintln!("Failed to start notification service: {}", e);
                  }

                  // Start database sync service using a connection pool or other thread-safe approach
                  if let Err(e) = start_database_sync_service(Arc::clone(&app_handle_arc), login_success).await {
                      eprintln!("Failed to start database sync service: {}", e);
                  }

                  // Start google sync service using a connection pool or other thread-safe approach
                  if let Err(e) = start_google_sync_service(Arc::clone(&app_handle_arc), login_success).await {
                      eprintln!("Failed to start google sync service: {}", e);
                  }
              });
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // Hide the window instead of closing it
                    if let Some(window) = app_handle.get_webview_window(&label) {
                        let _ = window.hide();
                    }
                    api.prevent_close();
                }
            }
            _ => {}
        });
    Ok(())
}
