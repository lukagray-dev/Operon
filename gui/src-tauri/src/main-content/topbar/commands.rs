//! Topbar backend Tauri commands.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::{GitDiffStatsDto, TopbarDataDto};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Resolves real Git diff insertions and deletions in a given workspace directory.
#[tauri::command]
pub async fn get_git_diff_stats(workspace_path: Option<String>) -> Result<GitDiffStatsDto, String> {
    let workspace = match workspace_path {
        Some(w) if !w.trim().is_empty() => PathBuf::from(w),
        _ => operon_rs::config::OperonPaths::resolve()
            .map(|p| p.workspace_dir)
            .unwrap_or_else(|_| PathBuf::from(".")),
    };

    if !workspace.exists() {
        return Ok(GitDiffStatsDto {
            insertions: 0,
            deletions: 0,
            files_changed: 0,
            is_git_repo: false,
        });
    }

    // Check if git directory exists in or above the workspace
    let mut git_cmd = Command::new("git");
    git_cmd.current_dir(&workspace);
    git_cmd.args(["rev-parse", "--is-inside-work-tree"]);

    #[cfg(windows)]
    git_cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let is_inside_repo = git_cmd
        .output()
        .map(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
        .unwrap_or(false);

    if !is_inside_repo {
        return Ok(GitDiffStatsDto {
            insertions: 0,
            deletions: 0,
            files_changed: 0,
            is_git_repo: false,
        });
    }

    // 1. Run git diff --numstat for tracked unstaged and staged changes
    let mut numstat_cmd = Command::new("git");
    numstat_cmd.current_dir(&workspace);
    numstat_cmd.args(["diff", "HEAD", "--numstat"]);

    #[cfg(windows)]
    numstat_cmd.creation_flags(0x08000000);

    let mut total_insertions = 0;
    let mut total_deletions = 0;
    let mut files_changed = 0;

    if let Ok(output) = numstat_cmd.output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let (Ok(ins), Ok(del)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                        total_insertions += ins;
                        total_deletions += del;
                        files_changed += 1;
                    }
                }
            }
        }
    }

    // 2. Also check untracked files with git status --porcelain
    let mut status_cmd = Command::new("git");
    status_cmd.current_dir(&workspace);
    status_cmd.args(["status", "--porcelain"]);

    #[cfg(windows)]
    status_cmd.creation_flags(0x08000000);

    if let Ok(output) = status_cmd.output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.starts_with("??") {
                    // Untracked file
                    files_changed += 1;
                }
            }
        }
    }

    Ok(GitDiffStatsDto {
        insertions: total_insertions,
        deletions: total_deletions,
        files_changed,
        is_git_repo: true,
    })
}

/// Retrieves the topbar metadata for the current session and workspace.
#[tauri::command]
pub async fn get_topbar_info(
    session_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<TopbarDataDto, String> {
    let mut title = "New Session".to_string();

    let mut unfinished_todo_count = 0;
    let mut total_todo_count = 0;

    if let Some(ref sid) = session_id {
        if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
            let session_file = paths.sessions_dir.join(format!("{}.json", sid));
            if session_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&session_file) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(t) = val.get("title").and_then(|v| v.as_str()) {
                            if !t.trim().is_empty() {
                                title = t.to_string();
                            }
                        }

                        // Inspect todos array to calculate pending/unfinished tasks
                        if let Some(todos_arr) = val.get("todos").and_then(|v| v.as_array()) {
                            total_todo_count = todos_arr.len();
                            for item in todos_arr {
                                let status = item
                                    .get("status")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("pending");
                                if status != "completed" {
                                    unfinished_todo_count += 1;
                                }
                            }
                        }
                    }
                }
            }

            if title == "New Session" {
                let db_path = paths.session_db(sid);
                if db_path.exists() {
                    if let Ok(store) = operon_rs::session::store::SessionStore::open(&db_path).await {
                        if let Ok(Some(first_msg)) = store.get_first_user_message_text(sid).await {
                            let trimmed = first_msg.trim();
                            if !trimmed.is_empty() {
                                title = trimmed
                                    .lines()
                                    .next()
                                    .unwrap_or(trimmed)
                                    .chars()
                                    .take(40)
                                    .collect();
                            }
                        }
                    }
                }
            }
        }
    }

    let is_project = workspace_path.as_ref().map_or(false, |w| !w.trim().is_empty());
    let project_name = workspace_path.as_ref().and_then(|w| {
        Path::new(w)
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
    });

    let git_stats = if is_project {
        get_git_diff_stats(workspace_path).await.ok()
    } else {
        None
    };

    Ok(TopbarDataDto {
        title,
        is_project,
        project_name,
        git_stats,
        unfinished_todo_count,
        total_todo_count,
    })
}
