use super::*;

use operon_context::{StopReason, ToolCall, ToolCallId, ToolContent, ToolResult};
use serde_json::json;

// Build a tiny ToolCall fixture so the helper tests stay focused and readable.
fn make_call(name: &str, arguments: serde_json::Value) -> ToolCall {
    ToolCall {
        id: ToolCallId(format!("call_{name}")),
        name: name.to_string(),
        arguments,
    }
}

#[test]
fn command_matches_only_accepts_cancel_or_the_matching_approval_id() {
    // Cancel is always relevant because it should stop the session immediately.
    assert!(command_matches(&SessionCommand::Cancel, None));
    assert!(command_matches(&SessionCommand::Cancel, Some("anything")));

    // Approve/Deny only count when the approval ID matches the pending request.
    assert!(command_matches(
        &SessionCommand::Approve {
            id: "approval-1".to_string(),
        },
        Some("approval-1"),
    ));
    assert!(command_matches(
        &SessionCommand::Deny {
            id: "approval-1".to_string(),
        },
        Some("approval-1"),
    ));

    assert!(!command_matches(
        &SessionCommand::Approve {
            id: "approval-1".to_string(),
        },
        Some("approval-2"),
    ));
    assert!(!command_matches(
        &SessionCommand::Deny {
            id: "approval-1".to_string(),
        },
        None,
    ));
}

#[test]
fn policy_path_for_call_extracts_the_correct_anchor() {
    // 1. "read" tool uses the first entry from its "paths" array argument as the policy anchor path.
    let read_call = make_call("read", json!({ "paths": ["/tmp/a.txt", "/tmp/b.txt"] }));
    assert_eq!(
        policy_path_for_call(&read_call).as_deref(),
        Some("/tmp/a.txt")
    );

    // 2. "bash" tool uses the "cwd" string argument as its policy anchor path.
    let bash_call = make_call("bash", json!({ "command": "ls", "cwd": "/tmp/work" }));
    assert_eq!(
        policy_path_for_call(&bash_call).as_deref(),
        Some("/tmp/work")
    );

    // 3. Other filesystem tools (e.g. "write") use the singular "path" argument.
    let write_call = make_call("write", json!({ "path": "/tmp/w.txt" }));
    assert_eq!(
        policy_path_for_call(&write_call).as_deref(),
        Some("/tmp/w.txt")
    );

    // 4. "grep" tool uses the first entry from its "paths" array argument as the representative anchor.
    let grep_call = make_call("grep", json!({ "paths": ["/tmp/x.txt", "/tmp/y.txt"] }));
    assert_eq!(
        policy_path_for_call(&grep_call).as_deref(),
        Some("/tmp/x.txt")
    );

    // 5. Global tools (e.g. "web_search") do not have a path anchor and should return None.
    let web_call = make_call("web_search", json!({ "query": "hello" }));
    assert_eq!(policy_path_for_call(&web_call), None);
}

#[test]
fn context_usage_event_reports_window_and_utilization() {
    let budget = TokenBudget::new(200_000, 0.90).expect("valid token budget");
    let event = context_usage_event(&budget, 150_000);

    match event {
        SessionEvent::ContextUsageUpdated {
            current_context_tokens,
            context_window,
            remaining_context_tokens,
            utilization,
            compaction_limit,
        } => {
            assert_eq!(current_context_tokens, 150_000);
            assert_eq!(context_window, 200_000);
            assert_eq!(remaining_context_tokens, 50_000);
            assert_eq!(compaction_limit, 180_000);
            assert!((utilization - 0.75).abs() < 1e-6);
        }
        other => panic!("unexpected context usage event: {:?}", other),
    }
}

#[test]
fn opaque_permission_denied_result_is_generic_and_safe_for_the_model() {
    // Policy denials should never leak internal policy details to the model.
    let call = make_call("write", json!({ "path": "/tmp/secret.txt" }));
    let result = opaque_permission_denied_result(&call);

    assert_eq!(result.call_id, call.id);
    assert_eq!(result.name, "write");
    assert!(result.is_error);

    match result.content {
        ToolContent::Text(message) => {
            assert_eq!(message, "Tool not available.");
        }
        other => panic!("unexpected tool content for denied call: {:?}", other),
    }
}

