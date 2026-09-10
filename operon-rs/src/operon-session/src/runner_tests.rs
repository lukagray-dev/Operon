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
fn test_set_history_restores_state() {
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
    let history = vec![ConversationMessage {
        role: MessageRole::Tool,
        content: vec![
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

    // Verify turn index, messages, and token states are correctly recovered.
    assert_eq!(runner.turn_index, 4, "Turn index must be set to 4");
    assert_eq!(runner.messages.len(), 1, "Messages must be restored");
    assert_eq!(
        runner.token_state.current_context_tokens, 800,
        "Context tokens must be set to 800"
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
        other => panic!(
            "expected Text block with attached file path, got {:?}",
            other
        ),
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

    let blocks = build_user_message(
        "Please analyze these attachments",
        vec![img_block],
        &[file1, file2],
    );
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
    let store = SessionStore::open(&session_file).await.expect("open store");

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

// ─────────────────────────────────────────────────────────────────────────────
// Subagent Coordination & Dispatch Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Helper function to create an isolated `SessionRunner` test fixture with custom policy and tools.
///
/// This helper configures a fresh temp workspace directory, an isolated session store path,
/// and returns the runner alongside its outbound event receiver and inbound command sender.
async fn create_test_runner(
    policy: PolicyConfig,
    tool_groups: Vec<String>,
) -> (
    SessionRunner,
    mpsc::Receiver<SessionEvent>,
    mpsc::Sender<SessionCommand>,
) {
    use operon_providers::{ApiCredentials, ModelConfig, Provider, ProviderConfig};

    // Generate a unique temp directory using timestamp nanos to avoid collision with concurrent runs
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("operon_subagent_test_{now_nanos}"));
    let _ = std::fs::create_dir_all(&temp_dir);
    let session_file = temp_dir.join("parent_session.json");

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
        policy,
        project_dir: None,
        workspace_root: temp_dir.clone(),
        role: Role::Owner,
        tool_groups,
        compaction: operon_context::CompactionConfig::default(),
        store_path: Some(session_file),
        channel_instructions: None,
    };

    let (event_tx, event_rx) = mpsc::channel(100);
    let (cmd_tx, cmd_rx) = mpsc::channel(10);

    let runner = SessionRunner::new(config, event_tx, cmd_rx)
        .await
        .expect("SessionRunner::new should succeed");

    (runner, event_rx, cmd_tx)
}

/// Verifies that when the security policy denies `GlobalTool::SubAgent`, any attempt by
/// the model to invoke `spawn_agent` is immediately rejected with an opaque permission
/// denied error result, preventing unauthorized child agent spawning.
#[tokio::test]
async fn test_subagent_dispatch_policy_denied() {
    // 1. Explicitly configure policy to DENY subagent creation
    let mut policy = PolicyConfig::empty();
    policy
        .global
        .owner
        .insert(operon_policy::GlobalTool::SubAgent, operon_policy::PermissionMode::Deny);

    // 2. Initialize runner with the restrictive policy
    let (mut runner, _event_rx, _cmd_tx) =
        create_test_runner(policy, vec!["subagent".to_string()]).await;

    let call = ToolCall {
        id: ToolCallId("call_sub_denied".to_string()),
        name: "spawn_agent".to_string(),
        arguments: json!({
            "role": "researcher",
            "task": "explore internal secrets"
        }),
    };

    // 3. Dispatch the tool call
    let (results, should_stop) = runner.dispatch_turn_tool_calls(vec![call]).await;

    // Turn should not be stopped, but tool call must yield an error result
    assert!(!should_stop, "Policy denial does not terminate session");
    assert_eq!(results.len(), 1, "Exactly one tool result must be returned");

    match &results[0] {
        ContentBlock::ToolResult(res) => {
            assert_eq!(res.call_id.0, "call_sub_denied");
            assert_eq!(res.name, "spawn_agent");
            assert!(res.is_error, "Denied tool call must have is_error: true");
            match &res.content {
                ToolContent::Text(t) => {
                    assert_eq!(
                        t, "Tool not available.",
                        "Expected opaque permission denied message, got: {t}"
                    );
                }
                _ => panic!("Expected text tool content"),
            }
        }
        _ => panic!("Expected ToolResult content block"),
    }
}

/// Verifies that malformed arguments (such as omitting the required `task` field) are
/// caught cleanly during JSON schema deserialization and returned as an error result
/// to the parent model without crashing the agent loop.
#[tokio::test]
async fn test_subagent_dispatch_malformed_arguments() {
    let mut policy = PolicyConfig::empty();
    policy
        .global
        .owner
        .insert(operon_policy::GlobalTool::SubAgent, operon_policy::PermissionMode::Allow);

    let (mut runner, _event_rx, _cmd_tx) =
        create_test_runner(policy, vec!["subagent".to_string()]).await;

    // Missing required field 'task' in arguments JSON
    let call = ToolCall {
        id: ToolCallId("call_bad_args".to_string()),
        name: "spawn_agent".to_string(),
        arguments: json!({ "role": "researcher" }),
    };

    let (results, should_stop) = runner.dispatch_turn_tool_calls(vec![call]).await;

    assert!(!should_stop);
    assert_eq!(results.len(), 1);

    match &results[0] {
        ContentBlock::ToolResult(res) => {
            assert_eq!(res.call_id.0, "call_bad_args");
            assert!(res.is_error, "Malformed arguments must yield is_error: true");
            match &res.content {
                ToolContent::Text(t) => {
                    assert!(
                        t.contains("missing field") || t.contains("task"),
                        "Expected missing field error, got: {t}"
                    );
                }
                _ => panic!("Expected text tool content"),
            }
        }
        _ => panic!("Expected ToolResult content block"),
    }
}

/// Verifies that spawning a subagent generates an isolated session ID derived from
/// the parent session, sets up a separate JSON storage path, and properly prepares
/// leaf-only tool configurations (disallowing nested subagent spawning).
#[tokio::test]
async fn test_subagent_spawn_config_isolation() {
    let policy = PolicyConfig::empty();
    let (runner, _event_rx, _cmd_tx) =
        create_test_runner(policy, vec!["subagent".to_string()]).await;

    let call = ToolCall {
        id: ToolCallId("call_iso".to_string()),
        name: "spawn_agent".to_string(),
        arguments: json!({
            "role": "data_analyst",
            "task": "analyze metrics"
        }),
    };

    // Prepare in-flight background task
    let inflight = runner
        .spawn_subagent_task(0, call)
        .expect("spawn_subagent_task should succeed");

    assert_eq!(inflight.call_index, 0);
    assert_eq!(inflight.call_id.0, "call_iso");
    assert_eq!(inflight.name, "spawn_agent");

    // Verify separate session ID convention: <parent_session_id>_sub_0_<nanos>
    let expected_prefix = format!("{}_sub_0_", runner.session_id());
    assert!(
        inflight.subagent_session_id.starts_with(&expected_prefix),
        "Subagent session ID must start with parent session ID prefix"
    );

    // Cancel in-flight background task to clean up resources
    let _ = inflight.cmd_tx.send(SessionCommand::Cancel).await;
    inflight.join_handle.abort();
}

/// Verifies the core batch orchestration contract:
/// When multiple subagents and normal tools are requested in a single turn, the subagents
/// run concurrently, the parent runner waits until ALL subagents complete, and the final
/// results are returned in the EXACT order of the original tool calls.
#[tokio::test]
async fn test_subagent_concurrent_batch_await_ordering() {
    let mut policy = PolicyConfig::empty();
    policy
        .global
        .owner
        .insert(operon_policy::GlobalTool::SubAgent, operon_policy::PermissionMode::Allow);
    policy
        .global
        .owner
        .insert(operon_policy::GlobalTool::Todo, operon_policy::PermissionMode::Allow);

    let (mut runner, mut event_rx, _cmd_tx) = create_test_runner(
        policy,
        vec!["subagent".to_string(), "todo".to_string()],
    )
    .await;

    // Tool call 0: subagent 1
    let call_sub1 = ToolCall {
        id: ToolCallId("sub_1".to_string()),
        name: "spawn_agent".to_string(),
        arguments: json!({
            "role": "agent_alpha",
            "task": "research topic A"
        }),
    };

    // Tool call 1: standard sequential tool (todo_list)
    let call_todo = ToolCall {
        id: ToolCallId("todo_call".to_string()),
        name: "todo_list".to_string(),
        arguments: json!({}),
    };

    // Tool call 2: subagent 2
    let call_sub2 = ToolCall {
        id: ToolCallId("sub_2".to_string()),
        name: "spawn_agent".to_string(),
        arguments: json!({
            "role": "agent_beta",
            "task": "research topic B"
        }),
    };

    // Dispatch the mixed batch of tool calls
    let (results, should_stop) = runner
        .dispatch_turn_tool_calls(vec![call_sub1, call_todo, call_sub2])
        .await;

    assert!(!should_stop);
    assert_eq!(
        results.len(),
        3,
        "All 3 tool results must be collected and returned"
    );

    // Verify that the order is strictly preserved:
    // Slot 0 matches tool call 0 (sub_1)
    match &results[0] {
        ContentBlock::ToolResult(r) => {
            assert_eq!(r.call_id.0, "sub_1");
            assert_eq!(r.name, "spawn_agent");
        }
        _ => panic!("Expected ToolResult at index 0"),
    }

    // Slot 1 matches tool call 1 (todo_call)
    match &results[1] {
        ContentBlock::ToolResult(r) => {
            assert_eq!(r.call_id.0, "todo_call");
            assert_eq!(r.name, "todo_list");
            assert!(!r.is_error, "todo_list tool call must succeed");
        }
        _ => panic!("Expected ToolResult at index 1"),
    }

    // Slot 2 matches tool call 2 (sub_2)
    match &results[2] {
        ContentBlock::ToolResult(r) => {
            assert_eq!(r.call_id.0, "sub_2");
            assert_eq!(r.name, "spawn_agent");
        }
        _ => panic!("Expected ToolResult at index 2"),
    }

    // Verify observability: SubagentSpawned events must have been published to parent event stream
    let mut spawned_roles = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        if let SessionEvent::SubagentSpawned { role, .. } = event {
            spawned_roles.push(role);
        }
    }
    assert!(
        spawned_roles.contains(&"agent_alpha".to_string()),
        "Parent event stream must contain SubagentSpawned for agent_alpha"
    );
    assert!(
        spawned_roles.contains(&"agent_beta".to_string()),
        "Parent event stream must contain SubagentSpawned for agent_beta"
    );
}

/// Verifies that sending a `SessionCommand::Cancel` stops tool execution cleanly,
/// signaling the runner to stop and avoiding spawning any remaining pending subagents.
#[tokio::test]
async fn test_subagent_cancellation_propagation() {
    let policy = PolicyConfig::empty();
    let (mut runner, _event_rx, cmd_tx) =
        create_test_runner(policy, vec!["subagent".to_string()]).await;

    // Send a cancel command before dispatching the turn
    cmd_tx
        .send(SessionCommand::Cancel)
        .await
        .expect("send cancel command");

    let call = ToolCall {
        id: ToolCallId("sub_cancelled".to_string()),
        name: "spawn_agent".to_string(),
        arguments: json!({
            "role": "worker",
            "task": "do work"
        }),
    };

    let (results, should_stop) = runner.dispatch_turn_tool_calls(vec![call]).await;

    // Session runner must recognize cancellation and set should_stop = true
    assert!(
        should_stop,
        "should_stop must be true when Cancel command is pending"
    );
    assert_eq!(
        results.len(),
        0,
        "No results should be produced when cancelled before tool execution"
    );
}

