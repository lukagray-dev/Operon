/// Basic usage example for the grep tool.
///
/// This example demonstrates how to use the grep tool to search for patterns
/// in files with various configurations: basic search, case-insensitive search,
/// context lines, filename filtering, and error handling.
use operon_context_normalize_tools::ToolCallId;
use operon_tools_fs_grep::{definition, execute};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    println!("=== Operon Grep Tool Example ===\n");

    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();

    // Create a Rust source file
    let rust_file = temp_dir.path().join("example.rs");
    fs::write(
        &rust_file,
        "// Example Rust file\n\
         fn main() {\n\
             println!(\"Hello, World!\");\n\
         }\n\
         \n\
         fn helper_function() {\n\
             // TODO: Implement this\n\
             println!(\"Helper called\");\n\
         }\n\
         \n\
         // Another TODO: Add tests\n\
         fn test_function() {\n\
             assert_eq!(2 + 2, 4);\n\
         }\n",
    )
    .unwrap();

    // Create a text file
    let text_file = temp_dir.path().join("notes.txt");
    fs::write(
        &text_file,
        "Project Notes\n\
         =============\n\
         \n\
         ERROR: Fix the login bug\n\
         Warning: Check memory usage\n\
         ERROR: Update documentation\n\
         \n\
         All systems operational.\n",
    )
    .unwrap();

    // Create a TypeScript file
    let ts_file = temp_dir.path().join("app.ts");
    fs::write(
        &ts_file,
        "// TypeScript application\n\
         function main(): void {\n\
             console.log('Hello from TypeScript');\n\
         }\n\
         \n\
         // TODO: Add error handling\n\
         main();\n",
    )
    .unwrap();

    // Example 1: Get the tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // Example 2: Basic search for "TODO" in a single file
    println!("2. Basic search for 'TODO' in Rust file:");
    let args = json!({
        "pattern": "TODO",
        "paths": [rust_file.to_str().unwrap()]
    });
    let result = execute(ToolCallId("call_1".to_string()), args)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total matches: {}", output["total_matches"]);
        println!("   Files with matches: {}", output["files_with_matches"]);
        println!("   Truncated: {}", output["truncated"]);
        if let Some(files) = output["files"].as_array() {
            for file in files {
                println!("   File: {}", file["path"]);
                println!("   Match count: {}", file["match_count"]);
                if let Some(matches) = file["matches"].as_array() {
                    for m in matches {
                        println!(
                            "     Line {}: {} (match: {})",
                            m["line_no"], m["content"], m["is_match"]
                        );
                    }
                }
            }
        }
    }
    println!();

    // Example 3: Case-insensitive search with context lines
    println!("3. Case-insensitive search for 'error' with 1 line of context:");
    let args = json!({
        "pattern": "error",
        "paths": [text_file.to_str().unwrap()],
        "case_insensitive": true,
        "context_lines": 1
    });
    let result = execute(ToolCallId("call_2".to_string()), args)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total matches: {}", output["total_matches"]);
        if let Some(files) = output["files"].as_array() {
            for file in files {
                if let Some(matches) = file["matches"].as_array() {
                    for m in matches {
                        let marker = if m["is_match"].as_bool().unwrap() {
                            ">"
                        } else {
                            " "
                        };
                        println!("   {} Line {}: {}", marker, m["line_no"], m["content"]);
                    }
                }
            }
        }
    }
    println!();

    // Example 4: Search directory with filename filter
    println!("4. Search for 'TODO' in all Rust files:");
    let args = json!({
        "pattern": "TODO",
        "paths": [temp_dir.path().to_str().unwrap()],
        "include": "*.rs"
    });
    let result = execute(ToolCallId("call_3".to_string()), args)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total matches: {}", output["total_matches"]);
        println!("   Files with matches: {}", output["files_with_matches"]);
    }
    println!();

    // Example 5: Search multiple paths
    println!("5. Search for 'main' in multiple files:");
    let args = json!({
        "pattern": "main",
        "paths": [
            rust_file.to_str().unwrap(),
            ts_file.to_str().unwrap()
        ]
    });
    let result = execute(ToolCallId("call_4".to_string()), args)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total matches: {}", output["total_matches"]);
        println!("   Files with matches: {}", output["files_with_matches"]);
        if let Some(files) = output["files"].as_array() {
            for file in files {
                println!(
                    "   - {}: {} matches",
                    file["path"]
                        .as_str()
                        .unwrap()
                        .split('\\')
                        .next_back()
                        .unwrap_or(""),
                    file["match_count"]
                );
            }
        }
    }
    println!();

    // Example 6: Regex pattern with word boundaries
    println!("6. Search for 'main' as a whole word (using \\b):");
    let args = json!({
        "pattern": r"\bmain\b",
        "paths": [rust_file.to_str().unwrap()]
    });
    let result = execute(ToolCallId("call_5".to_string()), args)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total matches: {}", output["total_matches"]);
    }
    println!();

    // Example 7: Invalid regex pattern (error handling)
    println!("7. Handling invalid regex pattern:");
    let args = json!({
        "pattern": "[invalid",  // Unclosed bracket
        "paths": [rust_file.to_str().unwrap()]
    });
    let result = execute(ToolCallId("call_6".to_string()), args)
        .await
        .unwrap();

    println!("   Is error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Text(msg) = &result.content {
        println!("   Error message: {}", msg);
    }
    println!();

    // Example 8: Search for function definitions
    println!("8. Search for function definitions (fn keyword):");
    let args = json!({
        "pattern": r"^fn \w+",
        "paths": [rust_file.to_str().unwrap()]
    });
    let result = execute(ToolCallId("call_7".to_string()), args)
        .await
        .unwrap();

    if let operon_context_normalize_tools::ToolContent::Json(output) = &result.content {
        println!("   Total matches: {}", output["total_matches"]);
        if let Some(files) = output["files"].as_array() {
            for file in files {
                if let Some(matches) = file["matches"].as_array() {
                    for m in matches {
                        if m["is_match"].as_bool().unwrap() {
                            println!("   Line {}: {}", m["line_no"], m["content"]);
                        }
                    }
                }
            }
        }
    }
    println!();

    println!("=== Example Complete ===");
}
