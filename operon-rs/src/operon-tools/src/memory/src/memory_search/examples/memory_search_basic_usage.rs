/// Basic usage example for the memory_search tool.
///
/// Hey friend! This example demonstrates how to use the `memory_search` tool to perform
/// full-text FTS5 search (BM25 relevance) over saved user memories.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_memory_search::{definition, execute};
use operon_tools_memory_store::MemoryStore;
use serde_json::json;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("=== Operon Memory Search Tool Example ===\n");

    let tmp = NamedTempFile::new().unwrap();
    let store = MemoryStore::connect(tmp.path()).await.unwrap();

    let _ = store
        .add(
            "User prefers dark theme in IDE and GUI".to_string(),
            vec!["ui".to_string()],
        )
        .await
        .unwrap();
    let _ = store
        .add(
            "User works on Rust backend systems".to_string(),
            vec!["tech".to_string()],
        )
        .await
        .unwrap();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 2. Search memories
    println!("2. Searching for 'dark theme':");
    let args = json!({
        "query": "dark theme",
        "limit": 5
    });

    let result = execute(ToolCallId("call_mem_s1".to_string()), args, &store)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Matched Count: {}", output["count"]);
        if let Some(memories) = output["memories"].as_array() {
            for mem in memories {
                println!("   - Match: {}", mem["content"]);
            }
        }
    }
    println!();

    println!("=== Memory Search Example Complete ===");
}
