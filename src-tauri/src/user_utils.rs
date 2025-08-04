use crate::NotificationServiceState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize)]
pub struct UserSettings {
    pub notification_service: bool,
    pub notification_lead_minutes: u32,
}

// Helper function -> to get the user file path //
fn get_user_file_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;

    fs::create_dir_all(&app_dir).map_err(|e| format!("Failed to create app directory: {}", e))?;

    Ok(app_dir.join("user.enc"))
}

// Function to save the current user ID to a file with encryption //
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn save_current_user_id(app_handle: &AppHandle, user_id: &str) -> Result<(), String> {
    use crate::encryption_utils::get_encryption_key;
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
    use rand::RngCore;

    let key = get_encryption_key().map_err(|e| format!("Key error: {}", e))?;
    let key = Key::from_slice(&key);
    let cipher = ChaCha20Poly1305::new(key);

    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let encrypted = cipher
        .encrypt(nonce, user_id.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut encrypted_data = Vec::with_capacity(nonce_bytes.len() + encrypted.len());
    encrypted_data.extend_from_slice(&nonce_bytes);
    encrypted_data.extend_from_slice(&encrypted);

    let file_path = get_user_file_path(app_handle)?;
    fs::write(file_path, encrypted_data).map_err(|e| format!("Failed to write user file: {}", e))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn save_current_user_id(_: &AppHandle, _: &str) -> Result<(), String> {
    Err("Use the tauri-plugin-keystore JS API to save user ID on Android/iOS".to_string())
}

// Function to get the current user ID from the encrypted file //
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_current_user_id(app_handle: &AppHandle) -> Result<String, String> {
    let file_path = get_user_file_path(app_handle)?;

    if !file_path.exists() {
        return Err("No user is currently logged in".to_string());
    }

    let encrypted_data =
        fs::read(&file_path).map_err(|e| format!("Failed to read user file: {}", e))?;

    use crate::encryption_utils::get_encryption_key;
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

    let key = get_encryption_key().map_err(|e| format!("Key error: {}", e))?;
    let key = Key::from_slice(&key);
    let cipher = ChaCha20Poly1305::new(key);

    if encrypted_data.len() < 12 {
        return Err("Encrypted data is too short".to_string());
    }
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let user_id_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    String::from_utf8(user_id_bytes).map_err(|e| format!("UTF-8 error: {}", e))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn get_current_user_id_mobile() -> Result<String, String> {
    crate::get_current_user_id_from_cache().await
}

// Function to clear the current user ID by removing the encrypted file //
pub fn clear_current_user_id(app_handle: &AppHandle) -> Result<(), String> {
    let file_path = get_user_file_path(app_handle)?;

    if file_path.exists() {
        fs::remove_file(file_path).map_err(|e| format!("Failed to remove user file: {}", e))?;
    }

    println!("User ID cleared successfully");
    Ok(())
}

// Function to set the notification service and lead time //
pub async fn set_notification_service(
    app_handle: AppHandle,
    enabled: bool,
    lead_minutes: Option<u32>,
) -> Result<(), String> {
    let lead = lead_minutes.unwrap_or(15); // default to 15 if not provided
    let settings = UserSettings {
        notification_service: enabled,
        notification_lead_minutes: lead,
    };
    std::fs::write("settings.json", serde_json::to_string(&settings).unwrap()).unwrap();
    println!(
        "Notification service set to: {}, lead: {} min",
        enabled, lead
    );

    let notification_state = app_handle.state::<NotificationServiceState>();

    if enabled {
        let mut service_guard = notification_state.lock().await;
        if service_guard.is_none() {
            let service = crate::notification_service::NotificationService::new();
            let app_handle_arc = Arc::new(app_handle.clone());
            service.start(app_handle_arc, true).await;
            *service_guard = Some(service);
        }
    } else {
        let service_opt = {
            let mut service_guard = notification_state.lock().await;
            service_guard.take()
        };
        if let Some(mut service) = service_opt {
            service.stop().await;
        }
    }

    Ok(())
}

// Function to set the notification lead time //
pub async fn set_notification_lead_time(
    app_handle: AppHandle,
    lead_minutes: u32,
) -> Result<(), String> {
    // Read current settings
    let mut settings: UserSettings = std::fs::read_to_string("settings.json")
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or(UserSettings {
            notification_service: true,
            notification_lead_minutes: 15,
        });
    settings.notification_lead_minutes = lead_minutes;
    std::fs::write("settings.json", serde_json::to_string(&settings).unwrap()).unwrap();
    println!("Notification lead time set to: {} min", lead_minutes);

    // Reschedule notifications if service is running
    let app_handle_cloned = app_handle.clone();
    let notification_state = app_handle_cloned.state::<NotificationServiceState>();
    let mut service_guard = notification_state.lock().await;
    if let Some(_service) = service_guard.as_mut() {
        // Reschedule all notifications
        crate::notification_service::NotificationService::check_and_schedule_all_notifications(
            &Arc::new(app_handle),
            true,
        )
        .await
        .ok();
    }
    Ok(())
}
