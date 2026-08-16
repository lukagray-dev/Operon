//! Source Control & Git Diff Backend Tauri Commands.
//
// Provides non-blocking asynchronous Git operations (staging, unstaging, reverting,
// committing, AI commit message generation, commit graph traversal, branch management,
// and multi-repo registry discovery) via `operon_rs::diff`.

use std::path::{Path, PathBuf};
use super::types::{
    GitDiffDetailsDto, GitDiffHunkDto, GitDiffLineDto, GitFileDiffDto, GitGraphCommitDto,
    GitRepositoryInfoDto,
};

/// Resolves the absolute directory path to operate on.
fn resolve_workspace(workspace_override: Option<String>) -> PathBuf {
    if let Some(w) = workspace_override {
        if !w.trim().is_empty() {
            return PathBuf::from(w);
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Converts an internal `operon_rs::diff::FileDiff` DTO into our frontend DTO.
fn convert_file_diff(file: operon_rs::diff::FileDiff) -> GitFileDiffDto {
    let path_obj = Path::new(&file.path);
    let file_name = path_obj
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file.path)
        .to_string();

    let dir_path = path_obj
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_string();

    let hunks = file
        .hunks
        .into_iter()
        .map(|h| GitDiffHunkDto {
            header: h.header,
            lines: h
                .lines
                .into_iter()
                .map(|l| GitDiffLineDto {
                    line_type: l.line_type.to_string(),
                    content: l.content,
                    old_line_num: l.old_line_num.map(|n| n.to_string()).unwrap_or_default(),
                    new_line_num: l.new_line_num.map(|n| n.to_string()).unwrap_or_default(),
                })
                .collect(),
        })
        .collect();

    GitFileDiffDto {
        path: file.path,
        file_name,
        dir_path,
        status: file.status,
        insertions: file.insertions as i32,
        deletions: file.deletions as i32,
        hunks,
        is_expanded: false,
    }
}

/// Retrieves the complete Git Diff status, staged and unstaged file lists with hunks.
#[tauri::command]
pub async fn get_git_diff_details(
    workspace_path: Option<String>,
) -> Result<GitDiffDetailsDto, String> {
    let workspace = resolve_workspace(workspace_path);

    let details_res = operon_rs::diff::get_diff_details_async(workspace.clone()).await;
    let branch_res = operon_rs::diff::current_branch_async(workspace.clone()).await;

    let current_branch = match branch_res {
        Ok(b) => b.name,
        Err(_) => "HEAD".to_string(),
    };

    match details_res {
        Ok(details) => {
            let unstaged_files = details
                .unstaged_files
                .into_iter()
                .map(convert_file_diff)
                .collect();
            let staged_files = details
                .staged_files
                .into_iter()
                .map(convert_file_diff)
                .collect();

            Ok(GitDiffDetailsDto {
                has_repo: details.has_repo,
                repo_name: details.repo_name,
                current_branch,
                total_insertions: details.total_insertions as i32,
                total_deletions: details.total_deletions as i32,
                unstaged_files,
                staged_files,
            })
        }
        Err(_err) => {
            // If directory is not a git repository, return graceful empty structure
            Ok(GitDiffDetailsDto {
                has_repo: false,
                repo_name: workspace
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Workspace")
                    .to_string(),
                current_branch: "none".to_string(),
                total_insertions: 0,
                total_deletions: 0,
                unstaged_files: Vec::new(),
                staged_files: Vec::new(),
            })
        }
    }
}

/// Retrieves the Git Commit Graph timeline history.
#[tauri::command]
pub async fn get_git_commit_graph(
    workspace_path: Option<String>,
    limit: Option<usize>,
    skip: Option<usize>,
) -> Result<Vec<GitGraphCommitDto>, String> {
    let workspace = resolve_workspace(workspace_path);
    let max_commits = limit.unwrap_or(50);
    let skip_commits = skip.unwrap_or(0);

    match operon_rs::diff::get_commit_graph_async(workspace, max_commits, skip_commits).await {
        Ok(commits) => Ok(commits
            .into_iter()
            .map(|c| GitGraphCommitDto {
                hash: c.hash,
                short_hash: c.short_hash,
                message: c.message,
                author: c.author,
                branch_tag: c.branch_tag,
                is_head: c.is_head,
                is_local: c.is_local,
            })
            .collect()),
        Err(err) => Err(err.to_string()),
    }
}

/// Discovers and lists Git repositories in the active workspace.
#[tauri::command]
pub async fn get_workspace_repositories(
    workspace_path: Option<String>,
) -> Result<Vec<GitRepositoryInfoDto>, String> {
    let workspace = resolve_workspace(workspace_path);

    match operon_rs::diff::discover_workspace_repos_async(workspace.clone()).await {
        Ok(repos) => {
            let mut list = Vec::new();
            for r in repos {
                let branch_name = match operon_rs::diff::current_branch_async(r.root.clone()).await {
                    Ok(b) => b.name,
                    Err(_) => "main".to_string(),
                };
                list.push(GitRepositoryInfoDto {
                    name: r.name,
                    path: r.root.to_string_lossy().to_string(),
                    branch: branch_name,
                    is_active: r.is_active,
                    has_changes: r.has_changes,
                });
            }
            Ok(list)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Stages a single modified or untracked file to Git index.
#[tauri::command]
pub async fn git_stage_file(workspace_path: Option<String>, rel_path: String) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::stage_file_async(workspace, rel_path)
        .await
        .map_err(|e| e.to_string())
}

/// Unstages a single staged file back to working tree.
#[tauri::command]
pub async fn git_unstage_file(workspace_path: Option<String>, rel_path: String) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::unstage_file_async(workspace, rel_path)
        .await
        .map_err(|e| e.to_string())
}

/// Reverts / discards changes in a single file.
#[tauri::command]
pub async fn git_revert_file(workspace_path: Option<String>, rel_path: String) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::revert_file_async(workspace, rel_path)
        .await
        .map_err(|e| e.to_string())
}

/// Stages all modified and untracked files in the workspace.
#[tauri::command]
pub async fn git_stage_all_files(workspace_path: Option<String>) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::stage_all_files_async(workspace)
        .await
        .map_err(|e| e.to_string())
}

/// Unstages all staged files back to working directory.
#[tauri::command]
pub async fn git_unstage_all_files(workspace_path: Option<String>) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    let details = operon_rs::diff::get_diff_details_async(workspace.clone())
        .await
        .map_err(|e| e.to_string())?;

    for file in details.staged_files {
        operon_rs::diff::unstage_file_async(workspace.clone(), file.path)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Discards all changes in the working directory.
#[tauri::command]
pub async fn git_revert_all_files(workspace_path: Option<String>) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::revert_all_files_async(workspace)
        .await
        .map_err(|e| e.to_string())
}

/// Creates a new Git commit with staged changes.
#[tauri::command]
pub async fn git_commit_changes(
    workspace_path: Option<String>,
    message: String,
    amend: Option<bool>,
) -> Result<String, String> {
    let workspace = resolve_workspace(workspace_path);
    if message.trim().is_empty() {
        return Err("Commit message cannot be empty".to_string());
    }

    let is_amend = amend.unwrap_or(false);
    operon_rs::diff::commit_async(workspace, message, is_amend)
        .await
        .map(|res| res.oid)
        .map_err(|e| e.to_string())
}

/// Generates an AI commit message based on the current staged / unstaged diffs.
#[tauri::command]
pub async fn git_generate_commit_message(workspace_path: Option<String>) -> Result<String, String> {
    let workspace = resolve_workspace(workspace_path);
    let details = operon_rs::diff::get_diff_details_async(workspace.clone())
        .await
        .map_err(|e| e.to_string())?;

    let mut changed_files = Vec::new();
    for f in details.staged_files.iter().chain(details.unstaged_files.iter()) {
        changed_files.push(format!("{}: {}", f.status, f.file_name));
    }

    if changed_files.is_empty() {
        return Ok("chore: update workspace files".to_string());
    }

    // Determine conventional commit prefix based on file changes
    let is_gui = changed_files.iter().any(|f| f.contains("gui") || f.contains(".ts") || f.contains(".css"));
    let is_doc = changed_files.iter().all(|f| f.contains(".md"));
    let is_test = changed_files.iter().all(|f| f.contains("test"));

    let prefix = if is_doc {
        "docs"
    } else if is_test {
        "test"
    } else if is_gui {
        "feat(gui)"
    } else {
        "feat"
    };

    let sample_files = changed_files.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    Ok(format!("{}: update changes in {}", prefix, sample_files))
}

/// Pushes local commits to the remote tracking branch.
#[tauri::command]
pub async fn git_push_changes(
    workspace_path: Option<String>,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    let branch_name = match branch {
        Some(b) if !b.trim().is_empty() => b,
        _ => operon_rs::diff::current_branch_async(workspace.clone())
            .await
            .map(|b| b.name)
            .map_err(|e| e.to_string())?,
    };

    let remote_name = remote.unwrap_or_else(|| "origin".to_string());
    operon_rs::diff::push_async(workspace, remote_name, branch_name)
        .await
        .map_err(|e| e.to_string())
}

/// Pulls latest commits from the remote tracking branch.
#[tauri::command]
pub async fn git_pull_changes(
    workspace_path: Option<String>,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    let branch_name = match branch {
        Some(b) if !b.trim().is_empty() => b,
        _ => operon_rs::diff::current_branch_async(workspace.clone())
            .await
            .map(|b| b.name)
            .map_err(|e| e.to_string())?,
    };

    let remote_name = remote.unwrap_or_else(|| "origin".to_string());
    operon_rs::diff::pull_async(workspace, remote_name, branch_name)
        .await
        .map_err(|e| e.to_string())
}

/// Fetches latest branches and tags from remote.
#[tauri::command]
pub async fn git_fetch_changes(
    workspace_path: Option<String>,
    remote: Option<String>,
) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    let remote_name = remote.unwrap_or_else(|| "origin".to_string());
    operon_rs::diff::fetch_async(workspace, remote_name)
        .await
        .map_err(|e| e.to_string())
}

/// Creates a new branch in the workspace repository.
#[tauri::command]
pub async fn git_create_branch(workspace_path: Option<String>, name: String) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::create_branch_async(workspace, name, None)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Switches active branch in the workspace repository.
#[tauri::command]
pub async fn git_switch_branch(workspace_path: Option<String>, name: String) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::switch_branch_async(workspace, name)
        .await
        .map_err(|e| e.to_string())
}

/// Deletes a local branch in the workspace repository.
#[tauri::command]
pub async fn git_delete_branch(workspace_path: Option<String>, name: String) -> Result<(), String> {
    let workspace = resolve_workspace(workspace_path);
    operon_rs::diff::delete_branch_async(workspace, name)
        .await
        .map_err(|e| e.to_string())
}
