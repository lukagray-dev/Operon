/// Basic usage example for the read tool.
///
/// This example demonstrates how to use the read tool to read files with
/// various configurations: full-file reads, line ranges, batch multi-file reads, and error handling.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_read::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    println!("=== Operon Read Tool Example ===\n");

    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("example.txt");
    fs::write(
        &test_file,
        "Line 1: Hello, World!\n\
         Line 2: This is a test file.\n\
         Line 3: It has multiple lines.\n\
         Line 4: We can read them all.\n\
         Line 5: Or just a subset.\n",
    )
    .unwrap();

    // Example 1: Get the tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // Example 2: Read entire file
    println!("2. Reading entire file:");
    let args = json!({
        "path": test_file.to_str().unwrap()
    });
    let result = execute(ToolCallId("call_1".to_string()), args)
        .await
        .unwrap();
    println!("   Result: {:?}", result);
    println!();

    // Example 3: Read with line range
    println!("3. Reading lines 2-4:");
    let target_range = format!("{}:2-4", test_file.to_str().unwrap());
    let args = json!({
        "path": target_range
    });
    let result = execute(ToolCallId("call_2".to_string()), args)
        .await
        .unwrap();
    println!("   Result: {:?}", result);
    println!();

    // Example 4: Read multiple files
    let test_file2 = temp_dir.path().join("example2.txt");
    fs::write(&test_file2, "Another file\nWith different content\n").unwrap();

    println!("4. Reading multiple files:");
    let args = json!({
        "paths": [
            test_file.to_str().unwrap(),
            test_file2.to_str().unwrap()
        ]
    });
    let result = execute(ToolCallId("call_3".to_string()), args)
        .await
        .unwrap();
    println!("   Result: {:?}", result);
    println!();

    // Example 5: Error handling (nonexistent file)
    println!("5. Handling nonexistent file:");
    let args = json!({
        "paths": ["C:\\nonexistent\\file.txt"]
    });
    let result = execute(ToolCallId("call_4".to_string()), args)
        .await
        .unwrap();
    println!("   Result: {:?}", result);
    println!();

    println!("=== Example Complete ===");
}
