//! DeepSeek streaming parser (OpenAI-compatible).

use crate::error::Result;
use crate::types::StreamEvent;

use super::openai;

const PROVIDER: &str = "DeepSeek";

/// Parse one DeepSeek stream payload line.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    openai::parse_line_with_provider(line, PROVIDER)
}
