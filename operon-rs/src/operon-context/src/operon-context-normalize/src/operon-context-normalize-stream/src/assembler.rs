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
                // If a signature already exists for our buffered reasoning and a new
                // delta arrives, it means the model is transitioning to a fresh/new reasoning
                // block. We take the current accumulated reasoning text and flush it first.
                if self.reasoning_signature.is_some() && !self.reasoning_text.is_empty() {
                    let flushed = AssemblerOutput::Reasoning {
                        text: std::mem::take(&mut self.reasoning_text),
                        signature: self.reasoning_signature.take(),
                    };
                    self.reasoning_text.push_str(&text);
                    return Ok(flushed);
                }

                // We append the reasoning text to our internal reasoning accumulator
                // so we can build the complete ReasoningBlock at the end. But we ALSO yield the
                // delta immediately so the UI/TUI can stream it to the user in real-time!
                self.reasoning_text.push_str(&text);
                Ok(AssemblerOutput::ReasoningDelta(text))
            }

            StreamEvent::ReasoningSignature { signature } => {
                self.reasoning_signature = Some(signature);
                Ok(AssemblerOutput::Pending)
            }

            StreamEvent::ToolCallStart { index, id, name } => {
                let buffer = self
                    .tool_call_buffers
                    .entry(index)
                    .or_insert(ToolCallBuffer {
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
                let buffer = self
                    .tool_call_buffers
                    .entry(index)
                    .or_insert(ToolCallBuffer {
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

            StreamEvent::StreamStart { .. } | StreamEvent::UsageMeta { .. } | StreamEvent::Ping => {
                Ok(AssemblerOutput::Pending)
            }
        }
    }

    /// Finish assembly after the final stream line has been processed.
    ///
    /// We drain any remaining outputs in the following order:
    /// 1. Finalize any uncompleted tool call buffers (or error if the provider requires explicit end events).
    /// 2. Take and return any pending reasoning/thinking text.
    /// 3. Return the final normalized stop reason.
    ///
    /// Since multiple outputs can be drained at the end (e.g. finalized tool calls AND a stop reason),
    /// this function returns a list of outputs rather than just one.
    pub fn finish(&mut self) -> Result<Vec<AssemblerOutput>> {
        let mut outputs = Vec::new();

        // Check our active provider type to determine how to handle unclosed tool calls.
        // Some providers (like Anthropic and Cohere) send explicit events when a tool call finishes.
        // If they finish streaming but still have buffers, it's a protocol mismatch/incomplete error.
        // Other providers (like OpenAI, Groq, DeepSeek, etc.) stream argument chunks but never send
        // an explicit end marker. We must finalize those automatically here!
        match self.provider {
            Provider::Anthropic | Provider::Cohere => {
                if !self.tool_call_buffers.is_empty() {
                    let mut pending_indices =
                        self.tool_call_buffers.keys().copied().collect::<Vec<_>>();
                    pending_indices.sort_unstable();
                    return Err(StreamNormalizeError::AssemblerIncomplete {
                        provider: provider_label(&self.provider),
                        detail: format!(
                            "unfinalized tool call buffers at indices: {:?}",
                            pending_indices
                        ),
                    });
                }
            }
            _ => {
                // For OpenAI-compatible endpoints, any tool call buffers still present
                // are completed, as the provider has stopped sending argument deltas.
                if !self.tool_call_buffers.is_empty() {
                    let mut pending_indices =
                        self.tool_call_buffers.keys().copied().collect::<Vec<_>>();
                    pending_indices.sort_unstable();
                    for index in pending_indices {
                        match self.finalize_tool_call(index) {
                            Ok(AssemblerOutput::ToolCall(call)) => {
                                outputs.push(AssemblerOutput::ToolCall(call));
                            }
                            Ok(_) => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
        }

        // If there's any remaining thinking/reasoning text that hasn't been emitted yet,
        // take it out of our internal buffer and yield it.
        if !self.reasoning_text.is_empty() {
            outputs.push(AssemblerOutput::Reasoning {
                text: std::mem::take(&mut self.reasoning_text),
                signature: self.reasoning_signature.take(),
            });
        }

        // Determine the final normalized stop reason from our raw stream value.
        let stop_reason = self
            .stop_reason
            .as_deref()
            .map(|raw| normalize_stop_reason(raw, &to_messages_provider(&self.provider)));

        outputs.push(AssemblerOutput::StreamEnded { stop_reason });

        Ok(outputs)
    }

    /// Finalize one buffered tool call into a canonical `ToolCall`.
    fn finalize_tool_call(&mut self, index: usize) -> Result<AssemblerOutput> {
        let buffer = self.tool_call_buffers.remove(&index).ok_or_else(|| {
            StreamNormalizeError::AssemblerIncomplete {
                provider: provider_label(&self.provider),
                detail: format!("missing tool call buffer for index {index}"),
            }
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
        Provider::NvidiaNim => MessageProvider::NvidiaNim,
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
        Provider::NvidiaNim => "NVIDIA NIM",
        Provider::Cohere => "Cohere",
    }
}
