//! # operon-channels-slack
//!
//! Slack channel integration sub-crate for Operon backend.
//!
//! Handles:
//! - Slack Socket Mode WebSocket connection (`apps.connections.open`) with App Token (`xapp-...`).
//! - Instant envelope ACKing (`{"envelope_id": "..."}`) and event ingestion.
//! - Slack Web API message posting (`chat.postMessage`) with Bot Token (`xoxb-...`).
//! - User ID allowlist role classification (`Owner` vs `External`).
//! - Single shared workspace root (configurable via `SlackConfig.workspace_dir`, defaulting to `~/.operon/workspace/`).
//! - Per-user session history isolation (`~/.operon/sessions/slack/<user_id>/<session_id>.json`).
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

pub use client::SlackClient;
pub use config::SlackConfig;
pub use error::SlackError;
pub use outbound::{split_slack_message, OutboundQueue, SlackOutboundMessage, SLACK_MAX_MESSAGE_LENGTH};
pub use router::{RouteOutcome, SlackRouter};
pub use runner_bridge::SessionRunnerBridge;
pub use service::SlackService;
pub use types::{ConnectionStatus, SlackChannelId, SlackMessage, SocketModeEnvelope, UserId};
pub use workspace::SlackWorkspaceManager;

