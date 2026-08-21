//! Native WhatsApp Web engine using `whatsapp-rust`.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{info, warn};

use parking_lot::RwLock;
use wacore::proto_helpers::MessageExt;
use wacore::store::DevicePropsOverride;
use wacore::types::events::Event;
use wacore_binary::jid::Jid;
use wacore_binary::jid::JidExt;
use waproto::whatsapp::device_props::PlatformType;
use whatsapp_rust::bot::Bot;
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::store::{Device, DeviceStore};
use whatsapp_rust::TokioRuntime;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

use crate::auth::WhatsAppAuth;
use crate::config::WhatsAppConfig;
use crate::error::WhatsAppError;
use crate::storage::{persisted_device_exists, RusqliteStore};
use crate::types::{ConnectionStatus, ContactId, PairingCodeState, QrCodeState, WhatsAppMessage};

/// High-level client managing WhatsApp Multi-Device connection, QR/Pairing code events,
/// and inbound/outbound message dispatching.
pub struct WhatsAppClient {
    /// Directory where auth credentials and SQLite session DB are stored.
    auth_dir: PathBuf,
    /// SQLite session DB file path (`auth_dir/session.db`).
    session_path: PathBuf,
    /// Phone number for pair code linking (optional).
    pair_phone: Option<String>,
    /// Bot phone number (digits only), resolved from owner_number or device identity at runtime.
    bot_phone: Arc<parking_lot::Mutex<Option<String>>>,
    /// Custom pair code (optional).
    pair_code: Option<String>,
    /// Override WebSocket URL (optional).
    ws_url: Option<String>,
    /// Active connection status shared thread-safely across components.
    status: Arc<RwLock<ConnectionStatus>>,
    /// Sender for QR code state updates sent to GUI/TUI.
    qr_tx: mpsc::Sender<QrCodeState>,
    /// Receiver for QR code state updates (consumed by caller).
    qr_rx: Arc<AsyncMutex<Option<mpsc::Receiver<QrCodeState>>>>,
    /// Sender for pairing code state updates issued by WhatsApp servers.
    pairing_code_tx: mpsc::Sender<PairingCodeState>,
    /// Receiver for pairing code state updates (consumed by GUI/TUI).
    pairing_code_rx: Arc<AsyncMutex<Option<mpsc::Receiver<PairingCodeState>>>>,
    /// Handle to the running `whatsapp-rust` bot.
    bot_handle: Arc<parking_lot::Mutex<Option<whatsapp_rust::bot::BotHandle>>>,
    /// Handle to the underlying `whatsapp-rust` client for outbound messaging.
    client: Arc<parking_lot::Mutex<Option<Arc<whatsapp_rust::Client>>>>,
    /// Inbound message sender.
    message_tx: mpsc::Sender<WhatsAppMessage>,
    /// Inbound message receiver (consumed by router).
    message_rx: Arc<AsyncMutex<Option<mpsc::Receiver<WhatsAppMessage>>>>,
    /// Sent message IDs to filter out bot outbound echoes.
    sent_message_ids: Arc<parking_lot::Mutex<std::collections::HashSet<String>>>,
    /// Last exact WhatsApp chat JID observed for each normalized contact.
    ///
    /// Replies must use the original WhatsApp namespace when available. In
    /// Multi-Device mode, inbound chats can arrive as LID JIDs; rebuilding a
    /// phone-number JID from digits can target the wrong namespace.
    reply_targets: Arc<parking_lot::Mutex<std::collections::HashMap<String, String>>>,
}

