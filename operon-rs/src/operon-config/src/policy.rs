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
    Memory,
    #[serde(other)]
    Unknown,
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
    Glob,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
            let canonical = std::fs::canonicalize(&dir_policy.path).map_err(|e| {
                PolicyError::PathCanonicalization {
                    path: dir_policy.path.display().to_string(),
                    reason: e.to_string(),
                }
            })?;
            // Hey friend! We strip any Windows extended-length verbatim prefix (`\\?\`) so all directory
            // paths stored in DirectoryPolicy are clean, standard DOS or UNC paths!
            dir_policy.path = clean_verbatim_path(canonical);
        }
        Ok(())
    }

    pub fn any_directory_covers(&self, canonical_path: &std::path::Path) -> bool {
        self.directories
            .iter()
            .any(|d| canonical_path.starts_with(&d.path))
    }
}

/// Strips the Windows verbatim/extended-length prefix (`\\?\` and `\\?\UNC\`) from a path.
pub fn clean_verbatim_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let path_str = path.to_string_lossy();
        if let Some(stripped) = path_str.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{}", stripped))
        } else if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            path
        }
    }
    #[cfg(not(windows))]
    {
        path
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

    #[test]
    fn test_clean_verbatim_path() {
        let p = PathBuf::from(r"\\?\D:\test\path");
        let cleaned = clean_verbatim_path(p);
        #[cfg(windows)]
        assert_eq!(cleaned, PathBuf::from(r"D:\test\path"));
    }
}
