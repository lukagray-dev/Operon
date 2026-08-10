// demo_telegram.rs — Demonstration example for operon-channels-telegram.
//
// Run via: `cargo run -p operon-channels-telegram --example demo_telegram`

use operon_channels_telegram::{
    ChatId, ConnectionStatus, TelegramClient, TelegramConfig, TelegramMessage, TelegramRouter,
    TelegramWorkspaceManager,
};
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Operon Telegram Channel Engine Demonstration...");

    let tmp_dir = TempDir::new()?;
    let base_ws = tmp_dir
        .path()
        .join("channels")
        .join("telegram")
        .join("workspace");
    let base_sess = tmp_dir.path().join("sessions").join("telegram");

    let owner_chat = ChatId::new(1001);
    let ext_chat = ChatId::new(9999);

    let config = TelegramConfig {
        enabled: true,
        bot_token: Some("123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11".to_string()),
        owner_chat_id: Some(owner_chat),
        allowlist: Vec::new(),
        workspace_dir: Some(base_ws.clone()),
        poll_interval_secs: Some(30),
    };

    let router = TelegramRouter::new(config.clone());
    let workspace_mgr = TelegramWorkspaceManager::with_paths(base_ws, base_sess);

    // 1. Client initialization
    let client = TelegramClient::new(config);
    println!("📡 Connection status: {:?}", client.status().await);
    assert_eq!(client.status().await, ConnectionStatus::Disconnected);

    // 2. Provision Workspaces & Role Channel Instructions
    let owner_ws = workspace_mgr.provision_workspace(&owner_chat, true)?;
    let ext_ws = workspace_mgr.provision_workspace(&ext_chat, false)?;

    println!("📁 Shared Workspace: {:?}", owner_ws);
    println!("📁 External Workspace (same shared root): {:?}", ext_ws);

    // 3. Simulate Inbound Owner Message
    let owner_msg = TelegramMessage {
        update_id: 1,
        message_id: 101,
        sender: owner_chat,
        text: "Hello Operon! Create a new project for me.".to_string(),
        timestamp: 1700000000,
        is_self: false,
    };

    let outcome1 = router.route(&owner_msg).await;
    println!("💬 Routed Owner Message: {:?}", outcome1);

    // 4. Simulate Inbound /new Command
    let slash_new_msg = TelegramMessage {
        update_id: 2,
        message_id: 102,
        sender: owner_chat,
        text: "/new".to_string(),
        timestamp: 1700000100,
        is_self: false,
    };

    let outcome2 = router.route(&slash_new_msg).await;
    println!("✨ Routed /new Command: {:?}", outcome2);

    println!("✅ Operon Telegram Channel Engine Demonstration completed successfully!");
    Ok(())
}
