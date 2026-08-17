//! Output types for the memory_delete tool.

use serde::{Deserialize, Serialize};

/// Output returned to the model after a memory is deleted.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryDeleteOutput {
    /// The id of the deleted memory (echoed back for confirmation).
    pub id: String,

    /// Total number of memories remaining in the store after deletion.
    pub remaining: i64,
}
