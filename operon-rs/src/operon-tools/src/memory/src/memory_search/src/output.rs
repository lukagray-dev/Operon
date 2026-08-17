//! Output types for the memory_search tool.

use operon_tools_memory_store::Memory;
use serde::{Deserialize, Serialize};

/// Output returned by memory_search.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemorySearchOutput {
    /// Matched memories, ranked by FTS5 relevance (most relevant first).
    pub memories: Vec<Memory>,

    /// Number of results returned (may be less than `limit` if fewer matched).
    pub count: usize,

    /// The query that was searched (echoed back for clarity).
    pub query: String,
}
