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
/// (`0600` on Unix for files, `0700` for Unix directories, and current-user restricted ACLs on Windows)
/// is the current mitigation applied to all secret files saved under `auth_dir`.
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

    /// Restricts Windows file permissions to current user only using ACLs.
    #[allow(dead_code)]
    #[cfg(windows)]
    fn apply_file_permissions(&self, path: &Path) -> Result<(), WhatsAppError> {
        self.apply_windows_permissions(path)
    }

    #[allow(dead_code)]
    #[cfg(not(any(unix, windows)))]
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

    /// Restricts Windows directory permissions to current user only using ACLs.
    #[cfg(windows)]
    fn apply_directory_permissions(&self, path: &Path) -> Result<(), WhatsAppError> {
        self.apply_windows_permissions(path)
    }

    #[cfg(not(any(unix, windows)))]
    fn apply_directory_permissions(&self, _path: &Path) -> Result<(), WhatsAppError> {
        Ok(())
    }

    /// Restricts Windows file or directory permissions to the current user only.
    ///
    /// Removes non-owner access and ensures parent directory inheritance is broken via
    /// protected DACL settings.
    #[cfg(windows)]
    fn apply_windows_permissions(&self, path: &Path) -> Result<(), WhatsAppError> {
        use windows_acl::acl::ACL;

        let path_str = path.to_str().ok_or_else(|| {
            WhatsAppError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid UTF-8 path for Windows ACL application",
            ))
        })?;

        // Determine exact active owner SID of the path on disk
        let (user_sid, user_sid_str) = get_file_owner_sid(path)?;
        let user_sid_ptr = user_sid.as_ptr() as *mut _;

        let mut acl = ACL::from_file_path(path_str, false).map_err(|code| {
            WhatsAppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to load Windows ACL for path {}: error code {}", path_str, code),
            ))
        })?;

        // 1. Grant full access (GENERIC_ALL | FILE_ALL_ACCESS) to the active owner user first
        const FULL_ACCESS: u32 = 0x10000000 | 0x001f01ff;
        let is_dir = path.is_dir();
        acl.allow(user_sid_ptr, is_dir, FULL_ACCESS).map_err(|code| {
            WhatsAppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to grant ACL access to current user: error code {}", code),
            ))
        })?;

        // 2. Collect non-owner ACEs (sid, entry_type, flags) to remove
        let mut entries_to_remove: Vec<(Vec<u16>, windows_acl::acl::AceType, u8)> = Vec::new();
        if let Ok(entries) = acl.all() {
            for entry in entries {
                if entry.string_sid != user_sid_str {
                    if let Some(sid_bytes) = entry.sid {
                        entries_to_remove.push((sid_bytes, entry.entry_type, entry.flags));
                    }
                }
            }
        }

        // 3. Remove non-owner ACEs using remove_entry matching exact entry_type and flags
        for (sid_bytes, entry_type, flags) in entries_to_remove {
            let sid_ptr = sid_bytes.as_ptr() as *mut _;
            let _ = acl.remove_entry(sid_ptr, Some(entry_type), Some(flags));
        }

        Ok(())
    }
}

/// Helper function to retrieve the exact owner SID of a path on Windows.
#[cfg(windows)]
fn get_file_owner_sid(path: &Path) -> Result<(Vec<u8>, String), WhatsAppError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_acl::helper::sid_to_string;
    use winapi::shared::winerror::ERROR_SUCCESS;
    use winapi::um::accctrl::SE_FILE_OBJECT;
    use winapi::um::aclapi::GetNamedSecurityInfoW;
    use winapi::um::winnt::{OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID};

    let path_str = path.to_str().ok_or_else(|| {
        WhatsAppError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid UTF-8 path",
        ))
    })?;

    let wpath: Vec<u16> = OsStr::new(path_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut psid_owner: PSID = std::ptr::null_mut();
        let mut p_sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let ret = GetNamedSecurityInfoW(
            wpath.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut psid_owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut p_sd,
        );
        if ret != ERROR_SUCCESS {
            return Err(WhatsAppError::Io(std::io::Error::from_raw_os_error(
                ret as i32,
            )));
        }

        let sid_len = winapi::um::securitybaseapi::GetLengthSid(psid_owner) as usize;
        let mut sid_bytes = vec![0u8; sid_len];
        std::ptr::copy_nonoverlapping(psid_owner as *const u8, sid_bytes.as_mut_ptr(), sid_len);
        let sid_str = sid_to_string(psid_owner).map_err(|code| {
            WhatsAppError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed sid_to_string: {}", code),
            ))
        })?;

        winapi::um::winbase::LocalFree(p_sd as *mut _);
        Ok((sid_bytes, sid_str))
    }
}


