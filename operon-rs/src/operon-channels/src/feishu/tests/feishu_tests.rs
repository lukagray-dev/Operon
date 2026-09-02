// feishu_tests.rs — Comprehensive unit and integration tests for operon-channels-feishu.

use std::path::PathBuf;
use tokio::sync::mpsc;

use operon_channels_feishu::config::FeishuConfig;
use operon_channels_feishu::outbound::{
    split_feishu_message, FeishuOutboundMessage, OutboundQueue, FEISHU_MAX_MESSAGE_LENGTH,
};
use operon_channels_feishu::router::{FeishuRouter, RouteOutcome};
use operon_channels_feishu::types::{
    ChatId, ConnectionStatus, FeishuDomain, FeishuMessage, UserId,
};
use operon_channels_feishu::workspace::{
    generate_external_channel_instructions, generate_owner_channel_instructions,
    FeishuWorkspaceManager,
};
use operon_policy::CallerRole;

#[test]
fn test_user_id_and_chat_id_sanitization() {
    let u1 = UserId::new("<@ou_1234567890abcdef>");
    assert_eq!(u1.as_str(), "ou_1234567890abcdef");
    assert_eq!(format!("{}", u1), "ou_1234567890abcdef");

    let u2 = UserId::new("  @ou_9876543210fedcba  ");
    assert_eq!(u2.as_str(), "ou_9876543210fedcba");

    let ch1 = ChatId::new("  oc_112233445566  ");
    assert_eq!(ch1.as_str(), "oc_112233445566");
}

#[test]
fn test_feishu_domain_urls() {
    let feishu = FeishuDomain::Feishu;
    assert_eq!(feishu.api_base_url(), "https://open.feishu.cn/open-apis");
    assert_eq!(feishu.websocket_url(), "wss://ws-open.feishu.cn/ws/v2");
    assert_eq!(format!("{}", feishu), "feishu");

    let lark = FeishuDomain::Lark;
    assert_eq!(lark.api_base_url(), "https://open.larksuite.com/open-apis");
    assert_eq!(lark.websocket_url(), "wss://ws-open.larksuite.com/ws/v2");
    assert_eq!(format!("{}", lark), "lark");
}

#[test]
fn test_feishu_config_is_owner() {
    let owner = UserId::new("ou_1000000001");
    let trusted = UserId::new("ou_1000000002");
    let stranger = UserId::new("ou_1000000003");

    let config = FeishuConfig {
        enabled: true,
        app_id: Some("cli_dummy".to_string()),
        app_secret: Some("secret_dummy".to_string()),
        domain: FeishuDomain::Feishu,
        owner_user_id: Some(owner.clone()),
        allowlist: vec![trusted.clone()],
        workspace_dir: None,
        verification_token: None,
        encrypt_key: None,
    };

    assert!(config.is_owner(&owner));
    assert!(config.is_owner(&trusted));
    assert!(!config.is_owner(&stranger));
}

#[test]
fn test_feishu_config_workspace_resolution() {
    let mut config = FeishuConfig::default();
    assert!(config.resolved_workspace_dir().ends_with("workspace"));

    config.workspace_dir = Some(PathBuf::from("/custom/feishu/workspace"));
    assert_eq!(
        config.resolved_workspace_dir(),
        PathBuf::from("/custom/feishu/workspace")
    );
}

