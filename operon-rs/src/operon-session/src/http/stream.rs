// stream.rs — Handles consuming the Server-Sent Events (SSE) stream from the LLM provider.
//
// Hey friend! This file consumes chunks of bytes from the HTTP response stream,
// processes them into line-buffered strings, feeds them into the normalizer parser,
// and emits live events (like text deltas) to the frontend.

use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::collections::VecDeque;
use tokio::sync::mpsc;

use operon_context::normalize::stream::{new_assembler, parse_line, AssemblerOutput, StreamEvent};
use operon_context::{StopReason, ToolCall};
use operon_events::{SessionCommand, SessionEvent};
use operon_providers::Provider;

use crate::error::SessionError;
use super::headers::build_headers;
use super::detector::{StreamingTagDetector, DetectorEvent};

/// Fully assembled output from consuming one SSE stream from the provider.
///
/// Returned by [`send_streaming`] once the stream ends. The runner uses this
/// to update conversation history and drive tool dispatch.
pub struct StreamResult {
    /// All text deltas concatenated into a single string.
    pub text: String,

    /// All complete tool calls assembled from fragmented stream events.
    pub tool_calls: Vec<ToolCall>,

    /// The stop reason, if one was emitted by the provider.
    pub stop_reason: Option<StopReason>,

    /// Raw usage metadata from the stream (if the provider emitted it).
    /// Used by the runner to record exact token counts via the token tracker.
    pub usage_raw: Option<Value>,

    /// The complete reasoning block accumulated during the stream.
    pub reasoning: Option<operon_context::ReasoningBlock>,
}

