#[cfg(target_os = "android")]
mod platform {
    use jni::objects::{JByteArray, JObject, JObjectArray};
    use jni::{JNIEnv, JavaVM};
    use tauri::AppHandle;
    use std::sync::{Arc, OnceLock};

    static ANDROID_JAVA_VM: OnceLock<Arc<JavaVM>> = OnceLock::new();

    pub fn initialize_android_jni_context(vm: JavaVM) -> Result<(), String> {
        ANDROID_JAVA_VM.set(Arc::new(vm))
            .map_err(|_| "Android JNI context already initialized".to_string())
    }

    fn get_jni_env() -> Result<JNIEnv<'static>, String> {
        let vm_arc = ANDROID_JAVA_VM.get()
            .ok_or_else(|| "Android JNI context not initialized. Call initialize_android_jni_context first.".to_string())?;
        
        vm_arc.attach_current_thread_as_daemon()
            .map_err(|e| format!("JNI attach error: {:?}", e))
    }

    pub fn encrypt_user_data(_app_handle: &AppHandle, _email: &str, data: &[u8]) -> Result<Vec<u8>, String> {
        let mut env = get_jni_env()?;
        let class = env.find_class("com/calendarassistant/casual/KeystoreHelper").map_err(|e| format!("Class not found: {:?}", e))?;

        let input = env.byte_array_from_slice(data).map_err(|e| format!("JNI byte array error: {:?}", e))?;

        let result_jobject = env.call_static_method(class, "encrypt", "([B)[[B", &[(&input).into()])
            .map_err(|e| format!("JNI call error: {:?}", e))?
            .l()
            .map_err(|e| format!("JNI result error: {:?}", e))?;

        // Cast directly to JObjectArray
        let result_array = JObjectArray::from(result_jobject);

        let iv_array: JByteArray = env.get_object_array_element(&result_array, 0).unwrap().into(); 
        let ct_array: JByteArray = env.get_object_array_element(&result_array, 1).unwrap().into();
        let iv = env.convert_byte_array(iv_array).unwrap();
        let ct = env.convert_byte_array(ct_array).unwrap();

        let mut out = Vec::with_capacity(iv.len() + ct.len());
        out.extend_from_slice(&iv);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn decrypt_user_data(_app_handle: &AppHandle, _email: &str, encrypted_data: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted_data.len() < 12 {
            return Err("Encrypted data is too short".to_string());
        }
        let (iv, ct) = encrypted_data.split_at(12);

        let mut env = get_jni_env()?;
        let class = env.find_class("com/calendarassistant/casual/KeystoreHelper").map_err(|e| format!("Class not found: {:?}", e))?;

        let iv_java = env.byte_array_from_slice(iv).map_err(|e| format!("JNI byte array error: {:?}", e))?;
        let ct_java = env.byte_array_from_slice(ct).map_err(|e| format!("JNI byte array error: {:?}", e))?;

        let result_jobject = env.call_static_method(class, "decrypt", "([B[B)[B", &[(&iv_java).into(), (&ct_java).into()])
            .map_err(|e| format!("JNI call error: {:?}", e))?
            .l()
            .map_err(|e| format!("JNI result error: {:?}", e))?;

        let result_array: JByteArray = result_jobject.into(); 
        let plaintext = env.convert_byte_array(result_array).unwrap();
        Ok(plaintext)
    }

    pub fn create_user_encryption_key(_app_handle: &AppHandle, _email: &str) -> Result<(), String> {
        let mut env = get_jni_env()?;
        let class = env.find_class("com/calendarassistant/casual/KeystoreHelper").map_err(|e| format!("Class not found: {:?}", e))?;
        env.call_static_method(class, "generateKey", "()V", &[])
            .map_err(|e| format!("JNI call error: {:?}", e))?;
        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
  use std::fs;
  use std::path::Path;
  use rand::Rng;
  use std::env;
  use base64::{ engine::general_purpose, Engine };
  use dotenvy::dotenv;
  use tauri::{ AppHandle, Manager };

    // Helper function -> generate a random 32-byte encryption key
    fn generate_encryption_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::rng().fill(&mut key);
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
        rand::rng().fill_bytes(&mut nonce_bytes);
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
}

// Public API
#[cfg(target_os = "android")]
pub use platform::{
    encrypt_user_data,
    decrypt_user_data,
    create_user_encryption_key,
};

#[cfg(not(target_os = "android"))]
pub use platform::{
    encrypt_user_data,
    decrypt_user_data,
    create_user_encryption_key,
    get_encryption_key,
    load_user_encryption_key,
};