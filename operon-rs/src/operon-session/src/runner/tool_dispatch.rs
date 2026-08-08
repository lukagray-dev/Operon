// runner/tool_dispatch.rs — Per-tool-call handling for the agent loop.
//
// This module contains the `handle_tool_call` method and the `ToolCallFlow`
// enum. Extracted from the inline tool-call loop body in `run()` so that
// the main loop (in loop_impl.rs) stays focused on the per-turn cycle.

use operon_context::{ContentBlock, ToolCall, ToolContent, ToolResult};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::PolicyDecision;
// Hey friend! We import AskArgs so we can parse input arguments for the ask tool.
use operon_tools_ask::AskArgs;

use super::message_build::{opaque_permission_denied_result, tool_result_content_json};
use super::policy_path::policy_path_for_call;
use super::SessionRunner;

// ─────────────────────────────────────────────────────────────────────────────
// ToolCallFlow
// ─────────────────────────────────────────────────────────────────────────────

/// Outcome of a single tool call dispatch.
///
/// Used by the main loop in `loop_impl.rs` to decide whether to continue
/// processing the remaining tool calls or break out of the loop.
pub(super) enum ToolCallFlow {
    /// Tool call was processed (success or policy denial). Continue with the
    /// next call in the batch.
    Continue,
    /// A cancellation was received (either via Cancel command during an
    /// approval wait, or via the ask-tool cancel path). The loop should break.
    Stop,
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_tool_call
// ─────────────────────────────────────────────────────────────────────────────

impl SessionRunner {
    /// Handle a single tool call: ask-tool interception, policy gate, dispatch.
    ///
    /// Returns `ToolCallFlow::Stop` when a Cancel command arrives during
    /// an interactive wait (approval or ask-tool), signalling the caller to
    /// break out of the tool-call loop.
    pub(super) async fn handle_tool_call(
        &mut self,
        call: ToolCall,
        tool_results: &mut Vec<ContentBlock>,
    ) -> ToolCallFlow {
        // ── ask tool: intercept before policy check ──────────────────────────
        // Hey friend! The ask tool is unique. It suspends the loop and waits for the
        // user's answer on the command channel, bypassing the dispatcher entirely.
        if call.name == "ask" {
            let ask_id = call.id.0.clone();

            // Hey friend! We parse and validate the arguments before suspending the loop.
            // If parsing fails (for example, if the options count is incorrect), we return
            // an error ToolResult immediately without suspending.
            let ask_result = match AskArgs::from_json(&call.arguments) {
                Err(reason) => {
                    let result = ToolResult {
                        call_id: call.id.clone(),
                        name: "ask".to_string(),
                        content: ToolContent::Text(reason.to_string()),
                        is_error: true,
                    };
                    let _ = self
                        .event_tx
                        .send(SessionEvent::ToolCallResult {
                            call_id: ask_id.clone(),
                            name: "ask".to_string(),
                            is_error: true,
                            content_json: tool_result_content_json(&result),
                        })
                        .await;
                    tool_results.push(ContentBlock::ToolResult(result));
                    return ToolCallFlow::Continue;
                }
                Ok(args) => args,
            };

            // Emit AskQuestion event. The frontend UI will receive this and render
            // the multiple-choice question widget to the user.
            let _ = self
                .event_tx
                .send(SessionEvent::AskQuestion {
                    id: ask_id.clone(),
                    question: ask_result.question.clone(),
                    options: ask_result.options.to_vec(),
                })
                .await;

            // Suspend the loop and block here until we receive the answer command or a cancel command.
            let answer = loop {
                match self.wait_for_relevant_command(Some(&ask_id)).await {
                    SessionCommand::AskResponse { id, answer } if id == ask_id => {
                        break answer;
                    }
                    SessionCommand::Cancel => {
                        return ToolCallFlow::Stop;
                    }
                    _ => continue,
                }
            };

            // Hey friend! We return the user's answer directly as plain text in ToolContent::Text.
            let content = ToolContent::Text(answer);
            let result = ToolResult {
                call_id: call.id.clone(),
                name: "ask".to_string(),
                content: content.clone(),
                is_error: false,
            };
            let _ = self
                .event_tx
                .send(SessionEvent::ToolCallResult {
                    call_id: ask_id.clone(),
                    name: "ask".to_string(),
                    is_error: false,
                    content_json: tool_result_content_json(&result),
                })
                .await;
            tool_results.push(ContentBlock::ToolResult(result));
            return ToolCallFlow::Continue; // Skip the rest of the body (no dispatcher call needed)
        }

        // Policy gate: Ask / Deny / Allow are handled before dispatch.
        match self.policy_resolver.check(&call, self.caller_role()) {
            PolicyDecision::Allow => {
                // Nothing special here — fall through to the dispatcher below.
            }
            PolicyDecision::Ask { reason } => {
                let approval_id = call.id.0.clone();
                let path = policy_path_for_call(&call);
                let approval_path = path.clone();
                let args_json = serde_json::to_string(&call.arguments).unwrap_or_default();

                tracing::info!(
                    tool = %call.name,
                    approval_id = %approval_id,
                    reason = %reason,
                    "Tool call requires approval"
                );

                let _ = self
                    .event_tx
                    .send(SessionEvent::ApprovalRequired {
                        id: approval_id.clone(),
                        tool: call.name.clone(),
                        path,
                        reason,
                        args_json,
                    })
                    .await;

                match self.wait_for_relevant_command(Some(&approval_id)).await {
                    SessionCommand::Approve { id } if id == approval_id => {
                        let _ = self
                            .event_tx
                            .send(SessionEvent::ApprovalGranted {
                                id: approval_id.clone(),
                                tool: call.name.clone(),
                                path: approval_path,
                            })
                            .await;

                        tracing::info!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            "Approval granted; dispatching tool call"
                        );
                    }
                    SessionCommand::Deny { id } if id == approval_id => {
                        tracing::info!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            "Approval denied by the user"
                        );

                        let path = policy_path_for_call(&call);
                        let _ = self
                            .event_tx
                            .send(SessionEvent::PermissionDenied {
                                tool: call.name.clone(),
                                path,
                                reason: "approval denied by the user".to_string(),
                            })
                            .await;

                        let result = opaque_permission_denied_result(&call);
                        let _ = self
                            .event_tx
                            .send(SessionEvent::ToolCallResult {
                                call_id: result.call_id.0.clone(),
                                name: result.name.clone(),
                                is_error: result.is_error,
                                content_json: tool_result_content_json(&result),
                            })
                            .await;

                        tool_results.push(ContentBlock::ToolResult(result));
                        return ToolCallFlow::Continue;
                    }
                    SessionCommand::Cancel => {
                        tracing::info!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            "Session cancelled while waiting for approval"
                        );
                        return ToolCallFlow::Stop;
                    }
                    _ => {
                        tracing::warn!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            "Unexpected command returned while waiting for approval"
                        );
                        return ToolCallFlow::Continue;
                    }
                }
            }
            PolicyDecision::Deny { reason } => {
                let path = policy_path_for_call(&call);
                tracing::warn!(
                    tool = %call.name,
                    ?path,
                    reason = %reason,
                    "Tool call denied by policy"
                );

                let _ = self
                    .event_tx
                    .send(SessionEvent::PermissionDenied {
                        tool: call.name.clone(),
                        path,
                        reason,
                    })
                    .await;

                let result = opaque_permission_denied_result(&call);
                let _ = self
                    .event_tx
                    .send(SessionEvent::ToolCallResult {
                        call_id: result.call_id.0.clone(),
                        name: result.name.clone(),
                        is_error: result.is_error,
                        content_json: tool_result_content_json(&result),
                    })
                    .await;

                tool_results.push(ContentBlock::ToolResult(result));
                return ToolCallFlow::Continue;
            }
        }

        // dispatch_with_progress() returns DispatchOutcome so we can observe degradation
        // while forwarding runtime progress events to the UI.
        let progress_emitter = self.tool_progress_emitter();
        let outcome = self
            .dispatcher
            .dispatch_with_progress(call, Some(progress_emitter))
            .await;

        // If this is the FIRST malformed call for this tool, emit ToolDegraded
        // so the TUI can show a warning badge on the tool.
        if let Some(ref name) = outcome.newly_degraded {
            let _ = self
                .event_tx
                .send(SessionEvent::ToolDegraded { name: name.clone() })
                .await;
        }

        let result = outcome.result;

        let content_json = tool_result_content_json(&result);

        let _ = self
            .event_tx
            .send(SessionEvent::ToolCallResult {
                call_id: result.call_id.0.clone(),
                name: result.name.clone(),
                is_error: result.is_error,
                content_json: content_json.clone(),
            })
            .await;

        tool_results.push(ContentBlock::ToolResult(result));

        ToolCallFlow::Continue
    }
}
