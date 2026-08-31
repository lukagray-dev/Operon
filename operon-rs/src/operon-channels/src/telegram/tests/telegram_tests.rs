// telegram_tests.rs — Test suite for operon-channels-telegram.
//
// Hey friend! This file tests all core Telegram channel components: chat ID handling,
// role classification, role-specific in-memory instructions generation, /new command routing,
// workspace provisioning, MarkdownV2 formatting/escaping, outbound FIFO queueing, and lock pruning.

use std::path::PathBuf;
use tempfile::TempDir;

use operon_channels_telegram::config::TelegramConfig;
use operon_channels_telegram::outbound::{
    format_for_telegram, OutboundQueue, TelegramOutboundMessage,
};
use operon_channels_telegram::router::{RouteOutcome, TelegramRouter};
use operon_channels_telegram::types::{ChatId, ConnectionStatus, TelegramMessage};
use operon_channels_telegram::workspace::{
    generate_external_channel_instructions, generate_owner_channel_instructions,
    TelegramWorkspaceManager,
};
use operon_context::{Role, SnapshotBuilder, SnapshotConfig};
use operon_policy::CallerRole;
use tokio::sync::mpsc;

#[test]
fn test_chat_id_value() {
    let chat = ChatId::new(123456789);
    assert_eq!(chat.as_i64(), 123456789);
    assert_eq!(chat.to_string(), "123456789");
}

#[test]
fn test_role_classification() {
    let owner_chat = ChatId::new(1001);
    let allow_chat = ChatId::new(1002);
    let ext_chat = ChatId::new(9999);

    let config = TelegramConfig {
        enabled: true,
        bot_token: Some("123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11".to_string()),
        owner_chat_id: Some(owner_chat),
        allowlist: vec![allow_chat],
        workspace_dir: None,
        poll_interval_secs: Some(30),
    };

    assert!(
        config.is_owner(&owner_chat),
        "Owner chat ID should be classified as owner"
    );
    assert!(
        config.is_owner(&allow_chat),
        "Allowlisted chat ID should be classified as owner"
    );
    assert!(
        !config.is_owner(&ext_chat),
        "Unlisted chat ID should NOT be owner"
    );
}

#[tokio::test]
async fn test_slash_new_command_detection() {
    let owner_chat = ChatId::new(1001);
    let config = TelegramConfig {
        enabled: true,
        bot_token: Some("123456:testtoken".to_string()),
        owner_chat_id: Some(owner_chat),
        allowlist: vec![],
        workspace_dir: None,
        poll_interval_secs: Some(30),
    };

    let router = TelegramRouter::new(config);
    let msg = TelegramMessage {
        update_id: 100,
        message_id: 1,
        sender: owner_chat,
        text: "/new".to_string(),
        timestamp: 1000,
        is_self: false,
    };

    let outcome = router.route(&msg).await;
    match outcome {
        RouteOutcome::FreshSessionRequested {
            chat,
            new_session_id,
            role,
        } => {
            assert_eq!(chat, owner_chat);
            assert_eq!(role, CallerRole::Owner);
            assert!(
                new_session_id.starts_with("tg-"),
                "Telegram session ID must start with tg-"
            );
        }
        _ => panic!("Expected FreshSessionRequested outcome"),
    }
}

#[test]
fn test_telegram_config_workspace_dir_resolution() {
    let mut config = TelegramConfig::default();
    assert!(
        config.resolved_workspace_dir().ends_with("workspace"),
        "Default workspace dir must resolve to workspace root"
    );

    let custom_path = PathBuf::from("/tmp/custom_telegram_ws");
    config.workspace_dir = Some(custom_path.clone());
    assert_eq!(
        config.resolved_workspace_dir(),
        custom_path,
        "Custom workspace dir must be returned when set"
    );
}

