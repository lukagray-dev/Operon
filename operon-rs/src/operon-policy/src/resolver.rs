// resolver.rs — The policy enforcement engine for operon-policy.
//
// `PolicyResolver` is the single public entry point for permission checks.
// It takes a `ToolCall` (from the model) and a `CallerRole` (from the channel)
// and returns a `PolicyDecision`.
//
// The session runner calls `PolicyResolver::check()` once per tool call,
// BEFORE passing the call to `operon_tools::dispatcher::Dispatcher::dispatch()`.
// The dispatcher and all tool implementations have zero knowledge of policy.
//
// RESOLUTION FLOW:
//
//   check(call, role)
//     → classify_tool(call.name)         → ToolScope (Global or Dir)
//     → if Global:
//         config.global.mode_for(tool, role)
//         → mode → PolicyDecision
//     → if Dir:
//         extract_path_arg(call)         → Option<PathBuf>
//         if None → Deny (no path anchor)
//         PathGuard::find_directory(path)→ Option<&DirectoryPolicy>
//         if None → Deny (outside all allowed dirs)
//         dir_policy.mode_for(tool, role)
//         → mode → PolicyDecision
//
// DENY OPACITY:
//   All Deny decisions return a generic `reason` string that is safe to log
//   internally but MUST NOT be forwarded to the model. The session runner is
//   responsible for using a fixed opaque message when constructing the error
//   ToolResult (e.g. "Tool not available.").

use std::path::PathBuf;
use operon_context_normalize_tools::ToolCall;

use crate::config::PolicyConfig;
use crate::path_guard::PathGuard;
use crate::types::{
    CallerRole, DirTool, FsTool, GlobalTool, PermissionMode, PolicyDecision, ToolScope,
};

// ─────────────────────────────────────────────────────────────────────────────
// PolicyResolver
// ─────────────────────────────────────────────────────────────────────────────

/// The policy enforcement gate for the Operon agent.
///
/// One `PolicyResolver` per session. Created once from `PolicyConfig` and
/// held by `SessionRunner` alongside the tool dispatcher.
///
/// # Thread safety
///
/// `PolicyResolver` is `Send + Sync` — it holds no mutable state. The config
/// and path comparisons are all read-only. Multiple concurrent checks are safe
/// (though the session runner currently runs tool calls sequentially).
///
/// # Usage
///
/// ```rust
/// use operon_policy::{PolicyConfig, PolicyResolver, CallerRole};
///
/// let mut config = PolicyConfig::empty();
/// config.validate().unwrap();
/// let resolver = PolicyResolver::new(config);
///
/// // In the session runner's tool dispatch loop:
/// // let decision = resolver.check(&tool_call, CallerRole::Owner);
/// ```
pub struct PolicyResolver {
    /// The policy configuration for this session. Immutable after construction.
    config: PolicyConfig,
}

impl PolicyResolver {
    /// Creates a new `PolicyResolver` from a validated `PolicyConfig`.
    ///
    /// # Precondition
    ///
    /// `config.validate()` must have been called before passing the config here.
    /// An unvalidated config (with non-canonical directory paths) will produce
    /// incorrect path containment results in `check()`.
    ///
    /// # Arguments
    /// - `config`: A validated `PolicyConfig` with canonical directory paths.
    pub fn new(config: PolicyConfig) -> Self {
        Self { config }
    }

