//! Stop-reason canonical type plus provider-specific mapping helpers.
//!
//! Providers encode termination reasons using different enum strings and
//! different field names (`stop_reason`, `finish_reason`, `finishReason`,
//! `done_reason`). This module maps those raw strings to a stable canonical
//! [`StopReason`] and back.

use serde::{Deserialize, Serialize};

use crate::provider::Provider;

/// Canonical conversation stop reason shared across providers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StopReason {
    /// Normal completion.
    EndTurn,
    /// Model requested a tool call.
    ToolUse,
    /// Output token budget was exhausted.
    MaxTokens,
    /// A configured stop sequence was reached.
    StopSequence,
    /// Content was filtered/blocked by safety policy.
    ContentFilter,
    /// The user manually cancelled/stopped the response.
    Stop,
    /// Provider-specific stop reason not mapped by this crate.
    Other(String),
}

/// Convert a provider raw stop string into canonical [`StopReason`].
pub fn normalize_stop_reason(raw: &str, provider: &Provider) -> StopReason {
    match provider {
        Provider::Anthropic => match raw {
            "end_turn" => StopReason::EndTurn,
            "tool_use" => StopReason::ToolUse,
            "max_tokens" => StopReason::MaxTokens,
            "stop_sequence" => StopReason::StopSequence,
            other => StopReason::Other(other.to_string()),
        },
        Provider::OpenAI
        | Provider::DeepSeek
        | Provider::OpenRouter
        | Provider::Groq
        | Provider::Mistral
        | Provider::XAI
        | Provider::NvidiaNim => match raw {
            "stop" => StopReason::EndTurn,
            "tool_calls" | "function_call" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            "content_filter" => StopReason::ContentFilter,
            other => StopReason::Other(other.to_string()),
        },
        Provider::Gemini => match raw {
            "STOP" => StopReason::EndTurn,
            "MAX_TOKENS" => StopReason::MaxTokens,
            "SAFETY" | "RECITATION" => StopReason::ContentFilter,
            "TOOL_CODE_SCHEDULING" => StopReason::ToolUse,
            "OTHER" => StopReason::Other("OTHER".to_string()),
            other => StopReason::Other(other.to_string()),
        },
        Provider::Cohere => match raw {
            "COMPLETE" => StopReason::EndTurn,
            "TOOL_CALL" => StopReason::ToolUse,
            "MAX_TOKENS" => StopReason::MaxTokens,
            "ERROR_TOXIC" => StopReason::ContentFilter,
            "ERROR" => StopReason::Other("ERROR".to_string()),
            other => StopReason::Other(other.to_string()),
        },
        Provider::Ollama => match raw {
            // OpenAI-compatible /v1 aliases
            "stop" => StopReason::EndTurn,
            "tool_calls" | "function_call" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            "content_filter" => StopReason::ContentFilter,
            other => StopReason::Other(other.to_string()),
        },
    }
}

/// Convert canonical [`StopReason`] into a provider raw stop string.
///
/// For [`StopReason::Other`], the inner value cannot be returned as `&'static str`,
/// so this function returns `"unknown"` in that case.
pub fn denormalize_stop_reason(reason: &StopReason, provider: &Provider) -> &'static str {
    match provider {
        Provider::Anthropic => match reason {
            StopReason::EndTurn => "end_turn",
            StopReason::ToolUse => "tool_use",
            StopReason::MaxTokens => "max_tokens",
            StopReason::StopSequence => "stop_sequence",
            StopReason::ContentFilter => "unknown",
            StopReason::Stop => "unknown",
            StopReason::Other(_) => "unknown",
        },
        Provider::OpenAI
        | Provider::DeepSeek
        | Provider::OpenRouter
        | Provider::Groq
        | Provider::Mistral
        | Provider::XAI
        | Provider::NvidiaNim => match reason {
            StopReason::EndTurn => "stop",
            StopReason::ToolUse => "tool_calls",
            StopReason::MaxTokens => "length",
            StopReason::StopSequence => "stop",
            StopReason::ContentFilter => "content_filter",
            StopReason::Stop => "unknown",
            StopReason::Other(_) => "unknown",
        },
        Provider::Gemini => match reason {
            StopReason::EndTurn => "STOP",
            StopReason::ToolUse => "TOOL_CODE_SCHEDULING",
            StopReason::MaxTokens => "MAX_TOKENS",
            StopReason::StopSequence => "STOP",
            StopReason::ContentFilter => "SAFETY",
            StopReason::Stop => "unknown",
            StopReason::Other(_) => "unknown",
        },
        Provider::Cohere => match reason {
            StopReason::EndTurn => "COMPLETE",
            StopReason::ToolUse => "TOOL_CALL",
            StopReason::MaxTokens => "MAX_TOKENS",
            StopReason::StopSequence => "COMPLETE",
            StopReason::ContentFilter => "ERROR_TOXIC",
            StopReason::Stop => "unknown",
            StopReason::Other(_) => "unknown",
        },
        Provider::Ollama => match reason {
            StopReason::EndTurn => "stop",
            StopReason::ToolUse => "tool_calls",
            StopReason::MaxTokens => "length",
            StopReason::StopSequence => "stop",
            StopReason::ContentFilter => "content_filter",
            StopReason::Stop => "unknown",
            StopReason::Other(_) => "unknown",
        },
    }
}
