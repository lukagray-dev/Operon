// config.rs — Policy configuration types for operon-policy.
//
// TODO: This module will be migrated to the `operon-config` crate once that
// crate is built. For now, PolicyConfig lives here as a self-contained
// serializable type — same pattern as SessionConfig in operon-session/config.rs.
// When migrating: move PolicyConfig, GlobalPolicy, and DirectoryPolicy to
// operon-config and re-export from here for backwards compatibility.
//
// PolicyConfig is the authoritative source of what Operon is permitted to do.
// It is loaded once at startup by the session runner and passed to PolicyResolver.
//
// Structure:
//   PolicyConfig
//   ├── global: GlobalPolicy          — permissions for non-filesystem tools
//   └── directories: Vec<DirectoryPolicy> — per-directory, per-role, per-tool permissions
//
// Every field is serde-compatible so the config can be stored as TOML or JSON
// by operon-config and loaded into this struct at runtime.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::PolicyError;
use crate::types::{CallerRole, DirTool, FsTool, GlobalTool, PermissionMode};

// ─────────────────────────────────────────────────────────────────────────────
// GlobalPolicy
// ─────────────────────────────────────────────────────────────────────────────

/// Permission settings for tools that are not directory-scoped.
///
/// Global tools (web, subagent, ask, todo, load_tools) have their permissions
/// set here once — they apply regardless of which directory the session operates in.
///
/// # Per-role design
///
/// Owner and External have independent permission maps. A common setup:
/// - Owner: all global tools set to Allow.
/// - External: web = Allow, todo = Allow, ask = Allow, subagent = Deny, load_tools = Allow.
///
/// # Missing entries
///
/// If a `GlobalTool` key is absent from a role's map, the resolver defaults to `Deny`.
/// This "default deny" posture means new tools are safe until explicitly enabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPolicy {
    /// Permissions for the owner role, keyed by GlobalTool variant.
    /// Missing entries default to Deny.
    #[serde(default)]
    pub owner: HashMap<GlobalTool, PermissionMode>,

    /// Permissions for the external role, keyed by GlobalTool variant.
    /// Missing entries default to Deny.
    #[serde(default)]
    pub external: HashMap<GlobalTool, PermissionMode>,
}

impl GlobalPolicy {
    /// Look up the permission mode for a tool + role combination.
    ///
    /// Returns `PermissionMode::Deny` if the tool is not present in the map.
    /// This enforces the "default deny" posture for unset entries.
    pub fn mode_for(&self, tool: GlobalTool, role: CallerRole) -> PermissionMode {
        let map = match role {
            CallerRole::Owner => &self.owner,
            CallerRole::External => &self.external,
        };
        // Default to Deny if the tool has no explicit entry.
        map.get(&tool).copied().unwrap_or(PermissionMode::Deny)
    }
}