/// Send one streaming request to the provider and consume the entire SSE response.
///
/// Events are pushed onto `event_tx` as they arrive in real time so the UI
/// can render streaming output. The fully assembled [`StreamResult`] is
/// returned once the stream terminates.
///
/// # SSE framing
///
/// The SSE protocol sends lines of the form:
/// ```text
/// data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}
/// ```
/// This function strips the `data: ` prefix and passes the payload to the
/// normalize-stream pipeline's `parse_line` function.
///
/// Empty lines (SSE event separators) and `:` keepalive lines are silently skipped.
///
/// # Errors
///
/// - [`SessionError::Stream`] on HTTP non-2xx responses (with status + body).
/// - [`SessionError::Stream`] on SSE parse failures.
/// - [`SessionError::Http`] on network-level failures (reqwest::Error).
pub async fn send_streaming(
    client: &Client,
    provider: &Provider,
    endpoint: &str,
    api_key: &str,
    body: Value,
    event_tx: &mpsc::Sender<SessionEvent>,
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
    pending_commands: &mut VecDeque<SessionCommand>,
    turn_index: usize,
) -> Result<StreamResult, SessionError> {
    // Build provider-specific headers.
    let headers = build_headers(provider, api_key);

    // Send the POST request. This does NOT yet read the body — it only sends
    // the request and waits for the response headers.
    let response = client
        .post(endpoint)
        .headers(headers)
        .json(&body)
        .send()
        .await?; // reqwest::Error → SessionError::Http via #[from]

    // Non-2xx status codes are provider-level errors (rate limits, invalid key,
    // malformed request, etc.). We extract the body text for a useful error message.
    // NOTE: We intentionally do NOT propagate these as SessionError::Http because
    // reqwest::Error has no public constructor for status errors.
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(SessionError::Stream(format!("HTTP {status}: {text}")));
    }

    // Wrap the response body in a byte stream — reqwest returns chunks as bytes.
    let mut byte_stream = response.bytes_stream();

    // Create a fresh assembler for this stream lifecycle.
    // The assembler accumulates fragmented tool call arguments and reasoning blocks.
    let mut assembler = new_assembler(provider);

    // Accumulate results as we consume stream events.
    let mut result = StreamResult {
        text: String::new(),
        tool_calls: Vec::new(),
        stop_reason: None,
        usage_raw: None,
        reasoning: None,
    };

    // Buffer for building complete lines character by character.
    // SSE chunks do not respect line boundaries, so we accumulate until '\n'.
    let mut line_buf = String::new();

    // Hey friend! We initialize the streaming tag detector for the current turn.
    // It sits between the SSE text output and our event channel, parsing tool tags on the fly.
    let mut detector = StreamingTagDetector::new(turn_index);

    // Process the byte stream chunk by chunk.
    loop {
        // Drain any immediately available commands to check if we received a Cancel command
        // before we block on stream data.
        while let Ok(cmd) = cmd_rx.try_recv() {
            pending_commands.push_back(cmd);
        }

        if pending_commands
            .iter()
            .any(|cmd| matches!(cmd, SessionCommand::Cancel))
        {
            tracing::info!(
                "Cancellation detected during stream init or progress; stopping stream."
            );
            result.stop_reason = Some(StopReason::Stop);
            break;
        }

        tokio::select! {
            chunk_opt = byte_stream.next() => {
                let chunk = match chunk_opt {
                    Some(chunk) => chunk,
                    None => break, // Stream ended normally
                };

                // Propagate network errors (dropped connection, TLS error, etc.).
                let chunk = chunk?; // reqwest::Error → SessionError::Http
                let chunk_str = String::from_utf8_lossy(&chunk);

                // Walk through the chunk character by character to split on newlines.
                for ch in chunk_str.chars() {
                    if ch == '\n' {
                        // Complete line ready — trim trailing whitespace (carriage return, spaces).
                        let line = line_buf.trim().to_string();
                        line_buf.clear();

                        // SSE protocol: lines not starting with "data: " are either
                        // event type lines ("event: ..."), keepalive pings (":"), or
                        // empty separators. We only care about data lines.
                        let payload = match line.strip_prefix("data: ") {
                            Some(p) => p,
                            None => continue,
                        };

                        // Parse the SSE payload into canonical stream events.
                        // parse_line handles "[DONE]" and empty payloads gracefully.
                        let events = parse_line(payload, provider)
                            .map_err(|e| SessionError::Stream(e.to_string()))?;

                        for event in events {
                            // Capture usage metadata before pushing to assembler — the
                            // assembler returns Pending for UsageMeta events.
                            if let StreamEvent::UsageMeta { raw } = &event {
                                result.usage_raw = Some(raw.clone());
                            }

                            // Feed the event into the assembler. The assembler converts
                            // fragmented events into complete output items.
                            match assembler
                                .push(event)
                                .map_err(|e| SessionError::Stream(e.to_string()))?
                            {
                                // Complete text delta — send to UI immediately and accumulate.
                                // Hey friend! We push the streamed text to our StreamingTagDetector
                                // which returns events telling us what is safe prose vs what is a tool call.
                                AssemblerOutput::Text(text) => {
                                    result.text.push_str(&text);
                                    let det_events = detector.push(&text);
                                    for det_event in det_events {
                                        match det_event {
                                            DetectorEvent::TextDelta(t) => {
                                                let _ = event_tx.send(SessionEvent::TextDelta { text: t }).await;
                                            }
                                            DetectorEvent::ToolCallDetected { call_id, name, attrs } => {
                                                let _ = event_tx.send(SessionEvent::ToolCallDetected {
                                                    call_id,
                                                    name,
                                                    attrs,
                                                }).await;
                                            }
                                            DetectorEvent::ToolBodyStarted { call_id, name, attrs } => {
                                                let _ = event_tx.send(SessionEvent::ToolCallDetected {
                                                    call_id: call_id.clone(),
                                                    name,
                                                    attrs,
                                                }).await;
                                            }
                                            DetectorEvent::ToolBodyDelta { call_id, text } => {
                                                let _ = event_tx.send(SessionEvent::ToolBodyDelta {
                                                    call_id,
                                                    text,
                                                }).await;
                                            }
                                            DetectorEvent::ToolCallComplete { .. } => {
                                                // No event needed — the full result arrives after parsing.
                                            }
                                        }
                                    }
                                }

                                // Hey friend! When we get a ReasoningDelta, it means the model is actively thinking.
                                // We send it to the UI immediately via event_tx so the user sees the thinking stream!
                                AssemblerOutput::ReasoningDelta(text) => {
                                    let _ = event_tx.send(SessionEvent::ThinkingDelta { text }).await;
                                }

                                // Hey friend! A Reasoning block was flushed or completed. We wrap it and store it in
                                // our StreamResult so we can add it to the final message block structure later.
                                AssemblerOutput::Reasoning { text, signature } => {
                                    result.reasoning = Some(operon_context::ReasoningBlock {
                                        thinking: text,
                                        signature: signature.map(operon_context::ReasoningSignature),
                                    });
                                }

                                // Stream ended — record the stop reason.
                                AssemblerOutput::StreamEnded { stop_reason } => {
                                    result.stop_reason = stop_reason;
                                }

                                // Assembler buffered state internally — no external output yet.
                                AssemblerOutput::Pending => {}
                            }
                        }
                    } else {
                        // Not a newline — accumulate into the current line buffer.
                        line_buf.push(ch);
                    }
                }
            }
            cmd_opt = cmd_rx.recv() => {
                match cmd_opt {
                    Some(cmd) => {
                        let is_cancel = matches!(cmd, SessionCommand::Cancel);
                        pending_commands.push_back(cmd);
                        if is_cancel {
                            tracing::info!("Cancellation received mid-stream; breaking stream select loop.");
                            result.stop_reason = Some(StopReason::Stop);
                            break;
                        }
                    }
                    None => {
                        // Command channel closed (e.g. frontend crashed).
                        break;
                    }
                }
            }
        }
    }

    // Signal the assembler that the stream is complete.
    // The assembler will now flush any final buffered outputs. Since finish() returns a Vec,
    // we iterate over all generated outputs (such as finalized tool calls, reasoning blocks,
    // and the final StreamEnded/stop reason) and process them.
    let final_outputs = assembler
        .finish()
        .map_err(|e| SessionError::Stream(e.to_string()))?;

    for output in final_outputs {
        match output {
            // Hey friend! Any leftover reasoning text in the assembler buffer is flushed at the end of the stream.
            // We store it in StreamResult so it's captured and saved as part of the assistant's message block.
            AssemblerOutput::Reasoning { text, signature } => {
                result.reasoning = Some(operon_context::ReasoningBlock {
                    thinking: text,
                    signature: signature.map(operon_context::ReasoningSignature),
                });
            }

            // Final stop reason from the assembler.
            AssemblerOutput::StreamEnded { stop_reason } => {
                // We prefer the stop reason already recorded directly from the stream events,
                // but use the assembler's finalized stop reason as a fallback if needed.
                if result.stop_reason.is_none() {
                    result.stop_reason = stop_reason;
                }
            }

            // Other outputs are unexpected at finish, so we can ignore them.
            _ => {}
        }
    }

    Ok(result)
}