impl WhatsAppClient {
    /// Creates a new `WhatsAppClient` configured with the provided settings.
    pub fn new(config: &WhatsAppConfig) -> Self {
        let auth_dir = config.resolved_auth_dir();
        let session_path = auth_dir.join("session.db");

        let (qr_tx, qr_rx) = mpsc::channel(16);
        // Channel for real pairing codes issued by WhatsApp servers (capacity 4
        // is enough — only one code is active at a time, but re-issuances happen).
        let (pairing_code_tx, pairing_code_rx) = mpsc::channel(4);
        let (message_tx, message_rx) = mpsc::channel(64);

        let initial_status = if persisted_device_exists(&session_path) {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        };

        let initial_bot_phone = config.owner_number.as_ref().map(|c| c.as_str().to_string());

        Self {
            auth_dir,
            session_path,
            pair_phone: initial_bot_phone.clone(),
            bot_phone: Arc::new(parking_lot::Mutex::new(initial_bot_phone)),
            pair_code: None,
            ws_url: None,
            status: Arc::new(RwLock::new(initial_status)),
            qr_tx,
            qr_rx: Arc::new(AsyncMutex::new(Some(qr_rx))),
            pairing_code_tx,
            pairing_code_rx: Arc::new(AsyncMutex::new(Some(pairing_code_rx))),
            bot_handle: Arc::new(parking_lot::Mutex::new(None)),
            client: Arc::new(parking_lot::Mutex::new(None)),
            message_tx,
            message_rx: Arc::new(AsyncMutex::new(Some(message_rx))),
            sent_message_ids: Arc::new(parking_lot::Mutex::new(std::collections::HashSet::new())),
            reply_targets: Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Returns true if the client event loop has been started via `connect()`.
    pub fn is_running(&self) -> bool {
        self.bot_handle.lock().is_some()
    }

    /// Returns the active connection status.
    pub async fn status(&self) -> ConnectionStatus {
        self.status.read().clone()
    }

    /// Takes the QR code receiver if not already taken.
    pub async fn take_qr_receiver(&self) -> Option<mpsc::Receiver<QrCodeState>> {
        self.qr_rx.lock().await.take()
    }

    /// Takes the pairing code receiver so the GUI/TUI can display the real
    /// server-issued code. Call this before `connect()` to avoid missing events.
    pub async fn take_pairing_code_receiver(&self) -> Option<mpsc::Receiver<PairingCodeState>> {
        self.pairing_code_rx.lock().await.take()
    }

    /// Takes the inbound message receiver so `router.rs` can consume incoming messages.
    pub async fn take_message_receiver(&self) -> Option<mpsc::Receiver<WhatsAppMessage>> {
        self.message_rx.lock().await.take()
    }

    /// Returns a sender handle to the inbound message channel (used for testing and message injection).
    pub fn message_tx(&self) -> mpsc::Sender<WhatsAppMessage> {
        self.message_tx.clone()
    }

    /// Connects to WhatsApp Web and runs the bot client event loop.
    pub async fn connect(&self) -> Result<(), WhatsAppError> {
        // Immediately transition to Connecting so any status pollers (e.g. the
        // GUI's 500ms poller) see a non-Disconnected state. This MUST happen
        // before any await point to win the race against the poller start.
        *self.status.write() = ConnectionStatus::Connecting;

        let auth = WhatsAppAuth::new(self.auth_dir.clone());
        auth.init()?;

        info!(
            session_path = %self.session_path.display(),
            "Starting WhatsApp Web client connection"
        );

        let storage = RusqliteStore::new(&self.session_path)
            .map_err(|e| WhatsAppError::AuthFailed(e.to_string()))?;
        let backend = Arc::new(storage);

        let mut device = Device::new(backend.clone());
        if backend.exists().await.unwrap_or(false) {
            if let Ok(Some(core_device)) = backend.load().await {
                if let Some(ref pn_jid) = core_device.pn {
                    let phone = pn_jid.user().to_string();
                    info!(owner_phone = %phone, "Loaded owner phone number from device identity");
                    *self.bot_phone.lock() = Some(phone);
                }
                device.load_from_serializable(core_device);
            }
        }

        let mut transport_factory = TokioWebSocketTransportFactory::new();
        if let Some(ref url) = self.ws_url {
            transport_factory = transport_factory.with_url(url.clone());
        }

        let http_client = UreqHttpClient::new();

        let status_clone = self.status.clone();
        let qr_tx_clone = self.qr_tx.clone();
        let pairing_code_tx_clone = self.pairing_code_tx.clone();
        let message_tx_clone = self.message_tx.clone();
        let sent_message_ids_clone = self.sent_message_ids.clone();
        let reply_targets_clone = self.reply_targets.clone();
        let bot_phone_clone = self.bot_phone.clone();
        let backend_clone = backend.clone();

        let mut builder = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(transport_factory)
            .with_http_client(http_client)
            .with_runtime(TokioRuntime)
            .with_device_props(
                DevicePropsOverride::new()
                    .with_os("Operon")
                    .with_platform_type(PlatformType::Desktop),
            )
            .on_event(move |event, client| {
                let status = status_clone.clone();
                let qr_tx = qr_tx_clone.clone();
                let pairing_code_tx = pairing_code_tx_clone.clone();
                let message_tx = message_tx_clone.clone();
                let sent_ids = sent_message_ids_clone.clone();
                let reply_targets = reply_targets_clone.clone();
                let bot_phone = bot_phone_clone.clone();
                let backend = backend_clone.clone();

                async move {
                    match event.as_ref() {
                        Event::Message(msg, info) => {
                            let is_connected = matches!(*status.read(), ConnectionStatus::Connected);
                            if !is_connected {
                                *status.write() = ConnectionStatus::Connected;
                            }

                            // Check if this message was sent by Operon itself
                            let is_bot_echo = sent_ids.lock().remove(&info.id);
                            if is_bot_echo {
                                info!(id = %info.id, "Ignoring outbound message echo sent by Operon bot");
                                return;
                            }

                            // Forward non-empty text messages to the inbound channel
                            let raw_text = msg
                                .text_content()
                                .map(|s| s.to_string())
                                .or_else(|| msg.conversation.clone())
                                .or_else(|| msg.extended_text_message.as_ref().and_then(|m| m.text.clone()));

                            if let Some(text) = raw_text {
                                let text = text.trim().to_string();
                                if !text.is_empty() {
                                    let sender_jid = info.source.sender.clone();
                                    let chat_jid = info.source.chat.clone();
                                    let active_bot_phone = bot_phone.lock().clone();
                                    let chat_phone_from_mapping = if chat_jid.is_lid() {
                                        match client.get_lid_pn_entry(&chat_jid).await {
                                            Ok(Some(entry)) => Some(entry.phone_number),
                                            Ok(None) => None,
                                            Err(e) => {
                                                warn!(
                                                    chat = %chat_jid,
                                                    "Failed to resolve WhatsApp LID chat to phone number: {}",
                                                    e
                                                );
                                                None
                                            }
                                        }
                                    } else {
                                        None
                                    };
                                    let sender_phone_from_mapping = if sender_jid.is_lid() {
                                        match client.get_lid_pn_entry(&sender_jid).await {
                                            Ok(Some(entry)) => Some(entry.phone_number),
                                            Ok(None) => None,
                                            Err(e) => {
                                                warn!(
                                                    sender = %sender_jid,
                                                    "Failed to resolve WhatsApp LID sender to phone number: {}",
                                                    e
                                                );
                                                None
                                            }
                                        }
                                    } else {
                                        None
                                    };

                                    let resolved_phone = resolve_contact_phone(
                                        &chat_jid,
                                        &sender_jid,
                                        chat_phone_from_mapping,
                                        sender_phone_from_mapping,
                                        info.source.is_from_me,
                                        active_bot_phone.as_deref(),
                                    );

                                    let contact = ContactId::new(&resolved_phone);
                                    reply_targets
                                        .lock()
                                        .insert(contact.as_str().to_string(), chat_jid.to_string());

                                    info!(
                                        id = %info.id,
                                        sender = %resolved_phone,
                                        raw_sender = %sender_jid.user(),
                                        reply_target = %chat_jid,
                                        is_from_me = info.source.is_from_me,
                                        text = %text,
                                        "Inbound WhatsApp text message received"
                                    );
                                    let wa_msg = WhatsAppMessage {
                                        id: info.id.clone(),
                                        sender: contact,
                                        text,
                                        timestamp: info.timestamp.timestamp(),
                                        is_self: info.source.is_from_me,
                                    };
                                    if let Err(e) = message_tx.try_send(wa_msg) {
                                        warn!("Failed to forward inbound WhatsApp message to channel: {}", e);
                                    }
                                }
                            }
                        }
                        Event::PairingQrCode { code, .. } => {
                            // Real QR payload from WhatsApp — convert to QrCodeState
                            // and push to both the shared status and the QR channel.
                            let qr_state = WhatsAppAuth::generate_qr_state(code, 60);
                            *status.write() = ConnectionStatus::QrRequired(qr_state.clone());
                            let _ = qr_tx.try_send(qr_state);
                        }
                        Event::PairingCode { code, .. } => {
                            // Real pairing code from WhatsApp servers. Format as
                            // XXXX-XXXX if the server sent it as a raw 8-char string.
                            let raw = code.as_str();
                            let formatted = if raw.len() == 8 && !raw.contains('-') {
                                format!("{}-{}", &raw[..4], &raw[4..])
                            } else {
                                raw.to_string()
                            };
                            // Pairing codes typically expire ~160 seconds after issuance.
                            let expires_at = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64
                                + 160;
                            let state = PairingCodeState {
                                code: formatted,
                                expires_at,
                            };
                            info!(
                                code = %state.code,
                                expires_at = state.expires_at,
                                "WhatsApp pairing code received from server"
                            );
                            *status.write() =
                                ConnectionStatus::PairingCodeIssued(state.clone());
                            let _ = pairing_code_tx.try_send(state);
                        }
                        Event::Connected(_) => {
                            info!("WhatsApp Web connected successfully");
                            *status.write() = ConnectionStatus::Connected;

                            if bot_phone.lock().is_none() {
                                let bot_phone = bot_phone.clone();
                                let backend = backend.clone();
                                tokio::spawn(async move {
                                    if let Ok(Some(core_device)) = backend.load().await {
                                        if let Some(ref pn_jid) = core_device.pn {
                                            let phone = pn_jid.user().to_string();
                                            info!(owner_phone = %phone, "Resolved owner phone number on Event::Connected");
                                            *bot_phone.lock() = Some(phone);
                                        }
                                    }
                                });
                            }
                        }
                        Event::LoggedOut(_) => {
                            warn!("WhatsApp Web logged out");
                            *status.write() = ConnectionStatus::Disconnected;
                        }
                        _ => {}
                    }
                }
            });

        if let Some(ref phone) = self.pair_phone {
            let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                builder = builder.with_pair_code(PairCodeOptions {
                    phone_number: digits,
                    custom_code: self.pair_code.clone(),
                    ..Default::default()
                });
            }
        }

        let mut bot = builder
            .build()
            .await
            .map_err(|e| WhatsAppError::ConnectionFailed(e.to_string()))?;

        *self.client.lock() = Some(bot.client());

        let handle = bot
            .run()
            .await
            .map_err(|e| WhatsAppError::ConnectionFailed(e.to_string()))?;

        *self.bot_handle.lock() = Some(handle);

        Ok(())
    }

    /// Disconnects the active WhatsApp client session.
    pub async fn disconnect(&self) {
        if let Some(handle) = self.bot_handle.lock().take() {
            handle.abort();
        }
        *self.client.lock() = None;
        *self.status.write() = ConnectionStatus::Disconnected;
        info!("WhatsApp Web disconnected");
    }

    /// Sends a text message to a specific contact JID or phone number over the active socket.
    pub async fn send_message(&self, recipient: &str, text: &str) -> Result<String, WhatsAppError> {
        let client = self
            .client
            .lock()
            .clone()
            .ok_or(WhatsAppError::NotConnected)?;

        let remembered_target = if recipient.contains('@') {
            None
        } else {
            self.reply_targets.lock().get(recipient).cloned()
        };
        let target = remembered_target.as_deref().unwrap_or(recipient);
        let to_jid = if target.contains('@') {
            target
                .parse::<Jid>()
                .map_err(|_| WhatsAppError::InvalidContact(target.to_string()))?
        } else {
            let digits: String = target.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                return Err(WhatsAppError::InvalidContact(target.to_string()));
            }
            Jid::pn(digits)
        };
        let outgoing = waproto::whatsapp::Message {
            conversation: Some(text.to_string()),
            ..Default::default()
        };

        let result = client
            .send_message(to_jid, outgoing)
            .await
            .map_err(|e| WhatsAppError::SendFailed(e.to_string()))?;

        self.sent_message_ids
            .lock()
            .insert(result.message_id.clone());

        Ok(result.message_id)
    }
}

