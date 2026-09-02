// service.rs — Central orchestration loop for Feishu / Lark channel.
//
// Hey friend! This module coordinates inbound Feishu messages, routing,
// sequential turn execution per user, and outbound message delivery.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{error, info, warn};

use crate::client::FeishuClient;
use crate::config::FeishuConfig;
use crate::error::FeishuError;
use crate::outbound::{FeishuOutboundMessage, OutboundQueue};
use crate::router::{FeishuRouter, RouteOutcome};
use crate::runner_bridge::SessionRunnerBridge;
use crate::types::{ConnectionStatus, UserId};
use crate::workspace::FeishuWorkspaceManager;
use operon_config::AppConfig;

/// Optional callback triggered when a turn completes for a user.
type TurnCompleteCallback = Arc<dyn Fn() + Send + Sync>;

/// Orchestrates inbound Feishu message consumption, routing, turn execution,
/// and outbound message delivery.
pub struct FeishuService {
    client: Arc<FeishuClient>,
    router: Arc<FeishuRouter>,
    bridge: Arc<SessionRunnerBridge>,
    outbound_queue: Arc<OutboundQueue>,
    bridge_rx: Arc<AsyncMutex<Option<mpsc::Receiver<FeishuOutboundMessage>>>>,
    client_rx: Arc<AsyncMutex<Option<mpsc::Receiver<FeishuOutboundMessage>>>>,
    user_locks: Arc<AsyncMutex<HashMap<UserId, Arc<AsyncMutex<()>>>>>,
    on_turn_complete: Option<TurnCompleteCallback>,
}

