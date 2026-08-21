/// Basic usage example for the ls tool.
///
/// Hey friend! This example demonstrates how to use the `ls` tool to list directory
/// contents with various configurations: basic directory listing, glob pattern filtering
/// (ignoring build artifacts like node_modules or target), and error handling for non-existent paths.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_ls::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    println!("=== Operon LS Tool Example ===\n");

    // 1. Create a temporary directory structure for our demonstration
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create some sample files and subdirectories
    fs::create_dir_all(temp_path.join("src")).unwrap();
    fs::create_dir_all(temp_path.join("target")).unwrap();
    fs::create_dir_all(temp_path.join("node_modules")).unwrap();
    fs::write(temp_path.join("src").join("main.rs"), "fn main() {}").unwrap();
    fs::write(temp_path.join("src").join("lib.rs"), "// library").unwrap();
    fs::write(temp_path.join("Cargo.toml"), "[package]\nname = \"demo\"").unwrap();
    fs::write(temp_path.join("Cargo.lock"), "# lockfile").unwrap();

    // 2. Display the tool definition provided to LLMs
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name());
    println!("   Short Description: {}", def.short.description);
    println!();

    // 3. Example 1: Basic directory listing of the root temp folder
    println!("2. Basic Directory Listing:");
    let args = json!({
        "path": temp_path.to_str().unwrap()
    });
    let result = execute(ToolCallId("call_ls_1".to_string()), args)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Text(content) = &result.content {
        println!("   Output:\n{}", content);
    }
    println!();

    // 4. Example 2: Directory listing with glob ignore filters
    println!("3. Listing with Glob Ignore Patterns (*.lock, target, node_modules):");
    let args_filtered = json!({
        "path": temp_path.to_str().unwrap(),
        "ignore": ["*.lock", "target", "node_modules"]
    });
    let result_filtered = execute(ToolCallId("call_ls_2".to_string()), args_filtered)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Text(content) = &result_filtered.content {
        println!("   Output:\n{}", content);
    }
    println!();

    // 5. Example 3: Listing a specific subdirectory (src)
    let src_path = temp_path.join("src");
    println!("4. Listing Subdirectory (src/):");
    let args_src = json!({
        "path": src_path.to_str().unwrap()
    });
    let result_src = execute(ToolCallId("call_ls_3".to_string()), args_src)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Text(content) = &result_src.content {
        println!("   Output:\n{}", content);
    }
    println!();

    println!("=== LS Example Complete ===");
}
