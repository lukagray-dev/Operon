//! Native WhatsApp Web engine using `whatsapp-rust`.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{info, warn};

use parking_lot::RwLock;
use wacore::proto_helpers::MessageExt;
use wacore::store::DevicePropsOverride;
use wacore::types::events::Event;
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
use crate::types::{ConnectionStatus, ContactId, QrCodeState, WhatsAppMessage};

/// High-level client managing WhatsApp Multi-Device connection, QR/Pairing code events,
/// and inbound/outbound message dispatching.
pub struct WhatsAppClient {
    /// Directory where auth credentials and SQLite session DB are stored.
    auth_dir: PathBuf,
    /// SQLite session DB file path (`auth_dir/session.db`).
    session_path: PathBuf,
    /// Phone number for pair code linking (optional).
    pair_phone: Option<String>,
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
    /// Handle to the running `whatsapp-rust` bot.
    bot_handle: Arc<parking_lot::Mutex<Option<whatsapp_rust::bot::BotHandle>>>,
    /// Handle to the underlying `whatsapp-rust` client for outbound messaging.
    client: Arc<parking_lot::Mutex<Option<Arc<whatsapp_rust::Client>>>>,
    /// Inbound message sender.
    message_tx: mpsc::Sender<WhatsAppMessage>,
    /// Inbound message receiver (consumed by router).
    message_rx: Arc<AsyncMutex<Option<mpsc::Receiver<WhatsAppMessage>>>>,
}

impl WhatsAppClient {
    /// Creates a new `WhatsAppClient` configured with the provided settings.
    pub fn new(config: &WhatsAppConfig) -> Self {
        let auth_dir = config
            .auth_dir
            .clone()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".operon/channels/whatsapp/auth"));
        let session_path = auth_dir.join("session.db");

        let (qr_tx, qr_rx) = mpsc::channel(16);
        let (message_tx, message_rx) = mpsc::channel(64);

        let initial_status = if persisted_device_exists(&session_path) {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        };

        Self {
            auth_dir,
            session_path,
            pair_phone: config.owner_number.as_ref().map(|c| c.as_str().to_string()),
            pair_code: None,
            ws_url: None,
            status: Arc::new(RwLock::new(initial_status)),
            qr_tx,
            qr_rx: Arc::new(AsyncMutex::new(Some(qr_rx))),
            bot_handle: Arc::new(parking_lot::Mutex::new(None)),
            client: Arc::new(parking_lot::Mutex::new(None)),
            message_tx,
            message_rx: Arc::new(AsyncMutex::new(Some(message_rx))),
        }
    }

    /// Sets an optional override WebSocket URL (for proxies or custom relays).
    pub fn with_ws_url(mut self, url: impl Into<String>) -> Self {
        self.ws_url = Some(url.into());
        self
    }

    /// Sets an optional pair code.
    pub fn with_pair_code(mut self, code: impl Into<String>) -> Self {
        self.pair_code = Some(code.into());
        self
    }

    /// Returns the active connection status.
    pub async fn status(&self) -> ConnectionStatus {
        self.status.read().clone()
    }

    /// Takes the QR code receiver if not already taken.
    pub async fn take_qr_receiver(&self) -> Option<mpsc::Receiver<QrCodeState>> {
        self.qr_rx.lock().await.take()
    }

    /// Takes the inbound message receiver so `router.rs` can consume incoming messages.
    pub async fn take_message_receiver(&self) -> Option<mpsc::Receiver<WhatsAppMessage>> {
        self.message_rx.lock().await.take()
    }

    /// Connects to WhatsApp Web and runs the bot client event loop.
    pub async fn connect(&self) -> Result<(), WhatsAppError> {
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
        let message_tx_clone = self.message_tx.clone();

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
            .on_event(move |event, _client| {
                let status = status_clone.clone();
                let qr_tx = qr_tx_clone.clone();
                let message_tx = message_tx_clone.clone();

                async move {
                    match event.as_ref() {
                        Event::Message(msg, info) => {
                            if let Some(text) = msg.text_content() {
                                let text = text.trim().to_string();
                                if !text.is_empty() {
                                    let sender_jid = info.source.sender.user();
                                    let wa_msg = WhatsAppMessage {
                                        id: info.id.clone(),
                                        sender: ContactId::new(sender_jid),
                                        text,
                                        timestamp: info.timestamp.timestamp(),
                                        is_self: info.source.is_from_me,
                                    };
                                    let _ = message_tx.try_send(wa_msg);
                                }
                            }
                        }
                        Event::PairingQrCode { code, .. } => {
                            let qr_state = WhatsAppAuth::generate_qr_state(code, 60);
                            *status.write() = ConnectionStatus::QrRequired(qr_state.clone());
                            let _ = qr_tx.try_send(qr_state);
                        }
                        Event::PairingCode { .. } => {
                            *status.write() = ConnectionStatus::Connecting;
                        }
                        Event::Connected(_) => {
                            info!("WhatsApp Web connected successfully");
                            *status.write() = ConnectionStatus::Connected;
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
            .ok_or_else(|| WhatsAppError::NotConnected)?;

        let digits: String = recipient.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return Err(WhatsAppError::InvalidContact(recipient.to_string()));
        }

        let to_jid = wacore_binary::jid::Jid::pn(digits);
        let outgoing = waproto::whatsapp::Message {
            conversation: Some(text.to_string()),
            ..Default::default()
        };

        let result = client
            .send_message(to_jid, outgoing)
            .await
            .map_err(|e| WhatsAppError::SendFailed(e.to_string()))?;

        Ok(result.message_id)
    }
}
