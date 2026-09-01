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

    let tg_saved = crate::shared::channels_manager::load_telegram_saved_config();
    let tg_has_token = !tg_saved.bot_token.trim().is_empty();
    let tg_status = if tg_has_token {
        "Connected".to_string()
    } else {
        "Disconnected".to_string()
    };

    let dc_saved = crate::shared::channels_manager::load_discord_saved_config();
    let dc_has_token = !dc_saved.bot_token.trim().is_empty();
    let dc_status = if dc_has_token {
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
            status: tg_status,
            is_active: tg_has_token,
            description: "Connect Operon to Telegram to receive and execute commands via a bot."
                .to_string(),
        },
        ChannelCardDto {
            id: "discord".to_string(),
            label: "Discord Channel".to_string(),
            status: dc_status,
            is_active: dc_has_token,
            description: "Connect Operon to Discord to receive and execute commands via a bot."
                .to_string(),
        },
    ])
}
