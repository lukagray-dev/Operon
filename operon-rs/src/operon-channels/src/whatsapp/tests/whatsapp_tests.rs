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
        RouteOutcome::FreshSessionRequested { contact, new_session_id } => {
            assert_eq!(contact, owner_num);
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
