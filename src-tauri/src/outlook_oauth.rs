#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use crate::credential_utils::fetch_outlook_credentials;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, CsrfToken,
    PkceCodeChallenge, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use open;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::time::Instant;
use tauri::{AppHandle, Manager};
use urlencoding;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutlookTokenExtraFields {
    pub id_token: Option<String>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub async fn outlook_oauth2_flow(app_handle: &AppHandle, timeout: u64) -> Result<String, String> {
    // Fetch credentials from Lambda
    let outlook_credentials = fetch_outlook_credentials(app_handle).await
        .map_err(|e| format!("Failed to fetch Outlook credentials: {}", e))?;
    
    let client_id_str = outlook_credentials
        .get("client_id")
        .and_then(|v| v.as_str())
        .ok_or("No client_id in Outlook credentials")?;
    
    // Use localhost redirect URI for desktop OAuth
    let redirect_uri_str = "http://127.0.0.1:1426";

    let client_id = ClientId::new(client_id_str.to_string());
    let redirect_url = RedirectUrl::new(redirect_uri_str.to_string()).unwrap();
    // Use /consumers endpoint for Microsoft personal accounts only
    let auth_url = AuthUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize".to_string()).unwrap();
    let token_url = TokenUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string()).unwrap();

    // Create client without client secret for public clients
    let client = BasicClient::new(client_id, None, auth_url, Some(token_url))
        .set_redirect_uri(redirect_url);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("offline_access".to_string()))
        .add_scope(Scope::new(
            "https://graph.microsoft.com/calendars.readwrite".to_string(),
        ))
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!("Generated CSRF token: {}", csrf_token.secret());
    println!("Opening OAuth URL: {}", auth_url);
    open::that(auth_url.as_str()).map_err(|e| e.to_string())?;

    // Listen for the redirect with the code
    let listener = TcpListener::bind("127.0.0.1:1426").map_err(|e| e.to_string())?;
    println!("Listening on http://127.0.0.1:1426 for OAuth callback...");
    
    let start_time = Instant::now();
    loop {
        // Check for timeout
        if start_time.elapsed().as_secs() > timeout {
            return Err("OAuth authentication timed out".to_string());
        }

        // Set a short timeout on accept to allow checking for global timeout
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        
        let (mut stream, addr) = match listener.accept() {
            Ok(conn) => conn,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };

        println!("Received connection from: {}", addr);

        let mut buffer = [0; 4096];
        let bytes_read = match stream.read(&mut buffer) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error reading from stream: {}", e);
                continue;
            }
        };

        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        println!("Received request:\n{}", request);

        // Parse the request line
        if let Some(first_line) = request.lines().next() {
            println!("Request line: {}", first_line);
            
            // Check if this is a GET request to the root path
            if first_line.starts_with("GET /") {
                // Extract the path and query string
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let path_and_query = parts[1];
                    println!("Path and query: {}", path_and_query);
                    
                    // Check if we have query parameters
                    if path_and_query.contains('?') {
                        let query_part = path_and_query.split('?').nth(1).unwrap_or("");
                        println!("Query parameters: {}", query_part);
                        
                        let found_code = false;
                        let mut found_error = false;
                        let mut error_description = String::new();
                        let mut state_token = String::new();
                        
                        // Parse all parameters first
                        for param in query_part.split('&') {
                            let mut key_value = param.splitn(2, '=');
                            if let (Some(key), Some(value)) = (key_value.next(), key_value.next()) {
                                let decoded_value = urlencoding::decode(value).unwrap_or_default().to_string();
                                println!("Parameter: {}={}", key, decoded_value);
                                
                                match key {
                                    "code" => {
                                        // Verify CSRF token if present
                                        if !state_token.is_empty() && state_token != *csrf_token.secret() {
                                            let csrf_error_response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                                                <html><body><h1>CSRF Token Mismatch</h1>\
                                                <p>Security error: Invalid state parameter.</p>\
                                                <script>window.close();</script></body></html>";
                                            let _ = stream.write_all(csrf_error_response.as_bytes());
                                            return Err("CSRF token mismatch".to_string());
                                        }
                                        
                                        // Send success response first
                                        let success_response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                            <html><body><h1>Authentication Successful!</h1>\
                                            <p>You can now close this window and return to the application.</p>\
                                            <script>setTimeout(() => window.close(), 1000);</script></body></html>";
                                        
                                        if let Err(e) = stream.write_all(success_response.as_bytes()) {
                                            eprintln!("Error writing success response: {}", e);
                                        }
                                        
                                        println!("Authorization code received: {}", decoded_value);
                                        
                                        // Exchange the code for tokens
                                        let token_result = client
                                            .exchange_code(AuthorizationCode::new(decoded_value))
                                            .set_pkce_verifier(pkce_verifier)
                                            .request_async(oauth2::reqwest::async_http_client)
                                            .await
                                            .map_err(|e| {
                                                println!("Token exchange error: {:?}", e);
                                                format!("Token exchange failed: {}", e)
                                            })?;

                                        let access_token = token_result.access_token().secret().to_string();
                                        let refresh_token = token_result.refresh_token().map(|t| t.secret().to_string());
                                        
                                        // Get the current user's email (user_id)
                                        let user_id = match get_current_user_id(&app_handle) {
                                            Ok(id) => id,
                                            Err(e) => {
                                                println!("Failed to get user ID: {}", e);
                                                return Err(format!("Failed to get user ID: {}", e));
                                            }
                                        };

                                        // Save all tokens in a user-specific file
                                        let token_data = serde_json::json!({
                                            "access_token": access_token,
                                            "refresh_token": refresh_token,
                                        });
                                        let token_path = app_handle
                                            .path()
                                            .app_data_dir()
                                            .map_err(|e| format!("Failed to get app data directory: {}", e))?
                                            .join(format!("outlook_tokens_{}.json", user_id));
                                        fs::create_dir_all(token_path.parent().unwrap()).map_err(|e| e.to_string())?;
                                        fs::write(token_path, token_data.to_string()).map_err(|e| e.to_string())?;
                                        println!("Outlook OAuth tokens saved for user: {}", user_id);

                                        return Ok(access_token);
                                    },
                                    "error" => {
                                        found_error = true;
                                        error_description = decoded_value;
                                    },
                                    "error_description" => {
                                        if !error_description.is_empty() {
                                            error_description = format!("{}: {}", error_description, decoded_value);
                                        } else {
                                            error_description = decoded_value;
                                        }
                                    },
                                    "state" => {
                                        state_token = decoded_value;
                                    },
                                    _ => {}
                                }
                            }
                        }
                        
                        // Handle errors after parsing all parameters
                        if found_error {
                            let detailed_error = if error_description.is_empty() {
                                "Unknown OAuth error".to_string()
                            } else {
                                error_description
                            };
                            
                            let error_response = format!(
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                                <html><body><h1>❌ Authentication Error</h1>\
                                <p>Error: {}</p>\
                                <p>Please try again or check your credentials.</p>\
                                <script>setTimeout(() => window.close(), 3000);</script></body></html>",
                                detailed_error
                            );
                            
                            let _ = stream.write_all(error_response.as_bytes());
                            return Err(format!("OAuth error: {}", detailed_error));
                        }
                        
                        if !found_code && !found_error {
                            // If we reach here, no code or error was found but it's a valid request
                            let waiting_response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                <html><body><h2>🔄 Waiting for Authentication...</h2>\
                                <p>Please complete the authentication in the opened window.</p>\
                                <script>setTimeout(() => window.location.reload(), 2000);</script></body></html>";
                            
                            if let Err(e) = stream.write_all(waiting_response.as_bytes()) {
                                eprintln!("Error writing waiting response: {}", e);
                            }
                        }
                    } else {
                        // No query parameters, show waiting page
                        let waiting_response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                            <html><body><h2>🔄 Waiting for Authentication...</h2>\
                            <p>Please complete the authentication in the opened window.</p>\
                            <script>setTimeout(() => window.location.reload(), 2000);</script></body></html>";
                        
                        if let Err(e) = stream.write_all(waiting_response.as_bytes()) {
                            eprintln!("Error writing waiting response: {}", e);
                        }
                    }
                } else {
                    // Not a GET request, send a method not allowed response
                    let method_not_allowed = "HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/html\r\n\r\n\
                        <html><body><h1>405 Method Not Allowed</h1></body></html>";
                    
                    let _ = stream.write_all(method_not_allowed.as_bytes());
                }
            } else {
                // Invalid request format
                let bad_request = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                    <html><body><h1>400 Bad Request</h1></body></html>";
                
                let _ = stream.write_all(bad_request.as_bytes());
            }
        } else {
            // Invalid request format
            let bad_request = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                <html><body><h1>400 Bad Request</h1></body></html>";
            
            let _ = stream.write_all(bad_request.as_bytes());
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn outlook_oauth2_flow(_app_handle: &AppHandle, _timeout: u64) -> Result<String, String> {
    Err("Outlook OAuth is not supported on Android/iOS in this build.".to_string())
}

// Helper function to refresh tokens using manual OAuth2
pub async fn refresh_outlook_token(app_handle: &AppHandle) -> Result<String, String> {
    let outlook_credentials = fetch_outlook_credentials(app_handle).await
        .map_err(|e| format!("Failed to fetch Outlook credentials: {}", e))?;
    
    let client_id = outlook_credentials
        .get("client_id")
        .and_then(|v| v.as_str())
        .ok_or("No client_id in Outlook credentials")?;
    
    // Note: Don't use client_secret for public clients

    let user_id = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(app_handle) {
                Ok(id) => id,
                Err(_) => "default_user".to_string(),
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            "default_user".to_string()
        }
    };

    let token_path = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?
        .join(format!("outlook_tokens_{}.json", user_id));

    let token_json = std::fs::read_to_string(&token_path)
        .map_err(|e| format!("Failed to read token file: {}", e))?;
    let token_data: serde_json::Value = serde_json::from_str(&token_json)
        .map_err(|e| format!("Failed to parse token JSON: {}", e))?;
    let refresh_token = token_data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .ok_or("No refresh_token in token file")?;

    // Use /consumers endpoint for token refresh
    let url = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
    let params = [
        ("client_id", client_id),
        // Don't include client_secret for public clients
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
        ("scope", "https://graph.microsoft.com/calendars.readwrite offline_access"),
    ];

    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to send refresh request: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Outlook token refresh failed: {} - {}",
            status, body
        ));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let access_token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("No access_token in response")?;

    // Update token file with new access token
    let mut updated_token_data = token_data;
    updated_token_data["access_token"] = serde_json::Value::String(access_token.to_string());
    if let Some(new_refresh_token) = json.get("refresh_token").and_then(|v| v.as_str()) {
        updated_token_data["refresh_token"] = serde_json::Value::String(new_refresh_token.to_string());
    }

    fs::write(&token_path, updated_token_data.to_string()).map_err(|e| e.to_string())?;
    
    Ok(access_token.to_string())
}