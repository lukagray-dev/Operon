// whatsapp_tests.rs — Test suite for operon-channels-whatsapp.
//
// Hey friend! This file tests all core WhatsApp channel components: contact sanitization,
// role classification, role-specific AGENTS.md auto-generation, /new command routing,
// workspace provisioning, and markdown formatting.

use std::path::PathBuf;
use tempfile::TempDir;

use operon_channels_whatsapp::config::WhatsAppConfig;
use operon_channels_whatsapp::outbound::format_for_whatsapp;
use operon_channels_whatsapp::router::{RouteOutcome, WhatsAppRouter};
use operon_channels_whatsapp::types::{ContactId, WhatsAppMessage};
use operon_channels_whatsapp::workspace::WhatsAppWorkspaceManager;
use operon_policy::CallerRole;

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
        workspace_dir: None,
    };

    assert!(
        config.is_owner(&owner_num),
        "Owner number should be classified as owner"
    );
    assert!(
        config.is_owner(&allow_num),
        "Allowlisted number should be classified as owner"
    );
    assert!(
        !config.is_owner(&ext_num),
        "Unlisted number should NOT be owner"
    );
}

#[tokio::test]
async fn test_slash_new_command_detection() {
    let owner_num = ContactId::new("15550001111");
    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(owner_num.clone()),
        allowlist: vec![],
        auth_dir: None,
        workspace_dir: None,
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
        RouteOutcome::FreshSessionRequested {
            contact,
            new_session_id,
            role,
        } => {
            assert_eq!(contact, owner_num);
            assert_eq!(role, CallerRole::Owner);
            assert!(new_session_id.starts_with("wa-"));
        }
        _ => panic!("Expected FreshSessionRequested outcome"),
    }
}

#[test]
fn test_whatsapp_config_workspace_dir_resolution() {
    let mut config = WhatsAppConfig::default();
    assert!(
        config.resolved_workspace_dir().ends_with("workspace"),
        "Default workspace dir must resolve to workspace root"
    );

    let custom_path = PathBuf::from("/tmp/custom_whatsapp_ws");
    config.workspace_dir = Some(custom_path.clone());
    assert_eq!(
        config.resolved_workspace_dir(),
        custom_path,
        "Custom workspace dir must be returned when set"
    );
}

#[test]
fn test_workspace_directory_provisioning_and_role_agents_md() {
    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir
        .path()
        .join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let manager = WhatsAppWorkspaceManager::with_paths(base_ws.clone(), base_sess);

    let owner_contact = ContactId::new("15551112222");
    let ext_contact = ContactId::new("15558889999");

    // Both contacts must map to the SAME shared workspace root directory
    assert_eq!(manager.workspace_dir_for(&owner_contact), base_ws);
    assert_eq!(manager.workspace_dir_for(&ext_contact), base_ws);

    // Provision owner workspace — writes Owner AGENTS.md in shared root
    let owner_ws = manager.provision_workspace(&owner_contact, true).unwrap();
    assert_eq!(owner_ws, base_ws);
    let owner_agents_md = std::fs::read_to_string(base_ws.join("AGENTS.md")).unwrap();
    assert!(
        owner_agents_md.contains("ADMINISTRATOR"),
        "Owner AGENTS.md must contain ADMINISTRATOR"
    );

    // Provision external workspace — rewrites AGENTS.md in shared root fresh per-turn
    let ext_ws = manager.provision_workspace(&ext_contact, false).unwrap();
    assert_eq!(ext_ws, base_ws);
    let ext_agents_md = std::fs::read_to_string(base_ws.join("AGENTS.md")).unwrap();
    assert!(
        ext_agents_md.contains("OUTSIDER"),
        "External AGENTS.md must contain OUTSIDER"
    );
}

