// service.rs — Central orchestration loop for Slack channel.
//
// Hey friend! This module coordinates inbound Slack messages, routing,
// sequential turn execution per user, and outbound message delivery.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{error, info, warn};

use crate::client::SlackClient;
use crate::config::SlackConfig;
use crate::error::SlackError;
use crate::outbound::{OutboundQueue, SlackOutboundMessage};
use crate::router::{RouteOutcome, SlackRouter};
use crate::runner_bridge::SessionRunnerBridge;
use crate::types::{ConnectionStatus, UserId};
use crate::workspace::SlackWorkspaceManager;
use operon_config::AppConfig;

/// Optional callback triggered when a turn completes for a user.
type TurnCompleteCallback = Arc<dyn Fn() + Send + Sync>;

/// Orchestrates inbound Slack message consumption, routing, turn execution,
/// and outbound message delivery.
pub struct SlackService {
    client: Arc<SlackClient>,
    router: Arc<SlackRouter>,
    bridge: Arc<SessionRunnerBridge>,
    outbound_queue: Arc<OutboundQueue>,
    bridge_rx: Arc<AsyncMutex<Option<mpsc::Receiver<SlackOutboundMessage>>>>,
    client_rx: Arc<AsyncMutex<Option<mpsc::Receiver<SlackOutboundMessage>>>>,
    user_locks: Arc<AsyncMutex<HashMap<UserId, Arc<AsyncMutex<()>>>>>,
    on_turn_complete: Option<TurnCompleteCallback>,
}

impl SlackService {
    /// Creates a new `SlackService` with standard channel components.
    pub fn new(
        client: Arc<SlackClient>,
        sl_config: SlackConfig,
        app_config: AppConfig,
    ) -> Self {
        // Check policy coverage for the resolved workspace directory
        sl_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(SlackRouter::new(sl_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("slack");
        let workspace_manager = SlackWorkspaceManager::with_paths(
            sl_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<SlackOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<SlackOutboundMessage>(64);
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

    /// Creates a new `SlackService` with an external `SessionEventHook`.
    pub fn with_event_hook(
        client: Arc<SlackClient>,
        sl_config: SlackConfig,
        app_config: AppConfig,
        event_hook: crate::runner_bridge::SessionEventHook,
    ) -> Self {
        sl_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(SlackRouter::new(sl_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("slack");
        let workspace_manager = SlackWorkspaceManager::with_paths(
            sl_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<SlackOutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<SlackOutboundMessage>(64);
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

    /// Creates a `SlackService` with explicit pre-built components and channels.
    pub fn with_components_and_receivers(
        client: Arc<SlackClient>,
        router: Arc<SlackRouter>,
        bridge: Arc<SessionRunnerBridge>,
        outbound_queue: Arc<OutboundQueue>,
        bridge_rx: mpsc::Receiver<SlackOutboundMessage>,
        client_rx: mpsc::Receiver<SlackOutboundMessage>,
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

    /// Returns a reference to the underlying `SlackClient`.
    pub fn client(&self) -> &Arc<SlackClient> {
        &self.client
    }

    /// Returns a reference to the `SlackRouter`.
    pub fn router(&self) -> &Arc<SlackRouter> {
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

    /// Runs the core Slack service loop.
    pub async fn run(&self) -> Result<(), SlackError> {
        // 1. Connect if not already running
        if !self.client.is_running() {
            info!("Slack client is not running. Initiating connect()...");
            self.client.connect().await?;
        } else {
            info!("Slack client is already running.");
        }

        // 2. Take the inbound message receiver
        let mut message_rx = self.client.take_message_receiver().await.ok_or_else(|| {
            error!("Inbound Slack message receiver has already been consumed");
            SlackError::ConnectionFailed("Inbound message receiver already consumed".to_string())
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
                        thread_ts = ?msg.thread_ts,
                        "Outbound message worker delivering reply to Slack"
                    );
                    match client
                        .send_message(
                            &crate::types::SlackChannelId::new(&msg.channel_id),
                            &msg.text,
                            msg.thread_ts.as_deref(),
                        )
                        .await
                    {
                        Ok(ts) => {
                            info!(
                                channel_id = %msg.channel_id,
                                ts = %ts,
                                "Successfully delivered outbound Slack message"
                            );
                        }
                        Err(SlackError::NotConnected) => {
                            warn!("Client not connected when sending outbound Slack message, re-enqueueing");
                            let st = client.status().await;
                            let _ = outbound_queue.enqueue(msg, &st).await;
                        }
                        Err(e) => {
                            warn!("Failed to send outbound Slack message: {}", e);
                        }
                    }
                }
            });
        }

        info!("SlackService orchestration loop active. Processing incoming messages...");

        // 4. Main message loop
        while let Some(msg) = message_rx.recv().await {
            info!(
                id = %msg.id,
                channel = %msg.channel_id,
                author = %msg.author_id,
                text = %msg.text,
                "SlackService processing inbound message"
            );

            let outcome = self.router.route(&msg).await;
            match outcome {
                RouteOutcome::FreshSessionRequested {
                    user_id,
                    channel_id,
                    thread_ts,
                    new_session_id,
                    role,
                } => {
                    info!(
                        user_id = %user_id,
                        channel_id = %channel_id,
                        session_id = %new_session_id,
                        role = ?role,
                        "Fresh Slack session requested via /new"
                    );
                    let notification = SlackOutboundMessage::new_threaded(
                        channel_id.as_str(),
                        "✨ Fresh session started.",
                        thread_ts,
                    );
                    let st = self.client.status().await;
                    let _ = self.outbound_queue.enqueue(notification, &st).await;
                }
                RouteOutcome::ProcessTurn {
                    user_id,
                    channel_id,
                    session_id,
                    thread_ts,
                    role,
                    is_first_time,
                } => {
                    info!(
                        user_id = %user_id,
                        channel_id = %channel_id,
                        session_id = %session_id,
                        role = ?role,
                        is_first_time = is_first_time,
                        "Dispatching Slack turn for user"
                    );

                    let user_lock = self.get_user_lock(&user_id).await;
                    let bridge = self.bridge.clone();
                    let user_clone = user_id.clone();
                    let channel_clone = channel_id.clone();
                    let session_clone = session_id.clone();
                    let thread_clone = thread_ts.clone();
                    let user_text = msg.text.clone();
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
                                    thread_clone,
                                    role,
                                    user_text,
                                    is_first_time,
                                )
                                .await
                            {
                                error!(
                                    "Error processing turn for Slack user {}: {}",
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

        info!("SlackService message receiver closed. Orchestration loop exiting.");
        Ok(())
    }

    /// Returns the current number of active entries in `user_locks`.
    pub async fn user_locks_len(&self) -> usize {
        self.user_locks.lock().await.len()
    }
}

