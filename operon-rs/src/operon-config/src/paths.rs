// paths.rs — Platform-specific path resolution for Operon's data directory.
//
// Everything Operon stores on disk lives under ~/.operon/ (or the OS equivalent):
//
//   ~/.operon/
//   ├── config.toml     ← main config file (created with defaults on first run)
//   ├── workspace/      ← Direction 1: default agent workspace, always accessible
//   │   └── AGENTS.md   ← global agent instructions (written by the owner)
//   └── sessions/       ← session SQLite databases, one per session ID
//       └── <id>.db
//
// On Windows, ~ resolves to %USERPROFILE% (C:\Users\<username>\).
// On macOS, ~ is /Users/<username>/.
// On Linux, ~ is /home/<username>/.
//
// `OperonPaths::resolve()` uses the `dirs` crate to find the home directory
// cross-platform, then derives all other paths from it. This is the ONLY place
// in the codebase that knows where these files live.

use std::fs;
use std::path::PathBuf;

use crate::error::ConfigError;

// ─────────────────────────────────────────────────────────────────────────────
// OperonPaths
// ─────────────────────────────────────────────────────────────────────────────

/// All filesystem paths Operon uses at runtime.
///
/// Constructed once via [`OperonPaths::resolve()`] and shared with the session
/// runner, snapshot builder, and persistence layer.
///
/// # Immutability
///
/// Once constructed, these paths do not change for the lifetime of the process.
/// There is no need to refresh them — they're derived from the home directory
/// which is fixed per process.
#[derive(Debug, Clone)]
pub struct OperonPaths {
    /// `~/.operon/` — the root data directory for all Operon state.
    ///
    /// Created by [`ensure_dirs_exist()`] if absent. Never removed by Operon.
    pub config_dir: PathBuf,

    /// `~/.operon/workspace/` — Direction 1: the default agent workspace.
    ///
    /// Always accessible to the agent. The owner cannot remove it from the
    /// allowed directory list. AGENTS.md in this directory carries global
    /// instructions loaded when Operon is opened in normal (non-project) mode.
    pub workspace_dir: PathBuf,

    /// `~/.operon/config.toml` — the main configuration file.
    ///
    /// Parsed by `operon-config::load()`. Created with sane defaults by
    /// [`write_default_config()`] if it does not exist.
    pub config_file: PathBuf,

    /// `~/.operon/sessions/` — directory containing per-session SQLite databases.
    ///
    /// Each session creates one file: `~/.operon/sessions/<session_id>.db`.
    /// The session runner is responsible for cleaning up old databases.
    pub sessions_dir: PathBuf,
}

impl OperonPaths {
    /// Resolves all paths from the current user's home directory.
    ///
    /// Does NOT create any directories — call [`ensure_dirs_exist()`] separately.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::NoHomeDir`] if the home directory cannot be
    /// determined (headless containers, unusual environments).
    pub fn resolve() -> Result<Self, ConfigError> {
        // dirs::home_dir() handles Windows (%USERPROFILE%), macOS, and Linux (HOME).
        let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;

        // All Operon state lives in ~/.operon/ — one directory, easy to back up / nuke.
        let config_dir = home.join(".operon");

        Ok(Self {
            workspace_dir: config_dir.join("workspace"),
            config_file: config_dir.join("config.toml"),
            sessions_dir: config_dir.join("sessions"),
            config_dir,
        })
    }

    /// Creates `~/.operon/`, `~/.operon/workspace/`, and `~/.operon/sessions/`
    /// if they do not already exist.
    ///
    /// This is idempotent — calling it repeatedly has no effect.
    /// Called once during [`crate::load()`] before reading the config file.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Io`] if the OS refuses to create the directories
    /// (permission denied, read-only filesystem, etc.).
    pub fn ensure_dirs_exist(&self) -> Result<(), ConfigError> {
        // create_dir_all is a no-op if the directory already exists.
        fs::create_dir_all(&self.workspace_dir)?;
        fs::create_dir_all(&self.sessions_dir)?;
        Ok(())
    }

    /// Returns the path to the session JSON file for a given session ID.
    ///
    /// Example: `~/.operon/sessions/abc123def456.json`
    ///
    /// Hey buddy! Instead of using a binary SQLite database, we now store the whole
    /// conversation and session metadata in a single JSON file. This function just
    /// figures out the absolute file path where we'll save that JSON file.
    pub fn session_db(&self, session_id: &str) -> PathBuf {
        // We append the `.json` extension to our session ID and join it with the sessions directory path.
        self.sessions_dir.join(format!("{session_id}.json"))
    }

    /// Returns the path to the AGENTS.md in the default workspace.
    ///
    /// This file carries global instructions loaded by the snapshot builder
    /// when Operon is opened in normal (non-project) mode.
    ///
    /// The file may or may not exist — the snapshot builder handles absence gracefully.
    pub fn workspace_agents_md(&self) -> PathBuf {
        self.workspace_dir.join("AGENTS.md")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_returns_paths_under_home() {
        // Can only test that resolve() succeeds and returns sensible paths.
        // We can't test the exact path since it varies per machine.
        let paths = OperonPaths::resolve().expect("resolve should succeed in test environment");

        assert!(paths.config_dir.ends_with(".operon"));
        assert!(paths.workspace_dir.ends_with("workspace"));
        assert!(paths.config_file.file_name().unwrap() == "config.toml");
        assert!(paths.sessions_dir.ends_with("sessions"));
    }

    #[test]
    fn test_session_db_includes_session_id() {
        // Let's verify that our helper function creates a path ending with .json
        // and containing our specific session ID!
        let paths = OperonPaths::resolve().unwrap();
        let db = paths.session_db("abc123");
        assert!(db.to_string_lossy().contains("abc123"));
        assert!(db.extension().unwrap() == "json");
    }

    #[test]
    fn test_workspace_agents_md_path() {
        let paths = OperonPaths::resolve().unwrap();
        let agents = paths.workspace_agents_md();
        assert!(agents.file_name().unwrap() == "AGENTS.md");
        assert!(agents.parent().unwrap().ends_with("workspace"));
    }

    #[test]
    fn test_ensure_dirs_exist_is_idempotent() {
        // Calling ensure_dirs_exist() twice must not error.
        // Use a temp dir to avoid touching the real ~/.operon.
        let tmp = tempfile::tempdir().unwrap();
        let fake_paths = OperonPaths {
            config_dir: tmp.path().join(".operon"),
            workspace_dir: tmp.path().join(".operon").join("workspace"),
            config_file: tmp.path().join(".operon").join("config.toml"),
            sessions_dir: tmp.path().join(".operon").join("sessions"),
        };

        fake_paths
            .ensure_dirs_exist()
            .expect("first call should succeed");
        fake_paths
            .ensure_dirs_exist()
            .expect("second call should be idempotent");

        assert!(fake_paths.workspace_dir.is_dir());
        assert!(fake_paths.sessions_dir.is_dir());
    }
}
