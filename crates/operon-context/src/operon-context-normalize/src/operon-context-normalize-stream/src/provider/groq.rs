//! Groq streaming parser (OpenAI-compatible).

use crate::error::Result;
use crate::types::StreamEvent;

use super::openai;

const PROVIDER: &str = "Groq";

/// Parse one Groq stream payload line.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    openai::parse_line_with_provider(line, PROVIDER)
}
