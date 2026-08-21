/// Basic usage example for the web_fetch tool.
///
/// Hey friend! This example demonstrates how to use the `web_fetch` tool to fetch
/// webpage content and convert HTML into clean, readable Markdown for agent consumption.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_web_fetch::{definition, execute};
use serde_json::json;

#[tokio::main]
async fn main() {
    println!("=== Operon Web Fetch Tool Example ===\n");

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Fetch webpage
    println!("2. Fetching webpage URL:");
    let args = json!({
        "url": "https://example.com",
        "timeout_ms": 10000
    });

    let result = execute(ToolCallId("call_fetch_1".to_string()), args)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Status Code: {}", output["status_code"]);
        println!("   Title: {}", output["title"]);
        println!("   Content Length: {}", output["character_count"]);
    }
    println!();

    println!("=== Web Fetch Example Complete ===");
}
