/// Basic usage example for the memory_retrieve tool.
///
/// Hey friend! This example demonstrates how to use the `memory_retrieve` tool to fetch
/// specific memories by ID or paginate through stored memories.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_memory_retrieve::{definition, execute};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("=== Operon Memory Retrieve Tool Example ===\n");

    let tmp = NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();

    let _ = store
        .add(
            "User speaks English and Spanish".to_string(),
            vec!["language".to_string()],
        )
        .await
        .unwrap();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Retrieve memories
    println!("2. Retrieving all memories:");
    let args = json!({
        "limit": 10,
        "offset": 0
    });

    let result = execute(ToolCallId("call_mem_r1".to_string()), args, &store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total: {}", output["total"]);
        if let Some(memories) = output["memories"].as_array() {
            for mem in memories {
                println!("   - [#{}] {}", mem["id"], mem["content"]);
            }
        }
    }
    println!();

    println!("=== Memory Retrieve Example Complete ===");
}
