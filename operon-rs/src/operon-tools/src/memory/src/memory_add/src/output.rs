//! Output types for the memory_add tool.

use operon_tools_memory_store::Memory;
use serde::{Deserialize, Serialize};

/// Output returned to the model after a memory is created.
///
/// Contains the newly created memory (including its auto-assigned id) and the
/// total count of all memories in the store after creation.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryAddOutput {
    /// The created memory, including its assigned id and timestamps.
    pub memory: Memory,

    /// Total number of memories in the store after this addition.
    /// Useful for the model to understand the memory store size.
    pub total: i64,
}