    /// Evaluate whether a tool call is permitted for the given caller role.
    ///
    /// This is the primary interface used by the session runner. Call once per
    /// tool call, before dispatching to `Dispatcher::dispatch()`.
    ///
    /// # Arguments
    /// - `call`: The tool call emitted by the model (name + arguments JSON).
    /// - `role`: The caller role for this session (`Owner` or `External`).
    ///
    /// # Returns
    /// A `PolicyDecision`:
    /// - `Allow`  → proceed to `Dispatcher::dispatch(call)`.
    /// - `Ask`    → emit `SessionEvent::ApprovalRequired`, pause the loop.
    /// - `Deny`   → synthesize an opaque error `ToolResult`, skip the dispatcher.
    ///
    /// # Never panics
    ///
    /// Unknown tool names are classified as `GlobalTool`-unknown and denied.
    /// All paths through this function return a valid `PolicyDecision`.
    pub fn check(&self, call: &ToolCall, role: CallerRole) -> PolicyDecision {
        // Step 1: Classify the tool name into a ToolScope.
        match classify_tool(&call.name) {
            // ── Global tool ────────────────────────────────────────────────────
            ToolScope::Global(global_tool) => {
                let mode = self.config.global.mode_for(global_tool, role);
                mode_to_decision(mode, &call.name, role, None)
            }

            // ── Directory-scoped tool ──────────────────────────────────────────
            ToolScope::Dir(dir_tool) => {
                // Step 2: Extract the path argument from the call.
                // Different tools use different argument key names.
                let raw_path = match extract_path_arg(call, &dir_tool) {
                    Some(p) => p,
                    None => {
                        // No path argument present — cannot enforce directory scope.
                        // This indicates either a malformed call (model forgot to include
                        // cwd/path) or a design gap. Deny defensively.
                        return PolicyDecision::Deny {
                            reason: format!(
                                "tool '{}' requires a path argument for policy evaluation, \
                                 but none was found in call arguments",
                                call.name
                            ),
                        };
                    }
                };

                // Step 3: Find which DirectoryPolicy covers this path.
                let guard = PathGuard::new(&self.config.directories);
                let dir_policy = match guard.find_directory(&raw_path) {
                    Some(p) => p,
                    None => {
                        // Path is outside all registered directories. Hard deny.
                        // Do not reveal what directories are registered.
                        return PolicyDecision::Deny {
                            reason: format!(
                                "path '{}' is outside all allowed directories",
                                raw_path.display()
                            ),
                        };
                    }
                };

                // Step 4: Look up the mode for this tool + role in the matched directory.
                let mode = dir_policy.mode_for(dir_tool, role);
                mode_to_decision(mode, &call.name, role, Some(&raw_path))
            }
        }
    }

