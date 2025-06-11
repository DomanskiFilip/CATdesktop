use serde::Deserialize;
use std::time::Instant;
use std::net::TcpListener;
use std::io::Read;
use urlencoding;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

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

use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenResponse, TokenUrl, basic::BasicClient,
};
use open;
use std::io::{self, Write};
use std::fs;

pub async fn oauth2_flow(app_handle: &AppHandle, timeout: u64) -> Result<String, String> {
    // Read and parse the client secret JSON
    let secret_json = fs::read_to_string("src/client_secret_1_99017034100-i04dv1p35v7rmbvjffjro807b8vupeku.apps.googleusercontent.com.json")
        .map_err(|e| e.to_string())?;
    let secret: GoogleSecret = serde_json::from_str(&secret_json).map_err(|e| e.to_string())?;

    let client_id = ClientId::new(secret.installed.client_id);
    let client_secret = ClientSecret::new(secret.installed.client_secret);
    let redirect_url = RedirectUrl::new(secret.installed.redirect_uris[0].clone()).unwrap();

    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap();
    let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap();

    let client = BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url))
        .set_redirect_uri(redirect_url);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid email profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

   open::that(auth_url.as_str()).map_err(|e| e.to_string())?;

    // Listen for the redirect with the code
    let listener = TcpListener::bind("127.0.0.1:1420").map_err(|e| e.to_string())?;
    let mut code = String::new();
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
                                    // Respond to browser
                                    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                        <html><body><h2>Authentication complete. You can close this window.</h2></body></html>";
                                    stream.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
                                    code = value.to_string();
                                    println!("Extracted code: {}", code);
                                    break 'outer; // <-- Break out of the main loop!
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Respond to browser for non-auth requests (e.g., favicon)
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                <html><body><h2>Waiting for authentication...</h2></body></html>";
            stream.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
        }
    }
    let code = urlencoding::decode(&code).map_err(|e| e.to_string())?.to_string();
    println!("Exchanging code for token...");
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| {
            println!("Token exchange error: {:?}", e);
            e.to_string()
        })?;

    let token = token_result.access_token().secret().to_string();
println!("Token exchange complete, writing access_token.txt...");

// Get a safe app data directory
let token_path = app_handle.path().app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?
        .join("access_token.txt");
fs::create_dir_all(token_path.parent().unwrap()).map_err(|e| e.to_string())?;
fs::write(token_path, &token).map_err(|e| e.to_string())?;
println!("data dir created and token written to access_token.txt");
Ok(token)
}