/// Basic usage example for the load_tools tool.
///
/// Hey friend! This example demonstrates how to use the `load_tools` tool to dynamically
/// discover available tool groups and inspect tool parameter schemas on demand.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_load::{definition, execute_list_groups};

#[tokio::main]
async fn main() {
    println!("=== Operon Load Tools Example ===\n");

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Discover tool groups
    println!("2. Listing available tool groups:");
    let available_groups = vec![
        "fs".to_string(),
        "shell".to_string(),
        "web".to_string(),
        "todo".to_string(),
        "memory".to_string(),
    ];

    let result = execute_list_groups(ToolCallId("call_load_1".to_string()), available_groups);

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Discovered Groups: {:?}", output["available_groups"]);
    }
    println!();

    println!("=== Load Tools Example Complete ===");
}
