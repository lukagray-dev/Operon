/// Basic usage example for the todo_delete tool.
///
/// Hey friend! This example demonstrates how to use the `todo_delete` tool to remove
/// accidentally created todo items from the TodoStore.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_core::{TodoPriority, TodoStore};
use operon_tools_todo_delete::{definition, execute};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("=== Operon Todo Delete Tool Example ===\n");

    let mut store = TodoStore::new();
    let item = store.create("Accidental task".to_string(), Some(TodoPriority::Low));

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 2. Delete the item
    println!("2. Deleting the item by ID:");
    let args = json!({
        "id": item.id
    });

    let result = execute(ToolCallId("call_todo_d1".to_string()), args, &mut store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Deleted ID: {}", output["id"]);
        println!("   Message: {}", output["message"]);
    }
    println!();

    println!("=== Todo Delete Example Complete ===");
}
