//! Right sidebar controller for Git Diff preview and staging actions.
//!
//! Hey friend! This module coordinates the Rust-side git integration for Operon's
//! right-sidebar diff panel. It delegates repository querying, staging, unstaging,
//! and reverting to `operon_rs::diff` and updates the Slint UI models thread-safely.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use crate::state::AppState;

/// Cache of expanded file paths to preserve file hunk expansion state during background refreshes.
fn get_expanded_files_cache() -> &'static Mutex<HashSet<String>> {
    static EXPANDED_FILES: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    EXPANDED_FILES.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Resolves the current workspace path based on whether the active session is project-specific.
fn resolve_workspace(state: &Rc<RefCell<AppState>>) -> (PathBuf, bool) {
    let borrowed = state.borrow();
    if let Some(dir) = borrowed.current_project_dir() {
        if !dir.trim().is_empty() {
            return (PathBuf::from(dir), true);
        }
    }

    let default_path = operon_rs::config::OperonPaths::resolve()
        .map(|p| p.workspace_dir)
        .unwrap_or_else(|_| PathBuf::from("."));
    (default_path, false)
}

/// Setup and wire all right-sidebar callbacks and background refresh tasks.
pub fn wire_right_sidebar(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    // 1. Refresh Git stats on initial wire
    refresh_git_details(window, Rc::clone(&state));

    // 2. Refresh callback requested by UI
    let window_weak_refresh = window.as_weak();
    let state_refresh = Rc::clone(&state);
    window.on_git_refresh_requested(move || {
        if let Some(win) = window_weak_refresh.upgrade() {
            refresh_git_details(&win, Rc::clone(&state_refresh));
        }
    });

    // 3. Stage single file callback
    let window_weak_stage = window.as_weak();
    let state_stage = Rc::clone(&state);
    window.on_git_stage_file_requested(move |rel_path| {
        let win_w = window_weak_stage.clone();
        let (workspace, is_project) = resolve_workspace(&state_stage);
        let path_str = rel_path.to_string();

        tokio::spawn(async move {
            let ws_clone = workspace.clone();
            let _ = tokio::task::spawn_blocking(move || {
                operon_rs::diff::stage_file(ws_clone, &path_str)
            }).await;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = win_w.upgrade() {
                    refresh_git_workspace(&win, workspace, is_project);
                }
            });
        });
    });

    // 4. Unstage single file callback
    let window_weak_unstage = window.as_weak();
    let state_unstage = Rc::clone(&state);
    window.on_git_unstage_file_requested(move |rel_path| {
        let win_w = window_weak_unstage.clone();
        let (workspace, is_project) = resolve_workspace(&state_unstage);
        let path_str = rel_path.to_string();

        tokio::spawn(async move {
            let ws_clone = workspace.clone();
            let _ = tokio::task::spawn_blocking(move || {
                operon_rs::diff::unstage_file(ws_clone, &path_str)
            }).await;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = win_w.upgrade() {
                    refresh_git_workspace(&win, workspace, is_project);
                }
            });
        });
    });

    // 5. Revert single file callback
    let window_weak_revert = window.as_weak();
    let state_revert = Rc::clone(&state);
    window.on_git_revert_file_requested(move |rel_path| {
        let win_w = window_weak_revert.clone();
        let (workspace, is_project) = resolve_workspace(&state_revert);
        let path_str = rel_path.to_string();

        get_expanded_files_cache().lock().unwrap().remove(&path_str);

        tokio::spawn(async move {
            let ws_clone = workspace.clone();
            let _ = tokio::task::spawn_blocking(move || {
                operon_rs::diff::revert_file(ws_clone, &path_str)
            }).await;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = win_w.upgrade() {
                    refresh_git_workspace(&win, workspace, is_project);
                }
            });
        });
    });

    // 6. Stage all files callback
    let window_weak_stage_all = window.as_weak();
    let state_stage_all = Rc::clone(&state);
    window.on_git_stage_all_requested(move || {
        let win_w = window_weak_stage_all.clone();
        let (workspace, is_project) = resolve_workspace(&state_stage_all);

        tokio::spawn(async move {
            let ws_clone = workspace.clone();
            let _ = tokio::task::spawn_blocking(move || {
                operon_rs::diff::stage_all_files(ws_clone)
            }).await;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = win_w.upgrade() {
                    refresh_git_workspace(&win, workspace, is_project);
                }
            });
        });
    });

    // 7. Revert all files callback
    let window_weak_revert_all = window.as_weak();
    let state_revert_all = Rc::clone(&state);
    window.on_git_revert_all_requested(move || {
        let win_w = window_weak_revert_all.clone();
        let (workspace, is_project) = resolve_workspace(&state_revert_all);

        get_expanded_files_cache().lock().unwrap().clear();

        tokio::spawn(async move {
            let ws_clone = workspace.clone();
            let _ = tokio::task::spawn_blocking(move || {
                operon_rs::diff::revert_all_files(ws_clone)
            }).await;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = win_w.upgrade() {
                    refresh_git_workspace(&win, workspace, is_project);
                }
            });
        });
    });

    // 8. Toggle file expanded hunk view callback
    let window_weak_expand = window.as_weak();
    window.on_git_file_expanded_toggled(move |is_staged, file_idx| {
        if let Some(win) = window_weak_expand.upgrade() {
            let files_model = if is_staged {
                win.get_git_staged_files()
            } else {
                win.get_git_unstaged_files()
            };

            if let Some(mut file_diff) = files_model.row_data(file_idx as usize) {
                file_diff.is_expanded = !file_diff.is_expanded;
                let path_str = file_diff.path.to_string();

                {
                    let mut cache = get_expanded_files_cache().lock().unwrap();
                    if file_diff.is_expanded {
                        cache.insert(path_str);
                    } else {
                        cache.remove(&path_str);
                    }
                }

                files_model.set_row_data(file_idx as usize, file_diff);
            }
        }
    });

    // 9. Periodic background sync timer (refreshes stats every 2 seconds when sidebar is open or project session active)
    let window_weak_loop = window.as_weak();
    let state_loop = Rc::clone(&state);
    let timer = Box::leak(Box::new(slint::Timer::default()));
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(2),
        move || {
            if let Some(win) = window_weak_loop.upgrade() {
                let open = win.get_right_sidebar_open();
                let proj = win.get_is_project_session();
                if open || proj {
                    refresh_git_details(&win, Rc::clone(&state_loop));
                }
            }
        },
    );
}

