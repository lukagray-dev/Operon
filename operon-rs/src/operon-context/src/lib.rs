//! Facade crate for the Operon context pipeline.
//!
//! This crate owns no runtime logic and defines no context types. It provides a
//! stable import surface over the independently usable context sub-crates, so
//! callers can either depend on this facade or depend on a focused sub-crate
//! directly outside Operon.
//!
//! Re-exported modules:
//! - [`token_tracker`]: token estimation, token budget checks, and session usage
//!   recording.
//! - [`snapshot`]: per-turn system snapshot construction from project state.
//! - [`sanitizer`]: conversation cleanup before each model call.
//! - [`compaction`]: threshold-driven conversation summarization and turn
//!   preservation.
//! - [`normalize`]: canonical message, tool, reasoning, and stream types across
//!   providers.
//!
//! Typical context flow:
//! 1. Build a fresh [`SessionSnapshot`] with [`SnapshotBuilder`].
//! 2. Run [`sanitize`] before sending messages to a provider.
//! 3. Track token usage every turn with the token-tracker API.
//! 4. Run [`compact`] when [`CompactionConfig`] thresholds say the conversation
//!    should be summarized.

pub use operon_context_compaction as compaction;
pub use operon_context_normalize as normalize;
pub use operon_context_sanitizer as sanitizer;
pub use operon_context_snapshot as snapshot;
pub use operon_context_token_tracker as token_tracker;

pub use operon_context_snapshot::{
    Role, SessionSnapshot, SnapshotBuilder, SnapshotConfig, SnapshotError,
};

pub use operon_context_sanitizer::{sanitize, SanitizerError};

#[cfg(any(test, feature = "test-utils"))]
pub use operon_context_compaction::MockCompactionClient;
pub use operon_context_compaction::{
    compact, CompactionClient, CompactionConfig, CompactionError, CompactionResult,
};
#[cfg(feature = "http-client")]
pub use operon_context_compaction::{
    AnthropicCompactionClient, GeminiCompactionClient, OpenAICompactionClient,
};

pub use operon_context_token_tracker::{
    EstimationTier, Result, SessionTokenState, TokenBudget, TokenEstimator, TokenRecorder,
    TokenTrackerError, UsageRecord,
};

pub use operon_context_normalize::messages::{
    ContentBlock, ConversationMessage, DocumentBlock, DocumentSource, ImageBlock, ImageSource,
    MessageRole, StopReason,
};
pub use operon_context_normalize::reasoning::{ReasoningBlock, ReasoningSignature};
pub use operon_context_normalize::stream::{AssemblerOutput, StreamEvent};
pub use operon_context_normalize::tools::{
    Provider, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult,
};

pub mod prelude {
    pub use crate::{
        compact, sanitize, CompactionClient, CompactionConfig, ContentBlock, ConversationMessage,
        DocumentBlock, DocumentSource, ImageBlock, ImageSource, MessageRole, ReasoningBlock,
        ReasoningSignature, Role, SessionSnapshot, SnapshotBuilder, SnapshotConfig, StreamEvent,
        ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult,
    };
}
