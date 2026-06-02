// MockAgent implementation
// Provides fake responses for UI development without requiring operon-rs backend
// Simulates realistic latency and response patterns

use super::AgentBridge;
use anyhow::Result;
use std::time::Duration;

/// MockAgent provides deterministic fake responses for UI testing
/// Simulates agent behavior without requiring the real backend
/// Useful for:
/// - UI development and iteration
/// - Testing UI responsiveness during agent "thinking" time
/// - Demonstrating the TUI without a full backend setup
#[allow(dead_code)]
pub struct MockAgent {
    /// Counter for generating varied responses
    message_count: std::sync::atomic::AtomicUsize,
}

impl MockAgent {
    /// Create a new MockAgent instance
    pub fn new() -> Self {
        Self {
            message_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl Default for MockAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentBridge for MockAgent {
    /// Send a message and receive a mock response
    /// Simulates realistic latency (500-1500ms) to test UI responsiveness
    /// Returns varied responses based on message count
    async fn send_message(&self, msg: &str) -> Result<String> {
        // Simulate network/processing latency
        // Real agent responses take time, so we simulate that here
        let latency_ms = 500
            + (self
                .message_count
                .load(std::sync::atomic::Ordering::Relaxed)
                % 10)
                * 100;
        tokio::time::sleep(Duration::from_millis(latency_ms as u64)).await;

        // Increment message counter for varied responses
        let count = self
            .message_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Generate mock response based on message content and count
        let response = if msg.to_lowercase().contains("hello") || msg.to_lowercase().contains("hi")
        {
            "Hello! I'm a mock agent. The real Operon backend is not yet connected. \
             I'm here to help you test the TUI interface."
                .to_string()
        } else if msg.to_lowercase().contains("help") {
            "Mock Agent Help:\n\
             - I respond to any message with a simulated delay\n\
             - I don't actually process your requests\n\
             - I'm useful for testing the UI without the backend\n\
             - Press Ctrl+Q to quit"
                .to_string()
        } else if count % 3 == 0 {
            "This is a mock response. The real agent would analyze your request, \
             execute tools, and provide a detailed answer. For now, I'm just \
             demonstrating that the chat interface works correctly."
                .to_string()
        } else if count % 3 == 1 {
            format!(
                "Mock agent response #{}. In production, this would be a real AI agent \
                 with access to your filesystem, terminal, and configured tools.",
                count
            )
        } else {
            format!(
                "I received your message: \"{}\"\n\n\
                 A real agent would process this and take appropriate action. \
                 The mock agent just echoes back to verify the UI is working.",
                msg
            )
        };

        Ok(response)
    }
}
