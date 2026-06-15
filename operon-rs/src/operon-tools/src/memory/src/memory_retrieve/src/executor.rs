//! Database query executor for the memory_retrieve tool.

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use crate::args::MemoryRetrieveArgs;
use crate::error::MemoryRetrieveToolError;
use sqlx::Row;

/// Retrieves the memory from the database and returns a formatted ToolResult.
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryRetrieveArgs,
) -> Result<ToolResult, MemoryRetrieveToolError> {
    // Open a database connection.
    let mut conn = operon_tools_memory::connect_db().await?;

    // Perform query.
    let result = sqlx::query(
        "SELECT content, updated_at FROM memories WHERE id = ?"
    )
    .bind(args.id)
    .fetch_optional(&mut conn)
    .await?;

    match result {
        Some(row) => {
            let content: String = row.get("content");
            let updated_at: String = row.get("updated_at");
            let output = format!("ID: {}\nUpdated: {}\n\n{}", args.id, updated_at, content);
            Ok(ToolResult {
                call_id,
                name: "memory_retrieve".to_string(),
                content: ToolContent::Text(output),
                is_error: false,
                read_paths: None,
            })
        }
        None => {
            Ok(ToolResult {
                call_id,
                name: "memory_retrieve".to_string(),
                content: ToolContent::Text(format!("No memory found with ID {}.", args.id)),
                is_error: true,
                read_paths: None,
            })
        }
    }
}
