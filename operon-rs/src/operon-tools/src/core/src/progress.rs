//! Progress payloads shared by all tool crates.
//!
//! Tool crates use these types to report coarse or fine-grained progress
//! without depending on the session event bus directly.

use operon_context_normalize::tools::ToolCallId;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Stage of a tool call as seen by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolProgressStage {
    /// The tool call has just started dispatching.
    Started,
    /// The tool has entered its main work phase.
    Running,
    /// The tool completed successfully.
    Completed,
    /// The tool failed.
    Failed,
}

/// A progress update emitted while a tool call is running.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolProgress {
    /// Model/provider tool-call ID used to correlate progress with the call.
    pub call_id: ToolCallId,
    /// Name of the tool being executed.
    pub tool: String,
    /// Current stage of the call.
    pub stage: ToolProgressStage,
    /// Optional target resource, such as a file path, URL, or working directory.
    pub target: Option<String>,
    /// Human-readable progress message for the UI.
    pub message: String,
}

/// Shared emitter type used by the dispatcher and tool crates.
pub type ToolProgressEmitter = Arc<dyn Fn(ToolProgress) + Send + Sync + 'static>;

impl ToolProgress {
    /// Build a new progress payload with the provided fields.
    pub fn new(
        call_id: ToolCallId,
        tool: impl Into<String>,
        stage: ToolProgressStage,
        target: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            call_id,
            tool: tool.into(),
            stage,
            target,
            message: message.into(),
        }
    }

    /// Convenience constructor for a started event.
    pub fn started(
        call_id: ToolCallId,
        tool: impl Into<String>,
        target: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(call_id, tool, ToolProgressStage::Started, target, message)
    }

    /// Convenience constructor for a running event.
    pub fn running(
        call_id: ToolCallId,
        tool: impl Into<String>,
        target: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(call_id, tool, ToolProgressStage::Running, target, message)
    }

    /// Convenience constructor for a completed event.
    pub fn completed(
        call_id: ToolCallId,
        tool: impl Into<String>,
        target: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(call_id, tool, ToolProgressStage::Completed, target, message)
    }

    /// Convenience constructor for a failed event.
    pub fn failed(
        call_id: ToolCallId,
        tool: impl Into<String>,
        target: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(call_id, tool, ToolProgressStage::Failed, target, message)
    }
}

/// Emit a progress update if an emitter is available.
pub fn emit_tool_progress(emitter: Option<&ToolProgressEmitter>, progress: ToolProgress) {
    if let Some(emitter) = emitter {
        (emitter.as_ref())(progress);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn tool_progress_constructors_populate_expected_fields() {
        let call_id = ToolCallId("call_123".to_string());
        let progress = ToolProgress::running(
            call_id.clone(),
            "write",
            Some("/tmp/file.txt".to_string()),
            "Writing /tmp/file.txt",
        );

        assert_eq!(progress.call_id, call_id);
        assert_eq!(progress.tool, "write");
        assert_eq!(progress.stage, ToolProgressStage::Running);
        assert_eq!(progress.target.as_deref(), Some("/tmp/file.txt"));
        assert_eq!(progress.message, "Writing /tmp/file.txt");
    }

    #[test]
    fn emit_tool_progress_invokes_emitter() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink: ToolProgressEmitter = {
            let seen = Arc::clone(&seen);
            Arc::new(move |progress: ToolProgress| {
                seen.lock().unwrap().push(progress.message);
            })
        };

        emit_tool_progress(
            Some(&sink),
            ToolProgress::completed(
                ToolCallId("call_456".to_string()),
                "read",
                None,
                "read completed",
            ),
        );

        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &["read completed".to_string()]
        );
    }
}
