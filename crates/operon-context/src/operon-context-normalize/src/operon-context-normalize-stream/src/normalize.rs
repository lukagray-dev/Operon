//! Public stream normalization entry points.

use crate::assembler::StreamAssembler;
use crate::error::Result;
use crate::provider;
use crate::types::StreamEvent;
use operon_context_normalize_tools::Provider;

/// Parse one already-split provider stream payload line into zero or more
/// canonical stream events.
///
/// Input assumptions:
/// - For SSE providers, the caller has already stripped any `data: ` prefix.
/// - For NDJSON providers, the line is passed directly.
/// - This function does not do I/O; it only parses one line.
pub fn parse_line(line: &str, provider: &Provider) -> Result<Vec<StreamEvent>> {
    let trimmed = line.trim();

    // Common "ignore" frames shared across providers.
    if trimmed.is_empty() || trimmed == "[DONE]" || trimmed.starts_with(':') {
        return Ok(Vec::new());
    }

    let mut events = provider::parse_line_for_provider(trimmed, provider)?;

    // Keep Ping in the canonical type system, but default parse_line behavior
    // returns empty for keepalive frames per crate contract.
    events.retain(|event| !matches!(event, StreamEvent::Ping));

    Ok(events)
}

/// Create a new stateful stream assembler for the given provider.
pub fn new_assembler(provider: &Provider) -> StreamAssembler {
    StreamAssembler::new(provider.clone())
}
