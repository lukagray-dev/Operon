//! Provider dispatch for stream line parsing.

pub mod anthropic;
pub mod cohere;
pub mod deepseek;
pub mod gemini;
pub mod groq;
pub mod mistral;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod xai;

use crate::error::Result;
use crate::types::StreamEvent;
use operon_context_normalize_tools::Provider;

/// Dispatch one payload line to the provider-specific parser.
pub fn parse_line_for_provider(line: &str, provider: &Provider) -> Result<Vec<StreamEvent>> {
    match provider {
        Provider::Anthropic => anthropic::parse_line(line),
        Provider::OpenAI => openai::parse_line(line),
        Provider::Gemini => gemini::parse_line(line),
        Provider::Ollama => ollama::parse_line(line),
        Provider::DeepSeek => deepseek::parse_line(line),
        Provider::OpenRouter => openrouter::parse_line(line),
        Provider::Groq => groq::parse_line(line),
        Provider::Mistral => mistral::parse_line(line),
        Provider::XAI => xai::parse_line(line),
        Provider::Cohere => cohere::parse_line(line),
    }
}