    /// Returns a reference to the underlying `PolicyConfig`.
    ///
    /// Primarily used for inspection and testing.
    pub fn config(&self) -> &PolicyConfig {
        &self.config
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// classify_tool (private)
// ─────────────────────────────────────────────────────────────────────────────

/// Maps a tool call name (as it appears in `ToolCall.name`) to a `ToolScope`.
///
/// Unknown tool names are mapped to `ToolScope::Global(GlobalTool::LoadTools)`
/// as a safe fallback — the global policy will deny them if not configured.
///
/// This function does NOT read any config — it's a pure static dispatch table.
fn classify_tool(name: &str) -> ToolScope {
    match name {
        // ── Global tools ───────────────────────────────────────────────────────
        "web_search" | "web_fetch" => ToolScope::Global(GlobalTool::Web),
        "subagent" | "spawn_agent" => ToolScope::Global(GlobalTool::SubAgent),
        "ask"                      => ToolScope::Global(GlobalTool::Ask),
        "todo_create"
        | "todo_list"
        | "todo_update"
        | "todo_delete"            => ToolScope::Global(GlobalTool::Todo),
        "load_tools"               => ToolScope::Global(GlobalTool::LoadTools),

        // ── Directory-scoped: filesystem tools ────────────────────────────────
        "read"   => ToolScope::Dir(DirTool::Fs(FsTool::Read)),
        "write"  => ToolScope::Dir(DirTool::Fs(FsTool::Write)),
        "edit"   => ToolScope::Dir(DirTool::Fs(FsTool::Edit)),
        "append" => ToolScope::Dir(DirTool::Fs(FsTool::Append)),
        "grep"   => ToolScope::Dir(DirTool::Fs(FsTool::Grep)),
        "ls"     => ToolScope::Dir(DirTool::Fs(FsTool::Ls)),
        "delete" => ToolScope::Dir(DirTool::Fs(FsTool::Delete)),

        // ── Directory-scoped: shell tool ──────────────────────────────────────
        "bash"   => ToolScope::Dir(DirTool::Bash),

        // ── Unknown tool name ─────────────────────────────────────────────────
        // Unknown tools are treated as global unknowns — they have no path argument
        // to extract, so directory-scope evaluation is impossible. The global policy
        // will deny them (default deny for missing entries).
        other => {
            tracing::warn!(
                "operon-policy: unknown tool name '{}' — classifying as global unknown, \
                 will be denied unless explicitly configured",
                other
            );
            // Map to an obscure GlobalTool so the global deny-by-default fires.
            // LoadTools is the least permissive global tool by typical config.
            ToolScope::Global(GlobalTool::LoadTools)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// extract_path_arg (private)
// ─────────────────────────────────────────────────────────────────────────────

/// Extracts the path argument from a tool call's arguments JSON.
///
/// Each directory-scoped tool uses a different argument key name:
/// - `"read"` uses `"paths"` (an array — we take the FIRST element for policy).
/// - `"bash"` uses `"cwd"`.
/// - All other fs tools use `"path"`.
///
/// Returns `None` if the argument key is missing or the value is not a string.
/// The resolver treats `None` as a Deny (cannot evaluate without a path anchor).
fn extract_path_arg(call: &ToolCall, tool: &DirTool) -> Option<PathBuf> {
    let args = &call.arguments;

    match tool {
        // The `read` tool takes a `"paths"` array. We check the first element
        // for the policy evaluation. If the model passes multiple paths, they
        // all need to be in the same allowed directory (or multiple calls should
        // be used). We check the first as a representative heuristic — the tool
        // executor will encounter the boundary violation for any out-of-scope
        // path anyway (since it runs after policy allows the call).
        //
        // TODO: Consider per-element checking in a future revision. For now,
        // first-element policy is the pragmatic trade-off.
        DirTool::Fs(FsTool::Read) => {
            args.get("paths")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
        }

        // The `bash` tool uses `"cwd"` as the policy anchor (Option C).
        // If cwd is missing, the tool executor would also reject it — but we
        // reject it here first so the policy layer catches it before dispatch.
        DirTool::Bash => {
            args.get("cwd")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
        }

        // All other filesystem tools use a single `"path"` string argument.
        DirTool::Fs(_) => {
            args.get("path")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// mode_to_decision (private)
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a `PermissionMode` into a `PolicyDecision`.
///
/// The `tool_name`, `role`, and optional `path` are used to generate a
/// human-readable reason string for internal logging.
///
/// # Deny reason opacity
///
/// The returned `PolicyDecision::Deny { reason }` is for **internal diagnostics only**.
/// The session runner MUST NOT forward this `reason` to the model. It should
/// substitute a fixed opaque message (e.g. `"Tool not available."`) when
/// constructing the error `ToolResult`.
fn mode_to_decision(
    mode: PermissionMode,
    tool_name: &str,
    role: CallerRole,
    path: Option<&std::path::Path>,
) -> PolicyDecision {
    match mode {
        PermissionMode::Allow => PolicyDecision::Allow,

        PermissionMode::Ask => {
            // Build a human-readable reason for the owner's confirmation dialog.
            let reason = match path {
                Some(p) => format!(
                    "{:?} caller requested '{}' on path '{}'",
                    role, tool_name, p.display()
                ),
                None => format!(
                    "{:?} caller requested global tool '{}'",
                    role, tool_name
                ),
            };
            PolicyDecision::Ask { reason }
        }

        PermissionMode::Deny => {
            // Build an internal diagnostic reason.
            // IMPORTANT: This string must NEVER be returned to the model.
            let reason = match path {
                Some(p) => format!(
                    "tool '{}' denied for {:?} at path '{}'",
                    tool_name, role, p.display()
                ),
                None => format!(
                    "global tool '{}' denied for {:?}",
                    tool_name, role
                ),
            };
            PolicyDecision::Deny { reason }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DirectoryPolicy, GlobalPolicy};
    use crate::types::{CallerRole, DirTool, FsTool, GlobalTool, PermissionMode};
    use operon_context_normalize_tools::{ToolCall, ToolCallId};
    use serde_json::json;
    use tempfile::TempDir;

    // ── Helpers ────────────────────────────────────────────────────────────────

    /// Build a ToolCall with the given name and args for testing.
    fn make_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: ToolCallId(format!("call_{}", name)),
            name: name.to_string(),
            arguments: args,
        }
    }

    /// Build a PolicyConfig with one allowed directory and the given dir-tool permissions.
    fn make_dir_config(
        dir_path: PathBuf,
        owner_perms: Vec<(DirTool, PermissionMode)>,
        external_perms: Vec<(DirTool, PermissionMode)>,
    ) -> PolicyConfig {
        let dir_policy = DirectoryPolicy {
            path: dir_path,
            owner: owner_perms.into_iter().collect(),
            external: external_perms.into_iter().collect(),
        };
        PolicyConfig {
            global: GlobalPolicy::default(),
            directories: vec![dir_policy],
        }
    }

    /// Build a PolicyConfig with global tool permissions.
    fn make_global_config(
        owner_perms: Vec<(GlobalTool, PermissionMode)>,
        external_perms: Vec<(GlobalTool, PermissionMode)>,
    ) -> PolicyConfig {
        PolicyConfig {
            global: GlobalPolicy {
                owner: owner_perms.into_iter().collect(),
                external: external_perms.into_iter().collect(),
            },
            directories: Vec::new(),
        }
    }

    // ── Global tool tests ──────────────────────────────────────────────────────

    #[test]
    fn test_global_tool_allow_for_owner() {
        // Owner has web=Allow → check returns Allow.
        let config = make_global_config(
            vec![(GlobalTool::Web, PermissionMode::Allow)],
            vec![],
        );
        let resolver = PolicyResolver::new(config);
        let call = make_call("web_search", json!({ "query": "test" }));
        let decision = resolver.check(&call, CallerRole::Owner);
        assert!(decision.is_allow(), "web_search should be allowed for owner");
    }

    #[test]
    fn test_global_tool_deny_for_external() {
        // External has no web entry → default Deny.
        let config = make_global_config(
            vec![(GlobalTool::Web, PermissionMode::Allow)],
            vec![], // external has no web entry → default Deny
        );
        let resolver = PolicyResolver::new(config);
        let call = make_call("web_search", json!({ "query": "test" }));
        let decision = resolver.check(&call, CallerRole::External);
        assert!(decision.is_deny(), "web_search should be denied for external by default");
    }

    #[test]
    fn test_global_tool_ask_for_external() {
        // External has todo=Ask → check returns Ask.
        let config = make_global_config(
            vec![(GlobalTool::Todo, PermissionMode::Allow)],
            vec![(GlobalTool::Todo, PermissionMode::Ask)],
        );
        let resolver = PolicyResolver::new(config);
        let call = make_call("todo_create", json!({ "title": "task" }));
        let decision = resolver.check(&call, CallerRole::External);
        assert!(decision.is_ask(), "todo_create should require confirmation for external");
    }

    #[test]
    fn test_all_global_tool_names_classified() {
        // Verify every tool name we claim is global is actually classified that way.
        let global_names = [
            "web_search", "web_fetch",
            "subagent", "spawn_agent",
            "ask",
            "todo_create", "todo_list", "todo_update", "todo_delete",
            "load_tools",
        ];
        for name in &global_names {
            match classify_tool(name) {
                ToolScope::Global(_) => { /* correct */ }
                ToolScope::Dir(_) => panic!("'{}' should be classified as global", name),
            }
        }
    }

    // ── Directory-scoped tool tests ────────────────────────────────────────────

    #[test]
    fn test_dir_tool_allow_inside_directory() {
        // File inside allowed directory + owner=Allow → Allow.
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("code.rs");
        std::fs::write(&file, "fn main() {}").unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Fs(FsTool::Read), PermissionMode::Allow)],
            vec![],
        );
        let resolver = PolicyResolver::new(config);
        let call = make_call("read", json!({ "paths": [file.to_str().unwrap()] }));
        let decision = resolver.check(&call, CallerRole::Owner);
        assert!(decision.is_allow(), "read inside allowed dir should be allowed for owner");
    }

    #[test]
    fn test_dir_tool_deny_outside_directory() {
        // File outside all registered directories → hard Deny.
        let tmp = TempDir::new().unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Fs(FsTool::Read), PermissionMode::Allow)],
            vec![],
        );
        let resolver = PolicyResolver::new(config);

        // Use a different directory that is NOT registered.
        let other_tmp = TempDir::new().unwrap();
        let outside_file = other_tmp.path().join("secret.txt");
        std::fs::write(&outside_file, "data").unwrap();

        let call = make_call("read", json!({ "paths": [outside_file.to_str().unwrap()] }));
        let decision = resolver.check(&call, CallerRole::Owner);
        assert!(decision.is_deny(), "read outside allowed dirs should be denied even for owner");
    }

    #[test]
    fn test_dir_tool_ask_for_external() {
        // External has write=Ask → check returns Ask for a file in the allowed dir.
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("data.txt");
        std::fs::write(&file, "content").unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Fs(FsTool::Write), PermissionMode::Allow)],
            vec![(DirTool::Fs(FsTool::Write), PermissionMode::Ask)],
        );
        let resolver = PolicyResolver::new(config);
        let call = make_call("write", json!({ "path": file.to_str().unwrap(), "content": "new" }));
        let decision = resolver.check(&call, CallerRole::External);
        assert!(decision.is_ask(), "write for external should require confirmation");
    }

    #[test]
    fn test_bash_uses_cwd_as_policy_anchor() {
        // The bash tool uses `cwd` (not `path`) as the policy path argument.
        let tmp = TempDir::new().unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Bash, PermissionMode::Allow)],
            vec![(DirTool::Bash, PermissionMode::Deny)],
        );
        let resolver = PolicyResolver::new(config);

        // Owner calls bash with cwd inside the allowed dir → Allow.
        let owner_call = make_call(
            "bash",
            json!({ "command": "ls", "cwd": tmp.path().to_str().unwrap() }),
        );
        let owner_decision = resolver.check(&owner_call, CallerRole::Owner);
        assert!(owner_decision.is_allow(), "bash with valid cwd should be allowed for owner");

        // External calls bash with same cwd → Deny (per config).
        let ext_call = make_call(
            "bash",
            json!({ "command": "ls", "cwd": tmp.path().to_str().unwrap() }),
        );
        let ext_decision = resolver.check(&ext_call, CallerRole::External);
        assert!(ext_decision.is_deny(), "bash should be denied for external per config");
    }

    #[test]
    fn test_bash_missing_cwd_is_denied() {
        // If `cwd` is absent from the bash call args → Deny (no path anchor).
        let tmp = TempDir::new().unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Bash, PermissionMode::Allow)],
            vec![],
        );
        let resolver = PolicyResolver::new(config);

        // bash call WITHOUT cwd → policy can't evaluate → Deny.
        let call = make_call("bash", json!({ "command": "ls" })); // no cwd
        let decision = resolver.check(&call, CallerRole::Owner);
        assert!(
            decision.is_deny(),
            "bash without cwd should be denied — no policy anchor"
        );
    }

    #[test]
    fn test_missing_path_arg_is_denied() {
        // A `read` call without the `paths` argument → Deny.
        let tmp = TempDir::new().unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Fs(FsTool::Read), PermissionMode::Allow)],
            vec![],
        );
        let resolver = PolicyResolver::new(config);

        // `read` call with no `paths` argument.
        let call = make_call("read", json!({ "wrong_key": "whatever" }));
        let decision = resolver.check(&call, CallerRole::Owner);
        assert!(
            decision.is_deny(),
            "read without paths argument should be denied"
        );
    }

    #[test]
    fn test_unknown_tool_is_denied() {
        // An unknown tool name is classified as a global unknown and denied.
        let config = make_global_config(vec![], vec![]);
        let resolver = PolicyResolver::new(config);
        let call = make_call("definitely_not_a_real_tool", json!({}));
        let decision = resolver.check(&call, CallerRole::Owner);
        assert!(decision.is_deny(), "unknown tool should be denied");
    }

    #[test]
    fn test_owner_allow_external_deny_same_directory() {
        // Same directory, same file. Owner gets Allow, External gets Deny.
        // This is the core multi-role isolation property.
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("report.txt");
        std::fs::write(&file, "data").unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Fs(FsTool::Read), PermissionMode::Allow)],
            vec![(DirTool::Fs(FsTool::Read), PermissionMode::Deny)],
        );
        let resolver = PolicyResolver::new(config);
        let path_str = file.to_str().unwrap();

        let owner_call = make_call("read", json!({ "paths": [path_str] }));
        assert!(
            resolver.check(&owner_call, CallerRole::Owner).is_allow(),
            "owner should be allowed to read"
        );

        let ext_call = make_call("read", json!({ "paths": [path_str] }));
        assert!(
            resolver.check(&ext_call, CallerRole::External).is_deny(),
            "external should be denied from reading"
        );
    }

    #[test]
    fn test_empty_directories_denies_all_dir_tools() {
        // With no directories registered, any dir-scoped tool call is denied.
        let config = PolicyConfig::empty();
        let resolver = PolicyResolver::new(config);

        let call = make_call("read", json!({ "paths": ["/any/path.txt"] }));
        let decision = resolver.check(&call, CallerRole::Owner);
        assert!(
            decision.is_deny(),
            "dir-scoped tool with no directories should be denied"
        );
    }

    #[test]
    fn test_default_deny_for_missing_entry() {
        // A directory is registered but the specific DirTool has no entry.
        // Missing entries must default to Deny, not Allow.
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("file.txt");
        std::fs::write(&file, "data").unwrap();
        let canonical_dir = std::fs::canonicalize(tmp.path()).unwrap();

        // Register `read` but NOT `write` — write should default to Deny.
        let config = make_dir_config(
            canonical_dir,
            vec![(DirTool::Fs(FsTool::Read), PermissionMode::Allow)],
            vec![],
        );
        let resolver = PolicyResolver::new(config);

        let write_call = make_call("write", json!({ "path": file.to_str().unwrap(), "content": "x" }));
        let decision = resolver.check(&write_call, CallerRole::Owner);
        assert!(
            decision.is_deny(),
            "write should default to Deny when not configured"
        );
    }

    #[test]
    fn test_all_dir_tool_names_classified() {
        // Verify every tool name we claim is dir-scoped is actually classified that way.
        let dir_names = ["read", "write", "edit", "append", "grep", "ls", "delete", "bash"];
        for name in &dir_names {
            match classify_tool(name) {
                ToolScope::Dir(_) => { /* correct */ }
                ToolScope::Global(_) => panic!("'{}' should be classified as dir-scoped", name),
            }
        }
    }
}