#[test]
fn test_workspace_directory_provisioning_and_in_memory_channel_instructions() {
    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir.path().join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("telegram");

    let manager = TelegramWorkspaceManager::with_paths(base_ws.clone(), base_sess);

    let owner_chat = ChatId::new(1001);
    let ext_chat = ChatId::new(9999);

    // Both chats must map to the SAME shared workspace root directory
    assert_eq!(manager.workspace_dir_for(&owner_chat), base_ws);
    assert_eq!(manager.workspace_dir_for(&ext_chat), base_ws);

    // Provisioning creates the shared workspace directory if missing
    let owner_ws = manager.provision_workspace(&owner_chat, true).unwrap();
    assert_eq!(owner_ws, base_ws);
    assert!(owner_ws.exists());

    // AGENTS.md must NOT be created on disk by provision_workspace
    assert!(
        !base_ws.join("AGENTS.md").exists(),
        "provision_workspace must not write AGENTS.md to disk"
    );

    // Role instructions are generated in-memory per-turn
    let owner_inst = generate_owner_channel_instructions(&owner_chat);
    assert!(owner_inst.contains("ADMINISTRATOR"));

    let ext_inst = generate_external_channel_instructions(&ext_chat);
    assert!(ext_inst.contains("OUTSIDER"));
}

#[test]
fn test_session_json_path_resolution() {
    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir.path().join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("telegram");

    let manager = TelegramWorkspaceManager::with_paths(base_ws, base_sess.clone());
    let chat = ChatId::new(12345);
    let session_id = "tg-abc12345";

    let session_path = manager.session_file_path_for(&chat, session_id);
    let expected = base_sess.join("12345").join("tg-abc12345.json");

    assert_eq!(session_path, expected);
}

#[tokio::test]
async fn test_outbound_queue_fifo_order_and_buffering() {
    let (tx, mut rx) = mpsc::channel::<TelegramOutboundMessage>(10);
    let queue = OutboundQueue::new(tx);

    let msg1 = TelegramOutboundMessage::new(100, "First message");
    let msg2 = TelegramOutboundMessage::new(100, "Second message");

    // Enqueue while disconnected -> buffers items
    queue
        .enqueue(msg1.clone(), &ConnectionStatus::Disconnected)
        .await
        .unwrap();
    queue
        .enqueue(msg2.clone(), &ConnectionStatus::Disconnected)
        .await
        .unwrap();

    assert_eq!(queue.buffered_count().await, 2);

    // Flush queue -> items delivered in strict FIFO order
    let count = queue.flush().await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(queue.buffered_count().await, 0);

    let rec1 = rx.recv().await.unwrap();
    let rec2 = rx.recv().await.unwrap();

    assert_eq!(rec1.text, "First message");
    assert_eq!(rec2.text, "Second message");
}

#[test]
fn test_markdown_v2_formatting_and_escaping() {
    let gfm_text = "Hello **world**! Check this: ~~deleted~~ and a formula: 1 + 2 = 3.";
    let chunks = format_for_telegram(gfm_text);

    assert_eq!(chunks.len(), 1);
    let formatted = &chunks[0];

    // GFM ** converted to Telegram *
    assert!(formatted.contains("*world*"));

    // GFM ~~ converted to Telegram ~
    assert!(formatted.contains("~deleted~"));

    // Reserved chars escaped with \
    assert!(formatted.contains("Hello *world*\\!"));
    assert!(formatted.contains("1 \\+ 2 \\= 3\\."));
}

#[test]
fn test_snapshot_integration_with_channel_instructions() {
    let tmp_dir = TempDir::new().unwrap();
    let chat = ChatId::new(12345);
    let channel_inst = generate_owner_channel_instructions(&chat);

    let config = SnapshotConfig {
        root: tmp_dir.path().to_path_buf(),
        role: Role::Owner,
        session_id: "tg-12345".to_string(),
        tree_depth: 1,
        channel_instructions: Some(channel_inst),
    };

    let mut builder = SnapshotBuilder::new(config).unwrap();
    let snapshot = builder.build().unwrap();

    assert!(
        snapshot
            .channel_instructions
            .as_ref()
            .unwrap()
            .contains("ADMINISTRATOR"),
        "Channel instructions must be incorporated into snapshot"
    );
}
