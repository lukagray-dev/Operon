//! # operon-policy
//!
//! Permission enforcement for the Operon agent.
//!
//! This crate sits between the session runner and the tool dispatcher.
//! It answers one question per tool call: **is this call permitted for this caller?**
//!
//! The dispatcher and all tool implementations have zero knowledge of policy.
//! Policy is enforced exclusively by the session runner using this crate.
//!
//! ## Architecture
//!
//! ```text
//! runner.rs (operon-session)
//!   ↓
//!   PolicyResolver::check(&call, role)  ← this crate
//!   ↓
//!   PolicyDecision { Allow | Ask | Deny }
//!   ↓ (if Allow)
//!   Dispatcher::dispatch(call)          ← operon-tools
//! ```
//!
//! ## Permission model summary
//!
//! Every tool belongs to one of two scope categories:
//!
//! - **Global tools** (web, subagent, ask, todo, load_tools): permissions set
//!   once globally per caller role.
//! - **Directory-scoped tools** (fs tools, bash): permissions set per directory,
//!   per caller role, per individual tool. Any path outside a registered directory
//!   is denied unconditionally.
//!
//! Every permission has three modes: `Allow`, `Ask`, or `Deny`.
//! Missing entries default to `Deny` — safe by default.
//!
//! ## Caller roles
//!
//! - **Owner**: the system owner. Set by the channel at session construction.
//! - **External**: any other caller (customer, public user).
//!
//! Each directory can have completely different rules for Owner vs. External.
//!
//! ## Config lifecycle
//!
//! ```text
//! PolicyConfig::empty()          ← start with all-deny
//!   → add directories + permissions
//!   → config.validate()          ← canonicalize all paths (required)
//!   → PolicyResolver::new(config)
//!   → resolver.check(call, role) ← called once per tool call in runner.rs
//! ```
//!
//! ## TODO: operon-config migration
//!
//! `PolicyConfig` and its sub-types currently live in `operon_policy::config`.
//! Once the `operon-config` crate is built, they will move there and be
//! re-exported from here. The public API of this crate will not change.

pub mod config;
pub mod error;
pub mod path_guard;
pub mod resolver;
pub mod types;

// ─────────────────────────────────────────────────────────────────────────────
// Public re-exports
//
// These are the types that `operon-session` (and tests) need to import.
// Everything else is an implementation detail inside the submodules.
// ─────────────────────────────────────────────────────────────────────────────

// Config types — used to build and load the policy at session startup.
pub use config::{DirectoryPolicy, GlobalPolicy, PolicyConfig};

// Error type — returned by PolicyConfig::validate().
pub use error::PolicyError;

// Core types — used by the session runner in the dispatch loop.
pub use types::{CallerRole, DirTool, FsTool, GlobalTool, PermissionMode, PolicyDecision};

// The resolver — the primary entry point for all policy checks.
pub use resolver::PolicyResolver;
