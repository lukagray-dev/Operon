//! Stateful stream event assembler.

use operon_context_normalize_messages::stop_reason::normalize_stop_reason;
use operon_context_normalize_messages::Provider as MessageProvider;
use operon_providers::Provider;

use crate::error::Result;
use crate::types::{AssemblerOutput, StreamEvent};

/// Stateful per-stream assembler that converts canonical stream events into
/// complete output items.
#[derive(Debug, Clone)]
pub struct StreamAssembler {
    provider: Provider,
    reasoning_text: String,
    reasoning_signature: Option<String>,
    stop_reason: Option<String>,
}

impl StreamAssembler {
    /// Construct a new empty assembler for one stream lifecycle.
    pub fn new(provider: Provider) -> Self {
        Self {
            provider,
            reasoning_text: String::new(),
            reasoning_signature: None,
            stop_reason: None,
        }
    }

    /// Push one canonical stream event into the assembler.
    ///
    /// Returns one output item:
    /// - `Text` when text completes immediately.
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
    /// 1. Take and return any pending reasoning/thinking text.
    /// 2. Return the final normalized stop reason.
    pub fn finish(&mut self) -> Result<Vec<AssemblerOutput>> {
        let mut outputs = Vec::new();

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
