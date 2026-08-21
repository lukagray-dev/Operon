/// Basic usage example for the memory_add tool.
///
/// Hey friend! This example demonstrates how to use the `memory_add` tool to persist
/// cross-session user preferences and facts to the persistent SQLite MemoryStore.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_memory_add::{definition, execute};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("=== Operon Memory Add Tool Example ===\n");

    let tmp = NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Add a memory
    println!("2. Adding a new memory with tags:");
    let args = json!({
        "content": "User prefers concise explanations with practical examples.",
        "tags": ["preferences", "style"]
    });

    let result = execute(ToolCallId("call_mem_a1".to_string()), args, &store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Memory ID: {}", output["memory"]["id"]);
        println!("   Total Memories: {}", output["total"]);
    }
    println!();

    println!("=== Memory Add Example Complete ===");
}
