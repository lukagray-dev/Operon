// memory_commands.rs — Tauri IPC command handlers for managing memories.
//
// These commands call into the `operon-tools-memory` library crate and expose
// CRUD capabilities to the frontend GUI settings panel.

use operon_rs::tools::memory::{
    self, MemoryItem,
};

/// Retrieve all memories currently stored in the global SQLite database.
/// Ordered by last update time descending (newest first).
#[tauri::command]
pub async fn get_memories() -> Result<Vec<MemoryItem>, String> {
    memory::get_all_memories().await.map_err(|e| e.to_string())
}

/// Add a new memory entry to the database.
/// Returns the fully populated MemoryItem struct with ID and timestamps.
#[tauri::command]
pub async fn add_memory(content: String) -> Result<MemoryItem, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Memory content cannot be empty".to_string());
    }
    memory::add_memory(trimmed).await.map_err(|e| e.to_string())
}

/// Update an existing memory entry's text content by its ID.
#[tauri::command]
pub async fn update_memory(id: i64, content: String) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Memory content cannot be empty".to_string());
    }
    memory::update_memory(id, trimmed).await.map_err(|e| e.to_string())
}

/// Delete a memory entry from the database by its ID.
#[tauri::command]
pub async fn delete_memory(id: i64) -> Result<(), String> {
    memory::delete_memory(id).await.map_err(|e| e.to_string())
}
