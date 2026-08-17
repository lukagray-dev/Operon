// service.rs — Central orchestration loop for WhatsApp channel.
//
// Hey friend! This module wires together `WhatsAppClient`, `WhatsAppRouter`,
// `SessionRunnerBridge`, and `OutboundQueue` into a unified `WhatsAppService`.
//
// It drains inbound messages from `WhatsAppClient`, routes them via `WhatsAppRouter`,
// enforces per-contact sequential execution of turns, sends `/new` notifications,
// and flushes outbound responses to the live socket connection.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{error, info, warn};

use crate::client::WhatsAppClient;
use crate::config::WhatsAppConfig;
use crate::error::WhatsAppError;
use crate::outbound::{OutboundMessage, OutboundQueue};
use crate::router::{RouteOutcome, WhatsAppRouter};
use crate::runner_bridge::SessionRunnerBridge;
use crate::types::{ConnectionStatus, ContactId};
use crate::workspace::WhatsAppWorkspaceManager;
use operon_config::AppConfig;

/// Optional callback triggered when a turn completes for a contact.
type TurnCompleteCallback = Arc<dyn Fn() + Send + Sync>;

/// Orchestrates inbound WhatsApp message consumption, routing, turn execution,
/// and outbound message delivery.
pub struct WhatsAppService {
    client: Arc<WhatsAppClient>,
    router: Arc<WhatsAppRouter>,
    bridge: Arc<SessionRunnerBridge>,
    outbound_queue: Arc<OutboundQueue>,
    bridge_rx: Arc<AsyncMutex<Option<mpsc::Receiver<OutboundMessage>>>>,
    client_rx: Arc<AsyncMutex<Option<mpsc::Receiver<OutboundMessage>>>>,
    contact_locks: Arc<AsyncMutex<HashMap<ContactId, Arc<AsyncMutex<()>>>>>,
    on_turn_complete: Option<TurnCompleteCallback>,
}

