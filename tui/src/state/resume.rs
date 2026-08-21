// resume.rs — Session discovery and resume state management for Operon TUI.
//
// ZERO BUSINESS LOGIC IN FRONTEND:
// Reads persisted JSON sessions directly from `~/.operon/sessions/*.json`,
// filters by the active workspace directory, and loads conversation turns for resumption.

use serde::Deserialize;

use crate::ui::screens::permissions::state::{clean_windows_path, is_same_path};

/// Summary metadata for a previous conversation session.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Unique session identifier (maps to `<id>.json` on disk).
    pub id: String,
    /// Human-friendly conversation title (derived from first user prompt or custom title).
    pub title: String,
    /// Absolute workspace directory associated with this session.
    #[allow(dead_code)]
    pub workspace: String,
    /// AI model used for this session (e.g. "claude-3-5-sonnet", "gpt-4o").
    pub model_id: String,
    /// AI provider used for this session (e.g. "Anthropic", "OpenAI").
    #[allow(dead_code)]
    pub provider: String,
    /// Number of conversation turns recorded.
    pub turn_count: usize,
    /// Unix timestamp when the session was created.
    pub created_at: i64,
    /// Formatted local/relative timestamp string for UI display.
    pub formatted_time: String,
}

/// Transient schema for quickly deserializing session files from disk.
#[derive(Debug, Deserialize)]
struct RawSessionFile {
    pub id: String,
    pub created_at: i64,
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub model_id: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub turns: Vec<RawTurnFile>,
}

#[derive(Debug, Deserialize)]
struct RawTurnFile {
    #[serde(default)]
    pub messages: Vec<serde_json::Value>,
}

/// Resume screen state holding discovered previous conversations.
#[derive(Debug, Clone, Default)]
pub struct ResumeState {
    /// List of sessions found for the active workspace, sorted newest first.
    pub sessions: Vec<SessionSummary>,
    /// Index of the currently highlighted session in the list.
    pub selected_index: usize,
    /// Canonical path of the active workspace being inspected.
    pub current_workspace: String,
    /// Error message if session discovery failed.
    pub error: Option<String>,
}

impl ResumeState {
    /// Create a new empty ResumeState.
    pub fn new() -> Self {
        Self::default()
    }

    /// Discovers all previous sessions for the active workspace directory.
    pub fn refresh_sessions(&mut self) {
        let current_dir = match std::env::current_dir() {
            Ok(d) => clean_windows_path(&d.to_string_lossy()),
            Err(_) => ".".to_string(),
        };
        self.current_workspace = current_dir.clone();

        let paths = match operon_rs::OperonPaths::resolve() {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(format!("Failed to resolve Operon paths: {}", e));
                self.sessions.clear();
                return;
            }
        };

        let sessions_dir = &paths.sessions_dir;
        if !sessions_dir.exists() {
            self.sessions.clear();
            self.error = None;
            return;
        }

        let mut discovered = Vec::new();

