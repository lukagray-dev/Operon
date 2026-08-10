// workspace.rs — Shared workspace directory & per-contact session manager.
//
// Hey friend! This module manages the single shared workspace root directory for WhatsApp session turn execution.
// By default, it targets `~/.operon/workspace/` (or a configured custom path) so that tool calls match pre-configured PolicyConfig rules.
//
// Role-specific instructions are pushed in-memory via `SessionConfig.channel_instructions` per-turn,
// rather than being written to a shared `AGENTS.md` file on disk. This completely eliminates concurrent turn races.
//
// Finally, it computes per-user JSON session file paths at `~/.operon/sessions/whatsapp/<contact_number>/<session_id>.json`
// to maintain complete conversation history isolation between contacts.

use std::path::PathBuf;
use tracing::info;

use crate::config::WhatsAppConfig;
use crate::error::WhatsAppError;
use crate::types::ContactId;

/// Workspace manager for shared workspace directory and per-contact session isolation in WhatsApp.
pub struct WhatsAppWorkspaceManager {
    /// Single shared base directory for channel workspace (`~/.operon/workspace/` by default).
    base_workspace_dir: PathBuf,
    /// Base directory for per-contact channel sessions (`~/.operon/sessions/whatsapp/`).
    base_sessions_dir: PathBuf,
}

impl Default for WhatsAppWorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WhatsAppWorkspaceManager {
    /// Creates a new `WhatsAppWorkspaceManager` using standard system default paths (`~/.operon/workspace`).
    pub fn new() -> Self {
        let base_workspace_dir = WhatsAppConfig::default().resolved_workspace_dir();
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let base_sessions_dir = home.join(".operon").join("sessions").join("whatsapp");

        Self {
            base_workspace_dir,
            base_sessions_dir,
        }
    }

    /// Creates a custom `WhatsAppWorkspaceManager` targeting specified root directories (useful for testing or custom paths).
    pub fn with_paths(base_workspace_dir: PathBuf, base_sessions_dir: PathBuf) -> Self {
        Self {
            base_workspace_dir,
            base_sessions_dir,
        }
    }

    /// Returns the single shared workspace directory path.
    ///
    /// The `_contact` argument is retained for signature compatibility and logging context, but all WhatsApp contacts
    /// now share the single configured workspace root to ensure policy coverage matches pre-configured DirectoryPolicy rules.
    pub fn workspace_dir_for(&self, _contact: &ContactId) -> PathBuf {
        self.base_workspace_dir.clone()
    }

    /// Computes the JSON session file path for a specific contact and session ID.
    ///
    /// Path format: `~/.operon/sessions/whatsapp/<contact_number>/<session_id>.json`
    pub fn session_file_path_for(&self, contact: &ContactId, session_id: &str) -> PathBuf {
        self.base_sessions_dir
            .join(contact.as_str())
            .join(format!("{}.json", session_id))
    }

    /// Provisions and ensures existence of the shared workspace folder.
    ///
    /// Role-specific instructions are now passed in-memory via `SessionConfig` per turn,
    /// leaving any on-disk `AGENTS.md` untouched for the user's custom instructions.
    pub fn provision_workspace(
        &self,
        contact: &ContactId,
        _is_owner: bool,
    ) -> Result<PathBuf, WhatsAppError> {
        let dir = self.workspace_dir_for(contact);

        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| {
                WhatsAppError::Workspace(format!("Failed to create workspace dir {:?}: {e}", dir))
            })?;
            info!(
                "Created shared workspace directory for WhatsApp: {:?}",
                dir
            );
        }

        Ok(dir)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Role-Specific Channel Instructions Generators
// ─────────────────────────────────────────────────────────────────────────────

/// Generates system prompt guidelines for contacts classified as `Owner` (main number / allowlist).
pub fn generate_owner_channel_instructions(contact: &ContactId) -> String {
    format!(
        r#"# AGENTS.md — Operon Channel Context

## User Identity
- Contact: `{contact}`
- Access Role: **OWNER / ADMINISTRATOR**

## Guidelines
- You are communicating with the system owner or an authorized allowlist user over WhatsApp.
- You have full access to owner tools, filesystem utilities, shell commands, and administrative capabilities according to the owner policy.
- Maintain a helpful, efficient, and concise communication style suitable for WhatsApp.
"#
    )
}

/// Generates system prompt guidelines for contacts classified as `External` (unlisted / outsiders).
pub fn generate_external_channel_instructions(contact: &ContactId) -> String {
    format!(
        r#"# AGENTS.md — Operon Channel Context

## User Identity
- Contact: `{contact}`
- Access Role: **EXTERNAL USER / OUTSIDER**

## Guidelines
- You are communicating with an external user over WhatsApp whose number is not in the system allowlist.
- You operate under RESTRICTED external policy permissions.
- Do NOT expose confidential system files, private credentials, or execute privileged commands.
- Maintain a polite, safe, and helpful demeanor while enforcing security boundaries.
"#
    )
}
