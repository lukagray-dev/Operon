// types.rs — Core types for the operon-policy permission model.
//
// This module defines the full type vocabulary for the policy system:
//
//   CallerRole       — who sent the prompt (Owner or External)
//   PermissionMode   — what the policy says for a given tool (Allow/Ask/Deny)
//   GlobalTool       — tools that have no filesystem path (web, subagent, ask, todo, load)
//   FsTool           — individual filesystem tools (read, write, edit, etc.)
//   DirTool          — tools that are scoped to a directory (Fs variants + Shell/Bash)
//   PolicyDecision   — the output of a policy check (the resolver's answer)
//
// DESIGN NOTE:
//   `PolicyDecision` is what the session runner acts on, not `PermissionMode`.
//   `PermissionMode` is what the config stores. The resolver converts one to the other.
//
// DESIGN NOTE:
//   Tools are NOT imported from operon-tools here. This crate only knows about
//   tool *names* as they appear in ToolCall.name (plain strings). The enums below
//   represent the policy-level groupings — not tool implementations.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// CallerRole
// ─────────────────────────────────────────────────────────────────────────────

/// The role of the entity that sent the current prompt.
///
/// Set once per session at construction time from the channel metadata
/// (e.g. a message from the owner's terminal = Owner; a message arriving
/// over a public WhatsApp number = External).
///
/// The policy resolver uses this to determine which permission column to
/// consult in the config (owner vs. external).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerRole {
    /// The system owner — has access to everything the config grants to owners.
    Owner,
    /// An external user (customer, public, untrusted) — subject to external permissions.
    External,
}

// ─────────────────────────────────────────────────────────────────────────────
// PermissionMode
// ─────────────────────────────────────────────────────────────────────────────

/// The three modes that any tool permission can be set to.
///
/// Stored in `PolicyConfig` and interpreted by `PolicyResolver`.
/// The resolver converts these into `PolicyDecision` values.
///
/// # Semantics
///
/// - `Allow` → the tool call proceeds immediately.
/// - `Ask`   → the session runner pauses and emits a `SessionEvent::ApprovalRequired`.
///             The call proceeds only if the owner confirms.
/// - `Deny`  → the call is rejected. The model receives a generic error ToolResult.
///             The reason for denial is never exposed to the model or the external user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Tool can be used freely for this role in this scope.
    Allow,
    /// Tool requires owner confirmation before executing for this role.
    Ask,
    /// Tool is blocked for this role in this scope, period.
    Deny,
}

// ─────────────────────────────────────────────────────────────────────────────
// GlobalTool
// ─────────────────────────────────────────────────────────────────────────────

/// Tools whose permissions are set globally (not per directory).
///
/// These tools do not touch the filesystem in a path-specific way, so
/// they cannot be directory-scoped. Their permissions apply uniformly
/// regardless of which workspace or directory the session operates in.
///
/// # Mapping to tool names
///
/// | Variant     | ToolCall.name values                          |
/// |-------------|-----------------------------------------------|
/// | `Web`       | `"web_search"`, `"web_fetch"`                 |
/// | `SubAgent`  | `"subagent"`, `"spawn_agent"`                 |
/// | `Ask`       | `"ask"`                                       |
/// | `Todo`      | `"todo_create"`, `"todo_list"`, `"todo_update"`, `"todo_delete"` |
/// | `LoadTools` | `"load_tools"`                                |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlobalTool {
    /// Web search and URL fetch tools.
    Web,
    /// Sub-agent spawning (requires session-layer trait — currently deferred).
    SubAgent,
    /// Ask-the-user clarifying question tool.
    Ask,
    /// Todo list management tools (create, list, update, delete).
    Todo,
    /// The load_tools meta-tool (always available, but still controllable).
    LoadTools,
}

// ─────────────────────────────────────────────────────────────────────────────
// FsTool
// ─────────────────────────────────────────────────────────────────────────────

