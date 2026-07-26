// auth.rs — QR code generation and auth credential persistence for WhatsApp.
//
// Hey friend! This file manages authentication credentials, session pairing keys, and
// QR code payload generation for the WhatsApp channel.

use std::path::PathBuf;
use qrcode::QrCode;
use tracing::info;

use crate::error::WhatsAppError;
use crate::types::QrCodeState;

/// Authentication and pairing manager for WhatsApp Web multi-device connection.
///
/// # Security & Credential Storage Note
/// Full credential encryption at rest is not yet implemented in this pass. File-permission hardening
/// (`0600` on Unix for files, `0700` for directories) is the current mitigation applied to all secret
/// files saved under `auth_dir`.
pub struct WhatsAppAuth {
    /// Directory where auth keys and credentials are stored on disk.
    auth_dir: PathBuf,
}

impl WhatsAppAuth {
    /// Creates a new `WhatsAppAuth` instance targeting the specified directory path.
    pub fn new(auth_dir: PathBuf) -> Self {
        Self { auth_dir }
    }

    /// Initializes the credentials directory, ensuring parent folders exist with secure permissions (`0700` on Unix).
    pub fn init(&self) -> Result<(), WhatsAppError> {
        if !self.auth_dir.exists() {
            std::fs::create_dir_all(&self.auth_dir).map_err(|e| {
                WhatsAppError::AuthFailed(format!("Failed to create auth dir {:?}: {e}", self.auth_dir))
            })?;
            let _ = harden_directory_permissions(&self.auth_dir);
        }
        Ok(())
    }

    /// Checks if saved credentials already exist in the auth directory.
    pub fn has_credentials(&self) -> bool {
        let creds_file = self.auth_dir.join("creds.json");
        creds_file.exists() && creds_file.metadata().map(|m| m.len() > 0).unwrap_or(false)
    }

    /// Generates a QR code state object and converts the raw string payload into an ASCII
    /// or SVG representation.
    pub fn generate_qr_state(raw_payload: &str, valid_seconds: i64) -> Result<QrCodeState, WhatsAppError> {
        let now = unix_timestamp_secs() as i64;
        let expires_at = now + valid_seconds;

        // Verify that the payload can be validly encoded into a QR code.
        let _code = QrCode::new(raw_payload.as_bytes()).map_err(|e| {
            WhatsAppError::AuthFailed(format!("Failed to encode QR code: {e}"))
        })?;

        Ok(QrCodeState {
            payload: raw_payload.to_string(),
            expires_at,
        })
    }

    /// Helper to render a QR code string as an ASCII string for terminal display.
    pub fn render_ascii(raw_payload: &str) -> Result<String, WhatsAppError> {
        let code = QrCode::new(raw_payload.as_bytes()).map_err(|e| {
            WhatsAppError::AuthFailed(format!("Failed to encode QR code: {e}"))
        })?;

        // Render as unicode blocks (ideal for terminal output)
        let string = code
            .render::<char>()
            .quiet_zone(true)
            .module_dimensions(2, 1)
            .build();

        Ok(string)
    }

    /// Clears saved credentials (used during sign-out / unlink).
    pub fn clear_credentials(&self) -> Result<(), WhatsAppError> {
        if self.auth_dir.exists() {
            std::fs::remove_dir_all(&self.auth_dir).map_err(|e| {
                WhatsAppError::AuthFailed(format!("Failed to clear auth dir {:?}: {e}", self.auth_dir))
            })?;
            info!("Cleared WhatsApp authentication credentials.");
        }
        Ok(())
    }
    /// Writes an authentication credential file under `auth_dir` with restrictive file permissions (`0600` on Unix).
    pub fn write_credential(&self, filename: &str, content: &[u8]) -> Result<PathBuf, WhatsAppError> {
        self.init()?;
        let path = self.auth_dir.join(filename);
        std::fs::write(&path, content).map_err(|e| {
            WhatsAppError::AuthFailed(format!("Failed to write credential file {:?}: {e}", path))
        })?;
        let _ = harden_file_permissions(&path);
        Ok(path)
    }
}

/// Helper function to set file permissions to 0600 (owner read/write only) on Unix systems.
#[cfg(unix)]
fn harden_file_permissions(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn harden_file_permissions(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Helper function to set directory permissions to 0700 (owner read/write/exec only) on Unix systems.
#[cfg(unix)]
fn harden_directory_permissions(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o700);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn harden_directory_permissions(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Helper function to get wall clock time in seconds.
fn unix_timestamp_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
