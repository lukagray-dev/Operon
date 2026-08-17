//! # operon-tools-memory
//!
//! Facade crate for all memory tools: add, edit, delete, retrieve, search.
//!
//! Re-exports all five memory tool sub-crates.
//! The memory tools implement a global persistent task memory for the agent using SQLite.
//! Unlike todo tools (session-scoped), memories survive process restarts indefinitely.

pub use operon_tools_memory_add as memory_add;
pub use operon_tools_memory_delete as memory_delete;
pub use operon_tools_memory_edit as memory_edit;
pub use operon_tools_memory_retrieve as memory_retrieve;
pub use operon_tools_memory_search as memory_search;
