/// Basic usage example for the memory_delete tool.
///
/// Hey friend! This example demonstrates how to use the `memory_delete` tool to permanently
/// remove an obsolete or incorrect memory from the SQLite MemoryStore.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_memory_delete::{definition, execute};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("=== Operon Memory Delete Tool Example ===\n");

    let tmp = NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();

    let memory = store
        .add("Temporary memo to delete".to_string(), vec![])
        .await
        .unwrap();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Delete memory
    println!("2. Deleting memory by ID:");
    let args = json!({
        "id": memory.id
    });

    let result = execute(ToolCallId("call_mem_d1".to_string()), args, &store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Deleted ID: {}", output["id"]);
        println!("   Remaining Memories: {}", output["remaining"]);
    }
    println!();

    println!("=== Memory Delete Example Complete ===");
}
