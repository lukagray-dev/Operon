//! # operon-channels-whatsapp
//!
//! WhatsApp channel integration sub-crate for Operon backend.
//!
//! Handles QR pairing, contact allowlist role classification (`Owner` vs `External`),
//! single shared workspace root (configurable via `WhatsAppConfig.workspace_dir`, defaulting to `~/.operon/workspace/`),
//! per-contact session history isolation (`~/.operon/sessions/whatsapp/<phone>/<session_id>.json`),
//! `/new` session resets, onboarding documentation, and response streaming over WhatsApp.


pub mod auth;
pub mod client;
pub mod config;
pub mod error;
pub mod outbound;
pub mod router;
pub mod runner_bridge;
pub mod service;
pub mod storage;
pub mod types;
pub mod workspace;

pub use auth::WhatsAppAuth;
pub use client::WhatsAppClient;
pub use config::WhatsAppConfig;
pub use error::WhatsAppError;
pub use outbound::{OutboundMessage, OutboundQueue};
pub use router::{RouteOutcome, WhatsAppRouter};
pub use runner_bridge::SessionRunnerBridge;
pub use service::WhatsAppService;
pub use storage::RusqliteStore;
pub use types::{ConnectionStatus, ContactId, PairingCodeState, QrCodeState, WhatsAppMessage};
pub use wacore::store::traits::DeviceStore;
pub use workspace::WhatsAppWorkspaceManager;