impl Default for GlobalPolicy {
    /// Creates a GlobalPolicy with all tools denied for all roles.
    ///
    /// This is the safest default — the owner must explicitly grant access.
    fn default() -> Self {
        Self {
            owner: HashMap::new(),
            external: HashMap::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DirectoryPolicy
// ─────────────────────────────────────────────────────────────────────────────

/// Permission settings for a single allowed directory.
///
/// Each `DirectoryPolicy` governs what Operon can do inside a specific directory
/// for each caller role. Multiple directories can have completely different rules.
///
/// # Canonical path
///
/// The `path` field is stored as a **canonical absolute path** after `PolicyConfig::validate()`
/// is called. Canonicalization resolves symlinks and `..` components so path comparison
/// in the resolver is reliable.
///
/// # Per-tool vs. group-level permissions
///
/// The `owner` and `external` maps use `DirTool` keys. Setting permissions at the
/// `DirTool::Fs(FsTool::Read)` level gives per-tool control. The UI can show a group-level
/// summary (e.g. "Custom" when individual tools have different modes), but the policy
/// always stores and enforces at the individual tool level.
///
/// # Missing entries
///
/// If a `DirTool` key is absent, the resolver defaults to `Deny`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryPolicy {
    /// The absolute, canonical path to this allowed directory.
    ///
    /// Anything that does not start with this path (after canonicalization)
    /// is outside this directory and must be checked against other entries.
    pub path: PathBuf,

    /// Tool permissions for the owner role inside this directory.
    /// Missing entries default to Deny.
    #[serde(default)]
    pub owner: HashMap<DirTool, PermissionMode>,

    /// Tool permissions for the external role inside this directory.
    /// Missing entries default to Deny.
    #[serde(default)]
    pub external: HashMap<DirTool, PermissionMode>,
}

impl DirectoryPolicy {
    /// Look up the permission mode for a dir-tool + role combination.
    ///
    /// Returns `PermissionMode::Deny` if the tool has no explicit entry.
    pub fn mode_for(&self, tool: DirTool, role: CallerRole) -> PermissionMode {
        let map = match role {
            CallerRole::Owner => &self.owner,
            CallerRole::External => &self.external,
        };
        map.get(&tool).copied().unwrap_or(PermissionMode::Deny)
    }

    /// Convenience: create a DirectoryPolicy with a fully-open owner permission
    /// and fully-denied external permission for all dir-tools.
    ///
    /// Useful for testing and for new directories added by the owner where
    /// the owner wants immediate full access and will configure external later.
    pub fn owner_full_access(path: PathBuf) -> Self {
        use DirTool::{Bash, Fs};
        use FsTool::*;
        let all_allow: HashMap<DirTool, PermissionMode> = [
            (Fs(Read), PermissionMode::Allow),
            (Fs(Write), PermissionMode::Allow),
            (Fs(Edit), PermissionMode::Allow),
            (Fs(Append), PermissionMode::Allow),
            (Fs(Grep), PermissionMode::Allow),
            (Fs(Ls), PermissionMode::Allow),
            (Fs(Delete), PermissionMode::Allow),
            (Bash, PermissionMode::Allow),
        ]
        .into_iter()
        .collect();

        Self {
            path,
            owner: all_allow,
            external: HashMap::new(), // All external tools default to Deny.
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PolicyConfig
// ─────────────────────────────────────────────────────────────────────────────

/// The top-level policy configuration for an Operon session.
///
/// Loaded once at startup and passed to `PolicyResolver::new()`.
/// The resolver holds an immutable reference to this config for the
/// lifetime of the session.
///
/// # TODO: migrate to operon-config
///
/// This type currently lives in `operon-policy` as a temporary measure.
/// Once `operon-config` is built (the dedicated config management crate),
/// `PolicyConfig` will be defined and loaded there, and this module will
/// simply re-export it. The shape will not change — only the crate boundary.
///
/// # Validation
///
/// Call `validate()` after loading to canonicalize all directory paths.
/// The resolver assumes all paths in `directories` are already canonical.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Permissions for global (non-filesystem) tools.
    #[serde(default)]
    pub global: GlobalPolicy,

    /// Permissions for directory-scoped tools, one entry per allowed directory.
    ///
    /// Order matters: the resolver checks directories in order and returns
    /// the first match. Put more specific paths before broader ones if needed.
    #[serde(default)]
    pub directories: Vec<DirectoryPolicy>,
}

impl PolicyConfig {
    /// Creates an empty PolicyConfig — all tools denied for all roles everywhere.
    ///
    /// This is the safest starting state. The owner adds directories and
    /// grants permissions explicitly.
    pub fn empty() -> Self {
        Self {
            global: GlobalPolicy::default(),
            directories: Vec::new(),
        }
    }

    /// Canonicalizes all directory paths in the config.
    ///
    /// Must be called after loading the config from disk before passing it to
    /// `PolicyResolver::new()`. The resolver compares paths using canonical forms,
    /// so un-canonicalized paths (containing `..`, symlinks, or relative segments)
    /// will not match correctly.
    ///
    /// # Errors
    ///
    /// Returns `PolicyError::PathCanonicalization` for any path that cannot be
    /// resolved (does not exist, permission denied, invalid UTF-8 on Windows).
    ///
    /// # Side effects
    ///
    /// Mutates all `DirectoryPolicy.path` fields in `self.directories` in place.
    pub fn validate(&mut self) -> Result<(), PolicyError> {
        for dir_policy in &mut self.directories {
            let canonical = std::fs::canonicalize(&dir_policy.path).map_err(|e| {
                PolicyError::PathCanonicalization {
                    path: dir_policy.path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
            dir_policy.path = canonical;
        }
        Ok(())
    }

    /// Returns true if the given absolute path is within any registered directory.
    ///
    /// Convenience method used internally by the path guard. Does NOT canonicalize
    /// the input path — callers must ensure it is already canonical before calling.
    pub fn any_directory_covers(&self, canonical_path: &std::path::Path) -> bool {
        self.directories
            .iter()
            .any(|d| canonical_path.starts_with(&d.path))
    }
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self::empty()
    }
}
