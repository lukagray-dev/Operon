// demo_whatsapp.rs — Demonstration example for operon-channels-whatsapp.
//
// Run via: `cargo run -p operon-channels-whatsapp --example demo_whatsapp`

use operon_channels_whatsapp::{
    ContactId, WhatsAppClient, WhatsAppConfig,
    WhatsAppMessage, WhatsAppRouter, WhatsAppWorkspaceManager,
};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Operon WhatsApp Channel Engine Demonstration...");

    let tmp_dir = TempDir::new()?;
    let base_ws = tmp_dir.path().join("channels").join("whatsapp").join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("whatsapp");

    let owner_contact = ContactId::new("+1 (555) 100-2000");
    let ext_contact = ContactId::new("+1 (555) 999-8888");

    let config = WhatsAppConfig {
        enabled: true,
        owner_number: Some(owner_contact.clone()),
        allowlist: Vec::new(),
        auth_dir: Some(tmp_dir.path().join("channels").join("whatsapp").join("auth")),
    };

    let router = WhatsAppRouter::new(config.clone());
    let workspace_mgr = WhatsAppWorkspaceManager::with_paths(base_ws, base_sess);

    // 1. Connect Client & Render QR Code
    let client = WhatsAppClient::new(config);
    client.connect().await?;

    println!("📡 Connection status: {:?}", client.status().await);

    // 2. Provision Workspaces & Role AGENTS.md
    let owner_ws = workspace_mgr.provision_workspace(&owner_contact, true)?;
    let ext_ws = workspace_mgr.provision_workspace(&ext_contact, false)?;

    println!("📁 Owner Workspace: {:?}", owner_ws);
    println!("📁 External Workspace: {:?}", ext_ws);

    // 3. Simulate Inbound Owner Message
    let owner_msg = WhatsAppMessage {
        id: "msg_owner_1".to_string(),
        sender: owner_contact.clone(),
        text: "Hello Operon! Create a new file for me.".to_string(),
        timestamp: 1700000000,
        is_self: false,
    };

    let outcome1 = router.route(&owner_msg).await;
    println!("💬 Routed Owner Message: {:?}", outcome1);

    // 4. Simulate Inbound /new Command
    let slash_new_msg = WhatsAppMessage {
        id: "msg_owner_2".to_string(),
        sender: owner_contact.clone(),
        text: "/new".to_string(),
        timestamp: 1700000100,
        is_self: false,
    };

    let outcome2 = router.route(&slash_new_msg).await;
    println!("✨ Routed /new Command: {:?}", outcome2);

    println!("✅ Operon WhatsApp Channel Engine Demonstration completed successfully!");
    Ok(())
}
