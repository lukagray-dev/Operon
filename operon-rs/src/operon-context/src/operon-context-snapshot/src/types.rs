use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Session owner role from the current runtime context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Owner,
    External,
}

impl Role {
    /// Stable string used by the rendered system message.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "Owner",
            Role::External => "External",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Fast-changing bootstrap values that are always refreshed per build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootstrapBlock {
    pub agent_name: String,
    pub timestamp: String,
    pub session_id: String,
    pub role: Role,
    /// Detailed guidelines, principles, and identity instructions for the agent.
    pub system_prompt: &'static str,
}

/// A helper structure that owns the system_prompt as an owned String during deserialization.
/// This allows us to deserialize the struct without forcing the deserialization input `'de` to outlive `'static`.
#[derive(Deserialize)]
struct BootstrapBlockHelper {
    agent_name: String,
    timestamp: String,
    session_id: String,
    role: Role,
    system_prompt: String,
}

// Manual implementation of Deserialize for BootstrapBlock.
// This is done to prevent Serde's derive macro from automatically generating a `'de: 'static` lifetime bound,
// which would break compilation of parent structs that contain BootstrapBlock.
impl<'de> Deserialize<'de> for BootstrapBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // 1. Deserialize into the helper struct first, which owns the strings.
        let helper = BootstrapBlockHelper::deserialize(deserializer)?;

        // 2. Leak the owned system prompt String to obtain a &'static str reference.
        // This is safe because snapshots are built or loaded once per turn.
        let system_prompt = Box::leak(helper.system_prompt.into_boxed_str());

        Ok(BootstrapBlock {
            agent_name: helper.agent_name,
            timestamp: helper.timestamp,
            session_id: helper.session_id,
            role: helper.role,
            system_prompt,
        })
    }
}

/// Compact git summary appended to the rendered snapshot when a repo is present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub insertions: u64,
    pub deletions: u64,
}

/// Pre-rendered project tree block that can be cached between turns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryTree {
    pub root: PathBuf,
    pub rendered: String,
}

/// Full snapshot for a single build cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub bootstrap: BootstrapBlock,
    pub agents_md: Option<String>,
    #[serde(default)]
    pub channel_instructions: Option<String>,
    pub tree: DirectoryTree,
    pub git: Option<GitStatus>,
}

impl SessionSnapshot {
    /// Renders all snapshot blocks into a single plain-text system message.
    pub fn render(&self) -> String {
        let mut output = String::new();

        // Prepend the system prompt at the very beginning of the rendered output.
        // It is followed by two newlines, then the "=== OPERON SESSION ===" header.
        output.push_str(self.bootstrap.system_prompt);
        output.push_str("\n\n");

        output.push_str("=== OPERON SESSION ===\n");
        output.push_str("Agent: ");
        output.push_str(&self.bootstrap.agent_name);
        output.push('\n');
        output.push_str("Role: ");
        output.push_str(self.bootstrap.role.as_str());
        output.push('\n');
        output.push_str("Session: ");
        output.push_str(&self.bootstrap.session_id);
        output.push('\n');
        output.push_str("Time: ");
        output.push_str(&self.bootstrap.timestamp);
        output.push('\n');

        output.push_str("=== INSTRUCTIONS ===\n");
        match self.agents_md.as_deref() {
            Some(text) if !text.trim().is_empty() => {
                output.push_str(text.trim_end_matches('\n'));
                output.push('\n');
            }
            _ => {
                output.push_str("(none)\n");
            }
        }

        if let Some(text) = self.channel_instructions.as_deref() {
            if !text.trim().is_empty() {
                output.push_str("=== CHANNEL CONTEXT ===\n");
                output.push_str(text.trim_end_matches('\n'));
                output.push('\n');
            }
        }

        output.push_str("=== PROJECT ===\n");
        output.push_str("Root: ");
        // Hey friend! We ensure the rendered root path never contains the Windows verbatim prefix (`\\?\` or `\\?\UNC\`),
        // so the LLM context prompt always receives clean, standard, friendly path strings!
        let root_str = self.tree.root.display().to_string();
        let clean_root = if let Some(stripped) = root_str.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{}", stripped)
        } else if let Some(stripped) = root_str.strip_prefix(r"\\?\") {
            stripped.to_string()
        } else {
            root_str
        };
        output.push_str(&clean_root);
        output.push('\n');

        let rendered_tree = self.tree.rendered.trim_end_matches('\n');
        if !rendered_tree.is_empty() {
            output.push_str(rendered_tree);
            output.push('\n');
        }

        if let Some(git) = &self.git {
            output.push_str("=== GIT ===\n");
            output.push_str("Branch: ");
            output.push_str(&git.branch);
            output.push('\n');
            output.push_str("Staged: ");
            output.push_str(&git.staged.to_string());
            output.push_str("  Unstaged: ");
            output.push_str(&git.unstaged.to_string());
            output.push_str("  Untracked: ");
            output.push_str(&git.untracked.to_string());
            output.push('\n');
            output.push_str("Modified lines: +");
            output.push_str(&git.insertions.to_string());
            output.push_str(" -");
            output.push_str(&git.deletions.to_string());
            output.push('\n');
        }

        while output.ends_with("\n\n") {
            output.pop();
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_cleans_verbatim_root_path() {
        // Hey friend! Let's verify that even if a SessionSnapshot contains a verbatim `\\?\` root path,
        // render() strips it cleanly before sending it to the model.
        let snapshot = SessionSnapshot {
            bootstrap: BootstrapBlock {
                agent_name: "Operon".to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                session_id: "test".to_string(),
                role: Role::Owner,
                system_prompt: "test prompt",
            },
            agents_md: None,
            channel_instructions: None,
            tree: DirectoryTree {
                root: PathBuf::from(r"\\?\D:\Operon\my_workspace"),
                rendered: "src/\n  main.rs\n".to_string(),
            },
            git: None,
        };

        let rendered = snapshot.render();
        assert!(
            rendered.contains("Root: D:\\Operon\\my_workspace"),
            "Rendered prompt should contain clean root path without \\\\?\\ prefix. Got:\n{}",
            rendered
        );
        assert!(
            !rendered.contains(r"\\?\"),
            "Rendered prompt should never contain verbatim prefix \\\\?\\"
        );
    }
}
