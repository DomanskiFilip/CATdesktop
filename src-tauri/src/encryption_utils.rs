use crate::login::DATABASE_TOKEN;

pub fn encrypt_user_data_base(_app_handle: &tauri::AppHandle, _user_id: &str, data: &[u8],) -> Result<Vec<u8>, String> {
    let key = {
        let cache = DATABASE_TOKEN.lock().unwrap();
        cache.clone().ok_or("Database token not set".to_string())?
    };
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
    use rand::RngCore;
    let key = Key::from_slice(&key);
    let cipher = ChaCha20Poly1305::new(key);
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    let mut encrypted_data = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    encrypted_data.extend_from_slice(&nonce_bytes);
    encrypted_data.extend_from_slice(&ciphertext);
    Ok(encrypted_data)
}

pub fn decrypt_user_data_base(_app_handle: &tauri::AppHandle, _user_id: &str, encrypted_data: &[u8],) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < 12 {
        return Err("Encrypted data is too short".to_string());
    }
    let key = {
        let cache = DATABASE_TOKEN.lock().unwrap();
        cache.clone().ok_or("Database token not set".to_string())?
    };
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
    let (nonce_bytes, ciphertext) = encrypted_data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let key = Key::from_slice(&key);
    let cipher = ChaCha20Poly1305::new(key);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    Ok(plaintext)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod platform {
    use keyring::Entry;
    use rand::Rng;
    use whoami;
    use tauri::AppHandle;
    use base64::Engine;
    use std::sync::{Mutex, OnceLock};

    const SERVICE_NAME: &str = "com.calendarassistant.casual";

    // In-memory cache for the encryption key to avoid repeated keyring calls
    static ENCRYPTION_KEY_CACHE: OnceLock<Mutex<Option<[u8; 32]>>> = OnceLock::new();

    fn get_user_name() -> String {
        let user_name = whoami::username();
        user_name
    }

    fn generate_encryption_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::rng().fill(&mut key);
        key
    }

    pub fn initialize_encryption_key(_app_handle: &AppHandle) -> Result<(), String> {
        // Check if we already have the key cached
        let cache = ENCRYPTION_KEY_CACHE.get_or_init(|| Mutex::new(None));
        if let Ok(guard) = cache.lock() {
            if guard.is_some() {
                println!("Encryption key already cached in memory");
                return Ok(());
            }
        }

        let user_name = get_user_name();
        
        let entry = Entry::new(SERVICE_NAME, &user_name).map_err(|e| format!("Keyring error: {}", e))?;
        
        // Check if key already exists in keyring
        match entry.get_password() {
            Ok(existing_key) => {
                // Verify the existing key is valid
                if let Ok(key_bytes) = base64::engine::general_purpose::STANDARD.decode(&existing_key) {
                    if key_bytes.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&key_bytes);
                        
                        // Cache the key
                        if let Ok(mut guard) = cache.lock() {
                            *guard = Some(key);
                        }
                        
                        println!("Valid encryption key already exists in keyring");
                        return Ok(());
                    }
                }
                println!("Existing key in keyring is invalid, regenerating");
            },
            Err(e) => {
                println!("No existing encryption key found in keyring: {}", e);
            }
        }

        // Generate a new key
        let key = generate_encryption_key();
        let key_base64 = base64::engine::general_purpose::STANDARD.encode(key);
        
        // Store in keyring
        entry.set_password(&key_base64).map_err(|e| format!("Failed to store key in keyring: {}", e))?;
        
        // Verify keyring storage by attempting immediate retrieval
        match entry.get_password() {
            Ok(retrieved_key) => {
                if retrieved_key == key_base64 {
                    // Cache the key
                    if let Ok(mut guard) = cache.lock() {
                        *guard = Some(key);
                    }
                    
                    return Ok(());
                } else {
                    return Err("Keyring verification failed - keys don't match".to_string());
                }
            },
            Err(e) => {
                return Err(format!("Keyring verification failed - cannot retrieve: {}", e));
            }
        }
    }

    pub fn get_encryption_key(_app_handle: &AppHandle) -> Result<[u8; 32], String> {
        // Check cache first
        let cache = ENCRYPTION_KEY_CACHE.get_or_init(|| Mutex::new(None));
        if let Ok(guard) = cache.lock() {
            if let Some(cached_key) = *guard {
                return Ok(cached_key);
            }
        }

        let user_name = get_user_name();
        
        let entry = Entry::new(SERVICE_NAME, &user_name).map_err(|e| format!("Keyring error: {}", e))?;
        
        // Try keyring
        match entry.get_password() {
            Ok(key_base64) => {
                let key_bytes = base64::engine::general_purpose::STANDARD.decode(&key_base64).map_err(|e| format!("Failed to decode key from keyring: {}", e))?;
                if key_bytes.len() != 32 {
                    return Err(format!("Invalid key length from keyring: expected 32 bytes, got {}", key_bytes.len()));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes);
                
                // Cache the key for future use
                if let Ok(mut guard) = cache.lock() {
                    *guard = Some(key);
                }
                
                return Ok(key);
            },
            Err(e) => {
                return Err(format!("Failed to retrieve key from keyring: {}", e));
            }
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use platform::{get_encryption_key, initialize_encryption_key};