/// Basic usage example for the todo_list tool.
///
/// Hey friend! This example demonstrates how to use the `todo_list` tool to inspect
/// and filter tasks by status and priority in the TodoStore.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_core::{TodoPriority, TodoStore};
use operon_tools_todo_list::{definition, execute};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("=== Operon Todo List Tool Example ===\n");

    let mut store = TodoStore::new();
    let _ = store.create(
        "Complete unit test validation".to_string(),
        Some(TodoPriority::High),
    );

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. List all todo items
    println!("2. Listing all todo items:");
    let args = json!({});

    let result = execute(ToolCallId("call_todo_l1".to_string()), args, &store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total Items: {}", output["total"]);
        if let Some(items) = output["items"].as_array() {
            for item in items {
                println!(
                    "   - [#{}] ({}) {} (Status: {})",
                    item["id"], item["priority"], item["content"], item["status"]
                );
            }
        }
    }
    println!();

    println!("=== Todo List Example Complete ===");
}
