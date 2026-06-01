// lifecycle.rs — Session execution lifecycle state machine.
//
// A `SessionRunner` goes through well-defined lifecycle states during its
// lifetime. This module provides the `LifecycleState` enum and the guard
// methods used by the runner before mutating state.
//
// State diagram:
//
//   Idle  ──run()──▶  Running  ──pause()──▶  Paused  ──run()──▶  Running
//                         │                               │
//                         └───(no tool calls)──▶  Done   │
//                         └───(fatal error)──▶  Failed   │
//                                                         └───(same)

// ─────────────────────────────────────────────────────────────────────────────
// LifecycleState
// ─────────────────────────────────────────────────────────────────────────────

/// State machine for a session's execution lifecycle.
///
/// The runner checks these states before performing operations so that callers
/// receive clear `InvalidState` errors instead of undefined behaviour.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleState {
    /// Session created but not yet started. `run()` may be called.
    Idle,

    /// Agent loop is actively running (HTTP call in-flight or tool dispatching).
    /// `pause()` may be called from a separate task to interrupt after the
    /// current turn completes.
    Running,

    /// Loop paused mid-session (e.g. awaiting user permission or input).
    /// `run()` or `resume()` may be called to continue.
    Paused,

    /// Loop finished naturally — the model returned `EndTurn` with no tool calls.
    /// No further operations are permitted.
    Done,

    /// Loop stopped due to a fatal error.
    /// No further operations are permitted. Inspect the error that caused this
    /// transition via the `Error` event on the event channel.
    Failed,
}

impl LifecycleState {
    /// Returns `true` if the runner may call `run()` or `resume()` in this state.
    ///
    /// Only `Idle` and `Paused` are valid pre-run states.
    pub fn can_run(&self) -> bool {
        matches!(self, Self::Idle | Self::Paused)
    }

    /// Returns `true` if the runner may be paused in this state.
    ///
    /// Only `Running` is a valid state for pausing — pausing a completed or
    /// failed session has no meaning.
    pub fn can_pause(&self) -> bool {
        matches!(self, Self::Running)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_can_run_but_not_pause() {
        // Idle is the starting state — run() should be allowed.
        assert!(LifecycleState::Idle.can_run());
        assert!(!LifecycleState::Idle.can_pause());
    }

    #[test]
    fn running_can_pause_but_not_run() {
        // While running, pause is the valid transition; starting another run is not.
        assert!(!LifecycleState::Running.can_run());
        assert!(LifecycleState::Running.can_pause());
    }

    #[test]
    fn paused_can_run_but_not_pause() {
        // A paused session can be resumed but cannot be paused again.
        assert!(LifecycleState::Paused.can_run());
        assert!(!LifecycleState::Paused.can_pause());
    }

    #[test]
    fn done_cannot_run_or_pause() {
        // Terminal state — no further operations allowed.
        assert!(!LifecycleState::Done.can_run());
        assert!(!LifecycleState::Done.can_pause());
    }

    #[test]
    fn failed_cannot_run_or_pause() {
        // Fatal terminal state — no further operations allowed.
        assert!(!LifecycleState::Failed.can_run());
        assert!(!LifecycleState::Failed.can_pause());
    }

    #[test]
    fn lifecycle_states_are_clone_and_partial_eq() {
        // These traits are required for ergonomic pattern matching in tests and logs.
        let a = LifecycleState::Running;
        let b = a.clone();
        assert_eq!(a, b);
    }
}
