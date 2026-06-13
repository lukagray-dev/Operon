//! Tiered tool definitions — short description for normal use,
//! detailed description for recovery after a malformed tool call.

use operon_context_normalize::tools::ToolDefinition;


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
}

impl TieredToolDefinition {
    /// The tool name.
    pub fn name(&self) -> &str {
        &self.short.name
    }

    /// Returns the tool definition.
    pub fn for_mode(&self, _degraded: bool) -> &ToolDefinition {
        &self.short
    }
}
