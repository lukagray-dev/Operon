//! Database query executor for the memory_search tool.

use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};
use crate::args::MemorySearchArgs;
use crate::error::MemorySearchToolError;
use sqlx::Row;

/// Searches the memories table and returns matching memory text blocks.
///
/// If the query is empty or not provided, it fetches and lists all memories stored
/// in the database, ordered by their last update timestamp (most recent first).
/// If a search query is provided, it uses a SQL LIKE operator to filter memories
/// that contain the search term as a substring.
pub async fn execute(
    call_id: ToolCallId,
    args: MemorySearchArgs,
) -> Result<ToolResult, MemorySearchToolError> {
    // Open a connection to our shared SQLite database.
    // In tests, this will connect to a temporary on-disk SQLite DB.
    let mut conn = operon_tools_memory::connect_db().await?;

    // Determine the query strategy:
    // 1. If the query string is empty, we load all memories from the database.
    // 2. Otherwise, we perform a substring match search using SQLite's LIKE clause.
    let rows = if args.query.is_empty() {
        // Retrieve all stored memories, sorted so the newest ones show up first.
        sqlx::query(
            "SELECT id, content, updated_at FROM memories ORDER BY updated_at DESC"
        )
        .fetch_all(&mut conn)
        .await?
    } else {
        // Construct a pattern for SQL LIKE, matching '%query%'.
        // This makes the search case-insensitive in SQLite by default and searches anywhere in the content.
        let search_pattern = format!("%{}%", args.query);
        sqlx::query(
            "SELECT id, content, updated_at FROM memories WHERE content LIKE ? ORDER BY updated_at DESC"
        )
        .bind(search_pattern)
        .fetch_all(&mut conn)
        .await?
    };

    // If the database query returned no records, handle it gracefully.
    if rows.is_empty() {
        let message = if args.query.is_empty() {
            // Friendly message indicating that the database is currently empty.
            "No memories stored yet.".to_string()
        } else {
            // Indicate that no matching records were found for the user's specific query.
            format!("No memories found matching query: '{}'.", args.query)
        };

        return Ok(ToolResult {
            call_id,
            name: "memory_search".to_string(),
            content: ToolContent::Text(message),
            is_error: false,
            read_paths: None,
        });
    }

    // Determine the result header depending on whether we performed a search or listed everything.
    let header = if args.query.is_empty() {
        format!("All stored memories ({} total):\n\n", rows.len())
    } else {
        format!("Found {} matching memory/memories:\n\n", rows.len())
    };

    // Build the plain text formatted list response.
    // For each database row, we extract the ID, content, and updated timestamp.
    let mut output = header;
    for row in rows {
        let id: i64 = row.get("id");
        let content: String = row.get("content");
        let updated_at: String = row.get("updated_at");
        output.push_str(&format!("--- ID: {} (Updated: {}) ---\n{}\n\n", id, updated_at, content));
    }

    // Return the successful ToolResult with the formatted text.
    Ok(ToolResult {
        call_id,
        name: "memory_search".to_string(),
        content: ToolContent::Text(output),
        is_error: false,
        read_paths: None,
    })
}

