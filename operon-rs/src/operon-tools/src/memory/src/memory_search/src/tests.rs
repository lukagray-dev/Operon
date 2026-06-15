//! Tests for the memory_search tool.

use super::*;
use operon_context_normalize::tools::{ToolCallId, ToolContent};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

// An atomic counter to ensure unique SQLite file names for parallel test executions.
static DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Helper function to generate a unique, isolated SQLite database file path
/// for each individual test to run completely in parallel without racing.
fn setup_test_db(test_name: &str) -> PathBuf {
    let count = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_path = std::env::temp_dir().join(format!("operon_test_search_{}_{}.db", test_name, count));
    
    // Set this path in the shared thread-local storage of the memory crate.
    operon_tools_memory::set_test_db_path(db_path.clone());
    
    db_path
}

/// Helper function to clean up the unique database file and thread-local state after a test completes.
fn cleanup_test_db(db_path: PathBuf) {
    // Clear the thread-local override so subsequent tests on the same thread are unaffected.
    operon_tools_memory::clear_test_db_path();
    
    // Attempt to remove the temporary database file from disk.
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test(flavor = "current_thread")]
async fn test_memory_search_success() {
    // Setup isolated SQLite database.
    let db_path = setup_test_db("test_memory_search_success");

    let call_id = ToolCallId("test_call".to_string());
    
    // Connect to the isolated DB and insert two memories.
    let mut conn = operon_tools_memory::connect_db().await.unwrap();
    sqlx::query("INSERT INTO memories (content) VALUES ('this is some useful info about a project api key')")
        .execute(&mut conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO memories (content) VALUES ('another entry about testing database settings')")
        .execute(&mut conn)
        .await
        .unwrap();

    // Query for "api key"
    let args = json!({
        "query": "api key"
    });

    let result = execute(call_id.clone(), args).await.expect("execution failed");
    assert!(!result.is_error);
    
    let ToolContent::Text(text) = result.content;
    assert!(text.contains("Found 1 matching memory/memories:"));
    assert!(text.contains("project api key"));

    // Query for "entry"
    let args2 = json!({
        "query": "entry"
    });

    let result2 = execute(call_id, args2).await.expect("execution failed");
    assert!(!result2.is_error);
    
    let ToolContent::Text(text2) = result2.content;
    assert!(text2.contains("testing database settings"));

    // Cleanup the database file.
    cleanup_test_db(db_path);
}

#[tokio::test(flavor = "current_thread")]
async fn test_memory_search_no_match() {
    // Setup isolated SQLite database.
    let db_path = setup_test_db("test_memory_search_no_match");

    let call_id = ToolCallId("test_call".to_string());
    let args = json!({
        "query": "non_existent_token"
    });

    let result = execute(call_id, args).await.expect("execution failed");
    assert!(!result.is_error);
    
    let ToolContent::Text(text) = result.content;
    assert!(text.contains("No memories found matching query"));

    // Cleanup the database file.
    cleanup_test_db(db_path);
}

#[tokio::test(flavor = "current_thread")]
async fn test_memory_search_all_memories() {
    // Setup isolated SQLite database.
    let db_path = setup_test_db("test_memory_search_all_memories");

    let call_id = ToolCallId("test_call".to_string());

    // Connect and populate the isolated DB with test memories.
    let mut conn = operon_tools_memory::connect_db().await.unwrap();
    sqlx::query("INSERT INTO memories (content) VALUES ('Memory one')")
        .execute(&mut conn)
        .await
        .unwrap();
    sqlx::query("INSERT INTO memories (content) VALUES ('Memory two')")
        .execute(&mut conn)
        .await
        .unwrap();

    // Query with an empty query string to retrieve all memories.
    let args = json!({
        "query": ""
    });

    let result = execute(call_id.clone(), args).await.expect("execution failed");
    assert!(!result.is_error);
    
    let ToolContent::Text(text) = result.content;
    assert!(text.contains("All stored memories (2 total):"));
    assert!(text.contains("Memory one"));
    assert!(text.contains("Memory two"));

    // Also test with omitted query attribute (equivalent to empty string).
    let args_omitted = json!({});
    let result_omitted = execute(call_id, args_omitted).await.expect("execution failed");
    assert!(!result_omitted.is_error);
    
    let ToolContent::Text(text_omitted) = result_omitted.content;
    assert!(text_omitted.contains("All stored memories (2 total):"));

    // Cleanup the database file.
    cleanup_test_db(db_path);
}

#[tokio::test(flavor = "current_thread")]
async fn test_memory_search_empty_db_all_memories() {
    // Setup isolated SQLite database.
    let db_path = setup_test_db("test_memory_search_empty_db_all_memories");

    let call_id = ToolCallId("test_call".to_string());

    // Connect to initialize the schema. Since it's a unique database file, it starts empty.
    let _conn = operon_tools_memory::connect_db().await.unwrap();

    let args = json!({
        "query": ""
    });

    let result = execute(call_id, args).await.expect("execution failed");
    assert!(!result.is_error);
    
    let ToolContent::Text(text) = result.content;
    assert!(text.contains("No memories stored yet."));

    // Cleanup the database file.
    cleanup_test_db(db_path);
}


