//! Tiered tool definitions — short description for normal use,
//! detailed description for recovery after a malformed tool call.

use operon_context_normalize_tools::ToolDefinition;

/// A tool definition with two description tiers.
///
/// # When each tier is used
///
/// The dispatcher starts every session by sending `short` to the model for all tools.
/// If the model produces a malformed call for a specific tool (args parse failure),
/// the dispatcher marks that tool as "degraded" for the rest of the session.
/// On the next request, the degraded tool's `detailed` definition is sent instead,
/// accompanied by an error ToolResult explaining what went wrong.
/// All other tools continue to use `short`.
///
/// Session reset → all tools revert to `short`.
///
/// # Field rules
/// - `short.name` and `detailed.name` MUST be identical.
/// - `short.parameters` and `detailed.parameters` MUST be identical (same JSON Schema).
///   Only `description` differs between the two.
/// - `short` description: ≤ 5 lines. States what the tool does and the key constraint.
///   No examples, no edge cases.
/// - `detailed` description: full explanation. Includes accepted input shapes,
///   edge case behaviour, worked examples, common mistakes.
#[derive(Debug, Clone)]
pub struct TieredToolDefinition {
    /// Sent to the model under normal conditions.
    pub short: ToolDefinition,

    /// Sent to the model after a malformed call for this tool in the current session.
    pub detailed: ToolDefinition,
}

impl TieredToolDefinition {
    /// The tool name. Guaranteed identical across both tiers.
    pub fn name(&self) -> &str {
        &self.short.name
    }

    /// Returns the appropriate `ToolDefinition` based on whether the tool
    /// is currently in degraded (detailed) mode.
    pub fn for_mode(&self, degraded: bool) -> &ToolDefinition {
        // Enforce the invariant that both tiers have the same name.
        debug_assert_eq!(
            self.short.name, self.detailed.name,
            "TieredToolDefinition name mismatch: short='{}' detailed='{}'",
            self.short.name, self.detailed.name
        );

        if degraded {
            &self.detailed
        } else {
            &self.short
        }
    }
}
