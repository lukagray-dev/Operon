//! # operon-policy
//!
//! Permission enforcement for the Operon agent.
//!
//! This crate owns the runtime resolver and re-exports the policy data model
//! from `operon-config` for backwards compatibility.

pub mod config;
pub mod error;
pub mod path_guard;
pub mod resolver;
pub mod types;

pub use config::{
    CallerRole, DirTool, DirectoryPolicy, FsTool, GlobalPolicy, GlobalTool, PermissionMode,
    PolicyConfig,
};
pub use error::PolicyError;
pub use resolver::PolicyResolver;
pub use types::PolicyDecision;
