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

use std::collections::VecDeque;
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use operon_context_normalize_messages::StopReason;
use operon_context_normalize_stream::types::StreamEvent;
use operon_context_normalize_stream::{new_assembler, parse_line, AssemblerOutput};
use operon_context_normalize_tools::ToolCall;
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
    };

    // Buffer for building complete lines character by character.
    // SSE chunks do not respect line boundaries, so we accumulate until '\n'.
    let mut line_buf = String::new();

    // Process the byte stream chunk by chunk.
    loop {
        // Drain any immediately available commands to check if we received a Cancel command
        // before we block on stream data.
        while let Ok(cmd) = cmd_rx.try_recv() {
            pending_commands.push_back(cmd);
        }

        if pending_commands.iter().any(|cmd| matches!(cmd, SessionCommand::Cancel)) {
            tracing::info!("Cancellation detected during stream init or progress; stopping stream.");
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
                                    // Best-effort send — we don't care if the receiver is closed.
                                    let _ = event_tx.send(SessionEvent::TextDelta { text }).await;
                                }

                                // Reasoning block flushed — send to UI for display.
                                AssemblerOutput::Reasoning { text, .. } => {
                                    let _ = event_tx.send(SessionEvent::ThinkingDelta { text }).await;
                                }

                                // Complete tool call — notify UI with start then args.
                                AssemblerOutput::ToolCall(call) => {
                                    // Serialize the call arguments for the ToolCallArgsReady event.
                                    // unwrap_or_default is safe — serde_json::to_string only fails on
                                    // non-serializable types, and ToolCall.arguments is a serde_json::Value.
                                    let args_json =
                                        serde_json::to_string(&call.arguments).unwrap_or_default();

                                    // Fire ToolCallStart FIRST — tells the TUI a dispatch is imminent.
                                    let _ = event_tx
                                        .send(SessionEvent::ToolCallStart {
                                            call_id: call.id.0.clone(),
                                            name: call.name.clone(),
                                        })
                                        .await;

                                    // Fire ToolCallArgsReady SECOND — full args are now available.
                                    // The TUI can show an expandable "Arguments" section.
                                    let _ = event_tx
                                        .send(SessionEvent::ToolCallArgsReady {
                                            call_id: call.id.0.clone(),
                                            name: call.name.clone(),
                                            args_json,
                                        })
                                        .await;
                                    result.tool_calls.push(call);
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
            // Flush any remaining buffered reasoning/thinking text.
            AssemblerOutput::Reasoning { text, .. } => {
                let _ = event_tx.send(SessionEvent::ThinkingDelta { text }).await;
            }

            // If a tool call was finalized at the end (e.g. for OpenAI-compatible providers
            // that do not emit explicit ToolCallEnd events), emit it to the UI and store it.
            AssemblerOutput::ToolCall(call) => {
                // Serialize the arguments to JSON so the UI can render them cleanly.
                let args_json = serde_json::to_string(&call.arguments).unwrap_or_default();

                // Notify UI that a tool call has started.
                let _ = event_tx
                    .send(SessionEvent::ToolCallStart {
                        call_id: call.id.0.clone(),
                        name: call.name.clone(),
                    })
                    .await;

                // Notify UI that the tool call arguments are fully ready/available.
                let _ = event_tx
                    .send(SessionEvent::ToolCallArgsReady {
                        call_id: call.id.0.clone(),
                        name: call.name.clone(),
                        args_json,
                    })
                    .await;

                // Accumulate the tool call in our stream result.
                result.tool_calls.push(call);
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