/// Asynchronously fetches git details for current state and updates Slint UI properties.
pub fn refresh_git_details(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let (workspace, is_project_session) = resolve_workspace(&state);
    refresh_git_workspace(window, workspace, is_project_session);
}

/// Formats a diff line with syntax highlighting or red color for deletions.
fn highlight_diff_line(content: &str, file_path: &str, line_type: char) -> String {
    if line_type == '-' {
        let escaped = crate::main_content::markdown::code_block::escape_html(content);
        format!("<font color=\"#f14c4c\">{}</font>", escaped)
    } else {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        crate::main_content::markdown::code_block::highlight_code_block(content, ext)
    }
}

/// Asynchronously fetches git details for a workspace path and updates Slint UI properties.
pub fn refresh_git_workspace(window: &crate::OperonWindow, workspace: PathBuf, is_project_session: bool) {
    let window_weak = window.as_weak();

    tokio::spawn(async move {
        let details_result = tokio::task::spawn_blocking(move || {
            operon_rs::diff::get_diff_details(workspace)
        }).await;

        let details = match details_result {
            Ok(Ok(d)) => d,
            _ => {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = window_weak.upgrade() {
                        ui.set_is_project_session(is_project_session);
                        ui.set_git_has_repo(false);
                        ui.set_git_total_insertions(0);
                        ui.set_git_total_deletions(0);
                    }
                });
                return;
            }
        };

        let expanded_cache = get_expanded_files_cache().lock().unwrap().clone();

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = window_weak.upgrade() {
                // Convert operon_rs::diff DTOs into Slint generated types on UI thread
                let convert_file = |f: operon_rs::diff::FileDiff| -> crate::GitFileDiff {
                    let file_name = std::path::Path::new(&f.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&f.path)
                        .to_string();

                    let is_expanded = expanded_cache.contains(&f.path);

                    let slint_hunks: Vec<crate::GitDiffHunk> = f.hunks
                        .into_iter()
                        .map(|h| {
                            let slint_lines: Vec<crate::GitDiffLine> = h.lines
                                .into_iter()
                                .map(|l| crate::GitDiffLine {
                                    line_type: l.line_type.to_string().into(),
                                    content: highlight_diff_line(&l.content, &f.path, l.line_type).into(),
                                    old_line_num: l.old_line_num.map(|n| n.to_string()).unwrap_or_default().into(),
                                    new_line_num: l.new_line_num.map(|n| n.to_string()).unwrap_or_default().into(),
                                })
                                .collect();

                            crate::GitDiffHunk {
                                header: h.header.into(),
                                lines: ModelRc::from(Rc::new(VecModel::from(slint_lines))),
                            }
                        })
                        .collect();

                    crate::GitFileDiff {
                        path: f.path.into(),
                        file_name: file_name.into(),
                        status: f.status.into(),
                        insertions: f.insertions as i32,
                        deletions: f.deletions as i32,
                        hunks: ModelRc::from(Rc::new(VecModel::from(slint_hunks))),
                        is_expanded,
                    }
                };

                let unstaged: Vec<crate::GitFileDiff> = details.unstaged_files.into_iter().map(convert_file).collect();
                let staged: Vec<crate::GitFileDiff> = details.staged_files.into_iter().map(convert_file).collect();

                ui.set_is_project_session(is_project_session);
                ui.set_git_has_repo(details.has_repo);
                ui.set_git_total_insertions(details.total_insertions as i32);
                ui.set_git_total_deletions(details.total_deletions as i32);
                ui.set_git_unstaged_files(ModelRc::from(Rc::new(VecModel::from(unstaged))));
                ui.set_git_staged_files(ModelRc::from(Rc::new(VecModel::from(staged))));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_workspace_project_session() {
        let state = Rc::new(RefCell::new(AppState::new()));
        state.borrow_mut().set_current_project_dir(Some("/path/to/project".to_string()));

        let (path, is_project) = resolve_workspace(&state);
        assert!(is_project);
        assert_eq!(path, PathBuf::from("/path/to/project"));
    }

    #[test]
    fn test_resolve_workspace_general_session() {
        let state = Rc::new(RefCell::new(AppState::new()));
        state.borrow_mut().set_current_project_dir(None);

        let (_, is_project) = resolve_workspace(&state);
        assert!(!is_project);
    }

    #[test]
    fn test_expanded_files_cache_operations() {
        let mut guard = get_expanded_files_cache().lock().unwrap();
        guard.insert("src/main.rs".to_string());
        assert!(guard.contains("src/main.rs"));
        guard.remove("src/main.rs");
        assert!(!guard.contains("src/main.rs"));
    }
}
