// AgentBridge trait
// The TUI's primary contact point with operon-rs backend
// All agent execution, prompt streaming, and cancellation happens through this trait

pub mod operon;

pub use operon::OperonAgent;

use crate::events::action::Action;
use anyhow::Result;
use tokio::sync::mpsc;

/// Interface between TUI and operon-rs backend session executor.
#[async_trait::async_trait]
pub trait AgentBridge: Send + Sync {
    /// Executes a user prompt through the agent loop and streams output events to `action_tx`.
    async fn execute_prompt(&self, prompt: String, action_tx: mpsc::Sender<Action>) -> Result<()>;

    /// Cancels the currently active prompt execution turn.
    async fn cancel(&self) -> Result<()>;

    /// Sets or clears the active session ID for turn persistence and resumption.
    fn set_session_id(&mut self, session_id: Option<String>);

    /// Returns the currently active session ID (if one is loaded).
    #[allow(dead_code)]
    fn session_id(&self) -> Option<String>;
}
