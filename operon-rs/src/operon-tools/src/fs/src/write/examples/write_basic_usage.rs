/// Basic usage example for the write tool.
///
/// Hey friend! This example demonstrates how to use the `write` tool to create new files,
/// automatically generate intermediate parent directories, overwrite existing files atomically,
/// and handle input errors.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_write::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    println!("=== Operon Write Tool Example ===\n");

    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Create a new file directly
    let file1 = temp_path.join("hello.txt");
    println!("2. Creating a new file:");
    let args1 = json!({
        "path": file1.to_str().unwrap(),
        "content": "Hello, Operon World!"
    });
    let result1 = execute(ToolCallId("call_write_1".to_string()), args1)
        .await
        .unwrap();

    println!("   Is Error: {}", result1.is_error);
    if let operon_context_normalize_tools::ToolContent::Text(msg) = &result1.content {
        println!("   Message: {}", msg);
    }
    println!("   File content on disk: {:?}", fs::read_to_string(&file1).unwrap());
    println!();

    // 3. Auto-creating deep intermediate directories
    let nested_file = temp_path.join("deep").join("nested").join("config.json");
    println!("3. Auto-creating nested folders during write:");
    let args2 = json!({
        "path": nested_file.to_str().unwrap(),
        "content": "{\n  \"status\": \"ready\"\n}"
    });
    let result2 = execute(ToolCallId("call_write_2".to_string()), args2)
        .await
        .unwrap();

    println!("   Is Error: {}", result2.is_error);
    if let operon_context_normalize_tools::ToolContent::Text(msg) = &result2.content {
        println!("   Message: {}", msg);
    }
    println!("   Nested file exists: {}", nested_file.exists());
    println!();

    // 4. Overwriting an existing file
    println!("4. Overwriting an existing file:");
    let args3 = json!({
        "path": file1.to_str().unwrap(),
        "content": "Updated content after overwrite!"
    });
    let result3 = execute(ToolCallId("call_write_3".to_string()), args3)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Text(msg) = &result3.content {
        println!("   Message: {}", msg);
    }
    println!("   New file content on disk: {:?}", fs::read_to_string(&file1).unwrap());
    println!();

    println!("=== Write Example Complete ===");
}
