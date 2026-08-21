/// Basic usage example for the delete tool.
///
/// Hey friend! This example demonstrates how to use the `delete` tool to safely
/// remove files or directories with either system trash recovery (default) or permanent deletion.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_delete::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    println!("=== Operon Delete Tool Example ===\n");

    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 2. Create a temporary file to delete permanently
    let file_to_delete = temp_path.join("unwanted.tmp");
    fs::write(&file_to_delete, "temporary content").unwrap();
    println!(
        "2. File exists before deletion: {}",
        file_to_delete.exists()
    );

    // 3. Delete the file permanently
    let args = json!({
        "path": file_to_delete.to_str().unwrap(),
        "permanent": true
    });

    let result = execute(ToolCallId("call_del_1".to_string()), args)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Text(msg) = &result.content {
        println!("   Message: {}", msg);
    }
    println!("   File exists after deletion: {}", file_to_delete.exists());
    println!();

    println!("=== Delete Example Complete ===");
}
