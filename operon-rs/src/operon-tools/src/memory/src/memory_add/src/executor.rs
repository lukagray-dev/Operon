//! Database query executor for the memory_add tool.

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use crate::args::MemoryAddArgs;
use crate::error::MemoryAddToolError;
use sqlx::Row;

/// Inserts the memory into the database and returns a formatted ToolResult.
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryAddArgs,
) -> Result<ToolResult, MemoryAddToolError> {
    // Open a database connection (creates/updates schema if needed).
    let mut conn = operon_tools_memory::connect_db().await?;

    // Insert the memory and retrieve its auto-generated unique ID.
    let result = sqlx::query(
        "INSERT INTO memories (content) VALUES (?) RETURNING id"
    )
    .bind(&args.content)
    .fetch_one(&mut conn)
    .await?;

    let id: i64 = result.get("id");

    Ok(ToolResult {
        call_id,
        name: "memory_add".to_string(),
        content: ToolContent::Text(format!("Memory added successfully with ID {}.", id)),
        is_error: false,
        read_paths: None,
    })
}
