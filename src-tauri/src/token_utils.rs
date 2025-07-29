use rand::Rng;
use std::fs;
use tauri::{ AppHandle, Manager };
#[cfg(target_os = "android")]
use base64::Engine;

#[cfg(not(target_os = "android"))]
use crate::encryption_utils::get_encryption_key;

fn get_tokens_path(app_handle: &AppHandle) -> Result<std::path::PathBuf, String> {
    let path = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    fs::create_dir_all(&path).map_err(|e| format!("Failed to create app data directory: {}", e))?;
    Ok(path.join("tokens.enc"))
}

fn generate_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    rand::rng().fill(&mut nonce);
    nonce
}

#[cfg(not(target_os = "android"))]
pub async fn save_tokens_to_file(app_handle: &AppHandle, access_token: &str, refresh_token: &str, database_token: Option<&[u8; 32]>,) -> Result<(), String> {
    use aes_gcm::aead::Aead;
    use aes_gcm::KeyInit;
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use base64::Engine;

    let key = get_encryption_key()?;
    let key = Key::<aes_gcm::aes::Aes256>::from_slice(&key);
    let cipher = Aes256Gcm::new(key);

    let db_token_b64 = database_token
        .map(|t| base64::engine::general_purpose::STANDARD.encode(t));

    let data = serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "database_token": db_token_b64,
    })
    .to_string();

    let nonce = generate_nonce();
    let nonce_slice = Nonce::from_slice(&nonce);

    let encrypted_data = cipher
        .encrypt(nonce_slice, data.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut file_data = nonce.to_vec();
    file_data.extend_from_slice(&encrypted_data);

    let file_path = get_tokens_path(app_handle)?;
    fs::write(file_path, file_data).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

#[cfg(not(target_os = "android"))]
pub async fn read_tokens_from_file(app_handle: &AppHandle) -> Result<(String, String, Option<[u8; 32]>), String> {
    use aes_gcm::aead::Aead;
    use aes_gcm::KeyInit;
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use base64::Engine;

    let key = get_encryption_key()?;
    let key = Key::<aes_gcm::aes::Aes256>::from_slice(&key);
    let cipher = Aes256Gcm::new(key);

    let file_path = get_tokens_path(app_handle)?;
    let file_data = fs::read(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    let (nonce, encrypted_data) = file_data.split_at(12);
    let nonce_slice = Nonce::from_slice(nonce);

    let decrypted_data = cipher
        .decrypt(nonce_slice, encrypted_data)
        .map_err(|e| format!("Decryption failed: {}", e))?;

    let data: serde_json::Value = serde_json::from_slice(&decrypted_data)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let access_token = data["access_token"].as_str().unwrap_or("").to_string();
    let refresh_token = data["refresh_token"].as_str().unwrap_or("").to_string();
    let database_token = data["database_token"].as_str().and_then(|b64| {
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

    Ok((access_token, refresh_token, database_token))
}

// On Android/iOS, use the plugin from JS/TS, not Rust!
#[cfg(target_os = "android")]
pub async fn save_tokens_to_file(_: &AppHandle, _: &str, _: &str, _: Option<&[u8; 32]>) -> Result<(), String> {
    // No-op: tokens are set via set_tokens_for_autologin from the frontend
    Ok(())
}

#[cfg(target_os = "android")]
pub async fn read_tokens_from_file(_: &AppHandle) -> Result<(String, String, Option<[u8; 32]>), String> {
    let (access_token, refresh_token, database_token) = crate::read_tokens_from_cache().await.ok_or("No tokens in cache".to_string())?;
    println!("Reading tokens from cache on Android: {:?}, {:?}, {:?}", access_token, refresh_token, database_token);
    Ok((access_token, refresh_token, database_token))
}

pub fn clear_tokens(app_handle: &AppHandle) -> Result<(), String> {
    let file_path = get_tokens_path(app_handle)?;
    if file_path.exists() {
        fs::remove_file(file_path).map_err(|e| format!("Failed to delete tokens file: {}", e))
    } else {
        Ok(())
    }
}
