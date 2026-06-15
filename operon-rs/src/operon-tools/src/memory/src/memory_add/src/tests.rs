//! Tests for the memory_add tool.

use super::*;
use operon_context_normalize::tools::ToolCallId;
use serde_json::json;

#[tokio::test]
async fn test_memory_add_success() {
    let call_id = ToolCallId("test_call".to_string());
    let args = json!({
        "content": "This is a test memory content"
    });

    let result = execute(call_id, args).await.expect("execution failed");
    assert!(!result.is_error);
    
    if let operon_context_normalize::tools::ToolContent::Text(text) = result.content {
        assert!(text.contains("Memory added successfully with ID"));
    } else {
        panic!("expected text content");
    }
}

#[tokio::test]
async fn test_memory_add_empty_content() {
    let call_id = ToolCallId("test_call".to_string());
    let args = json!({
        "content": ""
    });

    let result = execute(call_id, args).await;
    assert!(result.is_err());
}
