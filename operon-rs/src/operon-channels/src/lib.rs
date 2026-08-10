//! # operon-channels
//!
//! Multi-channel messaging integration facade for Operon (WhatsApp, Telegram, etc.).
//!
//! Provides the generic `Channel` trait, `ChannelRegistry`, and re-exports platform-specific
//! channel sub-crates (such as `operon-channels-whatsapp`).

pub mod registry;
pub mod trait_def;

pub use registry::{ChannelError, ChannelRegistry};
pub use trait_def::{Channel, ChannelId, ChannelMessage, ChannelStatus, QrCodeState};

/// Re-exported WhatsApp channel sub-crate.
pub use operon_channels_whatsapp as whatsapp;

/// Re-exported Telegram channel sub-crate.
pub use operon_channels_telegram as telegram;