impl FeishuService {
    /// Creates a new `FeishuService` with standard channel components.
    pub fn new(
        client: Arc<FeishuClient>,
        fs_config: FeishuConfig,
        app_config: AppConfig,
    ) -> Self {
        // Check policy coverage for the resolved workspace directory
        fs_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(FeishuRouter::new(fs_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("feishu");
        let workspace_manager = FeishuWorkspaceManager::with_paths(
            fs_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<FeishuOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<FeishuOutboundMessage>(64);
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

    /// Creates a new `FeishuService` with an external `SessionEventHook`.
    pub fn with_event_hook(
        client: Arc<FeishuClient>,
        fs_config: FeishuConfig,
        app_config: AppConfig,
        event_hook: crate::runner_bridge::SessionEventHook,
    ) -> Self {
        fs_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(FeishuRouter::new(fs_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("feishu");
        let workspace_manager = FeishuWorkspaceManager::with_paths(
            fs_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<FeishuOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<FeishuOutboundMessage>(64);
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

    /// Creates a `FeishuService` with explicit pre-built components and channels.
    pub fn with_components_and_receivers(
        client: Arc<FeishuClient>,
        router: Arc<FeishuRouter>,
        bridge: Arc<SessionRunnerBridge>,
        outbound_queue: Arc<OutboundQueue>,
        bridge_rx: mpsc::Receiver<FeishuOutboundMessage>,
        client_rx: mpsc::Receiver<FeishuOutboundMessage>,
    ) -> Self {
        Self {
            client,
            router,
            bridge,
            outbound_queue,
            bridge_rx: Arc::new(AsyncMutex::new(Some(bridge_rx))),
            client_rx: Arc::new(AsyncMutex::new(Some(client_rx))),
            user_locks: Arc::new(AsyncMutex::new(HashMap::new())),
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

    /// Returns a reference to the underlying `FeishuClient`.
    pub fn client(&self) -> &Arc<FeishuClient> {
        &self.client
    }

    /// Returns a reference to the `FeishuRouter`.
    pub fn router(&self) -> &Arc<FeishuRouter> {
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

    /// Retrieves or creates a per-user mutex to guarantee sequential turn processing.
    pub async fn get_user_lock(&self, user_id: &UserId) -> Arc<AsyncMutex<()>> {
        let mut locks = self.user_locks.lock().await;
        locks
            .entry(user_id.clone())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    /// Runs the core Feishu service loop.
    pub async fn run(&self) -> Result<(), FeishuError> {
        // 1. Connect if not already running
        if !self.client.is_running() {
            info!("Feishu client is not running. Initiating connect()...");
            self.client.connect().await?;
        } else {
            info!("Feishu client is already running.");
        }

        // 2. Take the inbound message receiver
        let mut message_rx = self.client.take_message_receiver().await.ok_or_else(|| {
            error!("Inbound Feishu message receiver has already been consumed");
            FeishuError::ConnectionFailed("Inbound message receiver already consumed".to_string())
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
                        receive_id = %msg.receive_id,
                        text = %msg.text,
                        reply_to = ?msg.reply_to_message_id,
                        "Outbound message worker delivering reply to Feishu"
                    );
                    match client
                        .send_message(
                            &msg.receive_id,
                            &msg.text,
                            msg.reply_to_message_id.as_deref(),
                        )
                        .await
                    {
                        Ok(msg_id) => {
                            info!(
                                receive_id = %msg.receive_id,
                                message_id = %msg_id,
                                "Successfully delivered outbound Feishu message"
                            );
                        }
                        Err(FeishuError::NotConnected) => {
                            warn!("Client not connected when sending outbound Feishu message, re-enqueueing");
                            let st = client.status().await;
                            let _ = outbound_queue.enqueue(msg, &st).await;
                        }
                        Err(e) => {
                            warn!("Failed to send outbound Feishu message: {}", e);
                        }
                    }
                }
            });
        }

        info!("FeishuService orchestration loop active. Processing incoming messages...");

        // 4. Main message loop
        while let Some(msg) = message_rx.recv().await {
            info!(
                id = %msg.id,
                chat = %msg.chat_id,
                author = %msg.author_id,
                text = %msg.text,
                "FeishuService processing inbound message"
            );

            let outcome = self.router.route(&msg).await;
            match outcome {
                RouteOutcome::FreshSessionRequested {
                    user_id,
                    message_id,
                    new_session_id,
                    role,
                    ..
                } => {
                    info!(
                        user_id = %user_id,
                        session_id = %new_session_id,
                        role = ?role,
                        "Fresh Feishu session requested via /new"
                    );
                    let notification = FeishuOutboundMessage::new_reply(
                        user_id.as_str(),
                        "✨ Fresh session started.",
                        Some(message_id),
                    );
                    let st = self.client.status().await;
                    let _ = self.outbound_queue.enqueue(notification, &st).await;
                }
                RouteOutcome::ProcessTurn {
                    user_id,
                    chat_id,
                    session_id,
                    message_id,
                    role,
                    is_first_time,
                } => {
                    info!(
                        user_id = %user_id,
                        chat_id = %chat_id,
                        session_id = %session_id,
                        role = ?role,
                        is_first_time = is_first_time,
                        "Dispatching Feishu turn for user"
                    );

                    let user_lock = self.get_user_lock(&user_id).await;
                    let bridge = self.bridge.clone();
                    let user_clone = user_id.clone();
                    let chat_clone = chat_id.clone();
                    let session_clone = session_id.clone();
                    let msg_id_clone = Some(message_id);
                    let user_text = msg.text.clone();
                    let on_turn_complete = self.on_turn_complete.clone();
                    let user_locks = self.user_locks.clone();

                    tokio::spawn(async move {
                        {
                            let _guard = user_lock.lock().await;
                            if let Err(e) = bridge
                                .process_turn(
                                    &user_clone,
                                    &chat_clone,
                                    &session_clone,
                                    msg_id_clone,
                                    role,
                                    user_text,
                                    is_first_time,
                                )
                                .await
                            {
                                error!(
                                    "Error processing turn for Feishu user {}: {}",
                                    user_clone, e
                                );
                            }
                            if let Some(cb) = on_turn_complete {
                                cb();
                            }
                        }

                        // Cleanup user lock if no other turns are queued
                        let mut locks = user_locks.lock().await;
                        if Arc::strong_count(&user_lock) == 2 {
                            if let Some(entry) = locks.get(&user_clone) {
                                if Arc::ptr_eq(entry, &user_lock) {
                                    locks.remove(&user_clone);
                                }
                            }
                        }
                    });
                }
            }
        }

        info!("FeishuService message receiver closed. Orchestration loop exiting.");
        Ok(())
    }

    /// Returns the current number of active entries in `user_locks`.
    pub async fn user_locks_len(&self) -> usize {
        self.user_locks.lock().await.len()
    }
}