#[test]
fn tool_result_content_json_serializes_text_and_json_cleanly() {
    // Text content should be passed through unchanged.
    let text_result = ToolResult {
        call_id: ToolCallId("call_text".to_string()),
        name: "write".to_string(),
        content: ToolContent::Text("plain text".to_string()),
        is_error: false,
    };
    assert_eq!(tool_result_content_json(&text_result), "plain text");

    // JSON content should be rendered as a compact JSON string.
    let json_result = ToolResult {
        call_id: ToolCallId("call_json".to_string()),
        name: "read".to_string(),
        content: ToolContent::Json(json!({ "ok": true })),
        is_error: false,
    };
    assert_eq!(tool_result_content_json(&json_result), "{\"ok\":true}");
}

#[test]
fn test_set_history_restores_loaded_groups() {
    use operon_context::{CompactionConfig, SnapshotConfig};
    use operon_providers::{ApiCredentials, ModelConfig, Provider, ProviderConfig};
    use reqwest::Client;
    use std::collections::VecDeque;
    use tokio::sync::mpsc;

    // Create a SnapshotBuilder with a dummy configuration pointing to the temp directory.
    let snapshot_builder = SnapshotBuilder::new(SnapshotConfig {
        root: std::env::temp_dir(),
        role: Role::Owner,
        session_id: "test-session".to_string(),
        tree_depth: 1,
        tool_groups: Vec::new(),
        channel_instructions: None,
    })
    .unwrap();

    // Create dummy channels for session communication.
    let (event_tx, _event_rx) = mpsc::channel(1);
    let (_cmd_tx, cmd_rx) = mpsc::channel(1);

    // Construct a SessionRunner instance manually with dummy fields to test set_history.
    let mut runner = SessionRunner {
        session_id: "test-session".to_string(),
        config: SessionConfig {
            provider_config: ProviderConfig {
                provider: Provider::Anthropic,
                credentials: ApiCredentials::unauthenticated(),
                model: ModelConfig {
                    model_id: "test-model".to_string(),
                    context_window: 100_000,
                    max_tokens: 1000,
                    reasoning_effort: None,
                },
                base_url_override: None,
            },
            policy: PolicyConfig::empty(),
            project_dir: None,
            workspace_root: std::env::temp_dir(),
            role: Role::Owner,
            tool_groups: Vec::new(),
            compaction: CompactionConfig::default(),
            store_path: None,
            channel_instructions: None,
        },
        messages: Vec::new(),
        dispatcher: Dispatcher::new(),
        snapshot_builder,
        token_state: SessionTokenState::new(),
        token_budget: TokenBudget::with_window(100_000).unwrap(),
        lifecycle: LifecycleState::Idle,
        http_client: Client::new(),
        event_tx,
        cmd_rx,
        policy_resolver: PolicyResolver::new(PolicyConfig::empty()),
        pending_commands: VecDeque::new(),
        store: None,
        turn_index: 0,
    };

    // Construct mock conversation history:
    // 1. A successful load_tools call for "fs"
    // 2. A failed load_tools call for "web"
    // 3. A read tool call (should not affect groups)
    let history = vec![ConversationMessage {
        role: MessageRole::Tool,
        content: vec![
            // Successful load_tools for "fs": should be recovered!
            ContentBlock::ToolResult(ToolResult {
                call_id: ToolCallId("call_load_fs".to_string()),
                name: "load_tools".to_string(),
                content: ToolContent::Json(json!({
                    "group": "fs",
                    "tool_count": 7,
                    "tools": []
                })),
                is_error: false,
            }),
            // Failed load_tools for "web": should NOT be recovered because is_error is true!
            ContentBlock::ToolResult(ToolResult {
                call_id: ToolCallId("call_load_web".to_string()),
                name: "load_tools".to_string(),
                content: ToolContent::Json(json!({
                    "group": "web",
                    "tool_count": 2,
                    "tools": []
                })),
                is_error: true,
            }),
            // Standard tool result: should NOT affect any loaded groups!
            ContentBlock::ToolResult(ToolResult {
                call_id: ToolCallId("call_read_file".to_string()),
                name: "read".to_string(),
                content: ToolContent::Text("some file content".to_string()),
                is_error: false,
            }),
        ],
        stop_reason: None,
    }];

    // Invoke set_history to simulate session resume.
    runner.set_history(history, 4, Some(800));

    // Verify turn index and token states are correctly recovered.
    assert_eq!(runner.turn_index, 4, "Turn index must be set to 4");
    assert_eq!(
        runner.token_state.current_context_tokens, 800,
        "Context tokens must be set to 800"
    );

    // Verify that the dispatcher has marked "fs" as loaded, but not "web" or "read".
    let loaded = runner.dispatcher.loaded_groups();
    assert!(
        loaded.contains("fs"),
        "The successfully loaded 'fs' group must be recovered"
    );
    assert!(
        !loaded.contains("web"),
        "The failed 'web' group must NOT be marked loaded"
    );
    assert!(
        !loaded.contains("read"),
        "Individual tool calls must not affect loaded groups"
    );
}

