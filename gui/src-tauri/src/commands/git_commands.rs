// git_commands.rs — Tauri IPC command handlers for Git Diff preview actions.
//
// Hey friend! This module registers all the standard commands that the web frontend
// can invoke through Tauri's IPC system to inspect git status, view hunks,
// and stage/unstage/revert changes.
//
// Every command resolves the workspace directory (whether running in a dedicated project
// directory or fallback default workspace directory) and delegates to `operon_rs::diff`.

use std::path::PathBuf;

/// Helper: Resolves the workspace path based on whether the frontend is operating
/// in PROJECT mode (providing `project_dir`) or NORMAL mode (using default workspace).
fn resolve_workspace_path(project_dir: Option<String>) -> Result<PathBuf, String> {
    if let Some(ref dir) = project_dir {
        if !dir.trim().is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    
    // Resolve platform-specific default path (~/.operon/workspace/)
    let paths = operon_rs::config::OperonPaths::resolve()
        .map_err(|e| format!("Failed to resolve Operon paths: {}", e))?;
    Ok(paths.workspace_dir)
}

/// Retrieve the additions/deletions quick counts for the top-right changes button.
#[tauri::command]
pub async fn get_git_diff_stats(
    project_dir: Option<String>,
) -> Result<operon_rs::diff::GitDiffStats, String> {
    let workspace = resolve_workspace_path(project_dir)?;
    operon_rs::diff::get_diff_stats(workspace)
        .map_err(|e| format!("Failed to query git stats: {}", e))
}

/// Query the detailed files list and hunk-level diffs to render the sidebar tree.
#[tauri::command]
pub async fn get_git_diff_details(
    project_dir: Option<String>,
) -> Result<operon_rs::diff::RepositoryDiff, String> {
    let workspace = resolve_workspace_path(project_dir)?;
    operon_rs::diff::get_diff_details(workspace)
        .map_err(|e| format!("Failed to query git details: {}", e))
}

/// Stage a modified/untracked file in the git index.
#[tauri::command]
pub async fn stage_git_file(
    project_dir: Option<String>,
    relative_path: String,
) -> Result<(), String> {
    let workspace = resolve_workspace_path(project_dir)?;
    operon_rs::diff::stage_file(workspace, &relative_path)
        .map_err(|e| format!("Failed to stage file: {}", e))
}

/// Unstage a file from the git index.
#[tauri::command]
pub async fn unstage_git_file(
    project_dir: Option<String>,
    relative_path: String,
) -> Result<(), String> {
    let workspace = resolve_workspace_path(project_dir)?;
    operon_rs::diff::unstage_file(workspace, &relative_path)
        .map_err(|e| format!("Failed to unstage file: {}", e))
}

/// Discard modifications to a file in the workdir.
#[tauri::command]
pub async fn revert_git_file(
    project_dir: Option<String>,
    relative_path: String,
) -> Result<(), String> {
    let workspace = resolve_workspace_path(project_dir)?;
    operon_rs::diff::revert_file(workspace, &relative_path)
        .map_err(|e| format!("Failed to revert file changes: {}", e))
}

/// Stage all modifications and untracked files in bulk.
#[tauri::command]
pub async fn stage_all_git_files(
    project_dir: Option<String>,
) -> Result<(), String> {
    let workspace = resolve_workspace_path(project_dir)?;
    operon_rs::diff::stage_all_files(workspace)
        .map_err(|e| format!("Failed to stage all files: {}", e))
}

/// Revert all modified files in bulk.
#[tauri::command]
pub async fn revert_all_git_files(
    project_dir: Option<String>,
) -> Result<(), String> {
    let workspace = resolve_workspace_path(project_dir)?;
    operon_rs::diff::revert_all_files(workspace)
        .map_err(|e| format!("Failed to revert all changes: {}", e))
}
