// permission_commands.rs — Tauri IPC command handlers for permissions.

use serde::Serialize;
use operon_rs::{
    add_allowed_directory as backend_add_allowed_directory,
    remove_allowed_directory as backend_remove_allowed_directory,
    update_permission as backend_update_permission,
    get_permission_rows as backend_get_permission_rows,
    get_allowed_directories_list as backend_get_allowed_directories_list,
    PermissionRow,
};

// ─────────────────────────────────────────────────────────────────────────────
// Data Transfer Objects (DTOs)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowedDirectoriesResponse {
    pub workspace_directory: String,
    pub directories: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

fn clean_windows_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", stripped)
    } else if let Some(stripped) = path.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        path.to_string()
    }
}

/// List allowed directories and identify the default workspace directory.
#[tauri::command]
pub async fn get_allowed_directories() -> Result<AllowedDirectoriesResponse, String> {
    let (directories, workspace_directory) = backend_get_allowed_directories_list().map_err(|e| e.to_string())?;

    let cleaned_workspace = clean_windows_path(&workspace_directory);
    let cleaned_directories = directories.into_iter().map(|d| clean_windows_path(&d)).collect();

    Ok(AllowedDirectoriesResponse {
        workspace_directory: cleaned_workspace,
        directories: cleaned_directories,
    })
}

/// Add a new allowed directory.
#[tauri::command]
pub async fn add_allowed_directory(directory: String) -> Result<AllowedDirectoriesResponse, String> {
    let path_str = directory.trim();
    if path_str.is_empty() {
        return Err("Directory path cannot be empty".to_string());
    }

    backend_add_allowed_directory(path_str).map_err(|e| e.to_string())?;

    get_allowed_directories().await
}

/// Remove an allowed directory.
#[tauri::command]
pub async fn remove_allowed_directory(directory: String) -> Result<AllowedDirectoriesResponse, String> {
    let path_str = directory.trim();
    if path_str.is_empty() {
        return Err("Directory path cannot be empty".to_string());
    }

    backend_remove_allowed_directory(path_str).map_err(|e| e.to_string())?;

    get_allowed_directories().await
}

/// List all permission rows (groups and tools) for global or directory scope.
#[tauri::command]
pub async fn get_permission_rows(
    scope: String,
    directory: Option<String>,
) -> Result<Vec<PermissionRow>, String> {
    let dir_val = directory.as_deref();
    backend_get_permission_rows(&scope, dir_val).map_err(|e| e.to_string())
}

/// Update permission mode (allow/ask/deny) for a tool/group.
#[tauri::command]
pub async fn update_permission_mode(
    scope: String,
    directory: Option<String>,
    key: String,
    mode: Option<String>,
) -> Result<(), String> {
    let dir_val = directory.as_deref();
    let mode_val = mode.as_deref();
    backend_update_permission(&scope, dir_val, &key, mode_val).map_err(|e| e.to_string())
}
