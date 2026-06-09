//! # operon-rs
//!
//! Public facade over the Operon backend crates.
//!
//! GUI and TUI frontends should depend on this crate only. It re-exports the
//! backend subsystems as modules and also exposes the most common types at the
//! crate root.

pub use operon_config as config;
pub use operon_context as context;
pub use operon_events as events;
pub use operon_markdown as markdown;
pub use operon_policy as policy;
pub use operon_providers as providers;
pub use operon_session as session;
pub use operon_tools as tools;
pub use operon_tools_core as tools_core;

pub use config::{
    add_allowed_directory, get_allowed_directories_list, get_permission_rows, load,
    remove_allowed_directory, save_provider, update_permission, AppConfig, ConfigError,
    OperonPaths, PermissionRow,
};
pub use context::{
    compact, sanitize, CompactionClient, CompactionConfig, CompactionError, CompactionResult,
    ContentBlock, ConversationMessage, DocumentBlock, DocumentSource, EstimationTier, ImageBlock,
    ImageSource, MessageRole, ReasoningBlock, ReasoningSignature, Role, SessionSnapshot,
    SessionTokenState, SnapshotBuilder, SnapshotConfig, SnapshotError, StopReason, StreamEvent,
    TokenBudget, TokenEstimator, TokenRecorder, TokenTrackerError, ToolCall, ToolCallId,
    ToolContent, ToolDefinition, ToolResult, UsageRecord,
};
pub use events::{SessionCommand, SessionEvent};
pub use policy::{
    CallerRole, DirTool, DirectoryPolicy, FsTool, GlobalPolicy, GlobalTool, PermissionMode,
    PolicyConfig, PolicyDecision, PolicyError, PolicyResolver,
};
pub use providers::{
    discover_models, ApiCredentials, AuthHeader, DiscoveredModel, DiscoveryResult, ModelConfig,
    Provider, ProviderCapabilities, ProviderConfig, SecretString,
};
pub use session::{LifecycleState, SessionConfig, SessionError, SessionRunner};
pub use tools::dispatcher::{DispatchOutcome, Dispatcher};
pub use tools::{emit_tool_progress, ToolProgress, ToolProgressEmitter, ToolProgressStage};
pub use tools_core::{
    ReadLedger, TieredToolDefinition, TodoItem, TodoPriority, TodoStatus, TodoStore,
    ToolDispatchError,
};

pub mod prelude {
    pub use crate::{
        load, ApiCredentials, AppConfig, CallerRole, CompactionClient, CompactionConfig,
        CompactionError, CompactionResult, ConfigError, ContentBlock, ConversationMessage, DirTool,
        DirectoryPolicy, DispatchOutcome, Dispatcher, DocumentBlock, DocumentSource,
        EstimationTier, FsTool, GlobalPolicy, GlobalTool, ImageBlock, ImageSource, LifecycleState,
        MessageRole, ModelConfig, OperonPaths, PermissionMode, PolicyConfig, PolicyDecision,
        PolicyError, PolicyResolver, Provider, ProviderCapabilities, ProviderConfig, ReadLedger,
        ReasoningBlock, ReasoningSignature, Role, SecretString, SessionCommand, SessionConfig,
        SessionError, SessionEvent, SessionRunner, SessionSnapshot, SessionTokenState,
        SnapshotBuilder, SnapshotConfig, SnapshotError, StopReason, StreamEvent,
        TieredToolDefinition, TodoItem, TodoPriority, TodoStatus, TodoStore, TokenBudget,
        TokenEstimator, TokenRecorder, TokenTrackerError, ToolCall, ToolCallId, ToolContent,
        ToolDefinition, ToolDispatchError, ToolProgress, ToolProgressEmitter, ToolProgressStage,
        ToolResult, UsageRecord,
    };
}
