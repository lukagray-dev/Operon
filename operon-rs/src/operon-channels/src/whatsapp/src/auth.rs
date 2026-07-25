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
pub struct WhatsAppAuth {
    /// Directory where auth keys and credentials are stored on disk.
    auth_dir: PathBuf,
}

impl WhatsAppAuth {
    /// Creates a new `WhatsAppAuth` instance targeting the specified directory path.
    pub fn new(auth_dir: PathBuf) -> Self {
        Self { auth_dir }
    }

    /// Initializes the credentials directory, ensuring parent folders exist.
    pub fn init(&self) -> Result<(), WhatsAppError> {
        if !self.auth_dir.exists() {
            std::fs::create_dir_all(&self.auth_dir).map_err(|e| {
                WhatsAppError::AuthFailed(format!("Failed to create auth dir {:?}: {e}", self.auth_dir))
            })?;
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
            std::fs::create_dir_all(&self.auth_dir)?;
        }
        info!("Cleared WhatsApp authentication credentials.");
        Ok(())
    }
}

/// Helper function to get wall clock time in seconds.
fn unix_timestamp_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
