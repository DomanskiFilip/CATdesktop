use std::fs;
use std::path::Path;
use rand::Rng;
use std::env;
use base64::{ engine::general_purpose, Engine };
use dotenvy::dotenv;
use tauri::{ AppHandle, Manager };
use chacha20poly1305::{ ChaCha20Poly1305, Key, Nonce };
use chacha20poly1305::aead::{ Aead, KeyInit };

// Helper function -> generate a random 32-byte encryption key
fn generate_encryption_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::thread_rng().fill(&mut key);
    key
}

// Application wide encription key //
pub fn initialize_encryption_key() -> Result<(), String> {
    let env_path = Path::new(".env");

    // Read the existing .env file content if it exists
    let mut env_content = if env_path.exists() {
        fs::read_to_string(env_path).map_err(|e| format!("Failed to read .env file: {}", e))?
    } else {
        String::new()
    };

    // Check if the ENCRYPTION_KEY already exists
    if env_content.contains("ENCRYPTION_KEY=") {
        return Ok(());
    }

    // Generate a new encryption key
    let key = generate_encryption_key();
    let key_base64 = general_purpose::STANDARD.encode(key);

    // Ensure ENCRYPTION_KEY is added on its own line
    if !env_content.ends_with('\n') {
        env_content.push('\n');
    }
    env_content.push_str(&format!("ENCRYPTION_KEY={}\n", key_base64));

    // Write the updated content back to the .env file
    fs::write(env_path, env_content).map_err(|e| format!("Failed to write to .env file: {}", e))?;

    // Reload the .env file to make the new ENCRYPTION_KEY available
    dotenv().ok();

    Ok(())
}

// Get application wide encription key //
pub fn get_encryption_key() -> Result<[u8; 32], String> {
  // Attempt to retrieve the encryption key from the environment
    if let Err(_) = env::var("ENCRYPTION_KEY") {
        // If the key is not set, initialize it
        initialize_encryption_key()?;
    }

    // Retrieve the encryption key from the environment
    let key_base64 = env::var("ENCRYPTION_KEY").map_err(|_| "ENCRYPTION_KEY not set in .env".to_string())?;
    let key_bytes = general_purpose::STANDARD
        .decode(key_base64)
        .map_err(|e| format!("Failed to decode ENCRYPTION_KEY: {}", e))?;

    // Ensure the decoded key is exactly 32 bytes
    if key_bytes.len() != 32 {
        return Err(format!(
            "Invalid ENCRYPTION_KEY length: expected 32 bytes, got {} bytes",
            key_bytes.len()
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);
    Ok(key)
}

// Helper function -> Load a user-specific encryption key //
pub fn load_user_encryption_key(app_handle: &AppHandle, email: &str) -> Result<[u8; 32], String> {
    let app_dir = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    // Generate key filename based on email
    let key_filename = format!("{}_ENC_KEY", email);
    let key_path = app_dir.join(&key_filename);
    
    if !key_path.exists() {
        return Err(format!("Encryption key does not exist for user: {}", email));
    }
    
    let key_base64 = fs::read_to_string(&key_path)
        .map_err(|e| format!("Failed to read user encryption key: {}", e))?;
    
    let key_bytes = general_purpose::STANDARD
        .decode(&key_base64)
        .map_err(|e| format!("Failed to decode user encryption key: {}", e))?;
    
    // Ensure the decoded key is exactly 32 bytes
    if key_bytes.len() != 32 {
        return Err(format!(
            "Invalid user encryption key length: expected 32 bytes, got {} bytes",
            key_bytes.len()
        ));
    }
    
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);
    Ok(key)
}

// Function to create a user-specific encryption key //
pub fn create_user_encryption_key(app_handle: &AppHandle, email: &str) -> Result<[u8; 32], String> {
    let app_dir = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;
    
    // Generate key filename based on email
    let key_filename = format!("{}_ENC_KEY", email);
    let key_path = app_dir.join(&key_filename);
    
    // Check if key already exists
    if key_path.exists() {
        // Load existing key
        return load_user_encryption_key(app_handle, email);
    }
    
    // Generate a new encryption key
    let key = generate_encryption_key();
    let key_base64 = general_purpose::STANDARD.encode(key);
    
    // Save encrypted key
    fs::write(&key_path, &key_base64)
        .map_err(|e| format!("Failed to write user encryption key: {}", e))?;
    
    Ok(key)
}


// Function to encrypt data with a user's key //
pub fn encrypt_user_data(app_handle: &AppHandle, email: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    // Get the user's encryption key
    let encryption_key = load_user_encryption_key(app_handle, email)?;
    
    // Use a crate for encryption
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
    use chacha20poly1305::aead::{Aead, KeyInit};
    use rand::RngCore;
    
    // Create a new key from the user's encryption key
    let key = Key::from_slice(&encryption_key);
    let cipher = ChaCha20Poly1305::new(key);
    
    // Generate a random nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Encrypt the data
    let ciphertext = cipher.encrypt(nonce, data)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    // Prepend the nonce to the ciphertext
    let mut encrypted_data = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    encrypted_data.extend_from_slice(&nonce_bytes);
    encrypted_data.extend_from_slice(&ciphertext);
    
    Ok(encrypted_data)
}

// Function to decrypt data with a user's key //
pub fn decrypt_user_data(app_handle: &AppHandle, email: &str, encrypted_data: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < 12 {
        return Err("Encrypted data is too short".to_string());
    }
    
    // Get the user's encryption key
    let encryption_key = load_user_encryption_key(app_handle, email)?;
    
    // Use the same crate for decryption
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
    use chacha20poly1305::aead::{Aead, KeyInit};
    
    // Extract the nonce and ciphertext
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    
    // Create a new key from the user's encryption key
    let key = Key::from_slice(&encryption_key);
    let cipher = ChaCha20Poly1305::new(key);
    
    // Decrypt the data
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    
    Ok(plaintext)
}