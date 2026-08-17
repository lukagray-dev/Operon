//! # operon-tools-memory-store
//!
//! Global persistent memory store (SQLite-backed) for the Operon agent.
//!
//! This is the **leaf crate** in the memory subsystem dependency graph:
//! - It depends on `sqlx`, `chrono`, `operon-config`, `thiserror`.
//! - It does NOT depend on any `memory_*` tool sub-crate (that would create a cycle).
//! - All five tool sub-crates (`memory_add`, `memory_edit`, etc.) depend on THIS crate.
//!
//! # Key types
//!
//! - [`MemoryStore`] — the async store. Clone-safe; wraps `sqlx::SqlitePool`.
//! - [`Memory`] — the data type for a single stored memory. Serializable.
//! - [`MemoryStoreError`] — all error variants from store operations.

mod error;
mod memory;
mod store;

#[cfg(test)]
mod tests;

pub use error::MemoryStoreError;
pub use memory::Memory;
pub use store::MemoryStore;