        if let Ok(entries) = std::fs::read_dir(sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(raw) = serde_json::from_str::<RawSessionFile>(&content) {
                            // Only include sessions that match the current workspace
                            let session_ws = raw.workspace.trim();
                            let is_match = session_ws.is_empty()
                                || is_same_path(session_ws, &current_dir)
                                || is_same_path(session_ws, &paths.workspace_dir.to_string_lossy());

                            if is_match {
                                let turn_count = raw.turns.len();
                                let title = match raw.title {
                                    Some(t) if !t.trim().is_empty() => t,
                                    _ => extract_first_user_prompt(&raw.turns),
                                };

                                let formatted_time = format_timestamp(raw.created_at);

                                discovered.push(SessionSummary {
                                    id: raw.id,
                                    title,
                                    workspace: raw.workspace,
                                    model_id: if raw.model_id.is_empty() {
                                        "default".to_string()
                                    } else {
                                        raw.model_id
                                    },
                                    provider: raw.provider,
                                    turn_count,
                                    created_at: raw.created_at,
                                    formatted_time,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Sort newest sessions first
        discovered.sort_by_key(|b| std::cmp::Reverse(b.created_at));

        self.sessions = discovered;
        self.selected_index = 0;
        self.error = None;
    }

    /// Returns the currently highlighted session summary (if any).
    pub fn selected_session(&self) -> Option<&SessionSummary> {
        self.sessions.get(self.selected_index)
    }

    /// Moves the selection cursor up.
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Moves the selection cursor down.
    pub fn move_down(&mut self) {
        if !self.sessions.is_empty() && self.selected_index < self.sessions.len() - 1 {
            self.selected_index += 1;
        }
    }
}

/// Extracts a clean, concise title from the first user message in turn history.
fn extract_first_user_prompt(turns: &[RawTurnFile]) -> String {
    for turn in turns {
        for msg in &turn.messages {
            if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                if role.eq_ignore_ascii_case("user") {
                    if let Some(content) = msg.get("content") {
                        if let Some(text) = content.as_str() {
                            return clean_prompt_snippet(text);
                        } else if let Some(arr) = content.as_array() {
                            for block in arr {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    if !text.trim().is_empty() {
                                        return clean_prompt_snippet(text);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    "Untitled Conversation".to_string()
}

/// Truncates and formats a prompt snippet cleanly on a single line.
fn clean_prompt_snippet(text: &str) -> String {
    let single_line = text.lines().next().unwrap_or("").trim();
    if single_line.len() > 60 {
        format!("{}...", &single_line[..57])
    } else if single_line.is_empty() {
        "Untitled Conversation".to_string()
    } else {
        single_line.to_string()
    }
}

/// Formats a Unix timestamp into a readable date/time string.
fn format_timestamp(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "-".to_string();
    }

    // Convert epoch seconds to local ISO-like string
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{}m ago", mins)
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{}h ago", hours)
    } else if diff < 604800 {
        let days = diff / 86400;
        format!("{}d ago", days)
    } else {
        // Simple date estimation
        let days_since_epoch = timestamp / 86400;
        let approx_year = 1970 + days_since_epoch / 365;
        let day_of_year = days_since_epoch % 365;
        let approx_month = (day_of_year / 30) + 1;
        let approx_day = (day_of_year % 30) + 1;
        format!(
            "{:04}-{:02}-{:02}",
            approx_year,
            approx_month.min(12),
            approx_day.min(31)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_prompt_snippet() {
        assert_eq!(clean_prompt_snippet(""), "Untitled Conversation");
        assert_eq!(clean_prompt_snippet("Hello World"), "Hello World");
        assert_eq!(clean_prompt_snippet("Line 1\nLine 2\nLine 3"), "Line 1");
        let long_prompt = "This is a very long prompt that contains more than sixty characters to test truncation";
        let cleaned = clean_prompt_snippet(long_prompt);
        assert!(cleaned.ends_with("..."));
        assert_eq!(cleaned.len(), 60);
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "-");
        assert_eq!(format_timestamp(-1), "-");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_timestamp(now), "Just now");
        assert_eq!(format_timestamp(now - 120), "2m ago");
        assert_eq!(format_timestamp(now - 7200), "2h ago");
    }

    #[test]
    fn test_resume_state_navigation() {
        let mut state = ResumeState::new();
        state.sessions = vec![
            SessionSummary {
                id: "1".to_string(),
                title: "One".to_string(),
                workspace: "".to_string(),
                model_id: "m".to_string(),
                provider: "p".to_string(),
                turn_count: 1,
                created_at: 100,
                formatted_time: "t".to_string(),
            },
            SessionSummary {
                id: "2".to_string(),
                title: "Two".to_string(),
                workspace: "".to_string(),
                model_id: "m".to_string(),
                provider: "p".to_string(),
                turn_count: 2,
                created_at: 200,
                formatted_time: "t".to_string(),
            },
        ];

        assert_eq!(state.selected_index, 0);
        assert_eq!(state.selected_session().unwrap().id, "1");

        state.move_down();
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.selected_session().unwrap().id, "2");

        state.move_down(); // Capped
        assert_eq!(state.selected_index, 1);

        state.move_up();
        assert_eq!(state.selected_index, 0);

        state.move_up(); // Capped at 0
        assert_eq!(state.selected_index, 0);
    }
}
