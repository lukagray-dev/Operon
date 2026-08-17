//! Argument types for the memory_delete tool.

use operon_tools_core::de::deserialize_flexible_id;
use serde::Deserialize;

/// Arguments for the memory_delete tool.
#[derive(Debug, Deserialize)]
pub struct MemoryDeleteArgs {
    /// Id of the memory to permanently delete.
    /// Supports string "1" or integer 1.
    #[serde(
        deserialize_with = "deserialize_flexible_id",
        alias = "memory_id",
        alias = "memoryId",
        alias = "item_id",
        alias = "itemId"
    )]
    pub id: String,
}
