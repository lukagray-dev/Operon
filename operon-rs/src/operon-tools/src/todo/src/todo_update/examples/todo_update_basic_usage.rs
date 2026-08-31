/// Basic usage example for the todo_update tool.
///
/// Hey friend! This example demonstrates how to use the `todo_update` tool to mark
/// tasks as `in_progress` or `completed` as work progresses.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_core::{TodoPriority, TodoStore};
use operon_tools_todo_update::{definition, execute};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("=== Operon Todo Update Tool Example ===\n");

    let mut store = TodoStore::new();
    let item = store.create(
        "Run comprehensive tests".to_string(),
        Some(TodoPriority::High),
    );

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 2. Update status to completed
    println!("2. Updating item status to completed:");
    let args = json!({
        "id": item.id,
        "status": "completed"
    });

    let result = execute(ToolCallId("call_todo_u1".to_string()), args, &mut store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Item ID: {}", output["id"]);
        println!("   New Status: {}", output["status"]);
    }
    println!();

    println!("=== Todo Update Example Complete ===");
}
