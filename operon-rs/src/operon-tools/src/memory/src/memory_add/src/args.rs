//! Argument types for the memory_add tool.
//!
//! Hey friend! This defines the defensive deserialization schema for memory_add.
//! We use generous aliases so models that name the field differently still work.
//! Tags are handled with `deserialize_flexible_string_list_opt` because models
//! often pass a single string instead of a JSON array (e.g., `"tags": "pref"`).

use operon_tools_core::de::{deserialize_flexible_string_list_opt};
use serde::Deserialize;

/// Arguments for the memory_add tool.
///
/// Only `content` is required. `tags` is optional and supports flexible input shapes.
#[derive(Debug, Deserialize)]
pub struct MemoryAddArgs {
    /// The memory content — a fact, preference, or note to store persistently.
    ///
    /// Use clear, self-contained sentences (e.g., "User prefers tabs over spaces").
    /// Validation: must be non-empty after trim.
    ///
    /// Generous aliases here because different model families name this field differently.
    #[serde(
        alias = "note",
        alias = "fact",
        alias = "text",
        alias = "memory",
        alias = "info"
    )]
    pub content: String,

    /// Optional tags for categorizing this memory (e.g. `["preference", "workflow"]`).
    ///
    /// Uses `deserialize_flexible_string_list_opt` to handle:
    ///   - Native arrays:  `["pref", "workflow"]`
    ///   - Single strings: `"pref"` → `Some(vec!["pref"])`
    ///   - Null/omitted:   → `None`
    ///   - Stringified:    `'["pref"]'` → parsed
    #[serde(
        default,
        alias = "tag",
        deserialize_with = "deserialize_flexible_string_list_opt"
    )]
    pub tags: Option<Vec<String>>,
}
