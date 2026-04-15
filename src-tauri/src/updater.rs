use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

// GitHub API types
#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
    prerelease: Option<bool>,
    draft: Option<bool>,
}

// Pending-update record written to disk
#[derive(Debug, Serialize, Deserialize)]
struct PendingUpdate {
    file: String,
    path: String,
    tag: String,
    downloaded_at: String,
}

// Linux package manager detection 
#[derive(Debug, Clone, Copy, PartialEq)]
enum LinuxPkgManager {
    Rpm,  // Fedora, RHEL, OpenSUSE, CentOS
    Dpkg, // Debian, Ubuntu, Mint, Pop!_OS
    None, // Fall back to AppImage
}

fn detect_linux_pkg_manager() -> LinuxPkgManager {
    // RPM-family: dnf (Fedora/RHEL) or zypper (OpenSUSE) present alongside rpm
    if which_exists("dnf") || which_exists("zypper") {
        return LinuxPkgManager::Rpm;
    }
    // DEB-family
    if which_exists("dpkg") {
        return LinuxPkgManager::Dpkg;
    }
    // rpm without dnf/zypper = older RHEL/CentOS
    if which_exists("rpm") {
        return LinuxPkgManager::Rpm;
    }
    LinuxPkgManager::None
}

fn which_exists(binary: &str) -> bool {
    std::process::Command::new("which")
        .arg(binary)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Public entry-poin
pub fn run(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // apply previously-downloaded update if present
        match apply_pending_update_if_any(&app).await {
            ApplyResult::Applied => {
                app.exit(0);
                return;
            }
            ApplyResult::NoPending => {}
            ApplyResult::Error(e) => {
                eprintln!("[updater] apply pending: {e}");
            }
        }

        // background check + download
        if let Err(e) = check_and_download(&app).await {
            eprintln!("[updater] background check failed: {e}");
        }
    });
}

// Apply pending update
enum ApplyResult {
    Applied,
    NoPending,
    Error(String),
}

async fn apply_pending_update_if_any(app: &AppHandle) -> ApplyResult {
    let meta_path = match pending_update_path(app) {
        Ok(p) => p,
        Err(e) => return ApplyResult::Error(e),
    };

    if !meta_path.exists() {
        return ApplyResult::NoPending;
    }

    let raw = match tokio::fs::read_to_string(&meta_path).await {
        Ok(s) => s,
        Err(e) => return ApplyResult::Error(format!("read metadata: {e}")),
    };

    let pending: PendingUpdate = match serde_json::from_str(&raw) {
        Ok(p) => p,
        Err(e) => return ApplyResult::Error(format!("parse metadata: {e}")),
    };

    let installer_path = PathBuf::from(&pending.path);
    if !installer_path.exists() {
        let _ = tokio::fs::remove_file(&meta_path).await;
        return ApplyResult::NoPending;
    }

    eprintln!(
        "[updater] Applying pending update {} from {}",
        pending.tag, pending.path
    );

    match launch_installer(&installer_path) {
        Ok(_) => {
            // Give the installer a short moment to start and the OS to settle
            // to avoid races on some platforms (e.g. Windows window class unregister).
            // This is intentionally short to avoid delaying exit unnecessarily.
            let _ = tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let _ = tokio::fs::remove_file(&meta_path).await;
            ApplyResult::Applied
        }
        Err(e) => {
            eprintln!("[updater] Failed to launch installer: {e}");
            ApplyResult::Error(e)
        }
    }
}

