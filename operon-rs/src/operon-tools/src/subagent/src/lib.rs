//! # operon-tools-subagent
//!
//! Facade crate for Operon subagent tools.
//!
//! Re-exports the subagent tools, including [`spawn_agent`].
//! Subagents are autonomous child agents running in separate sessions, capable
//! of concurrent background execution to parallelize complex workflows.

pub use operon_tools_subagent_spawn as spawn_agent;
pub use operon_tools_subagent_spawn::{
    definition as spawn_agent_definition, SpawnAgentArgs, SpawnAgentOutput, SpawnAgentToolError,
};

