// ============================================================================
// Stdio JSON-RPC Communication Engine
// ============================================================================

pub mod types;

use std::sync::Arc;
use tokio::io::{AsyncWriteExt, Stdout};
use tokio::sync::Mutex;
pub use types::{RpcError, RpcNotification, RpcRequest, RpcResponse};

#[derive(Clone)]
pub struct RpcTransport {
    stdout: Arc<Mutex<Stdout>>,
}

impl Default for RpcTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcTransport {
    pub fn new() -> Self {
        Self {
            stdout: Arc::new(Mutex::new(tokio::io::stdout())),
        }
    }

    /// Sends a JSON-RPC response back over stdout.
    pub async fn send_response(&self, resp: RpcResponse) {
        if let Ok(line) = serde_json::to_string(&resp) {
            let mut out = self.stdout.lock().await;
            let _ = out.write_all(line.as_bytes()).await;
            let _ = out.write_all(b"\n").await;
            let _ = out.flush().await;
        }
    }

    /// Sends a successful JSON-RPC response.
    pub async fn send_success(&self, id: Option<serde_json::Value>, result: serde_json::Value) {
        self.send_response(RpcResponse::success(id, result)).await;
    }

    /// Sends an error JSON-RPC response.
    pub async fn send_error(&self, id: Option<serde_json::Value>, code: i64, message: &str) {
        self.send_response(RpcResponse::error(id, code, message.to_string())).await;
    }

    /// Sends an asynchronous JSON-RPC notification event (streaming token, tool call, etc.).
    pub async fn send_notification(&self, method: &str, params: serde_json::Value) {
        let notif = RpcNotification {
            method: method.to_string(),
            params,
        };
        if let Ok(line) = serde_json::to_string(&notif) {
            let mut out = self.stdout.lock().await;
            let _ = out.write_all(line.as_bytes()).await;
            let _ = out.write_all(b"\n").await;
            let _ = out.flush().await;
        }
    }
}