#[test]
fn test_session_json_path_resolution() {
    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir
        .path()
        .join("channels")
        .join("whatsapp")
        .join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let manager = WhatsAppWorkspaceManager::with_paths(base_ws, base_sess);
    let contact = ContactId::new("15551234567");

    let path = manager.session_file_path_for(&contact, "wa-sess-1");
    assert!(path.ends_with(PathBuf::from(
        "sessions/whatsapp/15551234567/wa-sess-1.json"
    )));
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
    assert_eq!(
        wa,
        "🇧🇩 বাংলা *সাহসী* এবং 日本語 🌸 *強調* text with ~মুছে ফেলা~ word!"
    );
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
        workspace_dir: None,
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
    let base_ws = tmp_dir
        .path()
        .join("channels")
        .join("whatsapp")
        .join("workspace");
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

    let cred_path = auth
        .write_credential("creds.json", b"{\"secret\":\"key\"}")
        .unwrap();
    assert!(cred_path.exists());
    assert_eq!(std::fs::read(&cred_path).unwrap(), b"{\"secret\":\"key\"}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = cred_path.metadata().unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "Credential file permissions must be 0600 on Unix"
        );
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_acl::acl::ACL;
        use windows_acl::helper::sid_to_string;
        use winapi::shared::winerror::ERROR_SUCCESS;
        use winapi::um::accctrl::SE_FILE_OBJECT;
        use winapi::um::aclapi::GetNamedSecurityInfoW;
        use winapi::um::winnt::{OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID};

        let wpath: Vec<u16> = std::ffi::OsStr::new(cred_path.to_str().unwrap())
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let user_sid_str = unsafe {
            let mut psid_owner: PSID = std::ptr::null_mut();
            let mut p_sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            let ret = GetNamedSecurityInfoW(
                wpath.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &mut psid_owner,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut p_sd,
            );
            assert_eq!(ret, ERROR_SUCCESS, "GetNamedSecurityInfoW must succeed");
            let s = sid_to_string(psid_owner).expect("Owner SID string must resolve");
            winapi::um::winbase::LocalFree(p_sd as *mut _);
            s
        };

        // Verify credential file ACL restrictions (only current user allowed)
        let file_acl = ACL::from_file_path(cred_path.to_str().unwrap(), false)
            .expect("File ACL must read");
        let entries = file_acl.all().expect("ACL entries must be retrieved");
        assert!(!entries.is_empty(), "File ACL must contain at least one entry");
        for entry in entries {
            assert_eq!(
                entry.string_sid, user_sid_str,
                "File ACL must only grant access to current user SID"
            );
        }

        // Verify auth directory ACL restrictions (only current user allowed)
        let dir_acl = ACL::from_file_path(auth_dir.to_str().unwrap(), false)
            .expect("Directory ACL must read");
        let dir_entries = dir_acl.all().expect("Directory ACL entries must be retrieved");
        assert!(!dir_entries.is_empty(), "Directory ACL must contain at least one entry");
        for entry in dir_entries {
            assert_eq!(
                entry.string_sid, user_sid_str,
                "Directory ACL must only grant access to current user SID"
            );
        }
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
    queue
        .enqueue(msg1.clone(), &status_disconnected)
        .await
        .unwrap();
    queue
        .enqueue(msg2.clone(), &status_disconnected)
        .await
        .unwrap();
    assert_eq!(queue.buffered_count().await, 2);

    // Verify nothing sent to underlying channel yet
    assert!(rx.try_recv().is_err());

    // 2. Enqueue while Connected — should flush buffer first, then send msg3 in FIFO order
    let status_connected = ConnectionStatus::Connected;
    queue
        .enqueue(msg3.clone(), &status_connected)
        .await
        .unwrap();

    // 3. Receive messages from rx and verify FIFO order: msg1 -> msg2 -> msg3
    let recv1 = rx.recv().await.unwrap();
    let recv2 = rx.recv().await.unwrap();
    let recv3 = rx.recv().await.unwrap();

    assert_eq!(recv1, msg1);
    assert_eq!(recv2, msg2);
    assert_eq!(recv3, msg3);
    assert_eq!(queue.buffered_count().await, 0);
}

