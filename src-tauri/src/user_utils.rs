use crate::encryption_utils::get_encryption_key;
use tauri::{AppHandle, Manager};
use std::path::PathBuf;
use std::fs;
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::Aead; 
use aes_gcm::KeyInit; 
use rand::RngCore;

// Helper function -> to get the user file path //
fn get_user_file_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_dir = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app directory: {}", e))?;
    
    Ok(app_dir.join("user.enc"))
}

// Function to save the current user ID to a file with encryption //
pub fn save_current_user_id(app_handle: &AppHandle, user_id: &str) -> Result<(), String> {
    // Get encryption key
    let key = get_encryption_key()?;
    let key = Key::<aes_gcm::aes::Aes256>::from_slice(&key);
    let cipher = Aes256Gcm::new(key);

    // Generate a random nonce
    let mut nonce = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce);
    let nonce_slice = Nonce::from_slice(&nonce);

    // Encrypt the user ID
    let encrypted_data = cipher
        .encrypt(nonce_slice, user_id.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Prepend the nonce to the encrypted data
    let mut file_data = nonce.to_vec();
    file_data.extend_from_slice(&encrypted_data);

    // Save to file
    let file_path = get_user_file_path(app_handle)?;
    fs::write(file_path, file_data)
        .map_err(|e| format!("Failed to write user file: {}", e))?;

    println!("User ID saved successfully");
    Ok(())
}

// Function to get the current user ID from the encrypted file //
pub fn get_current_user_id(app_handle: &AppHandle) -> Result<String, String> {
    let file_path = get_user_file_path(app_handle)?;
    
    if !file_path.exists() {
        return Err("No user is currently logged in".to_string());
    }
    
    // Get encryption key
    let key = get_encryption_key()?;
    let key = Key::<aes_gcm::aes::Aes256>::from_slice(&key);
    let cipher = Aes256Gcm::new(key);

    // Read file data
    let file_data = fs::read(&file_path)
        .map_err(|e| format!("Failed to read user file: {}", e))?;

    // Extract nonce and encrypted data
    if file_data.len() <= 12 {
        return Err("Invalid user file format".to_string());
    }
    
    let (nonce, encrypted_data) = file_data.split_at(12);
    let nonce_slice = Nonce::from_slice(nonce);

    // Decrypt
    let decrypted_data = cipher
        .decrypt(nonce_slice, encrypted_data)
        .map_err(|e| format!("Decryption failed: {}", e))?;

    // Convert to string
    let user_id = String::from_utf8(decrypted_data)
        .map_err(|e| format!("Invalid UTF-8 in user ID: {}", e))?;

    Ok(user_id)
}

// Function to clear the current user ID by removing the encrypted file //
pub fn clear_current_user_id(app_handle: &AppHandle) -> Result<(), String> {
    let file_path = get_user_file_path(app_handle)?;
    
    if file_path.exists() {
        fs::remove_file(file_path)
            .map_err(|e| format!("Failed to remove user file: {}", e))?;
    }
    
    println!("User ID cleared successfully");
    Ok(())
}