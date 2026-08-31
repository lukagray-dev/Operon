/// Basic usage example for the edit tool.
///
/// Hey friend! This example demonstrates how to use the `edit` tool to apply targeted
/// hunk replacements (`old_string` -> `new_string`) against existing files with fuzzy matching
/// and atomic file updates.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_edit::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::NamedTempFile;

#[tokio::main]
async fn main() {
    println!("=== Operon Edit Tool Example ===\n");

    // 1. Create a sample code file
    let file = NamedTempFile::new().unwrap();
    let path = file.path().to_string_lossy().to_string();
    fs::write(
        &path,
        "function calculateTotal(subtotal, tax) {\n    return subtotal + tax;\n}\n",
    )
    .unwrap();

    // 2. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 3. Perform a single-hunk edit
    println!("2. Performing a single-hunk edit:");
    let args = json!({
        "path": path,
        "edits": [
            {
                "old_string": "function calculateTotal(subtotal, tax) {",
                "new_string": "function calculateTotal(subtotal: number, tax: number): number {"
            }
        ]
    });

    let result = execute(ToolCallId("call_edit_1".to_string()), args)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Applied Hunks: {}", output["hunks_applied"]);
        println!("   Message: {}", output["message"]);
    }
    println!();

    println!("3. Modified File Content:");
    println!("{}", fs::read_to_string(&path).unwrap());

    println!("=== Edit Example Complete ===");
}
