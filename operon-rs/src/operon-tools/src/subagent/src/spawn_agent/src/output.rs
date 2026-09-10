// output.rs — Output and completion result types for the `spawn_agent` tool.
//
// Hey friend! When an autonomous subagent finishes working on its assigned task,
// we package up its work into this clean `SpawnAgentOutput` structure.
//
// This gives the parent agent everything it needs to understand what happened:
// - Which subagent session performed the work (`subagent_session_id`)
// - What role it was operating as (`role`)
// - Whether it completed successfully or ran into trouble (`status`)
// - And most importantly: the subagent's final answer, findings, or code summary (`summary`)!

use serde::{Deserialize, Serialize};

/// The execution outcome returned to the parent model after a subagent finishes its task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnAgentOutput {
    /// The unique session identifier assigned to this subagent.
    ///
    /// Useful for debugging, audit trails, and retrieving full conversation logs
    /// from `~/.operon/sessions/<subagent_session_id>.json`.
    pub subagent_session_id: String,

    /// The persona or role the subagent executed under (e.g. "researcher", "tester").
    pub role: String,

    /// Final execution status: "completed", "failed", or "cancelled".
    pub status: String,

    /// The comprehensive final summary or answer produced by the subagent.
    ///
    /// This is what the parent model reads to incorporate the subagent's work
    /// into its ongoing reasoning and final response.
    pub summary: String,
}

impl SpawnAgentOutput {
    /// Creates a successful completion output.
    pub fn completed(
        subagent_session_id: impl Into<String>,
        role: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            subagent_session_id: subagent_session_id.into(),
            role: role.into(),
            status: "completed".to_string(),
            summary: summary.into(),
        }
    }

    /// Creates a failure outcome reporting an error message.
    pub fn failed(
        subagent_session_id: impl Into<String>,
        role: impl Into<String>,
        error_message: impl Into<String>,
    ) -> Self {
        Self {
            subagent_session_id: subagent_session_id.into(),
            role: role.into(),
            status: "failed".to_string(),
            summary: error_message.into(),
        }
    }

    /// Creates a cancellation outcome when the user interrupted execution.
    pub fn cancelled(subagent_session_id: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            subagent_session_id: subagent_session_id.into(),
            role: role.into(),
            status: "cancelled".to_string(),
            summary: "Subagent task was cancelled before completion.".to_string(),
        }
    }

    /// Formats the output as a human-friendly string for the parent model.
    pub fn format_for_parent(&self) -> String {
        format!(
            "### Subagent [{role}] ({status})\n**Session ID:** `{session}`\n\n{summary}",
            role = self.role,
            status = self.status,
            session = self.subagent_session_id,
            summary = self.summary
        )
    }
}

