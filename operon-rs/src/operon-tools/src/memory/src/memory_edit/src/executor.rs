//! Database query executor for the memory_edit tool.

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use crate::args::MemoryEditArgs;
use crate::error::MemoryEditToolError;

/// Updates the memory row in the database and returns a formatted ToolResult.
pub async fn execute(
    call_id: ToolCallId,
    args: MemoryEditArgs,
) -> Result<ToolResult, MemoryEditToolError> {
    // Open a database connection.
    let mut conn = operon_tools_memory::connect_db().await?;

    // Perform the update.
    let result = sqlx::query(
        "UPDATE memories SET content = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(&args.content)
    .bind(args.id)
    .execute(&mut conn)
    .await?;

    let rows_affected = result.rows_affected();
    let content = if rows_affected > 0 {
        format!("Memory with ID {} updated successfully.", args.id)
    } else {
        format!("No memory found with ID {}.", args.id)
    };

    Ok(ToolResult {
        call_id,
        name: "memory_edit".to_string(),
        content: ToolContent::Text(content),
        is_error: rows_affected == 0,
        read_paths: None,
    })
}