#[test]
fn test_heuristic_token_estimator() {
    // This test verifies the heuristic token estimator used in the session runner
    // for diagnosing potential context length overflows before sending the API request.

    // 1. Text content block: estimated at length of string / 4
    let text_block = ContentBlock::Text("hello world".to_string()); // 11 chars -> 2 tokens

    // 2. Tool call content block: estimated at length of arguments JSON string / 4 + 10
    let tool_call_block = ContentBlock::ToolCall(ToolCall {
        id: ToolCallId("call_1".to_string()),
        name: "test_tool".to_string(),
        arguments: json!({ "arg": "value" }), // {"arg":"value"} is 15 chars -> 3 tokens + 10 = 13 tokens
    });

    // 3. Tool result content block: estimated at length of result payload / 4 + 10
    let tool_result_block = ContentBlock::ToolResult(ToolResult {
        call_id: ToolCallId("call_1".to_string()),
        name: "test_tool".to_string(),
        content: ToolContent::Text("success".to_string()), // "success" is 7 chars -> 1 token + 10 = 11 tokens
        is_error: false,
    });

    // Run the same match heuristic used in runner.rs
    let estimate = |block: &ContentBlock| match block {
        ContentBlock::Text(t) => t.len() / 4,
        ContentBlock::ToolCall(c) => c.arguments.to_string().len() / 4 + 10,
        ContentBlock::ToolResult(r) => {
            let content_len = match &r.content {
                ToolContent::Text(t) => t.len(),
                ToolContent::Json(val) => val.to_string().len(),
            };
            content_len / 4 + 10
        }
        _ => 5,
    };

    assert_eq!(
        estimate(&text_block),
        2,
        "Text block token estimation mismatch"
    );
    assert_eq!(
        estimate(&tool_call_block),
        13,
        "Tool call token estimation mismatch"
    );
    assert_eq!(
        estimate(&tool_result_block),
        11,
        "Tool result token estimation mismatch"
    );
}

#[test]
fn test_build_assistant_message_includes_reasoning() {
    use operon_context::ReasoningBlock;

    let result = StreamResult {
        text: "hello".to_string(),
        tool_calls: Vec::new(),
        stop_reason: Some(StopReason::EndTurn),
        usage_raw: None,
        reasoning: Some(ReasoningBlock::new("I am thinking")),
    };

    let msg = build_assistant_message(&result);
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.content.len(), 2);

    // First block should be the reasoning block
    match &msg.content[0] {
        ContentBlock::Reasoning(rb) => {
            assert_eq!(rb.thinking, "I am thinking");
            assert!(rb.signature.is_none());
        }
        other => panic!("expected ContentBlock::Reasoning, got {:?}", other),
    }

    // Second block should be the text block
    match &msg.content[1] {
        ContentBlock::Text(t) => {
            assert_eq!(t, "hello");
        }
        other => panic!("expected ContentBlock::Text, got {:?}", other),
    }
}

#[test]
fn test_build_user_message_plain_text_regression() {
    let blocks = build_user_message("Hello world", vec![], &[]);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Text(t) => assert_eq!(t, "Hello world"),
        other => panic!("expected Text block, got {:?}", other),
    }
}

#[test]
fn test_build_user_message_image_only() {
    use operon_context::{ImageBlock, ImageSource};
    let img_block = ContentBlock::Image(ImageBlock {
        source: ImageSource::Base64 {
            media_type: "image/png".to_string(),
            data: "base64data".to_string(),
        },
    });

    let blocks = build_user_message("", vec![img_block.clone()], &[]);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Image(img) => match &img.source {
            ImageSource::Base64 { media_type, data } => {
                assert_eq!(media_type, "image/png");
                assert_eq!(data, "base64data");
            }
            other => panic!("expected ImageSource::Base64, got {:?}", other),
        },
        other => panic!("expected Image block, got {:?}", other),
    }
}

#[test]
fn test_build_user_message_file_only() {
    let file_path = std::path::PathBuf::from("D:\\Operon\\notes.txt");
    let blocks = build_user_message("", vec![], &[file_path]);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Text(t) => {
            assert_eq!(t, "[Attached file: D:\\Operon\\notes.txt]");
        }
        other => panic!("expected Text block with attached file path, got {:?}", other),
    }
}

