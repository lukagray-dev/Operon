// service.rs — Central orchestration loop for Telegram channel.
//
// Hey friend! This module wires together `TelegramClient`, `TelegramRouter`,
// `SessionRunnerBridge`, and `OutboundQueue` into a unified `TelegramService`.
//
// It drains inbound messages from `TelegramClient`, routes them via `TelegramRouter`,
// enforces per-chat sequential execution of turns, sends `/new` notifications,
// and flushes outbound responses to the live Bot API HTTPS connection.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{error, info, warn};

use crate::client::TelegramClient;
use crate::config::TelegramConfig;
use crate::error::TelegramError;
use crate::outbound::{OutboundQueue, TelegramOutboundMessage};
use crate::router::{RouteOutcome, TelegramRouter};
use crate::runner_bridge::SessionRunnerBridge;
use crate::types::{ChatId, ConnectionStatus};
use crate::workspace::TelegramWorkspaceManager;
use operon_config::AppConfig;

/// Optional callback triggered when a turn completes for a chat.
type TurnCompleteCallback = Arc<dyn Fn() + Send + Sync>;

/// Orchestrates inbound Telegram message consumption, routing, turn execution,
/// and outbound message delivery.
pub struct TelegramService {
    client: Arc<TelegramClient>,
    router: Arc<TelegramRouter>,
    bridge: Arc<SessionRunnerBridge>,
    outbound_queue: Arc<OutboundQueue>,
    bridge_rx: Arc<AsyncMutex<Option<mpsc::Receiver<TelegramOutboundMessage>>>>,
    client_rx: Arc<AsyncMutex<Option<mpsc::Receiver<TelegramOutboundMessage>>>>,
    contact_locks: Arc<AsyncMutex<HashMap<ChatId, Arc<AsyncMutex<()>>>>>,
    on_turn_complete: Option<TurnCompleteCallback>,
}

