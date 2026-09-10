// tests.rs — Unit tests for `spawn_agent` argument parsing, schemas, and outputs.
//
// Hey friend! Writing clean, robust unit tests ensures that our tool definitions and
// schema validations behave deterministically under real-world conditions.

use serde_json::json;

use super::*;

#[test]
fn test_definition_schema_contains_required_fields() {
    let def = definition();
    assert_eq!(def.name, "spawn_agent");

    let parameters = def.parameters;
    assert_eq!(parameters["type"], "object");
    let required = parameters["required"].as_array().expect("required fields array");
    assert!(
        required.iter().any(|v| v == "task"),
        "task must be a required field"
    );

    let props = &parameters["properties"];
    assert!(props.get("task").is_some());
    assert!(props.get("role").is_some());
    assert!(props.get("description").is_some());
}

#[test]
fn test_parse_args_minimal_required() {
    let payload = json!({
        "task": "Investigate authentication error in auth.rs"
    });

    let args = SpawnAgentArgs::from_json(&payload).expect("should parse successfully");
    assert_eq!(args.task, "Investigate authentication error in auth.rs");
    assert_eq!(args.role, "general");
    assert_eq!(args.description, None);
}

#[test]
fn test_parse_args_full_fields() {
    let payload = json!({
        "task": "Review pull request #42 for potential security leaks",
        "role": "security_auditor",
        "description": "Auditing PR 42 for sensitive token leaks"
    });

    let args = SpawnAgentArgs::from_json(&payload).expect("should parse successfully");
    assert_eq!(args.task, "Review pull request #42 for potential security leaks");
    assert_eq!(args.role, "security_auditor");
    assert_eq!(
        args.description.as_deref(),
        Some("Auditing PR 42 for sensitive token leaks")
    );
}

#[test]
fn test_parse_args_missing_task_fails() {
    let payload = json!({
        "role": "tester"
    });

    let err = SpawnAgentArgs::from_json(&payload).unwrap_err();
    assert!(matches!(err, SpawnAgentToolError::ArgsParse(_)));
}

#[test]
fn test_output_formatting() {
    let output = SpawnAgentOutput::completed(
        "sess_sub_123",
        "code_reviewer",
        "Found 2 bugs in the regex matcher.",
    );

    assert_eq!(output.status, "completed");
    assert_eq!(output.role, "code_reviewer");
    let formatted = output.format_for_parent();
    assert!(formatted.contains("Subagent [code_reviewer] (completed)"));
    assert!(formatted.contains("Found 2 bugs"));
}

