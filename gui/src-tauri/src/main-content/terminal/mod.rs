//! Terminal Backend Module.
//!
//! This module provides Tauri IPC commands for managing interactive pseudo-terminal (PTY)
//! processes (PowerShell on Windows, bash on Unix). It streams stdout/stderr asynchronously
//! to the frontend xterm.js instance via Tauri events and receives keystrokes and resize signals.
//!
//! # Architecture:
//! - PTY sessions are managed via `operon_rs::terminal::TerminalSession` (built on `portable-pty`).
//! - Output is emitted via the `"terminal-output"` event (`{ id: string, data: string }`).
//! - Process termination is signaled via `"terminal-closed"` (`{ id: string }`).
//! - Workdir resolution automatically chooses the active project directory in project sessions
//!   or falls back to Operon's default global workspace directory in general chat sessions.

use crate::shared::AppState;
use operon_rs::config::OperonPaths;
use operon_rs::terminal::TerminalSession;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{Emitter, State, WebviewWindow};

/// Type alias for the thread-safe active terminal session map.
pub type ActiveTerminalRegistry = Arc<Mutex<HashMap<String, Arc<TerminalSession>>>>;

/// Global thread-safe registry of active PTY sessions indexed by unique session ID.
static ACTIVE_TERMINALS: OnceLock<ActiveTerminalRegistry> = OnceLock::new();

/// Returns the singleton reference to the active terminals map.
pub fn get_active_terminals() -> &'static ActiveTerminalRegistry {
    ACTIVE_TERMINALS.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

/// Payload emitted to the frontend when standard output is received from a PTY process.
#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputPayload {
    /// Unique tab session ID (e.g. "term_123456").
    pub id: String,
    /// Terminal character stream / ANSI escape sequence chunk.
    pub data: String,
}

/// Payload emitted to the frontend when a PTY shell process terminates.
#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TerminalClosedPayload {
    /// Unique tab session ID that exited.
    pub id: String,
}

/// Resolves the starting working directory for a new terminal session.
///
/// # Priority:
/// 1. Explicit `workdir` passed from frontend (if non-empty and valid).
/// 2. Active project path from `AppState` (when inside a project-specific session).
/// 3. Default Operon workspace directory (`~/.operon/workspace/`).
pub fn resolve_terminal_workdir(
    explicit_workdir: Option<&str>,
    active_project: Option<&str>,
) -> Option<String> {
    // 1. Check explicit workdir
    if let Some(dir) = explicit_workdir {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.exists() {
                return Some(trimmed.to_string());
            }
        }
    }

    // 2. Check active project path from current session state
    if let Some(project) = active_project {
        let trimmed = project.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.exists() {
                return Some(trimmed.to_string());
            }
        }
    }

    // 3. Fallback to Operon's global workspace directory
    if let Ok(paths) = OperonPaths::resolve() {
        if paths.workspace_dir.exists() {
            return Some(paths.workspace_dir.to_string_lossy().to_string());
        }
    }

    None
}

/// Creates and spawns a new pseudo-terminal process with the specified ID and dimensions.
#[tauri::command]
pub async fn create_terminal(
    id: String,
    cols: u16,
    rows: u16,
    workdir: Option<String>,
    state: State<'_, AppState>,
    window: WebviewWindow,
) -> Result<(), String> {
    let registry = get_active_terminals();

    // 1. Check if a terminal with this ID already exists
    {
        let guard = registry
            .lock()
            .map_err(|e| format!("Failed to lock terminal registry: {}", e))?;
        if guard.contains_key(&id) {
            return Err(format!("Terminal session '{}' already exists", id));
        }
    }

    // 2. Resolve initial working directory
    let active_proj = {
        if let Ok(lock) = state.state_lock.lock() {
            lock.active_project.clone()
        } else {
            None
        }
    };

    let resolved_workdir = resolve_terminal_workdir(workdir.as_deref(), active_proj.as_deref());

    // 3. Setup output listener forwarding to frontend via Tauri event
    let window_output = window.clone();
    let id_output = id.clone();
    let on_output = move |data: &str| {
        let _ = window_output.emit(
            "terminal-output",
            TerminalOutputPayload {
                id: id_output.clone(),
                data: data.to_string(),
            },
        );
    };

    // 4. Setup exit listener notifying frontend and removing session
    let window_exit = window.clone();
    let id_exit = id.clone();
    let on_exit = move || {
        let _ = window_exit.emit(
            "terminal-closed",
            TerminalClosedPayload {
                id: id_exit.clone(),
            },
        );
        if let Ok(mut guard) = get_active_terminals().lock() {
            guard.remove(&id_exit);
        }
    };

    // 5. Spawn the PTY process using operon_rs::terminal
    let session =
        TerminalSession::new(id.clone(), resolved_workdir, cols, rows, on_output, on_exit)
            .map_err(|e| format!("Failed to spawn shell PTY process: {}", e))?;

    // 6. Insert into global registry
    {
        let mut guard = registry
            .lock()
            .map_err(|e| format!("Failed to lock terminal registry: {}", e))?;
        guard.insert(id.clone(), Arc::new(session));
    }

    tracing::info!("Created PTY terminal session '{}'", id);
    Ok(())
}