/// Individual filesystem tool variants.
///
/// Stored inside `DirTool::Fs(FsTool)` to enable per-tool overrides
/// within the filesystem group. When the UI shows "Custom" for a
/// directory's filesystem permissions, it means different `FsTool`
/// variants have different `PermissionMode` values.
///
/// # Mapping to tool names
///
/// | Variant   | ToolCall.name | path arg key |
/// |-----------|---------------|--------------|
/// | `Read`    | `"read"`      | `"paths"` (array — first element used for policy) |
/// | `Write`   | `"write"`     | `"path"`     |
/// | `Edit`    | `"edit"`      | `"path"`     |
/// | `Append`  | `"append"`    | `"path"`     |
/// | `Grep`    | `"grep"`      | `"path"`     |
/// | `Ls`      | `"ls"`        | `"path"`     |
/// | `Delete`  | `"delete"`    | `"path"`     |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsTool {
    /// Read one or more files (path arg: `"paths"` array).
    Read,
    /// Write/overwrite a file (path arg: `"path"`).
    Write,
    /// Edit a file with string replacements (path arg: `"path"`).
    Edit,
    /// Append content to a file (path arg: `"path"`).
    Append,
    /// Search file contents with regex (path arg: `"path"`).
    Grep,
    /// List directory contents (path arg: `"path"`).
    Ls,
    /// Delete a file or directory (path arg: `"path"`).
    Delete,
}

// ─────────────────────────────────────────────────────────────────────────────
// DirTool
// ─────────────────────────────────────────────────────────────────────────────

/// Tools that are directory-scoped — their permissions are evaluated per directory.
///
/// Every `DirTool` call must carry a path argument that the resolver uses to
/// determine which `DirectoryPolicy` applies. If the path falls outside all
/// registered directories, the call is denied unconditionally.
///
/// # Bash (Shell)
///
/// The `Bash` tool uses a required `cwd` argument (not `path`). This was a
/// deliberate design choice (Option C) to force the model to declare the
/// working directory explicitly, making the policy anchor unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirTool {
    /// A specific filesystem tool. Permissions can be set per individual tool.
    Fs(FsTool),
    /// The bash shell tool. Uses `cwd` as the policy path anchor.
    Bash,
}

// ─────────────────────────────────────────────────────────────────────────────
// PolicyDecision
// ─────────────────────────────────────────────────────────────────────────────

/// The output of a `PolicyResolver::check()` call.
///
/// The session runner matches on this value to decide what to do with the tool call:
/// - `Allow`  → pass the call to `Dispatcher::dispatch()` as normal.
/// - `Ask`    → emit `SessionEvent::ApprovalRequired` and pause the loop.
/// - `Deny`   → synthesize an error `ToolResult` and skip the dispatcher entirely.
///
/// # Deny message opacity
///
/// The `Deny` variant's `reason` field is for internal logging only.
/// The session runner MUST NOT forward this reason to the model or to the
/// external user. The tool result returned to the model must always use a
/// generic, information-free message like `"Tool not available."` to prevent
/// directory enumeration attacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// The call is permitted. Proceed to the dispatcher.
    Allow,

    /// The call requires owner confirmation before proceeding.
    ///
    /// `reason` describes why confirmation is needed (e.g. "External caller
    /// requesting file write in ~/work/client-project"). Shown to the owner
    /// in the confirmation UI — NOT forwarded to the external user.
    Ask { reason: String },

    /// The call is blocked. Do not proceed to the dispatcher.
    ///
    /// `reason` is an internal diagnostic string for logging only.
    /// NEVER send this reason to the model or external user.
    Deny { reason: String },
}

impl PolicyDecision {
    /// Returns true if this decision allows the tool call to proceed.
    pub fn is_allow(&self) -> bool {
        matches!(self, PolicyDecision::Allow)
    }

    /// Returns true if this decision blocks the tool call entirely.
    pub fn is_deny(&self) -> bool {
        matches!(self, PolicyDecision::Deny { .. })
    }

    /// Returns true if this decision requires owner confirmation.
    pub fn is_ask(&self) -> bool {
        matches!(self, PolicyDecision::Ask { .. })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ToolScope
// ─────────────────────────────────────────────────────────────────────────────

/// The scope category of a tool — determines which part of PolicyConfig to consult.
///
/// Returned by `classify_tool()` in the resolver. Internal to the resolution process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToolScope {
    /// A global tool — check `PolicyConfig.global`.
    Global(GlobalTool),
    /// A directory-scoped tool — check the matching `DirectoryPolicy`.
    Dir(DirTool),
}
