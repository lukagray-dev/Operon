// error.rs — Error types for the operon-policy crate.
//
// All fallible operations in this crate produce a `PolicyError`.
// The error set is intentionally small — most policy decisions are
// non-fallible (they return a PolicyDecision variant instead of Err).
//
// PolicyError is reserved for configuration-level failures:
// invalid directory paths, malformed config, and path canonicalization
// errors. These are setup-time errors, not per-call errors.

use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// PolicyError
// ─────────────────────────────────────────────────────────────────────────────

/// Errors produced by the `operon-policy` crate.
///
/// These are always configuration-level failures, not per-call failures.
/// Per-call failures (unknown tool, path outside allowed directories) are
/// encoded as `PolicyDecision::Deny` — never as `Err(PolicyError)`.
#[derive(Debug, Error)]
pub enum PolicyError {
    /// A directory path in the `PolicyConfig` could not be canonicalized.
    ///
    /// This happens when:
    /// - The path does not exist on disk at config-load time.
    /// - The path contains non-UTF-8 components on Windows.
    /// - The calling process lacks permission to resolve the path.
    ///
    /// Fix: ensure all directories in the config exist before loading.
    #[error("failed to canonicalize directory path '{path}': {reason}")]
    PathCanonicalization { path: String, reason: String },

    /// The `PolicyConfig` structure is internally inconsistent.
    ///
    /// Example: a `DirectoryPolicy` references a tool name that is not
    /// a valid `DirTool` variant (may occur during TOML deserialization
    /// of hand-edited config files).
    #[error("invalid policy configuration: {reason}")]
    InvalidConfig { reason: String },
}
