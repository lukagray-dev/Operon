/// Basic usage example for the append tool.
///
/// Hey friend! This example demonstrates how to use the `append` tool to add text
/// to the end of an existing file without modifying or re-writing the existing content.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_append::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("=== Operon Append Tool Example ===\n");

    // 1. Create a file with initial log entries
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(&path, "[2026-08-21 10:00:00] Service started\n").unwrap();

    // 2. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 3. Append a new line to the file
    println!("2. Appending a log line:");
    let args = json!({
        "path": path,
        "content": "[2026-08-21 10:01:00] User logged in successfully\n"
    });

    let result = execute(ToolCallId("call_append_1".to_string()), args)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Text(msg) = &result.content {
        println!("   Message: {}", msg);
    }
    println!();

    println!("3. Resulting File Content on Disk:");
    println!("{}", fs::read_to_string(&path).unwrap());

    println!("=== Append Example Complete ===");
}
