// AgentBridge trait
// The TUI's only contact point with operon-rs backend
// All business logic, config loading, and agent execution happens through this trait
// MockAgent in mock.rs provides a fake implementation for UI-only development

pub mod mock;

use anyhow::Result;

/// AgentBridge is the interface between TUI and operon-rs backend
#[allow(dead_code)]
/// The TUI never directly accesses business logic — everything goes through this trait
/// This allows the TUI to be developed and tested independently of the backend
///
/// Future methods to add when backend is ready:
/// - async fn load_config() -> Result<Config>
/// - async fn list_models() -> Result<Vec<ModelInfo>>
/// - async fn set_active_model(model_id: &str) -> Result<()>
/// - async fn get_permissions() -> Result<PermissionRules>
/// - async fn set_permission(rule: PermissionRule) -> Result<()>
/// - async fn list_skills() -> Result<Vec<SkillInfo>>
/// - async fn toggle_skill(skill_id: &str, enabled: bool) -> Result<()>
/// - async fn execute_command(cmd: &str) -> Result<CommandOutput>
#[async_trait::async_trait]
pub trait AgentBridge: Send + Sync {
    /// Send a message to the agent and receive a response
    /// This is the primary interaction method for the chat interface
    ///
    /// # Arguments
    /// * `msg` - The user's message text
    ///
    /// # Returns
    /// * `Ok(String)` - The agent's response text
    /// * `Err(...)` - If the agent failed to process the message
    async fn send_message(&self, msg: &str) -> Result<String>;
}
