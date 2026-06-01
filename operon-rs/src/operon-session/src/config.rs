// config.rs — Session configuration for operon-session.
//
// TODO: Migrate to operon-config when that crate exists.
// For now this is a self-contained stub holding all runtime parameters needed
// to spin up a SessionRunner.
//
// Design note: SnapshotConfig and CompactionConfig are derived *from* this
// struct rather than composed inside it, to keep a clean separation between
// the session-level concept and its subsystem sub-configurations.

use std::path::PathBuf;

use operon_context_compaction::CompactionConfig;
use operon_context_snapshot::{Role, SnapshotConfig};
use operon_context_normalize_tools::Provider;

// ─────────────────────────────────────────────────────────────────────────────
// SessionConfig
// ─────────────────────────────────────────────────────────────────────────────

/// All runtime parameters required to create and run a `SessionRunner`.
///
/// Construct once at startup and pass into [`crate::runner::SessionRunner::new`].
/// This type is not `Clone` by default because `PathBuf` is cheap to clone and
/// the struct is typically moved into the runner anyway.
pub struct SessionConfig {
    /// LLM provider to use for this session (selects wire format + endpoint).
    pub provider: Provider,

    /// API key for the provider. Passed verbatim in request headers.
    /// Keep this out of logs — use `tracing` redaction or avoid logging the config.
    pub api_key: String,

    /// Model identifier string sent in the request body, e.g.
    /// `"claude-sonnet-4-20250514"` for Anthropic or `"gpt-4o"` for OpenAI.
    pub model_id: String,

    /// Context window size for the model in tokens.
    /// Used by `TokenBudget` to compute the compaction threshold limit.
    /// Common values: 200_000 (Claude), 128_000 (GPT-4o).
    pub context_window: usize,

    /// `max_tokens` to request per turn (i.e. the maximum output token budget).
    /// Typical values: 4096–16384. Must be <= the model's output token limit.
    pub max_tokens: usize,

    /// Tool groups to register on the `Dispatcher` at startup.
    /// Valid group names: `"fs"`, `"shell"`, `"web"`, `"todo"`.
    /// Unknown names are logged as warnings and skipped.
    pub tool_groups: Vec<String>,

    /// Compaction configuration — threshold percentage and preserved turn count.
    pub compaction: CompactionConfig,

    /// Workspace root directory passed to `SnapshotBuilder`.
    /// The builder will watch this directory for filesystem changes.
    pub workspace_root: PathBuf,

    /// Agent role used by the snapshot and sanitizer (Owner or External).
    pub role: Role,

    /// Path to the SQLite persistence database file.
    /// If `None`, persistence is disabled and turns are only kept in memory.
    /// The file is created if it does not already exist.
    pub store_path: Option<PathBuf>,
}

impl SessionConfig {
    /// Derive a [`SnapshotConfig`] for this session.
    ///
    /// Called once in `SessionRunner::new` after the session ID is generated.
    /// The config borrows from `self` so the root path and tool_groups lists
    /// are cloned cheaply rather than moved.
    pub fn snapshot_config(&self, session_id: &str) -> SnapshotConfig {
        SnapshotConfig {
            root: self.workspace_root.clone(),
            role: self.role,
            session_id: session_id.to_string(),
            // One-level tree traversal gives the agent enough context without
            // flooding the system prompt with deeply nested paths.
            tree_depth: 1,
            tool_groups: self.tool_groups.clone(),
        }
    }
}
