//! Tool dispatcher — routes tool calls to implementations, enforces read-before-write
//! safety, and manages session tools and state.
//!
//! # Architecture
//!
//! - [`mod@register`]: Group registration methods (`register_fs_tools`, `register_shell_tools`, etc.).
//! - [`mod@dispatch`]: Core dispatch pipeline, stateful tool interception (todo/memory), and progress updates.
//! - [`mod@ledger`]: Read ledger recording and path tracking.

mod dispatch;
mod ledger;
mod register;

use operon_context_normalize_tools::{
    ToolCallId, ToolContent, ToolDefinition, ToolResult,
};
use operon_tools_core::{ReadLedger, ToolProgressEmitter};
use operon_tools_memory_store::MemoryStore;
use std::collections::HashMap;

/// Type alias for the type-erased async tool executor function.
pub type ToolExecutorFn = Box<
    dyn Fn(
            ToolCallId,
            serde_json::Value,
            Option<ToolProgressEmitter>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<ToolResult, String>> + Send>,
        > + Send
        + Sync,
>;

/// A registered tool: its canonical definition and async execute function.
pub(crate) struct ToolEntry {
    pub(crate) definition: ToolDefinition,
    /// Type-erased async execute fn.
    /// Takes (call_id, args_json, progress) → ToolResult.
    /// Malformed args must be surfaced as Err(String) describing the parse failure.
    pub(crate) execute: ToolExecutorFn,
}

/// The result of a single tool dispatch.
///
/// Returned by `Dispatcher::dispatch_with_progress()` so the caller receives the
/// executed `ToolResult`.
pub struct DispatchOutcome {
    /// The tool result to feed back to the model and push into conversation history.
    pub result: ToolResult,
}

/// The tool dispatcher. One instance per agent session.
///
/// # Usage
///
/// ```rust
/// use operon_tools::dispatcher::Dispatcher;
///
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register_fs_tools();
///
/// // Get definitions to include in the next model request
/// let defs: Vec<_> = dispatcher.definitions().collect();
/// ```
pub struct Dispatcher {
    pub(crate) tools: HashMap<String, ToolEntry>,
    /// Tracks paths read this session for read-before-write/edit enforcement.
    pub(crate) read_ledger: ReadLedger,
    /// In-memory todo list for the current agent session.
    pub(crate) todo_store: operon_tools_core::TodoStore,
    /// Global persistent memory store — shared across sessions, SQLite-backed.
    pub(crate) memory_store: Option<MemoryStore>,
}

impl Dispatcher {
    /// Creates an empty dispatcher with no tools registered.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            read_ledger: ReadLedger::new(),
            todo_store: operon_tools_core::TodoStore::new(),
            memory_store: None,
        }
    }

    /// Attaches the global persistent memory store to the dispatcher.
    ///
    /// Called by the session runner after `MemoryStore::connect_default().await` succeeds.
    /// Until this is called, any memory tool call returns an error describing the situation.
    pub fn attach_memory_store(&mut self, store: MemoryStore) {
        self.memory_store = Some(store);
    }

    /// Clears the read ledger after context compaction.
    ///
    /// Must be called by the session runner whenever compaction fires.
    /// Compaction summarizes context — the model's mental model of file contents
    /// becomes stale. Clearing the ledger forces re-reads before any subsequent
    /// write or edit.
    pub fn notify_compaction(&mut self) {
        self.read_ledger.clear();
    }

    /// Returns the canonical tool definitions to include in the model request.
    ///
    /// All registered tools are available from turn 1.
    pub fn definitions(&self) -> impl Iterator<Item = &ToolDefinition> {
        self.tools.values().map(|entry| &entry.definition)
    }

    /// Returns a reference to the read ledger.
    ///
    /// Primarily for testing — allows asserting ledger state after tool calls.
    pub fn read_ledger(&self) -> &ReadLedger {
        &self.read_ledger
    }

    /// Returns an immutable reference to the todo store.
    ///
    /// Allows the session runner to snapshot current todos and persist them.
    pub fn todo_store(&self) -> &operon_tools_core::TodoStore {
        &self.todo_store
    }

    /// Returns a mutable reference to the todo store.
    ///
    /// Used by todo tool execute functions which receive the store as a parameter.
    pub fn todo_store_mut(&mut self) -> &mut operon_tools_core::TodoStore {
        &mut self.todo_store
    }

    /// Restores or populates the todo store from a list of previously saved `TodoItem` items.
    pub fn load_todos(&mut self, items: Vec<operon_tools_core::TodoItem>) {
        self.todo_store = operon_tools_core::TodoStore::from_items(items);
    }

    /// Returns an optional reference to the memory store.
    ///
    /// Returns None if `attach_memory_store` has not been called yet.
    pub fn memory_store(&self) -> Option<&MemoryStore> {
        self.memory_store.as_ref()
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructs an error `ToolResult` for dispatch-level failures.
pub(crate) fn error_result(call_id: ToolCallId, tool_name: &str, reason: &str) -> ToolResult {
    ToolResult {
        call_id,
        name: tool_name.to_string(),
        content: ToolContent::Text(reason.to_string()),
        is_error: true,
    }
}