/// Writes user keystroke data or command string into the running terminal's stdin.
#[tauri::command]
pub async fn write_terminal(id: String, input: String) -> Result<(), String> {
    let session = {
        let guard = get_active_terminals()
            .lock()
            .map_err(|e| format!("Failed to lock terminal registry: {}", e))?;
        guard.get(&id).cloned()
    };

    if let Some(session) = session {
        session
            .write(&input)
            .map_err(|e| format!("Failed writing to terminal stdin: {}", e))?;
        Ok(())
    } else {
        Err(format!("Terminal session '{}' not found", id))
    }
}

/// Resizes the character grid dimensions (columns and rows) of an active PTY session.
#[tauri::command]
pub async fn resize_terminal(id: String, cols: u16, rows: u16) -> Result<(), String> {
    let session = {
        let guard = get_active_terminals()
            .lock()
            .map_err(|e| format!("Failed to lock terminal registry: {}", e))?;
        guard.get(&id).cloned()
    };

    if let Some(session) = session {
        session
            .resize(cols, rows)
            .map_err(|e| format!("Failed resizing terminal: {}", e))?;
        Ok(())
    } else {
        Err(format!("Terminal session '{}' not found", id))
    }
}

/// Closes a terminal session by removing it from the registry and dropping process handles.
#[tauri::command]
pub async fn close_terminal(id: String) -> Result<(), String> {
    let session = {
        let mut guard = get_active_terminals()
            .lock()
            .map_err(|e| format!("Failed to lock terminal registry: {}", e))?;
        guard.remove(&id)
    };

    if session.is_some() {
        tracing::info!("Closed terminal session '{}'", id);
        Ok(())
    } else {
        Err(format!(
            "Terminal session '{}' not found or already closed",
            id
        ))
    }
}

/// Returns the currently resolved default workspace or active project directory.
#[tauri::command]
pub async fn get_terminal_default_workdir(state: State<'_, AppState>) -> Result<String, String> {
    let active_proj = {
        if let Ok(lock) = state.state_lock.lock() {
            lock.active_project.clone()
        } else {
            None
        }
    };

    let resolved = resolve_terminal_workdir(None, active_proj.as_deref());
    resolved.ok_or_else(|| "Could not resolve default workspace directory".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_terminal_workdir_precedence() {
        let temp_dir = std::env::temp_dir();
        let temp_str = temp_dir.to_string_lossy().to_string();

        // 1. Explicit valid workdir takes top priority
        let res1 = resolve_terminal_workdir(Some(&temp_str), Some("C:\\invalid_fake_path_12345"));
        assert_eq!(res1, Some(temp_str.clone()));

        // 2. Active project is used when explicit is None
        let res2 = resolve_terminal_workdir(None, Some(&temp_str));
        assert_eq!(res2, Some(temp_str));

        // 3. Fallback when both are invalid/empty
        let res3 = resolve_terminal_workdir(Some(""), Some(""));
        // Should resolve default workspace if it exists, or None
        if let Ok(paths) = OperonPaths::resolve() {
            if paths.workspace_dir.exists() {
                assert_eq!(
                    res3,
                    Some(paths.workspace_dir.to_string_lossy().to_string())
                );
            }
        }
    }

    #[test]
    fn test_terminal_session_lifecycle() {
        let registry = get_active_terminals();
        let session_id = "test_unit_term_1";

        // Should start empty
        {
            let mut guard = registry.lock().unwrap();
            guard.remove(session_id);
        }

        // Spawn a real test session with echo callbacks
        let output_received = Arc::new(Mutex::new(false));
        let out_clone = output_received.clone();

        let session = TerminalSession::new(
            session_id.to_string(),
            None,
            80,
            24,
            move |_| {
                if let Ok(mut l) = out_clone.lock() {
                    *l = true;
                }
            },
            || {},
        );

        assert!(session.is_ok());
        let session = session.unwrap();

        // Test writing command
        let write_res = session.write("Write-Output 'Hello Operon'\r\n");
        assert!(write_res.is_ok());

        // Test resizing
        let resize_res = session.resize(100, 30);
        assert!(resize_res.is_ok());
    }
}
