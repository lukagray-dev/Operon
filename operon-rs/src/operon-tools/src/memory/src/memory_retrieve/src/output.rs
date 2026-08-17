//! Output types for the memory_retrieve tool.

use operon_tools_memory_store::Memory;
use serde::{Deserialize, Serialize};

/// Output returned by memory_retrieve.
///
/// Used for both single-record and list-mode responses — `memories` is always a Vec,
/// with one item in single-record mode. This avoids two separate types.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryRetrieveOutput {
    /// The retrieved memories. One item in single-id mode; multiple in list mode.
    pub memories: Vec<Memory>,

    /// Total memories in the store (not just this page). Useful for pagination.
    pub total: i64,

    /// Current page offset (echoed back from request for pagination awareness).
    pub offset: usize,

    /// Limit used for this request (echoed back for pagination awareness).
    pub limit: usize,
}
