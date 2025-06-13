#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub mod window;
mod oauth;
mod config;
mod login;
mod register;
mod token_utils;
mod theme_utils;
mod encription_key;
mod auto_login;
mod sqlite;
mod sqlite_sync_service;
mod notification_service;

use tauri::Manager;
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::menu::{Menu, MenuItem};
use auto_launch::AutoLaunchBuilder;
use std::env;

use notification_service::NotificationService;

// auto-login command
#[tauri::command]
async fn auto_login(app_handle: tauri::AppHandle) -> Result<bool, String> {
    crate::auto_login::auto_login_lambda(&app_handle).await
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
    sqlite::save_event(&app_handle, event)
}

#[tauri::command]
async fn get_events(app_handle: tauri::AppHandle) -> Result<Vec<String>, String> {
    sqlite::get_events(&app_handle).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_event(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    sqlite::delete_event(&app_handle, id).map_err(|e| e.to_string())
}

// clean old events comand
#[tauri::command]
async fn clean_old_events(app_handle: tauri::AppHandle) -> Result<(), String> {
    sqlite::clean_old_events(&app_handle).map_err(|e| e.to_string())
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
// Schedule event notification command
#[tauri::command]
async fn schedule_event_notification(
    _app_handle: tauri::AppHandle,
    event_json: String
) -> Result<(), String> {
    let _event = sqlite::CalendarEvent::from_json(&event_json)?;
    
    // Access the global notification service and schedule
    // This requires proper state management
    
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
                    // Handle left click on tray icon
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
            auto_login,
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
            setup_auto_launch,
            schedule_event_notification
        ])
        .setup(|app| {
            // Initialize database on app startup
            sqlite::init_db(&app.handle()).map_err(|e| e.to_string())?;

            // Create system tray
            create_system_tray(&app.handle())?;

            //put window always on top
            crate::window::set_always_on_top(&app.handle(), true);

            // Start notification service
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut notification_service = NotificationService::new().await
                    .expect("Failed to create notification service");
                
                notification_service.start(app_handle).await
                    .expect("Failed to start notification service");
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .on_window_event(|_window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Hide the window instead of closing it
                _window.hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .run(tauri::generate_context!())?;
    Ok(())
}