/// Basic usage example for the web_search tool.
///
/// Hey friend! This example demonstrates how to use the `web_search` tool to execute
/// search queries and receive structured results (rank, title, url, snippet).
use operon_context_normalize_tools::ToolCallId;
use operon_tools_web_search::{definition, execute};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("=== Operon Web Search Tool Example ===\n");

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Query web search
    println!("2. Performing search query:");
    let args = json!({
        "query": "Rust language documentation",
        "max_results": 3
    });

    let result = execute(ToolCallId("call_search_1".to_string()), args)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total Results: {}", output["total_results"]);
        if let Some(results) = output["results"].as_array() {
            for item in results {
                println!(
                    "   - [Rank {}] {}: {}",
                    item["rank"], item["title"], item["url"]
                );
            }
        }
    }
    println!();

    println!("=== Web Search Example Complete ===");
}
