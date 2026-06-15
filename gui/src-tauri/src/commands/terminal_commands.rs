// terminal_commands.rs — Tauri IPC command handlers for managing GUI terminal panels.
//
// Hey friend! This module registers standard commands to create, write to, resize, and close
// pseudo-terminals running shell processes in the background, bridging them to the front-end
// xterm.js instance using Tauri's cross-thread Event emitter.

use crate::commands::model_commands::SharedState;
use operon_rs::terminal::TerminalSession;
use std::sync::Arc;
use tauri::{Emitter, State, WebviewWindow};

/// Payload sent to the front-end when the terminal writes output.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TerminalOutputPayload {
    id: String,
    data: String,
}

/// Payload sent to the front-end when the PTY process has terminated.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TerminalClosedPayload {
    id: String,
}

/// Create a new terminal session with a unique ID and hook up listener threads to forward output.
#[tauri::command]
pub async fn create_terminal(
    id: String,
    cols: u16,
    rows: u16,
    workdir: Option<String>,
    state: State<'_, SharedState>,
    window: WebviewWindow,
) -> Result<(), String> {
    // 1. Lock AppState to inspect if a terminal session with the same ID exists
    let mut state_guard = state
        .lock()
        .map_err(|e| format!("Failed to lock AppState: {}", e))?;
    
    if state_guard.active_terminals.contains_key(&id) {
        return Err(format!("Terminal session '{}' already exists", id));
    }

    // 2. Clone the window handle to emit data events inside the background reader thread
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

    // 3. Clone handles to emit a closure event to the GUI, cleaning up standard state
    let window_exit = window.clone();
    let id_exit = id.clone();
    let state_clone = state.inner().clone();
    let on_exit = move || {
        // Notify the frontend that the shell process has exited
        let _ = window_exit.emit(
            "terminal-closed",
            TerminalClosedPayload {
                id: id_exit.clone(),
            },
        );
        
        // Remove the session handle from our global registry
        if let Ok(mut lock) = state_clone.lock() {
            lock.active_terminals.remove(&id_exit);
        }
    };

    // 4. Initialize PTY via the portable-pty implementation
    let session = TerminalSession::new(id.clone(), workdir, cols, rows, on_output, on_exit)
        .map_err(|e| format!("Failed to spawn shell PTY process: {}", e))?;

    // 5. Track the handle for subsequent write/resize/close commands
    state_guard.active_terminals.insert(id, Arc::new(session));

    tracing::info!("Created terminal session '{}'", state_guard.active_terminals.len());
    Ok(())
}

/// Write command input or raw keystroke bytes to a running terminal.
#[tauri::command]
pub async fn write_terminal(
    id: String,
    input: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let session = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock AppState: {}", e))?;
        state_guard.active_terminals.get(&id).cloned()
    };

    if let Some(session) = session {
        session
            .write(&input)
            .map_err(|e| format!("Failed writing input to terminal process: {}", e))?;
        Ok(())
    } else {
        Err(format!("Terminal session '{}' not found", id))
    }
}

/// Dynamic window size update handler for the terminal character grid columns and rows.
#[tauri::command]
pub async fn resize_terminal(
    id: String,
    cols: u16,
    rows: u16,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let session = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock AppState: {}", e))?;
        state_guard.active_terminals.get(&id).cloned()
    };

    if let Some(session) = session {
        session
            .resize(cols, rows)
            .map_err(|e| format!("Failed executing PTY resize command: {}", e))?;
        Ok(())
    } else {
        Err(format!("Terminal session '{}' not found", id))
    }
}

/// Close a terminal session by removing it from the registry and dropping its resources.
#[tauri::command]
pub async fn close_terminal(
    id: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let session = {
        let mut state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock AppState: {}", e))?;
        state_guard.active_terminals.remove(&id)
    };

    if let Some(_session) = session {
        // Dropping the TerminalSession handle terminates the background reader thread
        // and sends a kill signal to the child shell process.
        Ok(())
    } else {
        Err(format!("Terminal session '{}' not found or already closed", id))
    }
}
