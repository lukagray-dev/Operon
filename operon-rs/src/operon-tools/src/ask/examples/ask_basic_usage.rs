/// Basic usage example for the ask tool.
///
/// Hey friend! This example demonstrates the `ask` tool definition used by Operon agents
/// to pause execution and solicit user choices with interactive options.
use operon_tools_ask::definition;

fn main() {
    println!("=== Operon Ask Tool Example ===\n");

    // 1. Tool definition
    println!("1. Tool Definition:");
    let def = definition();
    println!("   Name: {}", def.name);
    println!("   Description: {}", def.description);
    println!();

    println!("2. Parameter JSON Schema:");
    println!(
        "{}",
        serde_json::to_string_pretty(&def.parameters).unwrap()
    );
    println!();

    println!("=== Ask Example Complete ===");
}
