//! Native JSON-RPC Stdio Bridge binary for Operon VS Code Extension.
//!
//! # Architecture & Protocol:
//! - Stdin: Receives JSON-RPC 2.0 requests and notifications as newline-delimited JSON.
//! - Stdout: Sends JSON-RPC 2.0 responses and unsolicited streaming event notifications.
//! - Stderr: Receives all diagnostic tracing / debug logs so stdout is never corrupted.

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info, warn};

use operon_vscode_bridge::router;
use operon_vscode_bridge::rpc::types::RpcRequest;
use operon_vscode_bridge::rpc::RpcTransport;
use operon_vscode_bridge::shared::channels_manager;
use operon_vscode_bridge::shared::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Direct all logging strictly to stderr to keep stdout 100% clean for JSON-RPC line frames
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting Operon VS Code JSON-RPC Native Bridge...");

    // 2. Initialize the thread-safe standard output transport
    let transport = RpcTransport::new();

    // 3. Initialize application state
    let state = Arc::new(AppState::new(transport.clone()));

    // 4. Register state into permission manager for async event broadcasts
    channels_manager::set_app_state(state.clone());

    // 5. Read newline-delimited JSON-RPC messages from standard input
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    info!("Operon VS Code Bridge listening on stdin...");

    while let Ok(Some(line)) = reader.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: RpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(err) => {
                warn!("Failed to parse incoming JSON-RPC frame: {err}. Raw: {trimmed}");
                transport
                    .send_error(None, -32700, &format!("Parse error: {err}"))
                    .await;
                continue;
            }
        };

        let state_clone = state.clone();
        let transport_clone = transport.clone();

        // Spawn a non-blocking asynchronous task for each request
        tokio::spawn(async move {
            let method = request.method.clone();
            let req_id = request.id.clone();
            let params = request.params.unwrap_or(serde_json::Value::Null);

            match router::dispatch(&method, params, &state_clone).await {
                Ok(result) => {
                    transport_clone.send_success(req_id, result).await;
                }
                Err(err_msg) => {
                    error!("Method execution failed for '{method}': {err_msg}");
                    transport_clone.send_error(req_id, -32603, &err_msg).await;
                }
            }
        });
    }

    info!("Stdin stream reached EOF. Shutting down Operon VS Code Bridge.");
    Ok(())
}
