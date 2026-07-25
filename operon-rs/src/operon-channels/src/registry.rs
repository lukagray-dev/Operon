// registry.rs — Channel instance registry and manager for operon-channels.
//
// Hey friend! This module provides `ChannelRegistry`, which maintains an in-memory
// collection of active channel engines (WhatsApp, Telegram, etc.).
//
// Frontends (GUI / TUI) use `ChannelRegistry` to start, stop, query status, and route
// messages across all registered channels from a single location.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::trait_def::{Channel, ChannelId, ChannelStatus};

/// Unified error type for channel registry operations.
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Channel '{0}' is not registered")]
    NotRegistered(ChannelId),

    #[error("Channel '{0}' is already running")]
    AlreadyRunning(ChannelId),

    #[error("Channel execution error: {0}")]
    Execution(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Thread-safe registry that owns and coordinates active channel adapters.
#[derive(Default, Clone)]
pub struct ChannelRegistry {
    /// Internal map from ChannelId to Arc-wrapped Channel trait object.
    channels: Arc<RwLock<HashMap<ChannelId, Arc<dyn Channel>>>>,
}

impl ChannelRegistry {
    /// Create a new, empty `ChannelRegistry`.
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new channel adapter instance.
    pub async fn register(&self, channel: Arc<dyn Channel>) {
        let id = channel.id();
        let mut map = self.channels.write().await;
        map.insert(id, channel);
    }

    /// Unregisters a channel adapter by ID.
    pub async fn unregister(&self, id: &ChannelId) -> Option<Arc<dyn Channel>> {
        let mut map = self.channels.write().await;
        map.remove(id)
    }

    /// Starts a registered channel adapter by ID.
    pub async fn start_channel(&self, id: &ChannelId) -> Result<(), ChannelError> {
        let map = self.channels.read().await;
        let channel = map.get(id).ok_or_else(|| ChannelError::NotRegistered(id.clone()))?;
        channel.start().await
    }

    /// Stops a registered channel adapter by ID.
    pub async fn stop_channel(&self, id: &ChannelId) -> Result<(), ChannelError> {
        let map = self.channels.read().await;
        let channel = map.get(id).ok_or_else(|| ChannelError::NotRegistered(id.clone()))?;
        channel.stop().await
    }

    /// Queries the current connection status of a registered channel by ID.
    pub async fn get_status(&self, id: &ChannelId) -> Result<ChannelStatus, ChannelError> {
        let map = self.channels.read().await;
        let channel = map.get(id).ok_or_else(|| ChannelError::NotRegistered(id.clone()))?;
        Ok(channel.status().await)
    }

    /// Returns a list of all registered ChannelIds.
    pub async fn list_channels(&self) -> Vec<ChannelId> {
        let map = self.channels.read().await;
        map.keys().cloned().collect()
    }
}
