/// Basic usage example for the memory_edit tool.
///
/// Hey friend! This example demonstrates how to use the `memory_edit` tool to update
/// an existing memory's content or tags in the persistent MemoryStore.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_memory_edit::{definition, execute};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("=== Operon Memory Edit Tool Example ===\n");

    let tmp = NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();

    let memory = store
        .add(
            "User lives in San Francisco".to_string(),
            vec!["location".to_string()],
        )
        .await
        .unwrap();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 2. Edit memory
    println!("2. Updating memory location:");
    let args = json!({
        "id": memory.id,
        "content": "User lives in Seattle, WA"
    });

    let result = execute(ToolCallId("call_mem_e1".to_string()), args, &store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Updated Content: {}", output["memory"]["content"]);
    }
    println!();

    println!("=== Memory Edit Example Complete ===");
}
