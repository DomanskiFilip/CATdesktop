use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::Aead;
use aes_gcm::KeyInit;
use rand::Rng;
use std::fs;
use tauri::{AppHandle, Manager};
use crate::encription_key::get_encryption_key;


// Helper function -> get platform-agnostic path for storing tokens //
fn get_tokens_path(app_handle: &AppHandle) -> Result<std::path::PathBuf, String> {
    let path = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    // Create directories if they don't exist
    fs::create_dir_all(&path)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;
    
    Ok(path.join("tokens.enc"))
}

fn generate_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill(&mut nonce);
    nonce
}

// Function to save access and refresh tokens to a file with encryption //
pub fn save_tokens_to_file(app_handle: &AppHandle, access_token: &str, refresh_token: &str) -> Result<(), String> {
    // Generate a key from an environment variable or other secure source
    let key = get_encryption_key()?;
    let key = Key::<aes_gcm::aes::Aes256>::from_slice(&key);
    let cipher = Aes256Gcm::new(key);

    // Serialize the tokens and expiration times
    let data = serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
    })
    .to_string();

    // Generate a random nonce
    let nonce = generate_nonce();
    let nonce_slice = Nonce::from_slice(&nonce);

    // Encrypt the data
    let encrypted_data = cipher
        .encrypt(nonce_slice, data.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Prepend the nonce to the encrypted data
    let mut file_data = nonce.to_vec();
    file_data.extend_from_slice(&encrypted_data);

    // Save the encrypted data to a file
    let file_path = get_tokens_path(app_handle)?;
    fs::write(file_path, file_data).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}

// Function to read access and refresh tokens from a file with decryption //
pub fn read_tokens_from_file(app_handle: &AppHandle) -> Result<(String, String), String> {
    // Retrieve and decode the encryption key
    let key = get_encryption_key()?;

    let key = Key::<aes_gcm::aes::Aes256>::from_slice(&key);
    let cipher = Aes256Gcm::new(key);

    // Read the encrypted data from the file
    let file_path = get_tokens_path(app_handle)?;
    let file_data = fs::read(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Extract the nonce (first 12 bytes) and the encrypted data
    let (nonce, encrypted_data) = file_data.split_at(12);
    let nonce_slice = Nonce::from_slice(nonce);

    // Decrypt the data
    let decrypted_data = cipher
        .decrypt(nonce_slice, encrypted_data)
        .map_err(|e| format!("Decryption failed: {}", e))?;

    // Deserialize the JSON data
    let data: serde_json::Value =
        serde_json::from_slice(&decrypted_data).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let access_token = data["access_token"].as_str().unwrap_or("").to_string();
    let refresh_token = data["refresh_token"].as_str().unwrap_or("").to_string();

    Ok((
        access_token,
        refresh_token,
    ))
}

// Function to clear tokens from the file //
pub fn clear_tokens(app_handle: &AppHandle) -> Result<(), String> {
    let file_path = get_tokens_path(app_handle)?;
    if file_path.exists() {
        fs::remove_file(file_path).map_err(|e| format!("Failed to delete tokens file: {}", e))
    } else {
        Ok(())
    }
}