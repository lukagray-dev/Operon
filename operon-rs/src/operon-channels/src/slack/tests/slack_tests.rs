// slack_tests.rs — Comprehensive unit and integration tests for operon-channels-slack.

use std::path::PathBuf;
use tokio::sync::mpsc;

use operon_channels_slack::config::SlackConfig;
use operon_channels_slack::outbound::{
    split_slack_message, OutboundQueue, SlackOutboundMessage, SLACK_MAX_MESSAGE_LENGTH,
};
use operon_channels_slack::router::{RouteOutcome, SlackRouter};
use operon_channels_slack::types::{
    ConnectionStatus, SlackChannelId, SlackMessage, UserId,
};
use operon_channels_slack::workspace::{
    generate_external_channel_instructions, generate_owner_channel_instructions,
    SlackWorkspaceManager,
};
use operon_policy::CallerRole;

#[test]
fn test_user_id_and_channel_id_sanitization() {
    let u1 = UserId::new("<@U1234567890>");
    assert_eq!(u1.as_str(), "U1234567890");
    assert_eq!(format!("{}", u1), "U1234567890");

    let u2 = UserId::new("  W9876543210  ");
    assert_eq!(u2.as_str(), "W9876543210");

    let ch1 = SlackChannelId::new("<#C1122334455|general>");
    assert_eq!(ch1.as_str(), "C1122334455");

    let ch2 = SlackChannelId::new("<#D9988776655>");
    assert_eq!(ch2.as_str(), "D9988776655");
}

#[test]
fn test_slack_config_is_owner() {
    let owner = UserId::new("U1000000001");
    let trusted = UserId::new("U1000000002");
    let stranger = UserId::new("U1000000003");

    let config = SlackConfig {
        enabled: true,
        bot_token: Some("xoxb-dummy".to_string()),
        app_token: Some("xapp-dummy".to_string()),
        owner_user_id: Some(owner.clone()),
        allowlist: vec![trusted.clone()],
        workspace_dir: None,
    };

    assert!(config.is_owner(&owner));
    assert!(config.is_owner(&trusted));
    assert!(!config.is_owner(&stranger));
}

#[test]
fn test_slack_config_workspace_resolution() {
    let mut config = SlackConfig::default();
    assert!(config.resolved_workspace_dir().ends_with("workspace"));

    config.workspace_dir = Some(PathBuf::from("/custom/slack/workspace"));
    assert_eq!(
        config.resolved_workspace_dir(),
        PathBuf::from("/custom/slack/workspace")
    );
}

#[tokio::test]
async fn test_slack_router_routing_and_new() {
    let owner_id = UserId::new("U1111111111");
    let guest_id = UserId::new("U2222222222");
    let channel_id = SlackChannelId::new("C9999999999");

    let config = SlackConfig {
        enabled: true,
        bot_token: Some("xoxb-test".to_string()),
        app_token: Some("xapp-test".to_string()),
        owner_user_id: Some(owner_id.clone()),
        allowlist: vec![],
        workspace_dir: None,
    };

    let router = SlackRouter::new(config);

    // 1. First message from owner
    let msg1 = SlackMessage {
        id: "1672531199.000100".to_string(),
        channel_id: channel_id.clone(),
        author_id: owner_id.clone(),
        text: "Hello Operon".to_string(),
        thread_ts: None,
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
            assert!(session_id.starts_with("sl-"));
        }
        _ => panic!("Expected ProcessTurn"),
    }

    // 2. Second message from owner (not first time, same session)
    let msg2 = SlackMessage {
        id: "1672531199.000200".to_string(),
        channel_id: channel_id.clone(),
        author_id: owner_id.clone(),
        text: "Followup message".to_string(),
        thread_ts: None,
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
    let msg3 = SlackMessage {
        id: "1672531199.000300".to_string(),
        channel_id: channel_id.clone(),
        author_id: owner_id.clone(),
        text: "/new".to_string(),
        thread_ts: None,
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
            assert!(new_session_id.starts_with("sl-"));
        }
        _ => panic!("Expected FreshSessionRequested"),
    }

    // 4. Message from guest (External role)
    let guest_msg = SlackMessage {
        id: "1672531199.000400".to_string(),
        channel_id: channel_id.clone(),
        author_id: guest_id.clone(),
        text: "Hi".to_string(),
        thread_ts: None,
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
fn test_slack_workspace_manager() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ws_path = temp_dir.path().join("workspace");
    let sessions_path = temp_dir.path().join("sessions");

    let mgr = SlackWorkspaceManager::with_paths(ws_path.clone(), sessions_path.clone());
    let user = UserId::new("U1234567890");

    let user_session_file = mgr.session_file_path_for(&user, "sl-100");
    assert_eq!(
        user_session_file,
        sessions_path.join("U1234567890").join("sl-100.json")
    );

    let provisioned_ws = mgr.provision_workspace(&user, true).unwrap();
    assert_eq!(provisioned_ws, ws_path);
    assert!(ws_path.exists());
    assert!(sessions_path.join("U1234567890").exists());

    let owner_inst = generate_owner_channel_instructions(&user);
    assert!(owner_inst.contains("Owner via Slack"));

    let ext_inst = generate_external_channel_instructions(&user);
    assert!(ext_inst.contains("External User via Slack"));
}

#[test]
fn test_split_slack_message_small() {
    let text = "This is a short message.";
    let chunks = split_slack_message(text, SLACK_MAX_MESSAGE_LENGTH);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], text);
}

#[test]
fn test_split_slack_message_with_code_block() {
    let long_code = "fn main() {\n".to_string() + &"    println!(\"hello\");\n".repeat(200) + "}\n";
    let message = format!("Here is the code:\n```rust\n{}```\nHope this helps!", long_code);

    let chunks = split_slack_message(&message, 500);
    assert!(chunks.len() > 1);

    for (i, chunk) in chunks.iter().enumerate() {
        assert!(chunk.len() <= 600); // Small tolerance for fence injection
        if i > 0 && chunk.contains("println!") && !chunk.starts_with("Here is") {
            assert!(chunk.starts_with("```rust\n"));
        }
    }
}

#[tokio::test]
async fn test_outbound_queue_buffering_and_flush() {
    let (tx, mut rx) = mpsc::channel::<SlackOutboundMessage>(10);
    let queue = OutboundQueue::new(tx);

    let msg = SlackOutboundMessage::new("C11223344", "Test Slack message");

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
    assert_eq!(received.channel_id, "C11223344");
    assert_eq!(received.text, "Test Slack message");
}

