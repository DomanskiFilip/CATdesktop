#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub mod window;
mod oauth;
mod api_utils;
mod login;
mod register;
mod token_utils;
mod theme_utils;
mod encription_key;
mod auto_login;
mod database_utils;
mod notification_service;

use tauri::{AppHandle, Manager, Emitter};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem};
use std::sync::Arc;
use tokio::sync::Mutex;
use auto_launch::AutoLaunchBuilder;
use std::env;
use crate::notification_service::NotificationService;


pub type NotificationServiceState = Arc<Mutex<Option<NotificationService>>>;

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
    crate::login::login_user_lambda(&app_handle, email, password).await
}

// register user command
#[tauri::command]
async fn register_user(email: String, password: String) -> Result<String, String> {
    crate::register::register_user_lambda(email, password).await
}

// logout user command
#[tauri::command]
async fn logout_user(app_handle: tauri::AppHandle) -> Result<bool, String> {
    crate::token_utils::clear_tokens(&app_handle).map(|_| true)
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
        tokio::runtime::Handle::current().block_on(crate::oauth::oauth2_flow(&app_handle, TIMEOUT))
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
async fn setup_auto_launch(app_handle: tauri::AppHandle) -> Result<(), String> {
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

// Start auto-login process
async fn start_auto_login(app_handle: AppHandle) -> Result<bool, String> {
    let login_success = match crate::auto_login::auto_login_lambda(&app_handle).await {
        Ok(result) => result,
        Err(e) => false,
    };
    
    // Emit login status to frontend
    app_handle.emit("auto-login-completed", login_success).ok();
    
    Ok(login_success)
}

// Start notification service
async fn start_notification_service(app_handle: AppHandle, user_logged_in: bool) -> Result<(), String> {
    let notification_state = app_handle.state::<NotificationServiceState>();
    let mut service_guard = notification_state.lock().await;
    
    if service_guard.is_none() {
        let mut service = NotificationService::new();
        service.start(app_handle.clone(), user_logged_in).await;
        *service_guard = Some(service);
    }
    
    Ok(())
}

// Schedule event notification command
#[tauri::command]
async fn schedule_event_notification(
    event_json: String,
    app_handle: AppHandle,
) -> Result<String, String> {
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

// Create system tray
fn create_system_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&quit])?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
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
            get_oauth_timeout,
            run_oauth2_flow,

            schedule_event_notification
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
            
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|app_handle, event| match event {
            tauri::RunEvent::Ready => {
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    // Start auto-login
                    let login_success = start_auto_login(app_handle_clone.clone()).await
                        .unwrap_or(false);
                        
                    // Start notification service
                    if let Err(e) = start_notification_service(app_handle_clone.clone(), login_success).await {
                        eprintln!("Failed to start notification service: {}", e);
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
