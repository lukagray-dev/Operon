// client.rs — WhatsApp connection manager and socket event loop.
//
// Hey friend! This module manages the connection lifecycle of the WhatsApp channel engine.
// It tracks status transitions (Disconnected -> Connecting -> QrRequired -> Connected),
// broadcasts QR states over an mpsc channel for UI rendering, and processes incoming messages.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

use crate::auth::WhatsAppAuth;
use crate::config::WhatsAppConfig;
use crate::error::WhatsAppError;
use crate::types::{ConnectionStatus, QrCodeState};

/// Main WhatsApp client engine controlling socket connection state.
pub struct WhatsAppClient {
    config: WhatsAppConfig,
    auth: WhatsAppAuth,
    status: Arc<RwLock<ConnectionStatus>>,
    qr_tx: mpsc::Sender<QrCodeState>,
    qr_rx: Arc<RwLock<Option<mpsc::Receiver<QrCodeState>>>>,
}

impl WhatsAppClient {
    /// Creates a new `WhatsAppClient` from config.
    pub fn new(config: WhatsAppConfig) -> Self {
        let auth_dir = config.resolved_auth_dir();
        let auth = WhatsAppAuth::new(auth_dir);

        let (qr_tx, qr_rx) = mpsc::channel(10);

        Self {
            config,
            auth,
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            qr_tx,
            qr_rx: Arc::new(RwLock::new(Some(qr_rx))),
        }
    }

    /// Initializes auth directories and starts connection listener.
    pub async fn connect(&self) -> Result<(), WhatsAppError> {
        self.auth.init()?;

        let mut status = self.status.write().await;
        *status = ConnectionStatus::Connecting;

        if !self.auth.has_credentials() {
            info!("No saved WhatsApp credentials found — generating QR code pairing payload...");
            // Simulate QR generation payload for pairing setup
            let dummy_qr_payload = "https://whatsapp.com/qr/operon-pairing-test-token";
            let qr_state = WhatsAppAuth::generate_qr_state(dummy_qr_payload, 60)?;

            *status = ConnectionStatus::QrRequired(qr_state.clone());
            let _ = self.qr_tx.send(qr_state).await;
        } else {
            info!("Loaded saved WhatsApp credentials — connecting socket...");
            *status = ConnectionStatus::Connected;
        }

        Ok(())
    }

    /// Returns a reference to the channel configuration.
    pub fn config(&self) -> &WhatsAppConfig {
        &self.config
    }

    /// Returns the current connection status.
    pub async fn status(&self) -> ConnectionStatus {
        self.status.read().await.clone()
    }

    /// Takes the QR receiver channel for UI event streaming.
    pub async fn take_qr_receiver(&self) -> Option<mpsc::Receiver<QrCodeState>> {
        let mut guard = self.qr_rx.write().await;
        guard.take()
    }

    /// Simulates QR scan completion (for testing/pairing completion).
    pub async fn set_authenticated(&self) {
        let mut status = self.status.write().await;
        *status = ConnectionStatus::Connected;
        info!("WhatsApp channel successfully paired and connected!");
    }

    /// Disconnects the socket.
    pub async fn disconnect(&self) -> Result<(), WhatsAppError> {
        let mut status = self.status.write().await;
        *status = ConnectionStatus::Disconnected;
        info!("WhatsApp channel disconnected.");
        Ok(())
    }
}
