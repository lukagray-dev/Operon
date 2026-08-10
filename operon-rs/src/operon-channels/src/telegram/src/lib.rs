//! # operon-channels-telegram
//!
//! Telegram channel integration sub-crate for Operon backend.
//!
//! Handles raw reqwest long-polling over Telegram Bot API, contact allowlist role classification (`Owner` vs `External`),
//! single shared workspace root (configurable via `TelegramConfig.workspace_dir`, defaulting to `~/.operon/workspace/`),
//! per-chat session history isolation (`~/.operon/sessions/telegram/<chat_id>/<session_id>.json`),
//! `/new` session resets, onboarding documentation, MarkdownV2 escaping, long-message splitting, and response streaming over Telegram.

pub mod client;
pub mod config;
pub mod error;
pub mod outbound;
pub mod router;
pub mod runner_bridge;
pub mod service;
pub mod types;
pub mod workspace;

pub use client::TelegramClient;
pub use config::TelegramConfig;
pub use error::TelegramError;
pub use outbound::{format_for_telegram, OutboundQueue, TelegramOutboundMessage};
pub use router::{RouteOutcome, TelegramRouter};
pub use runner_bridge::SessionRunnerBridge;
pub use service::TelegramService;
pub use types::{ChatId, ConnectionStatus, TelegramMessage};
pub use workspace::TelegramWorkspaceManager;
