//! Authentication credentials and permission management for WhatsApp Web.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::error::WhatsAppError;
use crate::storage::persisted_device_exists;
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

    /// Returns a reference to the auth directory path.
    pub fn auth_dir(&self) -> &Path {
        &self.auth_dir
    }

    /// Initializes the credentials directory on disk with restricted 0700 Unix permissions.
    pub fn init(&self) -> Result<(), WhatsAppError> {
        if !self.auth_dir.exists() {
            fs::create_dir_all(&self.auth_dir)?;
            info!(
                path = %self.auth_dir.display(),
                "Created WhatsApp auth directory"
            );
        }

        self.apply_directory_permissions(&self.auth_dir)?;
        Ok(())
    }

    /// Returns `true` if a linked session database exists in `auth_dir/session.db`.
    pub fn has_credentials(&self) -> bool {
        let db_path = self.auth_dir.join("session.db");
        persisted_device_exists(&db_path)
    }

    /// Helper to convert a raw QR code string into a structured [`QrCodeState`].
    pub fn generate_qr_state(payload: impl Into<String>, ttl_secs: u64) -> QrCodeState {
        let payload_str = payload.into();
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + ttl_secs;

        QrCodeState {
            payload: payload_str,
            expires_at: expires_at as i64,
        }
    }

    /// Renders a QR code payload into an SVG string for GUI rendering.
    pub fn render_svg(payload: &str) -> Result<String, WhatsAppError> {
        use qrcode::render::svg;
        let qr = qrcode::QrCode::new(payload.as_bytes())
            .map_err(|e| WhatsAppError::QrGenerationFailed(e.to_string()))?;
        let image = qr
            .render::<svg::Color>()
            .min_dimensions(260, 260)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build();
        Ok(image)
    }

    /// Renders a QR code payload into an ASCII string for terminal (TUI) display.
    pub fn render_ascii(payload: &str) -> Result<String, WhatsAppError> {
        let qr = qrcode::QrCode::new(payload.as_bytes())
            .map_err(|e| WhatsAppError::QrGenerationFailed(e.to_string()))?;
        let ascii = qr
            .render::<qrcode::render::unicode::Dense1x2>()
            .quiet_zone(true)
            .build();
        Ok(ascii)
    }

    /// Clears session credentials by removing `session.db` and its sidecar files (`-wal`, `-shm`).
    pub fn clear_credentials(&self) -> Result<(), WhatsAppError> {
        let primary = self.auth_dir.join("session.db");
        let wal = self.auth_dir.join("session.db-wal");
        let shm = self.auth_dir.join("session.db-shm");

        for path in &[primary, wal, shm] {
            if path.exists() {
                fs::remove_file(path)?;
                info!(path = %path.display(), "Removed WhatsApp session file");
            }
        }
        Ok(())
    }

    /// Writes a credential file inside `auth_dir` with `0600` permissions on Unix.
    pub fn write_credential(
        &self,
        filename: &str,
        content: &[u8],
    ) -> Result<PathBuf, WhatsAppError> {
        self.init()?;
        let path = self.auth_dir.join(filename);
        fs::write(&path, content)?;
        self.apply_file_permissions(&path)?;
        Ok(path)
    }

    /// Restricts Unix file permissions to `0600` (owner read/write only).
    #[allow(dead_code)]
    #[cfg(unix)]
    fn apply_file_permissions(&self, path: &Path) -> Result<(), WhatsAppError> {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }

    #[allow(dead_code)]
    #[cfg(not(unix))]
    fn apply_file_permissions(&self, _path: &Path) -> Result<(), WhatsAppError> {
        Ok(())
    }

    /// Restricts Unix directory permissions to `0700` (owner read/write/execute only).
    #[cfg(unix)]
    fn apply_directory_permissions(&self, path: &Path) -> Result<(), WhatsAppError> {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn apply_directory_permissions(&self, _path: &Path) -> Result<(), WhatsAppError> {
        Ok(())
    }
}