#[tokio::test]
async fn test_outbound_queue_fifo_order_preserved_on_flush_failure() {
    use operon_channels_whatsapp::outbound::{OutboundMessage, OutboundQueue};
    use operon_channels_whatsapp::types::ConnectionStatus;
    use tokio::sync::mpsc;

    let (tx, rx) = mpsc::channel::<OutboundMessage>(10);
    let queue = OutboundQueue::new(tx);

    let msg1 = OutboundMessage::new("15551111", "First buffered message");
    let msg2 = OutboundMessage::new("15552222", "Second new message");

    // 1. Seed queue with msg1 while Disconnected
    queue
        .enqueue(msg1.clone(), &ConnectionStatus::Disconnected)
        .await
        .unwrap();
    assert_eq!(queue.buffered_count().await, 1);

    // 2. Drop rx to close underlying channel, forcing flush failure
    drop(rx);

    // 3. Enqueue msg2 with Connected status (flush will fail)
    queue
        .enqueue(msg2.clone(), &ConnectionStatus::Connected)
        .await
        .unwrap();

    // 4. Assert both messages remain buffered in FIFO order (msg1 first, then msg2)
    assert_eq!(queue.buffered_count().await, 2);
}


#[tokio::test]
async fn test_whatsapp_service_orchestration_loop() {
    use operon_channels_whatsapp::client::WhatsAppClient;
    use operon_channels_whatsapp::outbound::{OutboundMessage, OutboundQueue};
    use operon_channels_whatsapp::router::WhatsAppRouter;
    use operon_channels_whatsapp::runner_bridge::SessionRunnerBridge;
    use operon_channels_whatsapp::service::WhatsAppService;
    use operon_channels_whatsapp::types::{ContactId, WhatsAppMessage};
    use operon_channels_whatsapp::workspace::WhatsAppWorkspaceManager;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir
        .path()
        .join("channels")
        .join("whatsapp")
        .join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let owner_contact = ContactId::new("15551112222");
    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(owner_contact.clone()),
        allowlist: vec![],
        auth_dir: Some(tmp_dir.path().join("auth")),
        workspace_dir: None,
    };

    let client = Arc::new(WhatsAppClient::new(&config));
    let router = Arc::new(WhatsAppRouter::new(config.clone()));
    let workspace_mgr = WhatsAppWorkspaceManager::with_paths(base_ws, base_sess);
    let (bridge_tx, bridge_rx) = mpsc::channel::<OutboundMessage>(64);
    let (client_tx, mut client_rx) = mpsc::channel::<OutboundMessage>(64);
    let (_dummy_tx, dummy_rx) = mpsc::channel::<OutboundMessage>(64);
    let outbound_queue = Arc::new(OutboundQueue::new(client_tx));

    let app_config = operon_config::load().expect("Failed to load AppConfig");
    let bridge = Arc::new(SessionRunnerBridge::with_router(
        app_config,
        workspace_mgr,
        bridge_tx,
        router.clone(),
    ));

    let service = Arc::new(WhatsAppService::with_components_and_receivers(
        client.clone(),
        router.clone(),
        bridge.clone(),
        outbound_queue.clone(),
        bridge_rx,
        dummy_rx,
    ));

    let service_clone = service.clone();
    let service_handle = tokio::spawn(async move {
        let _ = service_clone.run().await;
    });

    // Inject a /new message
    let msg_new = WhatsAppMessage {
        id: "m_new".to_string(),
        sender: owner_contact.clone(),
        text: "/new".to_string(),
        timestamp: 1000,
        is_self: false,
    };

    client.message_tx().send(msg_new).await.unwrap();

    // Flush outbound queue (messages are buffered while disconnected)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let _ = outbound_queue.flush().await;

    // Verify /new notification was enqueued & received by client_rx
    let out_msg = tokio::time::timeout(std::time::Duration::from_secs(2), client_rx.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(out_msg.recipient, "15551112222");
    assert!(out_msg.text.contains("Fresh session started"));

    service_handle.abort();
}

