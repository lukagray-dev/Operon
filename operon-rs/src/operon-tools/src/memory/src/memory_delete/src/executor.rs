//! Database query executor for the memory_delete tool.

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use crate::args::MemoryDeleteArgs;
use crate::error::MemoryDeleteToolError;

/// Deletes the memory from the database and returns a formatted ToolResult.
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryDeleteArgs,
) -> Result<ToolResult, MemoryDeleteToolError> {
    // Open a database connection.
    let mut conn = operon_tools_memory::connect_db().await?;

    // Perform the deletion.
    let result = sqlx::query(
        "DELETE FROM memories WHERE id = ?"
    )
    .bind(args.id)
    .execute(&mut conn)
    .await?;

    let rows_affected = result.rows_affected();
    let content = if rows_affected > 0 {
        format!("Memory with ID {} deleted successfully.", args.id)
    } else {
        format!("No memory found with ID {}.", args.id)
    };

    Ok(ToolResult {
        call_id,
        name: "memory_delete".to_string(),
        content: ToolContent::Text(content),
        is_error: rows_affected == 0,
        read_paths: None,
    })
}
