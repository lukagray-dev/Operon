//! Output types for the memory_edit tool.

use operon_tools_memory_store::Memory;
use serde::{Deserialize, Serialize};

/// Output returned to the model after a memory is edited.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryEditOutput {
    /// The updated memory with new field values and refreshed `updated_at` timestamp.
    pub memory: Memory,
}