/// Resolves actual phone number string from WhatsApp chat/sender JIDs, optional LID mappings, and message source flags.
///
/// Hey newbie friend! In WhatsApp Multi-Device mode, contacts may arrive using LID (Linked Device / Hidden User) JIDs
/// with `@lid` server domain suffixes. Protocol-correct LID detection checks `!jid.is_lid()`, NEVER arbitrary numeric prefixes.
pub fn resolve_contact_phone(
    chat_jid: &Jid,
    sender_jid: &Jid,
    chat_phone_from_mapping: Option<String>,
    sender_phone_from_mapping: Option<String>,
    is_from_me: bool,
    active_bot_phone: Option<&str>,
) -> String {
    let chat_user = chat_jid.user();
    let sender_user = sender_jid.user();

    if let Some(phone) = chat_phone_from_mapping {
        phone
    } else if let Some(phone) = sender_phone_from_mapping {
        phone
    } else if !chat_user.is_empty() && !chat_jid.is_lid() {
        chat_user.to_string()
    } else if !sender_user.is_empty() && !sender_jid.is_lid() {
        sender_user.to_string()
    } else if is_from_me {
        active_bot_phone
            .map(|s| s.to_string())
            .unwrap_or_else(|| chat_user.to_string())
    } else if !chat_user.is_empty() {
        warn!(
            chat = %chat_jid,
            sender = %sender_jid,
            "Using WhatsApp chat user as contact because no phone mapping was available"
        );
        chat_user.to_string()
    } else if !sender_user.is_empty() {
        warn!(
            chat = %chat_jid,
            sender = %sender_jid,
            "Using WhatsApp sender user as contact because no phone mapping was available"
        );
        sender_user.to_string()
    } else {
        warn!(
            chat_user = %chat_user,
            sender_user = %sender_user,
            "Fallback to chat_user for resolved_phone: both chat_user and sender_user were empty/LID-prefixed and bot_phone is None"
        );
        chat_user.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::jid::Jid;

    #[test]
    fn test_resolve_contact_phone_lid_vs_phone_number() {
        let phone_jid: Jid = "15558889999@s.whatsapp.net".parse().unwrap();
        let lid_jid: Jid = "23888123456789@lid".parse().unwrap();

        // 1. Standard phone JID (@s.whatsapp.net) accepted directly without mapping
        let resolved = resolve_contact_phone(&phone_jid, &phone_jid, None, None, false, None);
        assert_eq!(resolved, "15558889999");

        // 2. LID JID (@lid) without mapping is rejected by direct phone branch and falls back to warning branch
        let resolved_lid_no_map =
            resolve_contact_phone(&lid_jid, &lid_jid, None, None, false, None);
        assert_eq!(resolved_lid_no_map, "23888123456789");

        // 3. LID JID (@lid) with mapping returns mapped phone number
        let resolved_lid_with_map = resolve_contact_phone(
            &lid_jid,
            &lid_jid,
            Some("15557776666".to_string()),
            None,
            false,
            None,
        );
        assert_eq!(resolved_lid_with_map, "15557776666");

        // 4. Valid phone number starting with "23888" (e.g. Sierra Leone +232 or similar 23888 prefix)
        // Must be accepted directly because its server domain is @s.whatsapp.net, not @lid.
        let valid_23888_jid: Jid = "23888999999@s.whatsapp.net".parse().unwrap();
        let resolved_23888 =
            resolve_contact_phone(&valid_23888_jid, &valid_23888_jid, None, None, false, None);
        assert_eq!(
            resolved_23888, "23888999999",
            "Valid phone numbers starting with '23888' must not be rejected when server is @s.whatsapp.net"
        );
    }
}