// Platform-specific installer launch
fn launch_installer(path: &PathBuf) -> Result<(), String> {
    let path_str = path.to_string_lossy();
    let name_lower = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    #[cfg(target_os = "windows")]
    {
        if name_lower.ends_with(".msi") {
            std::process::Command::new("msiexec")
                .args(["/i", &path_str, "/qb", "REBOOT=ReallySuppress"])
                .spawn()
                .map_err(|e| format!("msiexec: {e}"))?;
        } else {
            // NSIS / Inno Setup
            std::process::Command::new(path)
                .arg("/S")
                .spawn()
                .map_err(|e| format!("exe installer: {e}"))?;
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        if name_lower.ends_with(".dmg") {
            let script = format!(
                r#"set -e
MOUNT=$(hdiutil attach -nobrowse -noautoopen '{path_str}' | awk 'END{{print $NF}}')
APP=$(find "$MOUNT" -maxdepth 1 -name '*.app' | head -1)
cp -R "$APP" /Applications/
hdiutil detach "$MOUNT" -quiet"#
            );
            std::process::Command::new("bash")
                .args(["-c", &script])
                .spawn()
                .map_err(|e| format!("dmg script: {e}"))?;
        } else if name_lower.ends_with(".pkg") {
            std::process::Command::new("installer")
                .args(["-pkg", &path_str, "-target", "/"])
                .spawn()
                .map_err(|e| format!("pkg installer: {e}"))?;
        }
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        if name_lower.ends_with(".appimage") {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("chmod appimage: {e}"))?;
            std::process::Command::new(path)
                .spawn()
                .map_err(|e| format!("appimage launch: {e}"))?;

        } else if name_lower.ends_with(".deb") {
            // Prefer apt (handles deps), fall back to dpkg
            let ok = std::process::Command::new("pkexec")
                .args(["apt", "install", "-y", &path_str])
                .spawn()
                .is_ok();
            if !ok {
                std::process::Command::new("pkexec")
                    .args(["dpkg", "-i", &path_str])
                    .spawn()
                    .map_err(|e| format!("dpkg: {e}"))?;
            }

        } else if name_lower.ends_with(".rpm") {
            // Prefer dnf (handles deps), fall back to raw rpm
            let ok = std::process::Command::new("pkexec")
                .args(["dnf", "install", "-y", &path_str])
                .spawn()
                .is_ok();
            if !ok {
                std::process::Command::new("pkexec")
                    .args(["rpm", "-Uvh", "--force", &path_str])
                    .spawn()
                    .map_err(|e| format!("rpm: {e}"))?;
            }
        }
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(format!("Unsupported OS: {}", std::env::consts::OS))
}

// Check GitHub and download
async fn check_and_download(app: &AppHandle) -> Result<(), String> {
    const OWNER: &str = "DomanskiFilip";
    const REPO: &str = "CATdesktop";

    let release = fetch_latest_release(OWNER, REPO).await?;

    if release.prerelease.unwrap_or(false) || release.draft.unwrap_or(false) {
        return Ok(());
    }

    let remote = release.tag_name.trim_start_matches('v');
    let local = env!("CARGO_PKG_VERSION");

    if !is_newer(remote, local) {
        eprintln!("[updater] Up to date ({local})");
        return Ok(());
    }

    eprintln!("[updater] New version: {remote} (running {local})");

    // If a pending update is already present, skip downloading again to avoid duplicates
    match pending_update_path(app) {
        Ok(p) if p.exists() => {
            eprintln!("[updater] Pending update already exists; skipping download");
            return Ok(());
        }
        Ok(_) => {}
        Err(e) => {
            // Non-fatal: log and continue
            eprintln!("[updater] failed to check pending update: {e}");
        }
    }

    let assets: Vec<(String, String, Option<u64>)> = release
        .assets
        .into_iter()
        .map(|a| (a.name, a.browser_download_url, a.size))
        .collect();

    let (name, url) =
        pick_asset(&assets).ok_or_else(|| "No suitable asset for this platform".to_string())?;

    eprintln!("[updater] Downloading: {name}");
    let dest = download(app, &name, &url).await?;
    write_pending_metadata(app, &name, &dest, &release.tag_name).await?;
    eprintln!("[updater] Ready → {}", dest.display());

    Ok(())
}

// GitHub fetch
async fn fetch_latest_release(owner: &str, repo: &str) -> Result<GhRelease, String> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header(
            "User-Agent",
            format!("CAT-Updater/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API: HTTP {}", resp.status()));
    }

    resp.json::<GhRelease>()
        .await
        .map_err(|e| format!("parse: {e}"))
}

// Asset selection
fn pick_asset(assets: &[(String, String, Option<u64>)]) -> Option<(String, String)> {
    let prefs: Vec<&str> = match std::env::consts::OS {
        "windows" => vec![".msi", ".exe", ".zip"],
        "macos"   => vec![".dmg", ".pkg", ".zip", ".tar.gz"],
        "linux"   => match detect_linux_pkg_manager() {
            LinuxPkgManager::Rpm  => vec![".rpm",      ".AppImage", ".deb"],
            LinuxPkgManager::Dpkg => vec![".deb",      ".AppImage", ".rpm"],
            LinuxPkgManager::None => vec![".AppImage", ".deb",      ".rpm"],
        },
        _ => vec![".zip", ".tar.gz"],
    };

    for ext in &prefs {
        if let Some((n, u, _)) = assets
            .iter()
            .find(|(n, _, _)| n.to_lowercase().ends_with(&ext.to_lowercase()))
        {
            return Some((n.clone(), u.clone()));
        }
    }

    // Absolute fallback: largest asset
    assets
        .iter()
        .max_by_key(|(_, _, s)| *s)
        .map(|(n, u, _)| (n.clone(), u.clone()))
}

// Streaming download 
async fn download(app: &AppHandle, name: &str, url: &str) -> Result<PathBuf, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            format!("CAT-Updater/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|e| format!("download start: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }

    let updates_dir = updates_dir(app)?;
    let dest = updates_dir.join(name);
    let mut file = tokio::fs::File::create(&dest)
        .await
        .map_err(|e| format!("create file: {e}"))?;

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("stream: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("write: {e}"))?;
        downloaded += chunk.len() as u64;

        // Frontend can optionally listen to "updater:progress" for a status bar
        let _ = app.emit(
            "updater:progress",
            serde_json::json!({
                "file": name,
                "downloaded": downloaded,
                "total": total,
                "percent": if total > 0 { downloaded as f64 / total as f64 } else { 0.0 }
            }),
        );
    }

    file.flush().await.map_err(|e| format!("flush: {e}"))?;
    Ok(dest)
}

// Metadata helpers
async fn write_pending_metadata(
    app: &AppHandle,
    name: &str,
    dest: &PathBuf,
    tag: &str,
) -> Result<(), String> {
    let pending = PendingUpdate {
        file: name.to_string(),
        path: dest.to_string_lossy().to_string(),
        tag: tag.to_string(),
        downloaded_at: chrono::Utc::now().to_rfc3339(),
    };
    let json =
        serde_json::to_string_pretty(&pending).map_err(|e| format!("serialize metadata: {e}"))?;
    let path = pending_update_path(app)?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| format!("write metadata: {e}"))?;
    Ok(())
}

fn pending_update_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(updates_dir(app)?.join("pending_update.json"))
}

fn updates_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    let dir = base.join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create updates dir: {e}"))?;
    Ok(dir)
}

// Semver comparison
fn is_newer(remote: &str, local: &str) -> bool {
    fn parse(s: &str) -> Option<(u64, u64, u64)> {
        let mut parts = s.splitn(3, '.').map(|p| p.parse::<u64>().ok());
        Some((parts.next()??, parts.next()??, parts.next()??))
    }
    match (parse(remote), parse(local)) {
        (Some(r), Some(l)) => r > l,
        _ => remote != local,
    }
}
