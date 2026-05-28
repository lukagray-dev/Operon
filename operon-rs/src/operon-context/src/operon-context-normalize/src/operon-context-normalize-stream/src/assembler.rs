//! Stateful stream event assembler.

use std::collections::HashMap;

use operon_context_normalize_messages::stop_reason::normalize_stop_reason;
use operon_context_normalize_messages::Provider as MessageProvider;
use operon_context_normalize_tools::{Provider, ToolCall, ToolCallId};

use crate::error::{Result, StreamNormalizeError};
use crate::types::{AssemblerOutput, StreamEvent, ToolCallBuffer};

/// Stateful per-stream assembler that converts canonical stream events into
/// complete output items.
#[derive(Debug, Clone)]
pub struct StreamAssembler {
    provider: Provider,
    tool_call_buffers: HashMap<usize, ToolCallBuffer>,
    reasoning_text: String,
    reasoning_signature: Option<String>,
    stop_reason: Option<String>,
}

impl StreamAssembler {
    /// Construct a new empty assembler for one stream lifecycle.
    pub fn new(provider: Provider) -> Self {
        Self {
            provider,
            tool_call_buffers: HashMap::new(),
            reasoning_text: String::new(),
            reasoning_signature: None,
            stop_reason: None,
        }
    }

    /// Push one canonical stream event into the assembler.
    ///
    /// Returns one output item:
    /// - `Text` and `ToolCall` when those complete immediately.
    /// - `Pending` when state was buffered.
    /// - `Reasoning` when a previous reasoning block is flushed by a new block.
    pub fn push(&mut self, event: StreamEvent) -> Result<AssemblerOutput> {
        match event {
            StreamEvent::TextDelta { text } => Ok(AssemblerOutput::Text(text)),

            StreamEvent::ReasoningDelta { text } => {
                // If a signature already exists for buffered reasoning and a new
                // delta arrives, treat this as the start of a fresh reasoning
                // block and flush the prior one first.
                if self.reasoning_signature.is_some() && !self.reasoning_text.is_empty() {
                    let flushed = AssemblerOutput::Reasoning {
                        text: std::mem::take(&mut self.reasoning_text),
                        signature: self.reasoning_signature.take(),
                    };
                    self.reasoning_text.push_str(&text);
                    return Ok(flushed);
                }

                self.reasoning_text.push_str(&text);
                Ok(AssemblerOutput::Pending)
            }

            StreamEvent::ReasoningSignature { signature } => {
                self.reasoning_signature = Some(signature);
                Ok(AssemblerOutput::Pending)
            }

            StreamEvent::ToolCallStart { index, id, name } => {
                let buffer = self.tool_call_buffers.entry(index).or_insert(ToolCallBuffer {
                    index,
                    id: None,
                    name: None,
                    arguments_json: String::new(),
                    complete: false,
                });

                if id.is_some() {
                    buffer.id = id;
                }
                if name.is_some() {
                    buffer.name = name;
                }

                Ok(AssemblerOutput::Pending)
            }

            StreamEvent::ToolCallDelta {
                index,
                arguments_fragment,
            } => {
                let buffer = self.tool_call_buffers.entry(index).or_insert(ToolCallBuffer {
                    index,
                    id: None,
                    name: None,
                    arguments_json: String::new(),
                    complete: false,
                });
                buffer.arguments_json.push_str(&arguments_fragment);
                Ok(AssemblerOutput::Pending)
            }

            StreamEvent::ToolCallEnd { index } => {
                if let Some(buffer) = self.tool_call_buffers.get_mut(&index) {
                    buffer.complete = true;
                    return self.finalize_tool_call(index);
                }
                // Some providers emit generic block-stop markers that are not
                // necessarily tool blocks. Ignore those safely.
                Ok(AssemblerOutput::Pending)
            }

            StreamEvent::ToolCallComplete {
                index,
                id,
                name,
                arguments,
            } => {
                let call = ToolCall {
                    id: ToolCallId(id.unwrap_or_else(|| format!("stream-call-{index}"))),
                    name,
                    arguments,
                };
                Ok(AssemblerOutput::ToolCall(call))
            }

            StreamEvent::StopReason { raw } => {
                self.stop_reason = Some(raw);
                Ok(AssemblerOutput::Pending)
            }

            StreamEvent::StreamStart { .. }
            | StreamEvent::UsageMeta { .. }
            | StreamEvent::Ping => Ok(AssemblerOutput::Pending),
        }
    }

