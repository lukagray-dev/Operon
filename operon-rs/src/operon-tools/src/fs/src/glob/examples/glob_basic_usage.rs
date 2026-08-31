//! Basic usage example for the glob tool.
//!
//! Hey friend! This example demonstrates how to find files matching wildcard patterns using the glob tool.

use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_glob::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    println!("=== Operon Glob Tool Example ===\n");

    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a sample directory tree
    fs::create_dir_all(temp_path.join("src").join("controllers")).unwrap();
    fs::create_dir_all(temp_path.join("src").join("models")).unwrap();
    fs::create_dir_all(temp_path.join("tests")).unwrap();

    fs::write(temp_path.join("Cargo.toml"), "[package]").unwrap();
    fs::write(temp_path.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(temp_path.join("src/controllers/user.rs"), "// controller").unwrap();
    fs::write(temp_path.join("src/models/user.rs"), "// model").unwrap();
    fs::write(temp_path.join("tests/integration_test.rs"), "// test").unwrap();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 2. Find all rust files recursively
    println!("2. Finding all Rust files (**/*.rs):");
    let args = json!({
        "pattern": "**/*.rs",
        "path": temp_path.to_str().unwrap()
    });

    let result = execute(ToolCallId("call_glob_1".to_string()), args)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Text(text) = result.content {
        println!("{}", text);
    }
}