impl TelegramService {
    /// Creates a new `TelegramService` with standard channel components.
    pub fn new(
        client: Arc<TelegramClient>,
        tg_config: TelegramConfig,
        app_config: AppConfig,
    ) -> Self {
        // Check policy coverage for the resolved workspace directory
        tg_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(TelegramRouter::new(tg_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("telegram");
        let workspace_manager = TelegramWorkspaceManager::with_paths(
            tg_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<TelegramOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<TelegramOutboundMessage>(64);
        let outbound_queue = Arc::new(OutboundQueue::new(client_tx));
        let bridge = Arc::new(SessionRunnerBridge::with_router(
            app_config,
            workspace_manager,
            bridge_tx,
            router.clone(),
        ));

        Self::with_components_and_receivers(
            client,
            router,
            bridge,
            outbound_queue,
            bridge_rx,
            client_rx,
        )
    }

    /// Creates a new `TelegramService` with an external `SessionEventHook`.
    pub fn with_event_hook(
        client: Arc<TelegramClient>,
        tg_config: TelegramConfig,
        app_config: AppConfig,
        event_hook: crate::runner_bridge::SessionEventHook,
    ) -> Self {
        tg_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(TelegramRouter::new(tg_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("telegram");
        let workspace_manager = TelegramWorkspaceManager::with_paths(
            tg_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<TelegramOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<TelegramOutboundMessage>(64);
        let outbound_queue = Arc::new(OutboundQueue::new(client_tx));
        let bridge = Arc::new(SessionRunnerBridge::with_router_and_hook(
            app_config,
            workspace_manager,
            bridge_tx,
            router.clone(),
            Some(event_hook),
        ));

        Self::with_components_and_receivers(
            client,
            router,
            bridge,
            outbound_queue,
            bridge_rx,
            client_rx,
        )
    }

    /// Creates a `TelegramService` with explicit pre-built components and channels.
    pub fn with_components_and_receivers(
        client: Arc<TelegramClient>,
        router: Arc<TelegramRouter>,
        bridge: Arc<SessionRunnerBridge>,
        outbound_queue: Arc<OutboundQueue>,
        bridge_rx: mpsc::Receiver<TelegramOutboundMessage>,
        client_rx: mpsc::Receiver<TelegramOutboundMessage>,
    ) -> Self {
        Self {
            client,
            router,
            bridge,
            outbound_queue,
            bridge_rx: Arc::new(AsyncMutex::new(Some(bridge_rx))),
            client_rx: Arc::new(AsyncMutex::new(Some(client_rx))),
            contact_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            on_turn_complete: None,
        }
    }

    /// Attaches an optional callback invoked when a turn finishes processing.
    pub fn with_on_turn_complete<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_turn_complete = Some(Arc::new(callback));
        self
    }

    /// Returns a reference to the underlying `TelegramClient`.
    pub fn client(&self) -> &Arc<TelegramClient> {
        &self.client
    }

    /// Returns a reference to the `TelegramRouter`.
    pub fn router(&self) -> &Arc<TelegramRouter> {
        &self.router
    }

    /// Returns a reference to the `SessionRunnerBridge`.
    pub fn bridge(&self) -> &Arc<SessionRunnerBridge> {
        &self.bridge
    }

    /// Returns a reference to the `OutboundQueue`.
    pub fn outbound_queue(&self) -> &Arc<OutboundQueue> {
        &self.outbound_queue
    }

    /// Retrieves or creates a per-chat mutex to guarantee sequential turn processing.
    pub async fn get_contact_lock(&self, chat: &ChatId) -> Arc<AsyncMutex<()>> {
        let mut locks = self.contact_locks.lock().await;
        locks
            .entry(*chat)
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    /// Prunes a contact lock after turn completion if no other tasks hold a reference to it.
    pub async fn prune_contact_lock(&self, chat: &ChatId, contact_lock: &Arc<AsyncMutex<()>>) {
        let mut locks = self.contact_locks.lock().await;
        if Arc::strong_count(contact_lock) == 2 {
            if let Some(entry) = locks.get(chat) {
                if Arc::ptr_eq(entry, contact_lock) {
                    locks.remove(chat);
                }
            }
        }
    }

    /// Runs the core Telegram service loop.
    pub async fn run(&self) -> Result<(), TelegramError> {
        // 1. Connect if not already running
        if !self.client.is_running() {
            info!("Telegram client event loop is not running. Initiating connect()...");
            self.client.connect().await?;
        } else {
            info!("Telegram client event loop is already running.");
        }

        // 2. Take the inbound message receiver
        let mut message_rx = self.client.take_message_receiver().await.ok_or_else(|| {
            error!("Inbound Telegram message receiver has already been consumed (double-init bug)");
            TelegramError::ConnectionFailed("Inbound message receiver already consumed".to_string())
        })?;

        // 3. Spawn outbound queue ingestion and delivery tasks
        let bridge_rx_option = self.bridge_rx.lock().await.take();
        if let Some(mut bridge_rx) = bridge_rx_option {
            let client = self.client.clone();
            let outbound_queue = self.outbound_queue.clone();
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(500));
                loop {
                    tokio::select! {
                        Some(msg) = bridge_rx.recv() => {
                            let st = client.status().await;
                            let _ = outbound_queue.enqueue(msg, &st).await;
                        }
                        _ = ticker.tick() => {
                            if matches!(client.status().await, ConnectionStatus::Connected)
                                && outbound_queue.buffered_count().await > 0
                            {
                                let _ = outbound_queue.flush().await;
                            }
                        }
                        else => break,
                    }
                }
            });
        }

        let client_rx_option = self.client_rx.lock().await.take();
        if let Some(mut client_rx) = client_rx_option {
            let client = self.client.clone();
            let outbound_queue = self.outbound_queue.clone();
            tokio::spawn(async move {
                while let Some(msg) = client_rx.recv().await {
                    info!(
                        chat_id = msg.chat_id,
                        text = %msg.text,
                        "Outbound message worker sending reply over Telegram HTTPS connection"
                    );
                    match client.send_message(msg.chat_id, &msg.text).await {
                        Ok(msg_id) => {
                            info!(
                                chat_id = msg.chat_id,
                                msg_id = %msg_id,
                                "Successfully delivered outbound Telegram message"
                            );
                        }
                        Err(TelegramError::NotConnected) => {
                            warn!("Client not connected when sending outbound Telegram message {:?}, re-enqueueing", msg);
                            let st = client.status().await;
                            let _ = outbound_queue.enqueue(msg, &st).await;
                        }
                        Err(e) => {
                            warn!("Failed to send outbound Telegram message {:?}: {}", msg, e);
                        }
                    }
                }
            });
        }

        info!("TelegramService orchestration loop active. Processing incoming messages...");

        // 4. Main message loop
        while let Some(msg) = message_rx.recv().await {
            info!(
                update_id = msg.update_id,
                message_id = msg.message_id,
                sender = %msg.sender,
                text = %msg.text,
                "TelegramService processing inbound message"
            );

            let outcome = self.router.route(&msg).await;
            match outcome {
                RouteOutcome::FreshSessionRequested {
                    chat,
                    new_session_id,
                    role,
                } => {
                    info!(
                        chat = %chat,
                        session_id = %new_session_id,
                        role = ?role,
                        "Fresh session requested via /new"
                    );
                    let notification =
                        TelegramOutboundMessage::new(chat.as_i64(), "✨ Fresh session started.");
                    let st = self.client.status().await;
                    let _ = self.outbound_queue.enqueue(notification, &st).await;
                }
                RouteOutcome::ProcessTurn {
                    chat,
                    session_id,
                    role,
                    is_first_time,
                } => {
                    info!(
                        chat = %chat,
                        session_id = %session_id,
                        role = ?role,
                        is_first_time = is_first_time,
                        "Dispatching Telegram turn for chat"
                    );

                    let contact_lock = self.get_contact_lock(&chat).await;
                    let bridge = self.bridge.clone();
                    let chat_clone = chat;
                    let session_clone = session_id.clone();
                    let user_text = msg.text.clone();
                    let on_turn_complete = self.on_turn_complete.clone();
                    let contact_locks = self.contact_locks.clone();

                    tokio::spawn(async move {
                        // Scope the per-chat mutex lock so it is released as soon as the turn finishes.
                        {
                            let _guard = contact_lock.lock().await;
                            if let Err(e) = bridge
                                .process_turn(
                                    &chat_clone,
                                    &session_clone,
                                    role,
                                    user_text,
                                    is_first_time,
                                )
                                .await
                            {
                                error!(
                                    "Error processing turn for Telegram chat {}: {}",
                                    chat_clone, e
                                );
                            }
                            if let Some(cb) = on_turn_complete {
                                cb();
                            }
                        }

                        // After turn finishes and per-chat mutex `_guard` is dropped, prune contact lock from map.
                        let mut locks = contact_locks.lock().await;
                        if Arc::strong_count(&contact_lock) == 2 {
                            if let Some(entry) = locks.get(&chat_clone) {
                                if Arc::ptr_eq(entry, &contact_lock) {
                                    locks.remove(&chat_clone);
                                }
                            }
                        }
                    });
                }
            }
        }

        info!("TelegramService message receiver closed. Orchestration loop exiting.");
        Ok(())
    }

    /// Returns the current number of active entries in `contact_locks` (useful for unit tests and diagnostics).
    pub async fn contact_locks_len(&self) -> usize {
        self.contact_locks.lock().await.len()
    }
}
