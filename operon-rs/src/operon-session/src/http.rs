// http.rs — Provider HTTP request sending and SSE stream consumption.
//
// This module is responsible for:
//   1. Building provider-specific request headers (API key, content-type, etc.)
//   2. Sending the POST request and handling HTTP-level errors.
//   3. Consuming the SSE byte stream line by line.
//   4. Delegating each SSE line to the normalize-stream pipeline for parsing.
//   5. Pushing canonical SessionEvents onto the event channel as they arrive.
//   6. Accumulating and returning a fully assembled StreamResult.
//
// This module does NOT:
//   - Parse provider wire formats (that's operon-context-normalize-stream).
//   - Dispatch tool calls (that's runner.rs).
//   - Know about session state or lifecycle (that's runner.rs).
//
// Important: HTTP non-2xx errors are returned as SessionError::Stream, NOT
// SessionError::Http, because reqwest::Error does not expose a constructor for
// status-level errors. See PROMPT.md §Implementation Notes #2.

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

// ─────────────────────────────────────────────────────────────────────────────
// StreamResult
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// build_headers (private)
// ─────────────────────────────────────────────────────────────────────────────

/// Build provider-specific request headers from the provider enum + API key.
///
/// Anthropic uses a custom `x-api-key` header plus an API version pin.
/// All other (OpenAI-family) providers use the standard `Authorization: Bearer` header.
fn build_headers(provider: &Provider, api_key: &str) -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};

    let mut headers = HeaderMap::new();

    // Every provider requires JSON — set this unconditionally.
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    match provider {
        Provider::Anthropic => {
            // Anthropic uses a custom x-api-key header, not Authorization: Bearer.
            // The unwrap is safe because API keys are ASCII strings.
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(api_key).expect("API key must be a valid header value"),
            );
            // Version pin — ensures we always get the same response shape regardless
            // of future Anthropic API changes.
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        }
        _ => {
            // OpenAI-family and all other providers use Bearer token auth.
            let bearer = format!("Bearer {api_key}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&bearer).expect("API key must be a valid header value"),
            );
        }
    }

    headers
}

// ─────────────────────────────────────────────────────────────────────────────
// send_streaming
// ─────────────────────────────────────────────────────────────────────────────

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
    let mut last_emitted_len = 0;

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
                                AssemblerOutput::Text(text) => {
                                    result.text.push_str(&text);
                                    let stripped = strip_in_progress_tool_tag(&result.text);
                                    let emit_len = stripped.len();
                                    if emit_len > last_emitted_len {
                                        let delta = stripped[last_emitted_len..emit_len].to_string();
                                        // Best-effort send — we don't care if the receiver is closed.
                                        let _ = event_tx.send(SessionEvent::TextDelta { text: delta }).await;
                                        last_emitted_len = emit_len;
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

/// Strip any in-progress tool call from the tail of streamed text.
///
/// During streaming the model may have emitted a partial or complete tool tag
/// that the parser will handle once the full response arrives. Until then, the
/// raw tag syntax must not be shown as prose to the user.
///
/// Strategy: find the last `<` in the text that is followed by an ASCII
/// alphabetic character (potential tag start). If found, check whether that
/// position through the end of text looks like an in-progress tag or body block
/// (i.e. no `>>>>` closing delimiter has appeared after it). If so, strip from
/// that `<` to the end of the streamed text before emitting as prose.
fn strip_in_progress_tool_tag(text: &str) -> &str {
    // Find the last potential tag-open `<[a-zA-Z]` in the text.
    let bytes = text.as_bytes();
    let len = bytes.len();

    // Walk backwards to find the last `<` followed by alpha.
    let mut last_tag_start: Option<usize> = None;
    let mut i = len.saturating_sub(1);
    loop {
        if bytes[i] == b'<' && i + 1 < len && bytes[i + 1].is_ascii_alphabetic() {
            last_tag_start = Some(i);
            break;
        }
        if i == 0 { break; }
        i -= 1;
    }

    let tag_start = match last_tag_start {
        Some(pos) => pos,
        None => return text, // no tag candidate found
    };

    let tail = &text[tag_start..];

    // If a complete `>>>>` closing delimiter appears after the tag start,
    // the tool call is fully emitted and the parser will handle it — don't strip.
    if tail.contains(">>>>") {
        return text;
    }

    // The tag (or its body) is still in progress. Strip from tag_start onward.
    // Trim any trailing whitespace/newlines left behind.
    text[..tag_start].trim_end()
}