#[tokio::test]
async fn test_feishu_router_routing_and_new() {
    let owner_id = UserId::new("ou_1111111111");
    let guest_id = UserId::new("ou_2222222222");
    let chat_id = ChatId::new("oc_9999999999");

    let config = FeishuConfig {
        enabled: true,
        app_id: Some("cli_test".to_string()),
        app_secret: Some("sec_test".to_string()),
        domain: FeishuDomain::Feishu,
        owner_user_id: Some(owner_id.clone()),
        allowlist: vec![],
        workspace_dir: None,
        verification_token: None,
        encrypt_key: None,
    };

    let router = FeishuRouter::new(config);

    // 1. First message from owner
    let msg1 = FeishuMessage {
        id: "om_0001".to_string(),
        chat_id: chat_id.clone(),
        author_id: owner_id.clone(),
        text: "Hello Operon".to_string(),
        root_id: None,
        parent_id: None,
        timestamp: 1000,
        is_bot: false,
    };

    let outcome1 = router.route(&msg1).await;
    match outcome1 {
        RouteOutcome::ProcessTurn {
            user_id,
            session_id,
            role,
            is_first_time,
            ..
        } => {
            assert_eq!(user_id, owner_id);
            assert_eq!(role, CallerRole::Owner);
            assert!(is_first_time);
            assert!(session_id.starts_with("fs-"));
        }
        _ => panic!("Expected ProcessTurn"),
    }

    // 2. Second message from owner (not first time, same session)
    let msg2 = FeishuMessage {
        id: "om_0002".to_string(),
        chat_id: chat_id.clone(),
        author_id: owner_id.clone(),
        text: "Followup message".to_string(),
        root_id: None,
        parent_id: None,
        timestamp: 1005,
        is_bot: false,
    };

    let outcome2 = router.route(&msg2).await;
    match outcome2 {
        RouteOutcome::ProcessTurn { is_first_time, .. } => {
            assert!(!is_first_time);
        }
        _ => panic!("Expected ProcessTurn"),
    }

    // 3. /new command from owner
    let msg3 = FeishuMessage {
        id: "om_0003".to_string(),
        chat_id: chat_id.clone(),
        author_id: owner_id.clone(),
        text: "/new".to_string(),
        root_id: None,
        parent_id: None,
        timestamp: 1010,
        is_bot: false,
    };

    let outcome3 = router.route(&msg3).await;
    match outcome3 {
        RouteOutcome::FreshSessionRequested {
            user_id,
            new_session_id,
            role,
            ..
        } => {
            assert_eq!(user_id, owner_id);
            assert_eq!(role, CallerRole::Owner);
            assert!(new_session_id.starts_with("fs-"));
        }
        _ => panic!("Expected FreshSessionRequested"),
    }

    // 4. Message from guest (External role)
    let guest_msg = FeishuMessage {
        id: "om_0004".to_string(),
        chat_id: chat_id.clone(),
        author_id: guest_id.clone(),
        text: "Hi".to_string(),
        root_id: None,
        parent_id: None,
        timestamp: 1020,
        is_bot: false,
    };

    let guest_outcome = router.route(&guest_msg).await;
    match guest_outcome {
        RouteOutcome::ProcessTurn { role, .. } => {
            assert_eq!(role, CallerRole::External);
        }
        _ => panic!("Expected ProcessTurn with External role"),
    }
}

#[test]
fn test_feishu_workspace_manager() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ws_path = temp_dir.path().join("workspace");
    let sessions_path = temp_dir.path().join("sessions");

    let mgr = FeishuWorkspaceManager::with_paths(ws_path.clone(), sessions_path.clone());
    let user = UserId::new("ou_1234567890");

    let user_session_file = mgr.session_file_path_for(&user, "fs-100");
    assert_eq!(
        user_session_file,
        sessions_path.join("ou_1234567890").join("fs-100.json")
    );

    let provisioned_ws = mgr.provision_workspace(&user, true).unwrap();
    assert_eq!(provisioned_ws, ws_path);
    assert!(ws_path.exists());
    assert!(sessions_path.join("ou_1234567890").exists());

    let owner_inst = generate_owner_channel_instructions(&user);
    assert!(owner_inst.contains("Owner via Feishu / Lark"));

    let ext_inst = generate_external_channel_instructions(&user);
    assert!(ext_inst.contains("External User via Feishu / Lark"));
}

#[test]
fn test_split_feishu_message_small() {
    let text = "This is a short message.";
    let chunks = split_feishu_message(text, FEISHU_MAX_MESSAGE_LENGTH);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], text);
}

#[test]
fn test_split_feishu_message_with_code_block() {
    let long_code = "fn main() {\n".to_string() + &"    println!(\"hello\");\n".repeat(200) + "}\n";
    let message = format!("Here is the code:\n```rust\n{}```\nHope this helps!", long_code);

    let chunks = split_feishu_message(&message, 500);
    assert!(chunks.len() > 1);

    for (i, chunk) in chunks.iter().enumerate() {
        assert!(chunk.len() <= 600);
        if i > 0 && chunk.contains("println!") && !chunk.starts_with("Here is") {
            assert!(chunk.starts_with("```rust\n"));
        }
    }
}

#[tokio::test]
async fn test_outbound_queue_buffering_and_flush() {
    let (tx, mut rx) = mpsc::channel::<FeishuOutboundMessage>(10);
    let queue = OutboundQueue::new(tx);

    let msg = FeishuOutboundMessage::new("ou_11223344", "Test Feishu message");

    // 1. Enqueue while disconnected -> must buffer
    queue
        .enqueue(msg.clone(), &ConnectionStatus::Disconnected)
        .await
        .unwrap();
    assert_eq!(queue.buffered_count().await, 1);
    assert!(rx.try_recv().is_err());

    // 2. Flush
    queue.flush().await.unwrap();
    assert_eq!(queue.buffered_count().await, 0);

    let received = rx.recv().await.unwrap();
    assert_eq!(received.receive_id, "ou_11223344");
    assert_eq!(received.text, "Test Feishu message");
}

