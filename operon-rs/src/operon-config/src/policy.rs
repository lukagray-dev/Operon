//! Policy data model for Operon config and permission files.
//!
//! This module owns the shared policy types used by `operon-config` and
//! re-exported by `operon-policy` for backwards compatibility.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerRole {
    Owner,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalTool {
    Web,
    SubAgent,
    Ask,
    Todo,
    LoadTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsTool {
    Read,
    Write,
    Edit,
    Append,
    Grep,
    Ls,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirTool {
    Fs(FsTool),
    Bash,
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("failed to canonicalize directory path '{path}': {reason}")]
    PathCanonicalization { path: String, reason: String },

    #[error("invalid policy configuration: {reason}")]
    InvalidConfig { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPolicy {
    #[serde(default)]
    pub owner: HashMap<GlobalTool, PermissionMode>,

    #[serde(default)]
    pub external: HashMap<GlobalTool, PermissionMode>,
}

impl GlobalPolicy {
    pub fn mode_for(&self, tool: GlobalTool, role: CallerRole) -> PermissionMode {
        let map = match role {
            CallerRole::Owner => &self.owner,
            CallerRole::External => &self.external,
        };
        map.get(&tool).copied().unwrap_or(PermissionMode::Deny)
    }
}

impl Default for GlobalPolicy {
    fn default() -> Self {
        Self {
            owner: HashMap::new(),
            external: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryPolicy {
    pub path: PathBuf,

    #[serde(default)]
    pub owner: HashMap<DirTool, PermissionMode>,

    #[serde(default)]
    pub external: HashMap<DirTool, PermissionMode>,
}

impl DirectoryPolicy {
    pub fn mode_for(&self, tool: DirTool, role: CallerRole) -> PermissionMode {
        let map = match role {
            CallerRole::Owner => &self.owner,
            CallerRole::External => &self.external,
        };
        map.get(&tool).copied().unwrap_or(PermissionMode::Deny)
    }

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
            external: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    #[serde(default)]
    pub global: GlobalPolicy,

    #[serde(default)]
    pub directories: Vec<DirectoryPolicy>,
}

impl PolicyConfig {
    pub fn empty() -> Self {
        Self {
            global: GlobalPolicy::default(),
            directories: Vec::new(),
        }
    }

    pub fn validate(&mut self) -> Result<(), PolicyError> {
        for dir_policy in &mut self.directories {
            let mut canonical = std::fs::canonicalize(&dir_policy.path).map_err(|e| {
                PolicyError::PathCanonicalization {
                    path: dir_policy.path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
            // Hey friend! std::fs::canonicalize() on Windows prepends the \\?\ prefix.
            // We strip it here so our allowed directory paths are stored in standard format.
            #[cfg(windows)]
            {
                let s = canonical.to_string_lossy();
                if s.starts_with(r"\\?\") {
                    canonical = PathBuf::from(&s[4..]);
                }
            }
            dir_policy.path = canonical;
        }
        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_default_denies_missing_entry() {
        let policy = GlobalPolicy::default();
        assert_eq!(
            policy.mode_for(GlobalTool::Web, CallerRole::Owner),
            PermissionMode::Deny
        );
    }

    #[test]
    fn directory_owner_full_access_includes_bash() {
        let policy = DirectoryPolicy::owner_full_access(PathBuf::from("/tmp/work"));
        assert_eq!(
            policy.mode_for(DirTool::Bash, CallerRole::Owner),
            PermissionMode::Allow
        );
    }

    #[test]
    fn empty_policy_denies_all() {
        let policy = PolicyConfig::empty();
        assert!(policy.directories.is_empty());
    }
}
