// config.rs — Session configuration for operon-session.
//
// `SessionConfig` is the single struct that carries everything `SessionRunner::new()`
// needs to initialize the full agent loop. It is constructed by the frontend
// (TUI or GUI) by assembling values from `operon_config::AppConfig`.
//
// ── Three-directional directory model ────────────────────────────────────────
//
// Operon uses three kinds of directories:
//
//   Direction 1 — Default workspace: ~/.operon/workspace/
//     Always accessible to the agent. Injected into the policy by operon-config.
//     Used as the snapshot root in NORMAL mode (no project open).
//     AGENTS.md, tree, and git block come from this directory.
//
//   Direction 2 — Allowed directories: listed in config.toml [[directories]]
//     Loaded once at startup via operon_config::load(). The agent can use
//     filesystem and shell tools inside these paths according to PolicyConfig.
//
//   Direction 3 — Project directory: opened VS Code-style by the user
//     Passed via `project_dir: Some(path)`.
//     The snapshot root switches to the project dir (PROJECT mode) — AGENTS.md,
//     directory tree, and git status are read from the project directory.
//     Policy is NOT modified at runtime. The project directory must already exist
//     in config.toml as a normal allowed directory (Direction 2).
//     In normal (non-project) sessions, that directory remains accessible to the
//     agent as any other allowed directory — only the snapshot root differs.
//
// ── Snapshot root selection ───────────────────────────────────────────────────
//
//   workspace_root is the snapshot root — the single directory from which:
//     - AGENTS.md is read
//     - The directory tree block is rendered
//     - Git status is read
//
//   NORMAL mode  → workspace_root = ~/.operon/workspace/ (from paths.workspace_dir)
//   PROJECT mode → workspace_root = project_dir.clone()
//
//   The caller (TUI/GUI) sets workspace_root before constructing SessionConfig.
//   SessionRunner::new() just uses it as-is.

use std::path::PathBuf;

use operon_config::PolicyConfig;
use operon_context::{CompactionConfig, Role, SnapshotConfig};
use operon_providers::ProviderConfig;

// ─────────────────────────────────────────────────────────────────────────────
// SessionConfig
// ─────────────────────────────────────────────────────────────────────────────

/// All runtime parameters required to create and run a `SessionRunner`.
///
/// Construct once at startup from `operon_config::AppConfig` and pass into
/// [`crate::runner::SessionRunner::new`]. This struct is consumed (moved) by
/// the runner — it is not shared across tasks.
///
/// # Construction pattern (caller side)
///
/// ```ignore
/// use operon_config::load;
/// use operon_session::SessionConfig;
///
/// let app = load()?;
/// let config = SessionConfig {
///     provider_config: app.provider,
///     policy:          app.policy,
///     project_dir:     None,                          // or Some(PathBuf::from("/my/project"))
///     workspace_root:  app.paths.workspace_dir.clone(), // or project_dir.clone()
///     role:            Role::Owner,
///     tool_groups:     SessionConfig::default_tool_groups(),
///     compaction:      CompactionConfig::default(),
///     store_path:      Some(app.paths.session_db("my-session-id")),
/// };
/// ```
pub struct SessionConfig {
    // ── Provider + model ──────────────────────────────────────────────────────
    /// Fully assembled provider configuration: which provider, model, and API key.
    ///
    /// Contains `provider` (enum), `credentials` (API key + optional org_id),
    /// `model` (model_id, context_window, max_tokens), and an optional base URL
    /// override (useful for Ollama running on a non-default port).
    ///
    /// Source: `operon_config::AppConfig.provider`
    pub provider_config: ProviderConfig,

    // ── Permission policy ─────────────────────────────────────────────────────
    /// Resolved tool permission policy for this session.
    ///
    /// Carries global tool permissions (web, subagent, ask, todo, load_tools)
    /// and per-directory permissions (filesystem + shell) for all allowed directories
    /// (Direction 1 + 2).
    ///
    /// All directory paths in this config must already be canonical (ensured by
    /// `operon_config::load()` which calls `PolicyConfig::validate()`).
    ///
    /// Source: `operon_config::AppConfig.policy`
    pub policy: PolicyConfig,

