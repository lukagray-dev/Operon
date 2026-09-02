//! # operon-channels-feishu
//!
//! Feishu / Lark channel integration sub-crate for Operon backend.
//!
//! Handles:
//! - Feishu / Lark `tenant_access_token` automatic caching & refresh (`POST /open-apis/auth/v3/tenant_access_token/internal`).
//! - Bot credentials test (`GET /open-apis/bot/v3/info`).
//! - WebSocket persistent event streaming (`wss://ws-open.feishu.cn/ws/v2` / `wss://ws-open.larksuite.com/ws/v2`).
//! - Outbound message delivery & threaded replies (`POST /open-apis/im/v1/messages`).
//! - User ID allowlist role classification (`Owner` vs `External`).
//! - Shared workspace provisioning (`~/.operon/workspace/`).
//! - Per-user session isolation (`~/.operon/sessions/feishu/<user_id>/<session_id>.json`).
//! - `/new` session resets, onboarding documentation, 4000-character message chunking, and live streaming.

pub mod client;
pub mod config;
pub mod error;
pub mod outbound;
pub mod router;
pub mod runner_bridge;
pub mod service;
pub mod types;
pub mod workspace;

pub use client::FeishuClient;
pub use config::FeishuConfig;
pub use error::FeishuError;
pub use outbound::{split_feishu_message, FeishuOutboundMessage, OutboundQueue, FEISHU_MAX_MESSAGE_LENGTH};
pub use router::{FeishuRouter, RouteOutcome};
pub use runner_bridge::SessionRunnerBridge;
pub use service::FeishuService;
pub use types::{ChatId, ConnectionStatus, FeishuDomain, FeishuMessage, UserId};
pub use workspace::FeishuWorkspaceManager;

