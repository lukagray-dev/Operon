// service.rs — Central orchestration loop for Discord channel.
//
// Hey friend! This module wires together `DiscordClient`, `DiscordRouter`,
// `SessionRunnerBridge`, and `OutboundQueue` into a unified `DiscordService`.
//
// It drains inbound messages from `DiscordClient`, routes them via `DiscordRouter`,
// enforces per-user sequential execution of turns, sends `/new` notifications,
// and flushes outbound responses to the Discord channel via REST API.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{error, info, warn};

use crate::client::DiscordClient;
use crate::config::DiscordConfig;
use crate::error::DiscordError;
use crate::outbound::{DiscordOutboundMessage, OutboundQueue};
use crate::router::{DiscordRouter, RouteOutcome};
use crate::runner_bridge::SessionRunnerBridge;
use crate::types::{ConnectionStatus, UserId};
use crate::workspace::DiscordWorkspaceManager;
use operon_config::AppConfig;

/// Optional callback triggered when a turn completes for a user.
type TurnCompleteCallback = Arc<dyn Fn() + Send + Sync>;

/// Orchestrates inbound Discord message consumption, routing, turn execution,
/// and outbound message delivery.
pub struct DiscordService {
    client: Arc<DiscordClient>,
    router: Arc<DiscordRouter>,
    bridge: Arc<SessionRunnerBridge>,
    outbound_queue: Arc<OutboundQueue>,
    bridge_rx: Arc<AsyncMutex<Option<mpsc::Receiver<DiscordOutboundMessage>>>>,
    client_rx: Arc<AsyncMutex<Option<mpsc::Receiver<DiscordOutboundMessage>>>>,
    user_locks: Arc<AsyncMutex<HashMap<UserId, Arc<AsyncMutex<()>>>>>,
    on_turn_complete: Option<TurnCompleteCallback>,
}

impl DiscordService {
    /// Creates a new `DiscordService` with standard channel components.
    pub fn new(
        client: Arc<DiscordClient>,
        dc_config: DiscordConfig,
        app_config: AppConfig,
    ) -> Self {
        // Check policy coverage for the resolved workspace directory
        dc_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(DiscordRouter::new(dc_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("discord");
        let workspace_manager = DiscordWorkspaceManager::with_paths(
            dc_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<DiscordOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<DiscordOutboundMessage>(64);
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

    /// Creates a new `DiscordService` with an external `SessionEventHook`.
    pub fn with_event_hook(
        client: Arc<DiscordClient>,
        dc_config: DiscordConfig,
        app_config: AppConfig,
        event_hook: crate::runner_bridge::SessionEventHook,
    ) -> Self {
        dc_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(DiscordRouter::new(dc_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("discord");
        let workspace_manager = DiscordWorkspaceManager::with_paths(
            dc_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<DiscordOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<DiscordOutboundMessage>(64);
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

    /// Creates a `DiscordService` with explicit pre-built components and channels.
    pub fn with_components_and_receivers(
        client: Arc<DiscordClient>,
        router: Arc<DiscordRouter>,
        bridge: Arc<SessionRunnerBridge>,
        outbound_queue: Arc<OutboundQueue>,
        bridge_rx: mpsc::Receiver<DiscordOutboundMessage>,
        client_rx: mpsc::Receiver<DiscordOutboundMessage>,
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

    /// Returns a reference to the underlying `DiscordClient`.
    pub fn client(&self) -> &Arc<DiscordClient> {
        &self.client
    }

    /// Returns a reference to the `DiscordRouter`.
    pub fn router(&self) -> &Arc<DiscordRouter> {
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

    /// Runs the core Discord service loop.
    pub async fn run(&self) -> Result<(), DiscordError> {
        // 1. Connect if not already running
        if !self.client.is_running() {
            info!("Discord client is not running. Initiating connect()...");
            self.client.connect().await?;
        } else {
            info!("Discord client is already running.");
        }

        // 2. Take the inbound message receiver
        let mut message_rx = self.client.take_message_receiver().await.ok_or_else(|| {
            error!("Inbound Discord message receiver has already been consumed");
            DiscordError::ConnectionFailed("Inbound message receiver already consumed".to_string())
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
                        channel_id = %msg.channel_id,
                        text = %msg.text,
                        "Outbound message worker delivering reply to Discord"
                    );
                    match client.send_message(&msg.channel_id, &msg.text).await {
                        Ok(msg_id) => {
                            info!(
                                channel_id = %msg.channel_id,
                                msg_id = %msg_id,
                                "Successfully delivered outbound Discord message"
                            );
                        }
                        Err(DiscordError::NotConnected) => {
                            warn!("Client not connected when sending outbound Discord message, re-enqueueing");
                            let st = client.status().await;
                            let _ = outbound_queue.enqueue(msg, &st).await;
                        }
                        Err(e) => {
                            warn!("Failed to send outbound Discord message: {}", e);
                        }
                    }
                }
            });
        }

        info!("DiscordService orchestration loop active. Processing incoming messages...");

        // 4. Main message loop
        while let Some(msg) = message_rx.recv().await {
            info!(
                id = %msg.id,
                channel = %msg.channel_id,
                author = %msg.author_id,
                content = %msg.content,
                "DiscordService processing inbound message"
            );

            let outcome = self.router.route(&msg).await;
            match outcome {
                RouteOutcome::FreshSessionRequested {
                    user_id,
                    channel_id,
                    new_session_id,
                    role,
                } => {
                    info!(
                        user_id = %user_id,
                        channel_id = %channel_id,
                        session_id = %new_session_id,
                        role = ?role,
                        "Fresh Discord session requested via /new"
                    );
                    let notification = DiscordOutboundMessage::new(
                        channel_id.as_str(),
                        "✨ Fresh session started.",
                    );
                    let st = self.client.status().await;
                    let _ = self.outbound_queue.enqueue(notification, &st).await;
                }
                RouteOutcome::ProcessTurn {
                    user_id,
                    channel_id,
                    session_id,
                    role,
                    is_first_time,
                } => {
                    info!(
                        user_id = %user_id,
                        channel_id = %channel_id,
                        session_id = %session_id,
                        role = ?role,
                        is_first_time = is_first_time,
                        "Dispatching Discord turn for user"
                    );

                    let user_lock = self.get_user_lock(&user_id).await;
                    let bridge = self.bridge.clone();
                    let user_clone = user_id.clone();
                    let channel_clone = channel_id.clone();
                    let session_clone = session_id.clone();
                    let user_text = msg.content.clone();
                    let on_turn_complete = self.on_turn_complete.clone();
                    let user_locks = self.user_locks.clone();

                    tokio::spawn(async move {
                        {
                            let _guard = user_lock.lock().await;
                            if let Err(e) = bridge
                                .process_turn(
                                    &user_clone,
                                    &channel_clone,
                                    &session_clone,
                                    role,
                                    user_text,
                                    is_first_time,
                                )
                                .await
                            {
                                error!(
                                    "Error processing turn for Discord user {}: {}",
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

        info!("DiscordService message receiver closed. Orchestration loop exiting.");
        Ok(())
    }

    /// Returns the current number of active entries in `user_locks`.
    pub async fn user_locks_len(&self) -> usize {
        self.user_locks.lock().await.len()
    }
}

