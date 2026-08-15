//! Channels Root Settings Commands.

use super::types::ChannelCardDto;
use operon_rs::channels::whatsapp::auth::WhatsAppAuth;

/// Fetches summary cards for all supported channels.
#[tauri::command]
pub async fn get_channels_list() -> Result<Vec<ChannelCardDto>, String> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let default_wa_auth = home
        .join(".operon")
        .join("channels")
        .join("whatsapp")
        .join("auth");

    let auth_checker = WhatsAppAuth::new(default_wa_auth);
    let wa_has_creds = auth_checker.has_credentials();
    let wa_status = if wa_has_creds {
        "Connected".to_string()
    } else {
        "Disconnected".to_string()
    };

    Ok(vec![
        ChannelCardDto {
            id: "whatsapp".to_string(),
            label: "WhatsApp Channel".to_string(),
            status: wa_status,
            is_active: wa_has_creds,
            description: "Scan QR code or use a mobile pairing code to pair Operon with WhatsApp."
                .to_string(),
        },
        ChannelCardDto {
            id: "telegram".to_string(),
            label: "Telegram Channel".to_string(),
            status: "Disconnected".to_string(),
            is_active: false,
            description: "Connect Operon to Telegram to receive and execute commands via a bot."
                .to_string(),
        },
    ])
}
