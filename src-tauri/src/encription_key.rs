use std::fs;
use std::path::Path;
use rand::Rng;
use std::env;
use base64::{engine::general_purpose, Engine};
use dotenvy::dotenv;

fn generate_encryption_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::thread_rng().fill(&mut key);
    key
}

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