//! Request dispatcher — maps each `RpcRequest` to the appropriate action and
//! drives `operon_rs::session::SessionRunner` for prompt execution.
//!
//! This module is intentionally a stub scaffold. The `submit_prompt` path will
//! be implemented once the full agent loop wiring is ready.

use tracing::{error, info, warn};

use crate::rpc::{
    PermissionDecisionParams, RpcEvent, RpcEventName, RpcMethod, RpcRequest, SubmitPromptParams,
};

/// Dispatches a single RPC request. Called from `main` in a spawned task.
///
/// Each method is handled independently — cancel/approve/deny are synchronous
/// operations on shared state; submit_prompt drives the async agent loop.
pub async fn dispatch(request: RpcRequest) {
    info!("dispatch: id={} method={:?}", request.id, request.method);

    match request.method {
        RpcMethod::SubmitPrompt => handle_submit_prompt(request).await,
        RpcMethod::Cancel => handle_cancel(request).await,
        RpcMethod::ApprovePermission => handle_permission_decision(request, true).await,
        RpcMethod::DenyPermission => handle_permission_decision(request, false).await,
        RpcMethod::LoadHistory => handle_load_history(request).await,
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Submits a user prompt to the agent `SessionRunner`.
///
/// Flow (to be implemented):
///   1. Parse `SubmitPromptParams` from `request.params`
///   2. Load `AppConfig` via `operon_rs::load()`
///   3. Build `SessionConfig` + mpsc channels
///   4. Spawn `SessionRunner::new(...).run(prompt, ...)` in background
///   5. Forward each `SessionEvent` to `rpc::RpcEvent::emit()` on stdout
async fn handle_submit_prompt(request: RpcRequest) {
    let params: SubmitPromptParams = match serde_json::from_value(request.params) {
        Ok(p) => p,
        Err(e) => {
            error!("submit_prompt: bad params: {}", e);
            RpcEvent {
                id: request.id,
                event: RpcEventName::AgentError,
                data: serde_json::json!({ "message": format!("invalid params: {}", e) }),
            }
            .emit();
            return;
        }
    };

    info!(
        "submit_prompt: session={:?} workspace={:?}",
        params.session_id, params.workspace_path
    );

    // TODO: wire SessionRunner here (full implementation in next pass)
    warn!("submit_prompt: handler not yet implemented");

    RpcEvent {
        id: request.id,
        event: RpcEventName::AgentError,
        data: serde_json::json!({ "message": "bridge not yet implemented" }),
    }
    .emit();
}

/// Cancels the currently running prompt, if any.
///
/// To be implemented: send `SessionCommand::Cancel` to the active `cmd_tx`.
async fn handle_cancel(request: RpcRequest) {
    info!("cancel: id={}", request.id);
    // TODO: signal the active SessionRunner via shared cmd_tx
    warn!("cancel: handler not yet implemented");
}

/// Approves or denies a pending tool permission request.
///
/// To be implemented: look up the pending `cmd_tx` by `permission_id` and
/// send `SessionCommand::Approve { id }` or `SessionCommand::Deny { id }`.
async fn handle_permission_decision(request: RpcRequest, approved: bool) {
    let params: PermissionDecisionParams = match serde_json::from_value(request.params) {
        Ok(p) => p,
        Err(e) => {
            error!("permission_decision: bad params: {}", e);
            return;
        }
    };

    info!(
        "permission_decision: id={} approved={} permission_id={}",
        request.id, approved, params.permission_id
    );

    // TODO: dispatch Approve/Deny command to the active SessionRunner
    warn!("permission_decision: handler not yet implemented");
}

/// Returns the full message history for a session as a JSON array.
///
/// To be implemented: open `SessionStore` and call `load_full_history()`.
async fn handle_load_history(request: RpcRequest) {
    let session_id = request.params["session_id"]
        .as_str()
        .unwrap_or("")
        .to_string();

    info!("load_history: id={} session={}", request.id, session_id);

    // TODO: load from SessionStore and emit as a custom event
    warn!("load_history: handler not yet implemented");
}
