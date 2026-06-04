//! Core types for the policy resolver.

pub use operon_config::{CallerRole, DirTool, FsTool, GlobalTool, PermissionMode};

/// The output of a `PolicyResolver::check()` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Ask { reason: String },
    Deny { reason: String },
}

impl PolicyDecision {
    pub fn is_allow(&self) -> bool {
        matches!(self, PolicyDecision::Allow)
    }

    pub fn is_deny(&self) -> bool {
        matches!(self, PolicyDecision::Deny { .. })
    }

    pub fn is_ask(&self) -> bool {
        matches!(self, PolicyDecision::Ask { .. })
    }
}

/// The scope category of a tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToolScope {
    Global(GlobalTool),
    Dir(DirTool),
}
