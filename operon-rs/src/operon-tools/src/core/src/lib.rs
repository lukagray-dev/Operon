//! # operon-tools-core
//!
//! Shared types and utilities for all Operon tool crates.
//!
//! ## What lives here
//!
//! - [`ToolDefinition`] — canonical tool definition with name, description, and JSON schema parameters.
//! - [`ToolDispatchError`] — error type for the dispatcher.
//! - [`de`] — defensive deserialization helpers for handling model schema quirks.
//!
//! ## What does NOT live here
//!
//! Tool implementations, tool registries, async runtimes, I/O. This is a pure types crate.

pub mod de;
pub mod dispatch;
pub mod progress;
pub mod read_ledger;
pub mod todo;
pub mod todo_store;

pub use dispatch::ToolDispatchError;
pub use operon_context_normalize_tools::ToolDefinition;
pub use progress::{emit_tool_progress, ToolProgress, ToolProgressEmitter, ToolProgressStage};
pub use read_ledger::ReadLedger;
pub use todo::{TodoItem, TodoPriority, TodoStatus};
pub use todo_store::TodoStore;
