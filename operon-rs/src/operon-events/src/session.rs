// session.rs — Canonical session event types for the Operon agent loop.
//
// These events are emitted by `operon-session`'s `SessionRunner` over an mpsc
// channel as the agent loop executes. Consumers (TUI, GUI, tests) receive them
// and react accordingly.
//
// Design constraints:
//   - No async, no I/O, no tokio. This is a pure types crate.
//   - No dependency on operon-tools — tool call IDs are plain strings here.
//   - Serializable with serde so events can be logged, replayed, or persisted.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// SessionEvent
// ─────────────────────────────────────────────────────────────────────────────

/// All events emitted by the Operon agent loop over its outbound mpsc channel.
///
/// The consumer (TUI, CLI, test harness) receives a stream of these values and
/// decides how to render or process each one. The loop never blocks on the
/// receiver — events are sent with a `try_send` or `send` best-effort approach.
///
/// # Event ordering
///
/// Events are emitted in the order they occur:
/// 1. `TextDelta` and `ThinkingDelta` stream as the model generates tokens.
/// 2. `ToolCallStart` fires when the model starts a tool call.
/// 3. `ToolCallResult` fires after the dispatcher returns the result.
/// 4. `TurnComplete` fires once after all tool calls in one turn are done.
/// 5. Steps 1–4 repeat for each agentic loop iteration.
/// 6. `Done` fires when the loop exits naturally (no more tool calls).
/// 7. `Error` fires instead of `Done` if the loop exits due to a fatal failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionEvent {
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

    /// The model started a tool call (name is now known, args not yet complete).
    ///
    /// Emitted as soon as the model's streaming output reveals the tool name.
    /// The `call_id` is a raw string — not a `ToolCallId` — because this crate
    /// deliberately has no dependency on `operon-context-normalize-tools`.
    ToolCallStart {
        /// Provider-specific tool call identifier (e.g. `"toolu_01A"` for Anthropic).
        call_id: String,
        /// The exact name of the tool the model intends to invoke.
        name: String,
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

    // ── Turn lifecycle ────────────────────────────────────────────────────────

    /// One full agent turn completed (model responded; all tool calls dispatched).
    ///
    /// `turn_index` is 0-based and increments monotonically within a session.
    /// Fires even on turns where no tool calls were made.
    TurnComplete { turn_index: usize },

    // ── Compaction ────────────────────────────────────────────────────────────

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
    /// because insufficient history.
    Warning { message: String },

    /// A fatal error occurred. The session has stopped.
    ///
    /// After `Error`, the runner transitions to `LifecycleState::Failed` and no
    /// further events will arrive on this channel.
    Error { message: String },
}
