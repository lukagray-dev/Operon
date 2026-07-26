// whatsapp_tests.rs — Test suite for operon-channels-whatsapp.
//
// Hey friend! This file tests all core WhatsApp channel components: contact sanitization,
// role classification, role-specific AGENTS.md auto-generation, /new command routing,
// workspace provisioning, and markdown formatting.

use std::path::PathBuf;
use tempfile::TempDir;

use operon_policy::CallerRole;
use operon_channels_whatsapp::config::WhatsAppConfig;
use operon_channels_whatsapp::outbound::format_for_whatsapp;
use operon_channels_whatsapp::router::{RouteOutcome, WhatsAppRouter};
use operon_channels_whatsapp::types::{ContactId, WhatsAppMessage};
use operon_channels_whatsapp::workspace::WhatsAppWorkspaceManager;

#[test]
fn test_contact_id_sanitization() {
    let raw = "+1 (555) 019-2834";
    let contact = ContactId::new(raw);
    assert_eq!(contact.as_str(), "15550192834");
}

#[test]
fn test_role_classification() {
    let owner_num = ContactId::new("+15550001111");
    let allow_num = ContactId::new("+15552223333");
    let ext_num = ContactId::new("+15559998888");

    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(owner_num.clone()),
        allowlist: vec![allow_num.clone()],
        auth_dir: None,
    };

    assert!(config.is_owner(&owner_num), "Owner number should be classified as owner");
    assert!(config.is_owner(&allow_num), "Allowlisted number should be classified as owner");
    assert!(!config.is_owner(&ext_num), "Unlisted number should NOT be owner");
}

#[tokio::test]
async fn test_slash_new_command_detection() {
    let owner_num = ContactId::new("15550001111");
    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(owner_num.clone()),
        allowlist: vec![],
        auth_dir: None,
    };

    let router = WhatsAppRouter::new(config);
    let msg = WhatsAppMessage {
        id: "msg1".to_string(),
        sender: owner_num.clone(),
        text: "/new".to_string(),
        timestamp: 1000,
        is_self: false,
    };

    let outcome = router.route(&msg).await;
    match outcome {
        RouteOutcome::FreshSessionRequested { contact, new_session_id, role } => {
            assert_eq!(contact, owner_num);
            assert_eq!(role, CallerRole::Owner);
            assert!(new_session_id.starts_with("wa-"));
        }
        _ => panic!("Expected FreshSessionRequested outcome"),
    }
}

#[test]
fn test_workspace_directory_provisioning_and_role_agents_md() {
    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir.path().join("channels").join("whatsapp").join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let manager = WhatsAppWorkspaceManager::with_paths(base_ws, base_sess);

    let owner_contact = ContactId::new("15551112222");
    let ext_contact = ContactId::new("15558889999");

    // Provision owner workspace
    let owner_ws = manager.provision_workspace(&owner_contact, true).unwrap();
    let owner_agents_md = std::fs::read_to_string(owner_ws.join("AGENTS.md")).unwrap();
    assert!(owner_agents_md.contains("ADMINISTRATOR"), "Owner AGENTS.md must contain ADMINISTRATOR");

    // Provision external workspace
    let ext_ws = manager.provision_workspace(&ext_contact, false).unwrap();
    let ext_agents_md = std::fs::read_to_string(ext_ws.join("AGENTS.md")).unwrap();
    assert!(ext_agents_md.contains("OUTSIDER"), "External AGENTS.md must contain OUTSIDER");
}

#[test]
fn test_session_json_path_resolution() {
    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir.path().join("channels").join("whatsapp").join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let manager = WhatsAppWorkspaceManager::with_paths(base_ws, base_sess);
    let contact = ContactId::new("15551234567");

    let path = manager.session_file_path_for(&contact, "wa-sess-1");
    assert!(path.ends_with(PathBuf::from("sessions/whatsapp/15551234567/wa-sess-1.json")));
}

#[test]
fn test_markdown_conversion() {
    let gfm = "Hello **world**! This is ~~deleted~~ text.";
    let wa = format_for_whatsapp(gfm);
    assert_eq!(wa, "Hello *world*! This is ~deleted~ text.");
}

#[test]
fn test_markdown_conversion_non_ascii_and_emojis() {
    let gfm = "🇧🇩 বাংলা **সাহসী** এবং 日本語 🌸 **強調** text with ~~মুছে ফেলা~~ word!";
    let wa = format_for_whatsapp(gfm);
    assert_eq!(wa, "🇧🇩 বাংলা *সাহসী* এবং 日本語 🌸 *強調* text with ~মুছে ফেলা~ word!");
}

