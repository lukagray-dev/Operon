//! Tests for the memory_edit tool.

use super::*;
use operon_context_normalize::tools::ToolCallId;
use serde_json::json;

#[tokio::test]
async fn test_memory_edit_success() {
    use sqlx::Row;
    let call_id = ToolCallId("test_call".to_string());
    
    // Add memory first and get returned ID.
    let mut conn = operon_tools_memory::connect_db().await.unwrap();
    let row = sqlx::query("INSERT INTO memories (content) VALUES ('initial test content') RETURNING id")
        .fetch_one(&mut conn)
        .await
        .unwrap();
    let id: i64 = row.get("id");

    let args = json!({
        "id": id.to_string(),
        "content": "updated test content"
    });

    let result = execute(call_id, args).await.expect("execution failed");
    assert!(!result.is_error);
    
    if let operon_context_normalize::tools::ToolContent::Text(text) = result.content {
        assert!(text.contains(&format!("Memory with ID {} updated successfully.", id)));
    } else {
        panic!("expected text content");
    }
}

#[tokio::test]
async fn test_memory_edit_not_found() {
    let call_id = ToolCallId("test_call".to_string());
    let args = json!({
        "id": "999",
        "content": "some text"
    });

    let result = execute(call_id, args).await.expect("execution failed");
    assert!(result.is_error);
    
    if let operon_context_normalize::tools::ToolContent::Text(text) = result.content {
        assert!(text.contains("No memory found with ID 999."));
    } else {
        panic!("expected text content");
    }
}
