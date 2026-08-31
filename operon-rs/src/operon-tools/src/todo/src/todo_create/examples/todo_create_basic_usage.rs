/// Basic usage example for the todo_create tool.
///
/// Hey friend! This example demonstrates how to use the `todo_create` tool to track
/// pending tasks and priorities in the session-scoped TodoStore.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_core::TodoStore;
use operon_tools_todo_create::{definition, execute};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("=== Operon Todo Create Tool Example ===\n");

    let mut store = TodoStore::new();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 2. Create a todo item
    println!("2. Creating a todo item:");
    let args = json!({
        "content": "Implement zero-warning builds for Windows release",
        "priority": "high"
    });

    let result = execute(ToolCallId("call_todo_c1".to_string()), args, &mut store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Created Item ID: {}", output["id"]);
        println!("   Status: {}", output["status"]);
        println!("   Priority: {}", output["priority"]);
    }
    println!();

    println!("=== Todo Create Example Complete ===");
}