    /// Finish assembly after the final stream line has been processed.
    ///
    /// Drain order:
    /// 1. Incomplete tool buffers -> error.
    /// 2. Pending reasoning text -> `Reasoning`.
    /// 3. Final normalized stop reason -> `StreamEnded`.
    pub fn finish(&mut self) -> Result<AssemblerOutput> {
        if !self.tool_call_buffers.is_empty() {
            let mut pending_indices = self.tool_call_buffers.keys().copied().collect::<Vec<_>>();
            pending_indices.sort_unstable();
            return Err(StreamNormalizeError::AssemblerIncomplete {
                provider: provider_label(&self.provider),
                detail: format!("unfinalized tool call buffers at indices: {:?}", pending_indices),
            });
        }

        if !self.reasoning_text.is_empty() {
            return Ok(AssemblerOutput::Reasoning {
                text: std::mem::take(&mut self.reasoning_text),
                signature: self.reasoning_signature.take(),
            });
        }

        let stop_reason = self
            .stop_reason
            .as_deref()
            .map(|raw| normalize_stop_reason(raw, &to_messages_provider(&self.provider)));

        Ok(AssemblerOutput::StreamEnded { stop_reason })
    }

    /// Finalize one buffered tool call into a canonical `ToolCall`.
    fn finalize_tool_call(&mut self, index: usize) -> Result<AssemblerOutput> {
        let buffer = self
            .tool_call_buffers
            .remove(&index)
            .ok_or_else(|| StreamNormalizeError::AssemblerIncomplete {
                provider: provider_label(&self.provider),
                detail: format!("missing tool call buffer for index {index}"),
            })?;

        let arguments = serde_json::from_str(&buffer.arguments_json).map_err(|source| {
            StreamNormalizeError::ToolArgsParseFailed {
                provider: provider_label(&self.provider),
                index,
                source,
            }
        })?;

        let call = ToolCall {
            id: ToolCallId(buffer.id.unwrap_or_else(|| format!("stream-call-{index}"))),
            name: buffer.name.unwrap_or_else(|| "unknown".to_string()),
            arguments,
        };

        Ok(AssemblerOutput::ToolCall(call))
    }
}

fn to_messages_provider(provider: &Provider) -> MessageProvider {
    match provider {
        Provider::Anthropic => MessageProvider::Anthropic,
        Provider::OpenAI => MessageProvider::OpenAI,
        Provider::Gemini => MessageProvider::Gemini,
        Provider::Ollama => MessageProvider::Ollama,
        Provider::DeepSeek => MessageProvider::DeepSeek,
        Provider::OpenRouter => MessageProvider::OpenRouter,
        Provider::Groq => MessageProvider::Groq,
        Provider::Mistral => MessageProvider::Mistral,
        Provider::XAI => MessageProvider::XAI,
        Provider::Cohere => MessageProvider::Cohere,
    }
}

fn provider_label(provider: &Provider) -> &'static str {
    match provider {
        Provider::Anthropic => "Anthropic",
        Provider::OpenAI => "OpenAI",
        Provider::Gemini => "Gemini",
        Provider::Ollama => "Ollama",
        Provider::DeepSeek => "DeepSeek",
        Provider::OpenRouter => "OpenRouter",
        Provider::Groq => "Groq",
        Provider::Mistral => "Mistral",
        Provider::XAI => "xAI",
        Provider::Cohere => "Cohere",
    }
}
