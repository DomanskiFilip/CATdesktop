use crate::login::DATABASE_TOKEN;

pub fn encrypt_user_data_base(
    _app_handle: &tauri::AppHandle,
    _user_id: &str,
    data: &[u8],
) -> Result<Vec<u8>, String> {
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
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    let mut encrypted_data = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    encrypted_data.extend_from_slice(&nonce_bytes);
    encrypted_data.extend_from_slice(&ciphertext);
    Ok(encrypted_data)
}

pub fn decrypt_user_data_base(
    _app_handle: &tauri::AppHandle,
    _user_id: &str,
    encrypted_data: &[u8],
) -> Result<Vec<u8>, String> {
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

#[cfg(not(target_os = "android"))]
mod platform {
    use base64::{engine::general_purpose, Engine};
    use dotenvy::dotenv;
    use rand::Rng;
    use std::env;
    use std::fs;
    use std::path::Path;

    fn generate_encryption_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::rng().fill(&mut key);
        key
    }

    pub fn initialize_encryption_key() -> Result<(), String> {
        let env_path = Path::new(".env");
        let mut env_content = if env_path.exists() {
            fs::read_to_string(env_path).map_err(|e| format!("Failed to read .env file: {}", e))?
        } else {
            String::new()
        };
        if env_content.contains("ENCRYPTION_KEY=") {
            return Ok(());
        }
        let key = generate_encryption_key();
        let key_base64 = general_purpose::STANDARD.encode(key);
        if !env_content.ends_with('\n') {
            env_content.push('\n');
        }
        env_content.push_str(&format!("ENCRYPTION_KEY={}\n", key_base64));
        fs::write(env_path, env_content)
            .map_err(|e| format!("Failed to write to .env file: {}", e))?;
        dotenv().ok();
        Ok(())
    }

    pub fn get_encryption_key() -> Result<[u8; 32], String> {
        if let Err(_) = env::var("ENCRYPTION_KEY") {
            initialize_encryption_key()?;
        }
        let key_base64 =
            env::var("ENCRYPTION_KEY").map_err(|_| "ENCRYPTION_KEY not set in .env".to_string())?;
        let key_bytes = general_purpose::STANDARD
            .decode(key_base64)
            .map_err(|e| format!("Failed to decode ENCRYPTION_KEY: {}", e))?;
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
}

#[cfg(not(target_os = "android"))]
pub use platform::get_encryption_key;
