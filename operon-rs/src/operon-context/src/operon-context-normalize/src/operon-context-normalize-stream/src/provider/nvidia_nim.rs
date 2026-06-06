//! NVIDIA NIM streaming parser (OpenAI-compatible SSE).
//!
//! NVIDIA NIM uses an OpenAI-compatible Server-Sent Events (SSE) stream format
//! for chat completions. All stream parsing logic delegates to [`super::openai`].

use crate::error::Result;
use crate::types::StreamEvent;

use super::openai;

/// The provider name used for error messages.
const PROVIDER: &str = "NVIDIA NIM";

/// Parse one NVIDIA NIM stream payload line.
///
/// Delegates to the OpenAI streaming implementation since the wire formats are identical.
/// The `PROVIDER` label is passed down to ensure errors refer to "NVIDIA NIM".
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    openai::parse_line_with_provider(line, PROVIDER)
}
