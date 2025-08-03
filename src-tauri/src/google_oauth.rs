#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::user_utils::get_current_user_id;
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::user_utils::get_current_user_id_mobile;
use oauth2::basic::BasicTokenType;
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
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

#[allow(dead_code)]
#[derive(Deserialize)]
struct Installed {
    client_id: String,
    client_secret: String,
    redirect_uris: Vec<String>,
}

#[derive(Deserialize)]
struct GoogleSecret {
    installed: Installed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GoogleTokenExtraFields {
    pub id_token: Option<String>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub async fn oauth2_flow(app_handle: &AppHandle, timeout: u64) -> Result<String, String> {
    let client_id = ClientId::new(
        "99017034100-stfl2943ef0lnp7c36upsqrstaub49ns.apps.googleusercontent.com".to_string(),
    );
    let secret_json = fs::read_to_string("google_client.json").map_err(|e| e.to_string())?;
    let secret: GoogleSecret = serde_json::from_str(&secret_json).map_err(|e| e.to_string())?;
    let client_secret = ClientSecret::new(secret.installed.client_secret);
    let redirect_url = RedirectUrl::new("http://127.0.0.1:1425".to_string()).unwrap();
    let auth_url =
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap();
    let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap();

    let client = BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url))
        .set_redirect_uri(redirect_url);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/calendar".to_string(),
        ))
        .set_pkce_challenge(pkce_challenge)
        .url();

    open::that(auth_url.as_str()).map_err(|e| e.to_string())?;

    // Listen for the redirect with the code
    let listener = TcpListener::bind("127.0.0.1:1425").map_err(|e| e.to_string())?;
    let code: String;
    let start_time = Instant::now();
    'outer: loop {
        // Check for timeout
        if start_time.elapsed().as_secs() > timeout {
            return Err("OAuth authentication timed out".to_string());
        }
        let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
        let mut buffer = [0; 2048];
        let bytes_read = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);
        // Try to extract the code from the request line
        if let Some(line) = request.lines().next() {
            if line.starts_with("GET /?") {
                if let Some(query_start) = line.find("/?") {
                    if let Some(space_pos) = line[query_start..].find(' ') {
                        let query = &line[query_start + 2..query_start + space_pos];
                        for param in query.split('&') {
                            let mut parts = param.splitn(2, '=');
                            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                                if key == "code" {
                                    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                        <html><body><h2>Authentication complete. You can close this window.</h2></body></html>";
                                    stream
                                        .write_all(response.as_bytes())
                                        .map_err(|e| e.to_string())?;
                                    code = value.to_string();
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                <html><body><h2>Waiting for authentication...</h2></body></html>";
            stream
                .write_all(response.as_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    let code = urlencoding::decode(&code)
        .map_err(|e| e.to_string())?
        .to_string();
    let token_result: oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, BasicTokenType> =
        client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| {
                println!("Token exchange error: {:?}", e);
                e.to_string()
            })?;

    let access_token = token_result.access_token().secret().to_string();
    let refresh_token = token_result.refresh_token().map(|t| t.secret().to_string());
    // Get the current user's email (user_id)
    let user_id: String = {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            match get_current_user_id(&app_handle) {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Ok(String::new());
                }
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            match get_current_user_id_mobile().await {
                Ok(id) => id,
                Err(e) => {
                    println!("Failed to get user ID: {}", e);
                    return Ok(String::new());
                }
            }
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
        .join(format!("google_tokens_{}.json", user_id));
    fs::create_dir_all(token_path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(token_path, token_data.to_string()).map_err(|e| e.to_string())?;
    println!("Google OAuth tokens saved for user: {}", user_id);

    Ok(access_token)
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn oauth2_flow(_app_handle: &AppHandle, _timeout: u64) -> Result<String, String> {
    Err("Google OAuth is not supported on Android in this build.".to_string())
}
