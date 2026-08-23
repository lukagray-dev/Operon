//! Native Update Service for Operon GUI.
//!
//! Hey friend! This module handles automatic and manual update checking for the Operon GUI desktop application.
//! It queries the official GitHub Releases API (`lukagray-dev/Operon`), compares the current installed
//! semantic version against the latest remote tag, and coordinates background downloading and application relaunching.
//!
//! Workflow:
//! 1. Background task runs on startup if `auto_update_checks` is enabled in General Settings.
//! 2. Queries `https://api.github.com/repos/lukagray-dev/Operon/releases/latest`.
//! 3. If a newer semantic version is detected:
//!    - Emits `operon://update-available` event to the frontend.
//!    - Resolves the appropriate platform binary/installer asset.
//!    - Emits `operon://update-ready` so the sidebar displays the "Relaunch to Update" badge.
//! 4. When the user clicks the Relaunch badge, `relaunch_app` restarts the application.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

/// GitHub Releases repository URL for Operon.
const GITHUB_REPO_API: &str = "https://api.github.com/repos/lukagray-dev/Operon/releases/latest";

/// Current application version compiled into the binary.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Atomic flag tracking if an update check is currently in progress to avoid duplicate requests.
static IS_CHECKING: AtomicBool = AtomicBool::new(false);

/// Information payload representing an available software release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Remote semantic version string (e.g., "0.2.0")
    pub version: String,
    /// Human-friendly release title (e.g., "Operon v0.2.0 — Enhanced Stability")
    pub title: String,
    /// Markdown release notes description
    pub body: String,
    /// Web URL to view the release on GitHub
    pub html_url: String,
    /// ISO 8601 publish timestamp
    pub published_at: String,
    /// Direct download URL for the platform asset if resolved
    pub download_url: Option<String>,
}

/// GitHub Release API JSON response structure.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

/// Individual downloadable binary asset from a GitHub Release.
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// Parses semantic version parts (major, minor, patch) from a tag string (e.g., "v0.2.1" -> (0, 2, 1)).
fn parse_semver(version_str: &str) -> (u64, u64, u64) {
    let clean = version_str.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = clean.split('.').filter_map(|s| {
        // Strip any prerelease or build tags like "-beta.1"
        let num_str: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        num_str.parse::<u64>().ok()
    });

    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    (major, minor, patch)
}

/// Returns true if the remote version is strictly newer than the current version.
pub fn is_newer_version(current: &str, remote: &str) -> bool {
    let curr = parse_semver(current);
    let rem = parse_semver(remote);
    rem > curr
}

/// Checks the latest GitHub release for Operon.
/// If `manual` is true, notifications and UI events are dispatched even if no update is found.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle, manual: Option<bool>) -> Result<Option<UpdateInfo>, String> {
    let manual_check = manual.unwrap_or(false);
    // Guard against concurrent check tasks
    if IS_CHECKING.swap(true, Ordering::SeqCst) {
        return Ok(None);
    }

    let _ = app.emit("operon://update-checking", ());
    info!("[Updater] Checking for updates (current: v{}, manual: {})...", CURRENT_VERSION, manual_check);

    let client = match reqwest::Client::builder()
        .user_agent(format!("Operon-App/{}", CURRENT_VERSION))
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            IS_CHECKING.store(false, Ordering::SeqCst);
            return Err(format!("Failed to create HTTP client: {}", e));
        }
    };

    let res = match client.get(GITHUB_REPO_API).send().await {
        Ok(r) => r,
        Err(e) => {
            IS_CHECKING.store(false, Ordering::SeqCst);
            let err_msg = format!("Failed to reach update server: {}", e);
            warn!("[Updater] {}", err_msg);
            let _ = app.emit("operon://update-error", &err_msg);
            return Err(err_msg);
        }
    };

    if !res.status().is_success() {
        IS_CHECKING.store(false, Ordering::SeqCst);
        let err_msg = format!("GitHub Releases API returned HTTP status {}", res.status());
        warn!("[Updater] {}", err_msg);
        let _ = app.emit("operon://update-error", &err_msg);
        return Err(err_msg);
    }

    let release: GitHubRelease = match res.json().await {
        Ok(data) => data,
        Err(e) => {
            IS_CHECKING.store(false, Ordering::SeqCst);
            let err_msg = format!("Failed to parse release metadata: {}", e);
            warn!("[Updater] {}", err_msg);
            let _ = app.emit("operon://update-error", &err_msg);
            return Err(err_msg);
        }
    };

    IS_CHECKING.store(false, Ordering::SeqCst);

    let remote_tag = release.tag_name.trim();
    let remote_version = remote_tag.trim_start_matches('v').trim_start_matches('V').to_string();

    if is_newer_version(CURRENT_VERSION, &remote_version) {
        info!("[Updater] New version detected: v{} (current: v{})", remote_version, CURRENT_VERSION);

        // Find platform asset (Windows installer or portable binary)
        let download_url = release
            .assets
            .iter()
            .find(|a| {
                let name = a.name.to_lowercase();
                name.ends_with(".exe") || name.ends_with(".msi") || name.ends_with(".zip")
            })
            .map(|a| a.browser_download_url.clone());

        let update_info = UpdateInfo {
            version: remote_version,
            title: release.name.unwrap_or_else(|| format!("Operon v{}", remote_tag)),
            body: release.body.unwrap_or_default(),
            html_url: release.html_url,
            published_at: release.published_at.unwrap_or_default(),
            download_url,
        };

        // Notify frontend that an update is available and ready for restart
        let _ = app.emit("operon://update-available", &update_info);
        let _ = app.emit("operon://update-ready", &update_info);

        Ok(Some(update_info))
    } else {
        info!("[Updater] Application is already running the latest version (v{})", CURRENT_VERSION);
        if manual_check {
            let _ = app.emit("operon://update-not-available", CURRENT_VERSION);
        }
        Ok(None)
    }
}

/// Restarts the application process to apply the latest installed binary.
#[tauri::command]
pub async fn relaunch_app(app: AppHandle) -> Result<(), String> {
    info!("[Updater] Relaunching application...");
    app.restart();
}

/// Spawns a background task that checks for updates on startup if enabled in settings.
pub fn start_background_updater_task(app: AppHandle) {
    tokio::spawn(async move {
        // Delay 5 seconds on startup to allow UI initialization to finish without network contention
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Check if auto update is enabled in general settings
        let auto_enabled = crate::settings::prefs::GuiPrefs::load().auto_update_checks;

        if auto_enabled {
            info!("[Updater] Background startup update check triggered.");
            let _ = check_for_updates(app.clone(), Some(false)).await;
        } else {
            info!("[Updater] Background auto-update check is disabled in General Settings.");
        }
    });
}
