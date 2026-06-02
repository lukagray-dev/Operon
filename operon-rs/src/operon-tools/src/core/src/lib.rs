//! # operon-tools-core
//!
//! Shared types for all Operon tool crates.
//!
//! ## What lives here
//!
//! - [`TieredToolDefinition`] — a tool definition with short + detailed description tiers,
//!   used by the dispatcher to recover gracefully from malformed model tool calls.
//! - [`ToolDispatchError`] — error type for the dispatcher.
//!
//! ## What does NOT live here
//!
//! Tool implementations, tool registries, async runtimes, I/O. This is a pure types crate.

pub mod dispatch;
pub mod progress;
pub mod read_ledger;
pub mod tiered;
pub mod todo;
pub mod todo_store;

pub use dispatch::ToolDispatchError;
pub use progress::{emit_tool_progress, ToolProgress, ToolProgressEmitter, ToolProgressStage};
pub use read_ledger::ReadLedger;
pub use tiered::TieredToolDefinition;
pub use todo::{TodoItem, TodoPriority, TodoStatus};
pub use todo_store::TodoStore;
