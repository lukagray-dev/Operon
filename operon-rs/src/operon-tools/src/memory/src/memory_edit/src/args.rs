//! Argument types for the memory_edit tool.

use operon_tools_core::de::{deserialize_flexible_id, deserialize_flexible_string_list_opt};
use serde::Deserialize;

/// Arguments for the memory_edit tool.
///
/// `id` is required. At least one of `content` or `tags` must be provided.
/// Only provided fields are updated — the rest remain unchanged (partial update semantics).
#[derive(Debug, Deserialize)]
pub struct MemoryEditArgs {
    /// Id of the memory to update. Required.
    /// Supports string "1" or numeric 1.
    #[serde(
        deserialize_with = "deserialize_flexible_id",
        alias = "memory_id",
        alias = "memoryId",
        alias = "item_id",
        alias = "itemId"
    )]
    pub id: String,

    /// New content for the memory. If None, content is unchanged.
    /// Must be non-empty after trim if provided.
    #[serde(
        default,
        alias = "note",
        alias = "fact",
        alias = "text",
        alias = "memory",
        alias = "info"
    )]
    pub content: Option<String>,

    /// New tags to replace the memory's current tags. If None, tags are unchanged.
    #[serde(
        default,
        alias = "tag",
        deserialize_with = "deserialize_flexible_string_list_opt"
    )]
    pub tags: Option<Vec<String>>,
}