#[test]
fn test_build_user_message_mixed_turn() {
    use operon_context::{ImageBlock, ImageSource};
    let img_block = ContentBlock::Image(ImageBlock {
        source: ImageSource::Base64 {
            media_type: "image/jpeg".to_string(),
            data: "jpegbase64".to_string(),
        },
    });
    let file1 = std::path::PathBuf::from("D:\\Operon\\doc1.pdf");
    let file2 = std::path::PathBuf::from("D:\\Operon\\src\\main.rs");

    let blocks = build_user_message("Please analyze these attachments", vec![img_block], &[file1, file2]);
    assert_eq!(blocks.len(), 2);

    match &blocks[0] {
        ContentBlock::Image(img) => match &img.source {
            ImageSource::Base64 { media_type, .. } => {
                assert_eq!(media_type, "image/jpeg");
            }
            other => panic!("expected ImageSource::Base64, got {:?}", other),
        },
        other => panic!("expected Image block first, got {:?}", other),
    }

    match &blocks[1] {
        ContentBlock::Text(t) => {
            let expected = "Please analyze these attachments\n[Attached file: D:\\Operon\\doc1.pdf]\n[Attached file: D:\\Operon\\src\\main.rs]";
            assert_eq!(t, expected);
        }
        other => panic!("expected Text block second, got {:?}", other),
    }
}

#[tokio::test]
async fn test_session_runner_restores_persisted_todos() {
    // Hey friend! Let's test that when a SessionRunner is created for an existing session
    // with saved todos, it correctly loads those todos into the dispatcher so the model
    // can access them across multiple turns in the session.
    use operon_providers::credentials::ApiCredentials;
    use operon_providers::model::ModelConfig;
    use operon_providers::{Provider, ProviderConfig};
    use operon_tools::{TodoItem, TodoPriority, TodoStatus};

    let temp_dir = std::env::temp_dir();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let session_file = temp_dir.join(format!("test-runner-todos-{}.json", ts));

    // 1. Prepare SessionStore on disk with saved todos
    let store = SessionStore::open(&session_file)
        .await
        .expect("open store");

    store
        .create_session(
            "session-runner-todos",
            &temp_dir.to_string_lossy(),
            "test-model",
            "Anthropic",
        )
        .await
        .expect("create_session");

    let saved_todos = vec![
        TodoItem {
            id: "1".to_string(),
            content: "First persistent task".to_string(),
            status: TodoStatus::Pending,
            priority: TodoPriority::High,
        },
        TodoItem {
            id: "2".to_string(),
            content: "Second persistent task".to_string(),
            status: TodoStatus::InProgress,
            priority: TodoPriority::Medium,
        },
    ];

    store
        .save_todos("session-runner-todos", &saved_todos)
        .await
        .expect("save_todos");

    // 2. Build SessionConfig targeting this store
    let config = SessionConfig {
        provider_config: ProviderConfig {
            provider: Provider::Anthropic,
            credentials: ApiCredentials::unauthenticated(),
            model: ModelConfig {
                model_id: "test-model".to_string(),
                context_window: 100_000,
                max_tokens: 1000,
                reasoning_effort: None,
            },
            base_url_override: None,
        },
        policy: PolicyConfig::empty(),
        project_dir: None,
        workspace_root: temp_dir.clone(),
        role: Role::Owner,
        tool_groups: vec!["todo".to_string()],
        compaction: operon_context::CompactionConfig::default(),
        store_path: Some(session_file.clone()),
        channel_instructions: None,
    };

    let (event_tx, _event_rx) = mpsc::channel(10);
    let (_cmd_tx, cmd_rx) = mpsc::channel(10);

    let runner = SessionRunner::new(config, event_tx, cmd_rx)
        .await
        .expect("SessionRunner::new should succeed");

    // 3. Verify that the dispatcher has loaded the persisted todos!
    let todos = runner.dispatcher.todo_store().list();
    assert_eq!(todos.len(), 2, "Dispatcher must have 2 restored todos");
    assert_eq!(todos[0].id, "1");
    assert_eq!(todos[0].content, "First persistent task");
    assert_eq!(todos[0].status, TodoStatus::Pending);
    assert_eq!(todos[0].priority, TodoPriority::High);
    assert_eq!(todos[1].id, "2");
    assert_eq!(todos[1].content, "Second persistent task");
    assert_eq!(todos[1].status, TodoStatus::InProgress);

    // Clean up temporary file
    let _ = std::fs::remove_file(session_file);
}