impl WhatsAppService {
    /// Creates a new `WhatsAppService` with standard channel components.
    pub fn new(
        client: Arc<WhatsAppClient>,
        wa_config: WhatsAppConfig,
        app_config: AppConfig,
    ) -> Self {
        // Check policy coverage for the resolved workspace directory
        wa_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(WhatsAppRouter::new(wa_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("whatsapp");
        let workspace_manager = WhatsAppWorkspaceManager::with_paths(
            wa_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<OutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<OutboundMessage>(64);
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

    /// Creates a new `WhatsAppService` with an external `SessionEventHook`.
    pub fn with_event_hook(
        client: Arc<WhatsAppClient>,
        wa_config: WhatsAppConfig,
        app_config: AppConfig,
        event_hook: crate::runner_bridge::SessionEventHook,
    ) -> Self {
        wa_config.check_policy_coverage(&app_config.policy);

        let router = Arc::new(WhatsAppRouter::new(wa_config.clone()));
        let base_sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".operon")
            .join("sessions")
            .join("whatsapp");
        let workspace_manager = WhatsAppWorkspaceManager::with_paths(
            wa_config.resolved_workspace_dir(),
            base_sessions_dir,
        );
        let (bridge_tx, bridge_rx) = mpsc::channel::<OutboundMessage>(64);
        let (client_tx, client_rx) = mpsc::channel::<OutboundMessage>(64);
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

    /// Creates a `WhatsAppService` with explicit pre-built components and channels.
    pub fn with_components_and_receivers(
        client: Arc<WhatsAppClient>,
        router: Arc<WhatsAppRouter>,
        bridge: Arc<SessionRunnerBridge>,
        outbound_queue: Arc<OutboundQueue>,
        bridge_rx: mpsc::Receiver<OutboundMessage>,
        client_rx: mpsc::Receiver<OutboundMessage>,
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

    /// Returns a reference to the underlying `WhatsAppClient`.
    pub fn client(&self) -> &Arc<WhatsAppClient> {
        &self.client
    }

    /// Returns a reference to the `WhatsAppRouter`.
    pub fn router(&self) -> &Arc<WhatsAppRouter> {
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

    /// Retrieves or creates a per-contact mutex to guarantee sequential turn processing.
    pub async fn get_contact_lock(&self, contact: &ContactId) -> Arc<AsyncMutex<()>> {
        let mut locks = self.contact_locks.lock().await;
        locks
            .entry(contact.clone())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    /// Prunes a contact lock after turn completion if no other tasks hold a reference to it.
    pub async fn prune_contact_lock(&self, contact: &ContactId, contact_lock: &Arc<AsyncMutex<()>>) {
        let mut locks = self.contact_locks.lock().await;
        if Arc::strong_count(contact_lock) == 2 {
            if let Some(entry) = locks.get(contact) {
                if Arc::ptr_eq(entry, contact_lock) {
                    locks.remove(contact);
                }
            }
        }
    }


    /// Runs the core WhatsApp service loop.
    pub async fn run(&self) -> Result<(), WhatsAppError> {
        // 1. Connect if not already running; otherwise retain active/connecting connection
        if !self.client.is_running() {
            info!("WhatsApp client event loop is not running. Initiating connect()...");
            self.client.connect().await?;
        } else {
            info!("WhatsApp client event loop is already running.");
        }

        // 2. Take the inbound message receiver
        let mut message_rx = self.client.take_message_receiver().await.ok_or_else(|| {
            error!("Inbound WhatsApp message receiver has already been consumed (double-init bug)");
            WhatsAppError::ConnectionFailed("Inbound message receiver already consumed".to_string())
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
                        recipient = %msg.recipient,
                        text = %msg.text,
                        "Outbound message worker sending reply over WhatsApp socket"
                    );
                    match client.send_message(&msg.recipient, &msg.text).await {
                        Ok(msg_id) => {
                            info!(
                                recipient = %msg.recipient,
                                msg_id = %msg_id,
                                "Successfully delivered outbound WhatsApp message over socket"
                            );
                        }
                        Err(WhatsAppError::NotConnected) => {
                            warn!("Client not connected when sending outbound WhatsApp message {:?}, re-enqueueing", msg);
                            let st = client.status().await;
                            let _ = outbound_queue.enqueue(msg, &st).await;
                        }
                        Err(e) => {
                            warn!("Failed to send outbound WhatsApp message {:?}: {}", msg, e);
                        }
                    }
                }
            });
        }

        info!("WhatsAppService orchestration loop active. Processing incoming messages...");

        // 4. Main message loop
        while let Some(msg) = message_rx.recv().await {
            info!(
                id = %msg.id,
                sender = %msg.sender,
                is_self = msg.is_self,
                text = %msg.text,
                "WhatsAppService processing inbound message"
            );

            let outcome = self.router.route(&msg).await;
            match outcome {
                RouteOutcome::FreshSessionRequested {
                    contact,
                    new_session_id,
                    role,
                } => {
                    info!(
                        contact = %contact,
                        session_id = %new_session_id,
                        role = ?role,
                        "Fresh session requested via /new"
                    );
                    let notification =
                        OutboundMessage::new(contact.as_str(), "✨ Fresh session started.");
                    let st = self.client.status().await;
                    let _ = self.outbound_queue.enqueue(notification, &st).await;
                }
                RouteOutcome::ProcessTurn {
                    contact,
                    session_id,
                    role,
                    is_first_time,
                } => {
                    info!(
                        contact = %contact,
                        session_id = %session_id,
                        role = ?role,
                        is_first_time = is_first_time,
                        "Dispatching WhatsApp turn for contact"
                    );

                    let contact_lock = self.get_contact_lock(&contact).await;
                    let bridge = self.bridge.clone();
                    let contact_clone = contact.clone();
                    let session_clone = session_id.clone();
                    let user_text = msg.text.clone();
                    let on_turn_complete = self.on_turn_complete.clone();
                    let contact_locks = self.contact_locks.clone();

                    tokio::spawn(async move {
                        // Scope the per-contact mutex lock so it is released as soon as the turn finishes.
                        {
                            let _guard = contact_lock.lock().await;
                            if let Err(e) = bridge
                                .process_turn(
                                    &contact_clone,
                                    &session_clone,
                                    role,
                                    user_text,
                                    is_first_time,
                                )
                                .await
                            {
                                error!(
                                    "Error processing turn for WhatsApp contact {}: {}",
                                    contact_clone, e
                                );
                            }
                            if let Some(cb) = on_turn_complete {
                                cb();
                            }
                        }

                        // Hey newbie friend! After the turn finishes and the per-contact mutex `_guard` is dropped,
                        // we check if any other in-flight turn is currently using or waiting for this contact's lock.
                        //
                        // `contact_locks` map holds 1 strong reference, and our local variable `contact_lock` holds 1.
                        // So if `Arc::strong_count(&contact_lock) == 2`, no other task is referencing this lock.
                        // By acquiring the main `contact_locks` mutex, we prevent a race where a new inbound message
                        // for the same contact might call `get_contact_lock` concurrently.
                        let mut locks = contact_locks.lock().await;
                        if Arc::strong_count(&contact_lock) == 2 {
                            if let Some(entry) = locks.get(&contact_clone) {
                                if Arc::ptr_eq(entry, &contact_lock) {
                                    locks.remove(&contact_clone);
                                }
                            }
                        }
                    });
                }
            }
        }

        info!("WhatsAppService message receiver closed. Orchestration loop exiting.");
        Ok(())
    }

    /// Returns the current number of active entries in `contact_locks` (useful for unit tests and diagnostics).
    pub async fn contact_locks_len(&self) -> usize {
        self.contact_locks.lock().await.len()
    }
}

