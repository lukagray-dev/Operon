/// Basic usage example for the bash tool.
///
/// Hey friend! This example demonstrates how to use the `bash` tool to execute
/// shell commands in an isolated subprocess with an explicit absolute working directory (`cwd`).
use operon_context_normalize_tools::ToolCallId;
use operon_tools_shell_bash::{definition, execute};
use serde_json::json;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    println!("=== Operon Bash Tool Example ===\n");

    let temp_dir = TempDir::new().unwrap();
    let cwd_path = temp_dir.path().to_string_lossy().to_string();

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    // 2. Execute a cross-platform command
    println!("2. Running echo command:");
    #[cfg(windows)]
    let command = "echo Hello from Operon Windows Shell";
    #[cfg(not(windows))]
    let command = "echo 'Hello from Operon Unix Shell'";

    let args = json!({
        "command": command,
        "cwd": cwd_path,
        "timeout_ms": 5000
    });

    let result = execute(ToolCallId("call_bash_1".to_string()), args)
        .await
        .unwrap();

    println!("   Is Error: {}", result.is_error);
    if let operon_context_normalize_tools::ToolContent::Text(output) = &result.content {
        println!("   Command Output:\n{}", output);
    }
    println!();

    println!("=== Bash Example Complete ===");
}
