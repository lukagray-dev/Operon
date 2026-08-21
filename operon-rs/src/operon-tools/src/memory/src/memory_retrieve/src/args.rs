//! Argument types for the memory_retrieve tool.
//!
//! Two modes:
//!  1. `id` provided → fetch a single memory by id.
//!  2. No `id` → paginate through all memories (limit/offset).

use operon_tools_core::de::deserialize_flexible_single_string_opt;
use serde::Deserialize;

/// Arguments for the memory_retrieve tool.
#[derive(Debug, Deserialize)]
pub struct MemoryRetrieveArgs {
    /// Optional memory id for single-record lookup.
    /// If provided, `limit` and `offset` are ignored.
    /// Accepts string "1" or integer 1.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_single_string_opt",
        alias = "memory_id",
        alias = "memoryId"
    )]
    pub id: Option<String>,

    /// Maximum number of memories to return in list mode. Defaults to 20.
    #[serde(default)]
    pub limit: Option<usize>,

    /// Number of memories to skip (for pagination). Defaults to 0.
    #[serde(default)]
    pub offset: Option<usize>,
}
