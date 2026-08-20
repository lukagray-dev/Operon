//! # operon-vscode-bridge
//!
//! Stdio JSON-RPC sidecar that drives the `operon-rs` agent loop on behalf of
//! the Operon VS Code extension.
//!
//! ## Runtime model
//!
//! The VS Code extension spawns this binary as a child process on activation.
//! Communication happens exclusively over stdin/stdout using newline-delimited
//! JSON. Stderr is reserved for diagnostic logs (captured by the extension and
//! written to the "Operon Bridge" output channel).
//!
//! ```text
//! Extension (TypeScript)
//!   │  stdin  → { "id": N, "method": "...", "params": {...} }
//!   │  stdout ← { "id": N, "event": "...", "data": {...} }
//!   └─ stderr ← tracing logs (info/debug/error)
//! ```
//!
//! ## Module layout
//!
//! | Module      | Responsibility                                              |
//! |-------------|-------------------------------------------------------------|
//! | `main`      | Process entry point: tokio runtime + stdio event loop      |
//! | `rpc`       | JSON-RPC request/response/event types (mirrors rpc.ts)      |
//! | `handler`   | Drives `SessionRunner`, maps `SessionEvent` → `RpcEvent`    |

mod handler;
mod rpc;

use std::io::{self, BufRead};

use tracing::info;

#[tokio::main]
async fn main() {
    // ── Logging ───────────────────────────────────────────────────────────────
    // All logs go to stderr — stdout is reserved for the RPC protocol.
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(
            std::env::var("OPERON_LOG")
                .unwrap_or_else(|_| "info".to_string()),
        )
        .init();

    info!("operon-vscode-bridge started");

    // ── Stdio RPC loop ────────────────────────────────────────────────────────
    // Read one JSON request per line from stdin; dispatch asynchronously.
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(e) => {
                tracing::error!("stdin read error: {}", e);
                break;
            }
        };

        // Parse the incoming RPC request
        let request: rpc::RpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("invalid RPC request: {} — raw: {}", e, line);
                continue;
            }
        };

        // Clone the stdout handle so the spawned task can write responses
        let stdout_clone = stdout.lock();
        drop(stdout_clone); // release immediately — tasks take their own lock

        // Dispatch each request in its own task so methods don't block each other
        tokio::spawn(handler::dispatch(request));
    }

    info!("operon-vscode-bridge shutting down (stdin closed)");
}
