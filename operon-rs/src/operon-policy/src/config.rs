//! Compatibility re-exports for policy configuration types.
//!
//! The actual policy data model now lives in `operon-config::policy`. This
//! module stays as a thin shim so older call sites can keep importing from
//! `operon-policy` while the rest of the codebase moves to the new home.

pub use operon_config::{
    CallerRole, DirTool, DirectoryPolicy, FsTool, GlobalPolicy, GlobalTool, PermissionMode,
    PolicyConfig, PolicyError,
};