    // ── Directory model ───────────────────────────────────────────────────────
    /// Optional project directory opened VS Code-style.
    ///
    /// `None`  → NORMAL mode: snapshot root = `workspace_root` (~/.operon/workspace/).
    /// `Some(path)` → PROJECT mode: snapshot root = `workspace_root` = `path`.
    ///
    /// When `Some`, the runner uses this path as the snapshot root only. No policy
    /// injection occurs at runtime — the project directory is expected to already be
    /// present in `config.toml` as a normal allowed directory with user-configured
    /// permissions. Use `operon_config::add_allowed_directory()` before starting the
    /// session if the directory is not yet in `config.toml`.
    pub project_dir: Option<PathBuf>,

    /// The snapshot root directory — the single directory from which AGENTS.md,
    /// the directory tree block, and git status are read.
    ///
    /// NORMAL mode:  set to `app.paths.workspace_dir` (~/.operon/workspace/).
    /// PROJECT mode: set to `project_dir.clone()` (same as the project directory).
    ///
    /// The `SnapshotBuilder` watches this directory for filesystem changes
    /// (to invalidate cached tree + AGENTS.md).
    pub workspace_root: PathBuf,

    // ── Agent identity ────────────────────────────────────────────────────────
    /// Agent role for this session — determines which policy column is consulted.
    ///
    /// `Role::Owner` for local (terminal, TUI, GUI) sessions.
    /// `Role::External` for remote (WhatsApp, Telegram, public channels) sessions.
    ///
    /// This is set at the channel level, not the user level. A message arriving
    /// over a public channel is External even if the sender is the system owner.
    pub role: Role,

    // ── Tool groups ───────────────────────────────────────────────────────────
    /// Names of tool groups to register on the `Dispatcher` at startup.
    ///
    /// Valid values: `"fs"`, `"shell"`, `"web"`, `"todo"`, `"ask"`, `"memory"`.
    /// Unknown names are logged as warnings and skipped.
    /// This list is also passed to `SnapshotBuilder` so the system prompt
    /// includes an accurate tool availability block.
    pub tool_groups: Vec<String>,

    // ── Context compaction ────────────────────────────────────────────────────
    /// Compaction settings — threshold percentage and preserved turn count.
    ///
    /// `CompactionConfig::default()` gives a sensible 90% threshold with 2 preserved turns.
    pub compaction: CompactionConfig,

    // ── Session persistence ───────────────────────────────────────────────────
    /// Path to the SQLite database file for turn persistence.
    ///
    /// `None` → persistence disabled (turns are in-memory only).
    /// `Some(path)` → the file is created if it does not already exist.
    ///
    /// Use `app.paths.session_db(session_id)` to compute the standard path
    /// under `~/.operon/sessions/`.
    pub store_path: Option<PathBuf>,

    // ── Channel Instructions ──────────────────────────────────────────────────
    /// Optional in-memory channel-specific role instructions (e.g. WhatsApp Owner/External).
    /// Populated per-turn by channel bridges (WhatsApp, Telegram) before constructing `SessionConfig`.
    /// Preserved as `None` for local GUI/TUI sessions.
    pub channel_instructions: Option<String>,
}

impl SessionConfig {
    /// Returns the standard set of built-in tool groups for an interactive session.
    ///
    /// Includes: `"fs"`, `"shell"`, `"web"`, `"todo"`, `"ask"`, `"memory"`.
    pub fn default_tool_groups() -> Vec<String> {
        vec![
            "fs".into(),
            "shell".into(),
            "web".into(),
            "todo".into(),
            "ask".into(),
            "memory".into(),
        ]
    }

    /// Derive a [`SnapshotConfig`] for this session.
    ///
    /// Called once in `SessionRunner::new()` after the session ID is generated.
    /// The `workspace_root` and `tool_groups` are cloned (both are cheap).
    pub fn snapshot_config(&self, session_id: &str) -> SnapshotConfig {
        SnapshotConfig {
            root: self.workspace_root.clone(),
            role: self.role,
            session_id: session_id.to_string(),
            // One-level tree traversal gives the agent enough context without
            // flooding the system prompt with deeply nested paths.
            tree_depth: 1,
            tool_groups: self.tool_groups.clone(),
            channel_instructions: self.channel_instructions.clone(),
        }
    }
}
