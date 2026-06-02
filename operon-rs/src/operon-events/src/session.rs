// session.rs — Canonical session event and command types for the Operon agent loop.
//
// Events flow FROM the runner TO the UI (TUI, GUI, tests) over an outbound mpsc channel.
// Commands flow FROM the UI TO the runner over a separate inbound mpsc channel.
//
// Design constraints:
//   - No async, no I/O, no tokio. This is a pure types crate.
//   - No dependency on operon-tools — tool call IDs are plain strings here.
//   - Serializable with serde so events can be logged, replayed, or persisted.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// SessionEvent — outbound (runner → UI)
// ─────────────────────────────────────────────────────────────────────────────

/// All events emitted by the Operon agent loop over its outbound mpsc channel.
///
/// The consumer (TUI, CLI, test harness) receives a stream of these values and
/// decides how to render or process each one. The loop never blocks on the
/// receiver — events are sent best-effort.
///
/// # Event ordering within a turn
///
/// ```text
/// SessionStarted
///   ↓
/// [ per turn: ]
///   CompactionStarted + CompactionOccurred  (if needed)
///   TextDelta* + ThinkingDelta*             (model streaming)
///   ToolCallStart                           (one per tool call)
///   ToolCallArgsReady                       (full args assembled)
///   [ PermissionDenied | ApprovalRequired ] (policy — future)
///   ToolCallResult                          (dispatch complete)
///   ToolDegraded                            (if args were malformed)
///   TokenUsageUpdated                       (from API usage block)
///   TurnComplete
///   ↓ (loop or end)
/// Done | Error
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionEvent {
    // ── Session lifecycle ─────────────────────────────────────────────────────

    /// The session has been initialized and is ready to run.
    ///
    /// Emitted once at the end of `SessionRunner::new()` before the first turn.
    /// Consumers can use `session_id` to label UI panels or log entries.
    SessionStarted {
        /// Unique session identifier (hex nanosecond timestamp).
        session_id: String,
    },

    // ── Streaming output ──────────────────────────────────────────────────────

    /// A streaming text delta from the model.
    ///
    /// Each `TextDelta` contains one fragment from the SSE stream. Concatenate
    /// all deltas for a turn to reconstruct the full assistant text output.
    TextDelta { text: String },

    /// A streaming reasoning/thinking delta from the model.
    ///
    /// Emitted for providers that expose chain-of-thought reasoning as a
    /// separate stream (e.g. Anthropic extended thinking). Not all providers
    /// emit reasoning deltas.
    ThinkingDelta { text: String },

    // ── Tool calls ────────────────────────────────────────────────────────────

    /// The model started a tool call — name is now known.
    ///
    /// Emitted when the assembler detects a complete tool call in the stream.
    /// Always followed immediately by `ToolCallArgsReady` with the full arguments.
    /// The `call_id` is a raw string — not a `ToolCallId` — to avoid a dependency
    /// on `operon-context-normalize-tools`.
    ToolCallStart {
        /// Provider-specific tool call identifier (e.g. `"toolu_01A"` for Anthropic).
        call_id: String,
        /// The exact name of the tool the model intends to invoke.
        name: String,
    },

    /// Tool call arguments are fully assembled — dispatch is about to begin.
    ///
    /// Fires immediately after `ToolCallStart`. The TUI can use `args_json` to
    /// show an expandable "Arguments" panel before the result arrives.
    ToolCallArgsReady {
        /// Matches the `call_id` in the preceding `ToolCallStart`.
        call_id: String,
        /// The tool name (same as in `ToolCallStart`).
        name: String,
        /// The full serialized JSON object of the tool call arguments.
        args_json: String,
    },

    /// A tool call completed and was dispatched. Contains the full result.
    ///
    /// Emitted after `Dispatcher::dispatch()` returns. Always follows a
    /// corresponding `ToolCallStart` with the same `call_id`.
    ToolCallResult {
        /// Matches the `call_id` in the corresponding `ToolCallStart`.
        call_id: String,
        /// The exact tool name that was invoked.
        name: String,
        /// `true` if the tool execution returned an error result.
        is_error: bool,
        /// Serialized tool content — either the plain text string or the
        /// JSON-encoded value returned by the tool.
        content_json: String,
    },

    /// A tool has been marked degraded (detailed description mode activated).
    ///
    /// Emitted the FIRST time a model sends malformed arguments for a tool.
    /// After this, the tool's detailed definition is sent to the model on
    /// subsequent turns so it can self-correct. The TUI can show a warning badge.
    ToolDegraded {
        /// Name of the tool that entered degraded mode.
        name: String,
    },

    // ── Policy decisions ──────────────────────────────────────────────────────
    // NOTE: These variants are defined now and emitted in a future phase when
    // the policy resolver is wired into the tool dispatcher.

    /// A tool call was flat-out denied by the permission policy.
    ///
    /// The tool is NOT dispatched. The runner returns an error ToolResult to the
    /// model explaining the denial. The TUI can show this as a blocked operation.
    PermissionDenied {
        /// Tool name that was denied.
        tool: String,
        /// The path argument, if the tool operates on a file or directory.
        path: Option<String>,
        /// Human-readable reason for the denial (e.g. "tool 'bash' is denied in /src").
        reason: String,
    },

    /// A tool call requires user approval before proceeding (Ask mode).
    ///
    /// The agent loop is suspended at this point. The UI must respond with a
    /// `SessionCommand::Approve` or `SessionCommand::Deny` using the same `id`.
    /// The loop will not advance until a response is received.
    ///
    /// This variant is defined now and wired in a future phase.
    ApprovalRequired {
        /// Unique ID for this approval request. Used to correlate with SessionCommand.
        id: String,
        /// Tool name requesting approval.
        tool: String,
        /// Path argument, if the tool operates on a file or directory.
        path: Option<String>,
        /// Full serialized JSON arguments of the pending tool call.
        args_json: String,
    },

    // ── Turn lifecycle ────────────────────────────────────────────────────────

    /// One full agent turn completed (model responded; all tool calls dispatched).
    ///
    /// `turn_index` is 0-based and increments monotonically within a session.
    /// Fires even on turns where no tool calls were made.
    TurnComplete { turn_index: usize },

    // ── Token usage ───────────────────────────────────────────────────────────

    /// Token usage reported by the provider after a turn completes.
    ///
    /// Emitted after the session token state is updated from the API usage block.
    /// The TUI uses this to populate the token budget indicator in the status bar.
    TokenUsageUpdated {
        /// Input tokens consumed this turn (prompt + context).
        input_tokens: usize,
        /// Output tokens generated this turn (completion).
        output_tokens: usize,
        /// Total context tokens currently in use (sum of all turns so far).
        context_total: usize,
        /// Anthropic prompt cache read tokens — `None` for non-Anthropic providers.
        cache_read_tokens: Option<usize>,
        /// Anthropic prompt cache write tokens — `None` for non-Anthropic providers.
        cache_write_tokens: Option<usize>,
    },

    // ── Compaction ────────────────────────────────────────────────────────────

    /// Context compaction is about to begin.
    ///
    /// Emitted immediately before the compaction API call. The TUI can show
    /// a spinner or "condensing context…" message.
    CompactionStarted {
        /// Token count that triggered the compaction threshold.
        tokens_before: usize,
    },

    /// Context compaction ran successfully.
    ///
    /// The conversation history has been condensed into a summary. Consumers
    /// can use these counts for progress display or analytics.
    CompactionOccurred {
        /// Token count before compaction (reported by the token tracker).
        tokens_before: usize,
        /// Heuristic token estimate for the rebuilt condensed history.
        tokens_after: usize,
    },

    // ── Terminal events ───────────────────────────────────────────────────────

    /// The agent loop finished naturally (model returned EndTurn with no tool calls).
    ///
    /// This is the expected happy-path terminal event. After `Done`, the runner
    /// transitions to `LifecycleState::Done` and no further events will arrive.
    Done,

    /// A non-fatal warning occurred. The session may continue.
    ///
    /// Examples: unknown tool group name during registration, compaction skipped
    /// because insufficient history, unsupported provider for compaction.
    Warning { message: String },

    /// A fatal error occurred. The session has stopped.
    ///
    /// After `Error`, the runner transitions to `LifecycleState::Failed` and no
    /// further events will arrive on this channel.
    Error { message: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// SessionCommand — inbound (UI → runner)
// ─────────────────────────────────────────────────────────────────────────────

/// Commands sent from the UI into the running session loop.
///
/// The runner holds an `mpsc::Receiver<SessionCommand>` and polls it between
/// agentic loop steps. Commands that arrive between steps are acted on immediately.
/// Commands that arrive mid-step are queued and processed at the next checkpoint.
///
/// # Channel setup
///
/// ```ignore
/// use tokio::sync::mpsc;
/// use operon_events::{SessionEvent, SessionCommand};
///
/// let (event_tx, event_rx) = mpsc::channel::<SessionEvent>(256);
/// let (cmd_tx, cmd_rx)     = mpsc::channel::<SessionCommand>(16);
///
/// let runner = SessionRunner::new(config, event_tx, cmd_rx).await?;
/// // Keep cmd_tx to send Approve/Deny/Cancel from the UI.
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionCommand {
    /// Approve a pending Ask-mode permission request.
    ///
    /// `id` must match the `id` field in the corresponding `ApprovalRequired` event.
    /// If no matching pending request exists, this command is silently ignored.
    Approve { id: String },

    /// Deny a pending Ask-mode permission request.
    ///
    /// `id` must match the `id` field in the corresponding `ApprovalRequired` event.
    /// The runner will return a permission-denied `ToolResult` to the model.
    Deny { id: String },

    /// Cancel the running session immediately.
    ///
    /// The runner finishes the current in-flight tool call (if any) and then exits
    /// the loop cleanly, emitting `SessionEvent::Done` before stopping. This is a
    /// graceful cancellation — use it for user-initiated stops.
    Cancel,
}
