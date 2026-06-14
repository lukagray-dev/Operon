// dispatch.rs — Dispatches non-ask tool calls after performing policy checks.
//
// Hey friend! This file wraps the tool execution phase. For each tool call, we check
// the policy:
//   - If Allowed, we execute it directly via the tool dispatcher.
//   - If Denied, we immediately return a denied/degraded message.
//   - If Ask, we prompt the user for approval and block until approved, denied, or cancelled.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc;

use operon_context::{ToolCall, ToolResult};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::{CallerRole, PolicyDecision, PolicyResolver};
use operon_tools::dispatcher::Dispatcher;

use super::commands::wait_for_relevant;
use super::events::tool_result_content_json;
use super::policy::{opaque_permission_denied_result, policy_path_for_call};

/// Outcome of dispatching a single tool call.
pub enum DispatchOutcome {
    /// The tool executed and we have a result.
    Result(ToolResult),
    /// The tool call was denied (by static policy or user rejection).
    Denied(ToolResult),
    /// The session loop was cancelled.
    Cancelled,
}

/// The extracted policy gate, approval loop, and dispatcher call.
pub async fn handle_tool_call(
    call: ToolCall,
    policy_resolver: &PolicyResolver,
    caller_role: CallerRole,
    dispatcher: &mut Dispatcher,
    event_tx: &mpsc::Sender<SessionEvent>,
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
    pending_commands: &mut VecDeque<SessionCommand>,
) -> DispatchOutcome {
    // Policy gate: Ask / Deny / Allow are handled before dispatch.
    match policy_resolver.check(&call, caller_role) {
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

            let _ = event_tx
                .send(SessionEvent::ApprovalRequired {
                    id: approval_id.clone(),
                    tool: call.name.clone(),
                    path,
                    reason,
                    args_json,
                })
                .await;

            loop {
                match wait_for_relevant(cmd_rx, pending_commands, Some(&approval_id)).await {
                    SessionCommand::Approve { id } if id == approval_id => {
                        let _ = event_tx
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
                        break; // Break the wait loop and proceed to dispatcher
                    }
                    SessionCommand::Deny { id } if id == approval_id => {
                        tracing::info!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            "Approval denied by the user"
                        );

                        let path = policy_path_for_call(&call);
                        let _ = event_tx
                            .send(SessionEvent::PermissionDenied {
                                tool: call.name.clone(),
                                path,
                                reason: "approval denied by the user".to_string(),
                            })
                            .await;

                        let result = opaque_permission_denied_result(&call);
                        let _ = event_tx
                            .send(SessionEvent::ToolCallResult {
                                call_id: result.call_id.0.clone(),
                                name: result.name.clone(),
                                is_error: result.is_error,
                                content_json: tool_result_content_json(&result),
                            })
                            .await;

                        return DispatchOutcome::Denied(result);
                    }
                    SessionCommand::Cancel => {
                        tracing::info!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            "Session cancelled while waiting for approval"
                        );
                        return DispatchOutcome::Cancelled;
                    }
                    _ => {
                        tracing::warn!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            "Unexpected command returned while waiting for approval"
                        );
                        continue;
                    }
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

            let _ = event_tx
                .send(SessionEvent::PermissionDenied {
                    tool: call.name.clone(),
                    path,
                    reason,
                })
                .await;

            let result = opaque_permission_denied_result(&call);
            let _ = event_tx
                .send(SessionEvent::ToolCallResult {
                    call_id: result.call_id.0.clone(),
                    name: result.name.clone(),
                    is_error: result.is_error,
                    content_json: tool_result_content_json(&result),
                })
                .await;

            return DispatchOutcome::Denied(result);
        }
    }

    // Build the progress emitter closure so the tool can stream runtime status.
    let event_tx_clone = event_tx.clone();
    let progress_emitter = Arc::new(move |progress| {
        let _ = event_tx_clone.try_send(SessionEvent::ToolProgress(progress));
    });

    let outcome = dispatcher
        .dispatch_with_progress(call, Some(progress_emitter))
        .await;

    // If this is the FIRST malformed call for this tool, emit ToolDegraded
    // so the TUI can show a warning badge on the tool.
    if let Some(ref name) = outcome.newly_degraded {
        let _ = event_tx
            .send(SessionEvent::ToolDegraded { name: name.clone() })
            .await;
    }

    let result = outcome.result;
    let content_json = tool_result_content_json(&result);

    let _ = event_tx
        .send(SessionEvent::ToolCallResult {
            call_id: result.call_id.0.clone(),
            name: result.name.clone(),
            is_error: result.is_error,
            content_json: content_json.clone(),
        })
        .await;

    DispatchOutcome::Result(result)
}