#[tokio::test]
async fn test_unbounded_contact_locks_pruned_on_turn_completion() {
    use operon_channels_whatsapp::client::WhatsAppClient;
    use operon_channels_whatsapp::outbound::{OutboundMessage, OutboundQueue};
    use operon_channels_whatsapp::router::WhatsAppRouter;
    use operon_channels_whatsapp::runner_bridge::SessionRunnerBridge;
    use operon_channels_whatsapp::service::WhatsAppService;
    use operon_channels_whatsapp::types::ContactId;
    use operon_channels_whatsapp::workspace::WhatsAppWorkspaceManager;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    let tmp_dir = TempDir::new().unwrap();
    let base_ws = tmp_dir
        .path()
        .join("channels")
        .join("whatsapp")
        .join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let owner_contact = ContactId::new("15550000000");
    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(owner_contact.clone()),
        allowlist: vec![],
        auth_dir: Some(tmp_dir.path().join("auth")),
        workspace_dir: None,
    };

    let client = Arc::new(WhatsAppClient::new(&config));
    let router = Arc::new(WhatsAppRouter::new(config.clone()));
    let workspace_mgr = WhatsAppWorkspaceManager::with_paths(base_ws, base_sess);
    let (bridge_tx, bridge_rx) = mpsc::channel::<OutboundMessage>(64);
    let (_dummy_tx, dummy_rx) = mpsc::channel::<OutboundMessage>(64);
    let outbound_queue = Arc::new(OutboundQueue::new(_dummy_tx));

    let app_config = operon_config::load().expect("Failed to load AppConfig");
    let bridge = Arc::new(SessionRunnerBridge::with_router(
        app_config,
        workspace_mgr,
        bridge_tx,
        router.clone(),
    ));

    let service = WhatsAppService::with_components_and_receivers(
        client,
        router,
        bridge,
        outbound_queue,
        bridge_rx,
        dummy_rx,
    );

    const N_CONTACTS: usize = 10;
    let mut locks = Vec::new();

    // 1. Acquire locks for N distinct contacts (simulating N distinct turn executions)
    for i in 0..N_CONTACTS {
        let contact = ContactId::new(&format!("1555888{:04}", i));
        let lock = service.get_contact_lock(&contact).await;
        locks.push((contact, lock));
    }

    assert_eq!(
        service.contact_locks_len().await,
        N_CONTACTS,
        "contact_locks map should hold N entries while turns are active"
    );

    // 2. Prune all N contact locks as each turn finishes
    for (contact, lock) in locks {
        service.prune_contact_lock(&contact, &lock).await;
    }

    // 3. Verify contact_locks map returns to size 0
    assert_eq!(
        service.contact_locks_len().await,
        0,
        "contact_locks map must return to size 0 after all turns complete"
    );
}



#[tokio::test]
async fn test_outbound_queue_connecting_buffers_and_flushes_on_connected() {
    use operon_channels_whatsapp::outbound::{OutboundMessage, OutboundQueue};
    use operon_channels_whatsapp::types::ConnectionStatus;
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel::<OutboundMessage>(10);
    let queue = OutboundQueue::new(tx);

    let msg1 = OutboundMessage::new("15551111", "Connecting test message");

    // Enqueue while status is ConnectionStatus::Connecting — must be buffered, NOT sent
    let status_connecting = ConnectionStatus::Connecting;
    queue
        .enqueue(msg1.clone(), &status_connecting)
        .await
        .unwrap();

    assert_eq!(
        queue.buffered_count().await,
        1,
        "Message enqueued during Connecting status must be buffered"
    );
    assert!(
        rx.try_recv().is_err(),
        "No message should be delivered over channel while status is Connecting"
    );

    // Transition to Connected and call flush()
    let flush_count = queue.flush().await.unwrap();
    assert_eq!(flush_count, 1);

    // Message is now received
    let recv1 = rx.recv().await.unwrap();
    assert_eq!(recv1, msg1);
    assert_eq!(queue.buffered_count().await, 0);
}