#[tokio::test]
async fn test_cancel_in_flight_turn_on_slash_new() {
    use operon_events::SessionCommand;
    use tokio::sync::mpsc;

    let contact = ContactId::new("15551112222");
    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(contact.clone()),
        allowlist: vec![],
        auth_dir: None,
    };
    let router = WhatsAppRouter::new(config);

    // Initial msg routes as regular turn
    let msg1 = WhatsAppMessage {
        id: "m1".to_string(),
        sender: contact.clone(),
        text: "hello".to_string(),
        timestamp: 100,
        is_self: false,
    };
    let outcome1 = router.route(&msg1).await;
    let session_id = match outcome1 {
        RouteOutcome::ProcessTurn { session_id, .. } => session_id,
        _ => panic!("expected ProcessTurn"),
    };

    // Register active cmd_tx channel simulating running turn
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<SessionCommand>(10);
    router.register_cmd_tx(&contact, &session_id, cmd_tx).await;

    // Send /new command
    let msg2 = WhatsAppMessage {
        id: "m2".to_string(),
        sender: contact.clone(),
        text: "/new".to_string(),
        timestamp: 101,
        is_self: false,
    };
    let outcome2 = router.route(&msg2).await;
    match outcome2 {
        RouteOutcome::FreshSessionRequested { new_session_id, .. } => {
            assert_ne!(new_session_id, session_id);
        }
        _ => panic!("expected FreshSessionRequested"),
    }

    // Verify SessionCommand::Cancel was sent to cmd_rx
    let received_cmd = cmd_rx.try_recv();
    assert_eq!(received_cmd, Ok(SessionCommand::Cancel));
}

#[test]
fn test_contact_promotion_updates_agents_md() {
    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir.path().join("channels").join("whatsapp").join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let manager = WhatsAppWorkspaceManager::with_paths(base_ws, base_sess);
    let contact = ContactId::new("15557778888");

    // 1. Provision as External
    let ws_path = manager.provision_workspace(&contact, false).unwrap();
    let initial_content = std::fs::read_to_string(ws_path.join("AGENTS.md")).unwrap();
    assert!(initial_content.contains("OUTSIDER"));

    // 2. Promote contact to Owner on next provision
    let updated_ws_path = manager.provision_workspace(&contact, true).unwrap();
    let updated_content = std::fs::read_to_string(updated_ws_path.join("AGENTS.md")).unwrap();
    assert!(updated_content.contains("ADMINISTRATOR"));
    assert!(!updated_content.contains("OUTSIDER"));
}

#[test]
fn test_auth_credential_file_permissions() {
    use operon_channels_whatsapp::auth::WhatsAppAuth;

    let tmp_dir = TempDir::new().unwrap();
    let auth_dir = tmp_dir.path().join("auth");
    let auth = WhatsAppAuth::new(auth_dir.clone());

    let cred_path = auth.write_credential("creds.json", b"{\"secret\":\"key\"}").unwrap();
    assert!(cred_path.exists());
    assert_eq!(std::fs::read(&cred_path).unwrap(), b"{\"secret\":\"key\"}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = cred_path.metadata().unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "Credential file permissions must be 0600 on Unix");
    }
}

#[tokio::test]
async fn test_outbound_queue_buffering_and_fifo_flush() {
    use operon_channels_whatsapp::outbound::{OutboundMessage, OutboundQueue};
    use operon_channels_whatsapp::types::ConnectionStatus;
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel::<OutboundMessage>(10);
    let queue = OutboundQueue::new(tx);

    let msg1 = OutboundMessage::new("15551111", "First message");
    let msg2 = OutboundMessage::new("15552222", "Second message");
    let msg3 = OutboundMessage::new("15553333", "Third message");

    // 1. Enqueue while Disconnected — should buffer
    let status_disconnected = ConnectionStatus::Disconnected;
    queue.enqueue(msg1.clone(), &status_disconnected).await.unwrap();
    queue.enqueue(msg2.clone(), &status_disconnected).await.unwrap();
    assert_eq!(queue.buffered_count().await, 2);

    // Verify nothing sent to underlying channel yet
    assert!(rx.try_recv().is_err());

    // 2. Enqueue while Connected — should flush buffer first, then send msg3 in FIFO order
    let status_connected = ConnectionStatus::Connected;
    queue.enqueue(msg3.clone(), &status_connected).await.unwrap();

    // 3. Receive messages from rx and verify FIFO order: msg1 -> msg2 -> msg3
    let recv1 = rx.recv().await.unwrap();
    let recv2 = rx.recv().await.unwrap();
    let recv3 = rx.recv().await.unwrap();

    assert_eq!(recv1, msg1);
    assert_eq!(recv2, msg2);
    assert_eq!(recv3, msg3);
    assert_eq!(queue.buffered_count().await, 0);
}
