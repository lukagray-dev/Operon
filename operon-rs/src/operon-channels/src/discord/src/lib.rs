//! # operon-channels-discord
//!
//! Discord channel integration sub-crate for Operon backend.
//!
//! Handles:
//! - Direct Discord REST API calls (`/users/@me`, `/channels/{id}/messages`) for authentication and messaging.
//! - Real-time Gateway WebSocket (`wss://gateway.discord.gg/?v=10&encoding=json`) event processing.
//! - User ID allowlist role classification (`Owner` vs `External`).
//! - Single shared workspace root (configurable via `DiscordConfig.workspace_dir`, defaulting to `~/.operon/workspace/`).
//! - Per-user session history isolation (`~/.operon/sessions/discord/<user_id>/<session_id>.json`).
//! - `/new` session resets, onboarding documentation, message chunking, and live response streaming.

pub mod client;
pub mod config;
pub mod error;
pub mod outbound;
pub mod router;
pub mod runner_bridge;
pub mod service;
pub mod types;
pub mod workspace;

pub use client::DiscordClient;
pub use config::DiscordConfig;
pub use error::DiscordError;
pub use outbound::{split_discord_message, DiscordOutboundMessage, OutboundQueue, DISCORD_MAX_MESSAGE_LENGTH};
pub use router::{RouteOutcome, DiscordRouter};
pub use runner_bridge::SessionRunnerBridge;
pub use service::DiscordService;
pub use types::{ConnectionStatus, DiscordChannelId, DiscordMessage, UserId};
pub use workspace::DiscordWorkspaceManager;

