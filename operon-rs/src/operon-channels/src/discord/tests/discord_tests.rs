// discord_tests.rs — Comprehensive unit and integration tests for operon-channels-discord.

use std::path::PathBuf;
use tokio::sync::mpsc;

use operon_channels_discord::config::DiscordConfig;
use operon_channels_discord::outbound::{
    split_discord_message, DiscordOutboundMessage, OutboundQueue, DISCORD_MAX_MESSAGE_LENGTH,
};
use operon_channels_discord::router::{DiscordRouter, RouteOutcome};
use operon_channels_discord::types::{
    ConnectionStatus, DiscordChannelId, DiscordMessage, UserId,
};
use operon_channels_discord::workspace::{
    generate_external_channel_instructions, generate_owner_channel_instructions,
    DiscordWorkspaceManager,
};
use operon_policy::CallerRole;

#[test]
fn test_user_id_sanitization() {
    let u1 = UserId::new("<@!123456789012345678>");
    assert_eq!(u1.as_str(), "123456789012345678");
    assert_eq!(format!("{}", u1), "123456789012345678");

    let u2 = UserId::new("  987654321098765432  ");
    assert_eq!(u2.as_str(), "987654321098765432");

    let ch = DiscordChannelId::new("<#112233445566778899>");
    assert_eq!(ch.as_str(), "112233445566778899");
}

#[test]
fn test_discord_config_is_owner() {
    let owner = UserId::new("100000000000000001");
    let trusted = UserId::new("100000000000000002");
    let stranger = UserId::new("100000000000000003");

    let config = DiscordConfig {
        enabled: true,
        bot_token: Some("dummy_token".to_string()),
        owner_user_id: Some(owner.clone()),
        allowlist: vec![trusted.clone()],
        guild_id: None,
        workspace_dir: None,
    };

    assert!(config.is_owner(&owner));
    assert!(config.is_owner(&trusted));
    assert!(!config.is_owner(&stranger));
}

#[test]
fn test_discord_config_workspace_resolution() {
    let mut config = DiscordConfig::default();
    assert!(config.resolved_workspace_dir().ends_with("workspace"));

    config.workspace_dir = Some(PathBuf::from("/custom/discord/workspace"));
    assert_eq!(
        config.resolved_workspace_dir(),
        PathBuf::from("/custom/discord/workspace")
    );
}

#[tokio::test]
async fn test_discord_router_routing_and_new() {
    let owner_id = UserId::new("111111111111111111");
    let guest_id = UserId::new("222222222222222222");
    let channel_id = DiscordChannelId::new("999999999999999999");

    let config = DiscordConfig {
        enabled: true,
        bot_token: Some("token".to_string()),
        owner_user_id: Some(owner_id.clone()),
        allowlist: vec![],
        guild_id: None,
        workspace_dir: None,
    };

    let router = DiscordRouter::new(config);

    // 1. First message from owner
    let msg1 = DiscordMessage {
        id: "1".to_string(),
        channel_id: channel_id.clone(),
        author_id: owner_id.clone(),
        author_username: "OwnerUser".to_string(),
        content: "Hello Operon".to_string(),
        timestamp: 1000,
        is_self: false,
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
            assert!(session_id.starts_with("dc-"));
        }
        _ => panic!("Expected ProcessTurn"),
    }

    // 2. Second message from owner (not first time, same session)
    let msg2 = DiscordMessage {
        id: "2".to_string(),
        channel_id: channel_id.clone(),
        author_id: owner_id.clone(),
        author_username: "OwnerUser".to_string(),
        content: "Followup message".to_string(),
        timestamp: 1005,
        is_self: false,
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
    let msg3 = DiscordMessage {
        id: "3".to_string(),
        channel_id: channel_id.clone(),
        author_id: owner_id.clone(),
        author_username: "OwnerUser".to_string(),
        content: "/new".to_string(),
        timestamp: 1010,
        is_self: false,
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
            assert!(new_session_id.starts_with("dc-"));
        }
        _ => panic!("Expected FreshSessionRequested"),
    }

    // 4. Message from guest (External role)
    let guest_msg = DiscordMessage {
        id: "4".to_string(),
        channel_id: channel_id.clone(),
        author_id: guest_id.clone(),
        author_username: "GuestUser".to_string(),
        content: "Hi".to_string(),
        timestamp: 1020,
        is_self: false,
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
fn test_discord_workspace_manager() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ws_path = temp_dir.path().join("workspace");
    let sessions_path = temp_dir.path().join("sessions");

    let mgr = DiscordWorkspaceManager::with_paths(ws_path.clone(), sessions_path.clone());
    let user = UserId::new("123456789");

    let user_session_file = mgr.session_file_path_for(&user, "dc-100");
    assert_eq!(
        user_session_file,
        sessions_path.join("123456789").join("dc-100.json")
    );

    let provisioned_ws = mgr.provision_workspace(&user, true).unwrap();
    assert_eq!(provisioned_ws, ws_path);
    assert!(ws_path.exists());
    assert!(sessions_path.join("123456789").exists());

    let owner_inst = generate_owner_channel_instructions(&user);
    assert!(owner_inst.contains("Owner via Discord"));

    let ext_inst = generate_external_channel_instructions(&user);
    assert!(ext_inst.contains("External User via Discord"));
}

#[test]
fn test_split_discord_message_small() {
    let text = "This is a short message.";
    let chunks = split_discord_message(text, DISCORD_MAX_MESSAGE_LENGTH);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], text);
}

#[test]
fn test_split_discord_message_with_code_block() {
    let long_code = "fn main() {\n".to_string() + &"    println!(\"hello\");\n".repeat(200) + "}\n";
    let message = format!("Here is the code:\n```rust\n{}```\nHope this helps!", long_code);

    let chunks = split_discord_message(&message, 500);
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
    let (tx, mut rx) = mpsc::channel::<DiscordOutboundMessage>(10);
    let queue = OutboundQueue::new(tx);

    let msg = DiscordOutboundMessage::new("ch-1", "Test message");

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
    assert_eq!(received.channel_id, "ch-1");
    assert_eq!(received.text, "Test message");
}
