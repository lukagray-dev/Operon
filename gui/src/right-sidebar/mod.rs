//! Right sidebar controller for Git Diff preview and staging actions.
//!
//! Hey friend! This module coordinates the Rust-side git integration for Operon's
//! right-sidebar diff panel. It delegates repository querying, staging, unstaging,
//! committing, graph timeline, remote sync, and multi-repo switching to the `operon_rs::diff`
//! async engine and updates the Slint UI models thread-safely.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Mutex;

use crate::state::AppState;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

/// Current repository list sort mode (0 = Discovery order, 1 = Name, 2 = Path).
static REPO_SORT_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Sets the active sorting mode for the repository list.
pub fn set_repo_sort_mode(mode: u8) {
    REPO_SORT_MODE.store(mode, std::sync::atomic::Ordering::Relaxed);
}

/// Helper to asynchronously read the commit message from the Slint UI thread.
pub async fn get_commit_message_async(win_w: &slint::Weak<crate::OperonWindow>) -> String {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let win_w2 = win_w.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let msg = win_w2
            .upgrade()
            .map(|w| w.get_git_commit_message().to_string())
            .unwrap_or_default();
        let _ = tx.send(msg);
    });
    rx.await.unwrap_or_default()
}

/// Cache of expanded file paths to preserve file hunk expansion state during background refreshes.
fn get_expanded_files_cache() -> &'static Mutex<HashSet<String>> {
    static EXPANDED_FILES: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
    EXPANDED_FILES.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Global repository registry tracking multi-repo workspaces across calls thread-safely.
fn get_repo_registry() -> &'static Mutex<operon_rs::diff::RepoRegistry> {
    static REGISTRY: std::sync::OnceLock<Mutex<operon_rs::diff::RepoRegistry>> =
        std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(operon_rs::diff::RepoRegistry::new()))
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

/// Helper to get active repository root from registry if available, else workspace default.
fn get_active_repo_root(state: &Rc<RefCell<AppState>>) -> (PathBuf, bool) {
    if let Ok(reg) = get_repo_registry().lock() {
        if let Some(active) = reg.active_repo() {
            let (_, is_project) = resolve_workspace(state);
            return (active.root.clone(), is_project);
        }
    }
    resolve_workspace(state)
}

/// Setup and wire all right-sidebar callbacks and background refresh tasks.
pub fn wire_right_sidebar(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    // 1. Initial workspace discovery & refresh on startup
    let (workspace, is_project) = resolve_workspace(&state);
    sync_workspace_repos(window, workspace.clone());
    refresh_git_workspace(window, workspace.clone(), is_project);
    refresh_git_graph(window, workspace);

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
        let (workspace, is_project) = get_active_repo_root(&state_stage);
        let path_str = rel_path.to_string();

        tokio::spawn(async move {
            let _ = operon_rs::diff::stage_file_async(workspace.clone(), path_str).await;

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
        let (workspace, is_project) = get_active_repo_root(&state_unstage);
        let path_str = rel_path.to_string();

        tokio::spawn(async move {
            let _ = operon_rs::diff::unstage_file_async(workspace.clone(), path_str).await;

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
        let (workspace, is_project) = get_active_repo_root(&state_revert);
        let path_str = rel_path.to_string();

        get_expanded_files_cache().lock().unwrap().remove(&path_str);

        tokio::spawn(async move {
            let _ = operon_rs::diff::revert_file_async(workspace.clone(), path_str).await;

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
        let (workspace, is_project) = get_active_repo_root(&state_stage_all);

        tokio::spawn(async move {
            let _ = operon_rs::diff::stage_all_files_async(workspace.clone()).await;

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
        let (workspace, is_project) = get_active_repo_root(&state_revert_all);

        get_expanded_files_cache().lock().unwrap().clear();

        tokio::spawn(async move {
            let _ = operon_rs::diff::revert_all_files_async(workspace.clone()).await;

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

    // 9. Repository selection callback
    let window_weak_repo_sel = window.as_weak();
    let state_repo_sel = Rc::clone(&state);
    window.on_git_repository_selected(move |name| {
        let win_w = window_weak_repo_sel.clone();
        let name_str = name.to_string();
        let (_, is_project) = resolve_workspace(&state_repo_sel);

        tokio::spawn(async move {
            let target_root = {
                if let Ok(mut reg) = get_repo_registry().lock() {
                    if let Some(entry) = reg.list_repos().into_iter().find(|r| r.name == name_str) {
                        let _ = reg.set_active(&entry.root);
                        Some(entry.root)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(root_path) = target_root {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_w.upgrade() {
                        sync_workspace_repos(&win, root_path.clone());
                        refresh_git_workspace(&win, root_path.clone(), is_project);
                        refresh_git_graph(&win, root_path);
                    }
                });
            }
        });
    });

    // 10. Repository refresh callback
    let window_weak_repo_ref = window.as_weak();
    let state_repo_ref = Rc::clone(&state);
    window.on_git_repo_refresh_requested(move |name| {
        let win_w = window_weak_repo_ref.clone();
        let name_str = name.to_string();
        let (_, is_project) = resolve_workspace(&state_repo_ref);

        tokio::spawn(async move {
            let target_root = {
                if let Ok(reg) = get_repo_registry().lock() {
                    reg.list_repos()
                        .into_iter()
                        .find(|r| r.name == name_str)
                        .map(|r| r.root)
                } else {
                    None
                }
            };

            if let Some(root_path) = target_root {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_w.upgrade() {
                        refresh_git_workspace(&win, root_path, is_project);
                    }
                });
            }
        });
    });

    // 11. Commit submitted callback
    let window_weak_commit = window.as_weak();
    let state_commit = Rc::clone(&state);
    window.on_git_commit_submitted(move |msg| {
        let msg_str = msg.to_string();
        if msg_str.trim().is_empty() {
            tracing::warn!("git: commit message is empty, ignoring commit request");
            return;
        }

        let win_w = window_weak_commit.clone();
        let (workspace, is_project) = get_active_repo_root(&state_commit);

        tokio::spawn(async move {
            match operon_rs::diff::commit_async(workspace.clone(), msg_str, false).await {
                Ok(res) => {
                    tracing::info!("git: committed successfully oid: {}", res.oid);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = win_w.upgrade() {
                            win.set_git_commit_message(slint::SharedString::from(""));
                            refresh_git_workspace(&win, workspace.clone(), is_project);
                            refresh_git_graph(&win, workspace);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("git commit failed: {e}");
                }
            }
        });
    });

    // 12. Generate commit message callback
    window.on_git_generate_commit_message_requested(|| {
        tracing::debug!("git: AI commit message generation not yet implemented");
    });

    // 13. Commit selected callback
    window.on_git_commit_selected(|hash| {
        tracing::debug!("git: commit details view for '{hash}' not yet implemented");
    });

    // 14. Graph refresh callback
    let window_weak_graph_ref = window.as_weak();
    let state_graph_ref = Rc::clone(&state);
    window.on_git_graph_refresh_requested(move || {
        if let Some(win) = window_weak_graph_ref.upgrade() {
            let (workspace, _) = get_active_repo_root(&state_graph_ref);
            refresh_git_graph(&win, workspace);
        }
    });

    // 15. Graph center HEAD callback
    window.on_git_graph_center_head_requested(|| {
        tracing::debug!(
            "git: graph_center_head requested (scroll position control not exposed in UI)"
        );
    });

    // 16. Graph pull callback
    let window_weak_pull = window.as_weak();
    let state_pull = Rc::clone(&state);
    window.on_git_graph_pull_requested(move || {
        if let Some(win) = window_weak_pull.upgrade() {
            execute_git_pull(&win, Rc::clone(&state_pull));
        }
    });

    // 17. Graph push callback
    let window_weak_push = window.as_weak();
    let state_push = Rc::clone(&state);
    window.on_git_graph_push_requested(move || {
        if let Some(win) = window_weak_push.upgrade() {
            execute_git_push(&win, Rc::clone(&state_push));
        }
    });

    // 18. Graph filter branch callback
    window.on_git_graph_filter_branch_requested(|| {
        tracing::debug!(
            "git: graph_filter_branch requested (branch filter UI state not yet implemented)"
        );
    });

    // 19. Open file requested callback
    let state_open = Rc::clone(&state);
    window.on_git_open_file_requested(move |path| {
        let path_str = path.to_string();
        let (workspace, _) = get_active_repo_root(&state_open);
        let full_path = if std::path::Path::new(&path_str).is_absolute() {
            PathBuf::from(&path_str)
        } else {
            workspace.join(&path_str)
        };

        if let Err(e) = std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", full_path.display()))
            .spawn()
        {
            tracing::error!("git: failed to open file in Explorer: {e}");
        }
    });

    // 20. Context menu item selected callback
    let window_weak_menu = window.as_weak();
    let state_menu = Rc::clone(&state);
    window.on_git_menu_item_selected(move |item_id| {
        let id = item_id.to_string();
        let win_w = window_weak_menu.clone();
        let (workspace, is_project) = get_active_repo_root(&state_menu);
        let (workspace_root, _) = resolve_workspace(&state_menu);
        let state_c = Rc::clone(&state_menu);

        match id.as_str() {
            // --- Repositories Heading & Sorting ---
            "sort_discovery" => {
                set_repo_sort_mode(0);
                if let Some(win) = win_w.upgrade() {
                    sync_workspace_repos(&win, workspace_root);
                }
            }
            "sort_name" => {
                set_repo_sort_mode(1);
                if let Some(win) = win_w.upgrade() {
                    sync_workspace_repos(&win, workspace_root);
                }
            }
            "sort_path" => {
                set_repo_sort_mode(2);
                if let Some(win) = win_w.upgrade() {
                    sync_workspace_repos(&win, workspace_root);
                }
            }
            "select_single" => {
                tracing::debug!("git: single-repo selection mode confirmed (already default)");
            }
            "select_multiple" => {
                tracing::debug!("git: multi-repo selection not yet implemented, needs UI checkbox support");
            }

            // --- Changes Submenu ---
            "cmd_stage_all" => {
                tokio::spawn(async move {
                    let _ = operon_rs::diff::stage_all_files_async(workspace.clone()).await;
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = win_w.upgrade() {
                            refresh_git_workspace(&win, workspace, is_project);
                        }
                    });
                });
            }
            "cmd_unstage_all" => {
                tokio::spawn(async move {
                    if let Ok(details) = operon_rs::diff::get_diff_details_async(workspace.clone()).await {
                        for f in details.staged_files {
                            let _ = operon_rs::diff::unstage_file_async(workspace.clone(), f.path).await;
                        }
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = win_w.upgrade() {
                            refresh_git_workspace(&win, workspace, is_project);
                        }
                    });
                });
            }
            "cmd_discard_all" => {
                tokio::spawn(async move {
                    let _ = operon_rs::diff::discard_all_including_untracked_async(workspace.clone()).await;
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(win) = win_w.upgrade() {
                            refresh_git_workspace(&win, workspace, is_project);
                        }
                    });
                });
            }

            // --- Pull / Push / Remote Sync ---
            "cmd_sync" => {
                if let Some(win) = win_w.upgrade() {
                    execute_git_sync(&win, state_c);
                }
            }
            "pull" | "cmd_pull" => {
                if let Some(win) = win_w.upgrade() {
                    execute_git_pull(&win, state_c);
                }
            }
            "push" | "cmd_push" => {
                if let Some(win) = win_w.upgrade() {
                    execute_git_push(&win, state_c);
                }
            }
            "fetch" | "cmd_fetch" => {
                tokio::spawn(async move {
                    match operon_rs::diff::fetch_async(workspace.clone(), "origin".to_string()).await {
                        Ok(()) => {
                            tracing::info!("git: fetch successful");
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_w.upgrade() {
                                    refresh_git_workspace(&win, workspace.clone(), is_project);
                                    refresh_git_graph(&win, workspace);
                                }
                            });
                        }
                        Err(operon_rs::diff::DiffError::RemoteAuth(msg)) => {
                            tracing::error!("git fetch auth error: {msg}");
                        }
                        Err(e) => {
                            tracing::error!("git fetch failed: {e}");
                        }
                    }
                });
            }
            "cmd_pull_rebase" => {
                tracing::warn!("git: pull with rebase has no backend implementation yet, needs operon-diff support for non-fast-forward pulls");
            }
            "cmd_pull_from" => {
                tracing::debug!("git: pull_from requested, needs remote/branch picker UI - not yet implemented");
            }
            "cmd_push_to" => {
                tracing::debug!("git: push_to requested, needs remote/branch picker UI - not yet implemented");
            }
            "cmd_fetch_prune" => {
                tracing::warn!("git: fetch with prune has no backend implementation yet, needs operon-diff support");
            }
            "cmd_fetch_all_remotes" => {
                tracing::warn!("git: fetch all remotes has no backend implementation yet, needs operon-diff support for listing remotes");
            }

            // --- Commit Submenu ---
            "cmd_commit_all" => {
                tokio::spawn(async move {
                    let msg_str = get_commit_message_async(&win_w).await;
                    if msg_str.trim().is_empty() {
                        tracing::warn!("git: commit message is empty, ignoring commit_all");
                        return;
                    }

                    let _ = operon_rs::diff::stage_all_files_async(workspace.clone()).await;
                    match operon_rs::diff::commit_async(workspace.clone(), msg_str, false).await {
                        Ok(res) => {
                            tracing::info!("git: committed all successfully oid: {}", res.oid);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_w.upgrade() {
                                    win.set_git_commit_message(slint::SharedString::from(""));
                                    refresh_git_workspace(&win, workspace.clone(), is_project);
                                    refresh_git_graph(&win, workspace);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("git commit_all failed: {e}");
                        }
                    }
                });
            }
            "cmd_commit_staged" | "cmd_commit" => {
                tokio::spawn(async move {
                    let msg_str = get_commit_message_async(&win_w).await;
                    if msg_str.trim().is_empty() {
                        tracing::warn!("git: commit message is empty, ignoring commit_staged");
                        return;
                    }

                    match operon_rs::diff::commit_async(workspace.clone(), msg_str, false).await {
                        Ok(res) => {
                            tracing::info!("git: committed staged successfully oid: {}", res.oid);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_w.upgrade() {
                                    win.set_git_commit_message(slint::SharedString::from(""));
                                    refresh_git_workspace(&win, workspace.clone(), is_project);
                                    refresh_git_graph(&win, workspace);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("git commit_staged failed: {e}");
                        }
                    }
                });
            }
            "cmd_commit_amend" | "cmd_commit_staged_amend" => {
                tokio::spawn(async move {
                    let msg_str = get_commit_message_async(&win_w).await;
                    match operon_rs::diff::commit_async(workspace.clone(), msg_str, true).await {
                        Ok(res) => {
                            tracing::info!("git: amended commit successfully oid: {}", res.oid);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_w.upgrade() {
                                    win.set_git_commit_message(slint::SharedString::from(""));
                                    refresh_git_workspace(&win, workspace.clone(), is_project);
                                    refresh_git_graph(&win, workspace);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("git commit_amend failed: {e}");
                        }
                    }
                });
            }
            "cmd_commit_all_amend" => {
                tokio::spawn(async move {
                    let msg_str = get_commit_message_async(&win_w).await;
                    let _ = operon_rs::diff::stage_all_files_async(workspace.clone()).await;
                    match operon_rs::diff::commit_async(workspace.clone(), msg_str, true).await {
                        Ok(res) => {
                            tracing::info!("git: committed all amend successfully oid: {}", res.oid);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(win) = win_w.upgrade() {
                                    win.set_git_commit_message(slint::SharedString::from(""));
                                    refresh_git_workspace(&win, workspace.clone(), is_project);
                                    refresh_git_graph(&win, workspace);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("git commit_all_amend failed: {e}");
                        }
                    }
                });
            }
            "cmd_undo_last" => {
                tracing::warn!("git: undo last commit has no backend implementation yet, needs operon-diff support");
            }
            "cmd_abort_rebase" => {
                tracing::warn!("git: abort rebase has no backend implementation yet, needs operon-diff support");
            }
            "cmd_commit_signed_off" | "cmd_commit_staged_signed_off" | "cmd_commit_all_signed_off" => {
                tracing::warn!("git: signed-off commit variant has no backend implementation yet, needs operon-diff support");
            }

            // --- Main Menu UI Actions & Out of Scope ---
            "clone" => {
                tracing::debug!("git: clone requested, needs URL input UI - not yet implemented");
            }
            "checkout_to" => {
                tracing::debug!("git: checkout_to requested, needs branch/ref picker UI - not yet implemented");
            }
            "show_git_output" => {
                tracing::debug!("git: show_git_output requested, needs output log panel UI - not yet implemented");
            }

            // --- Branch Submenu ---
            "cmd_create_branch" => {
                tracing::debug!("git: create branch requested: not yet implemented, needs branch-name input UI");
            }
            "cmd_create_branch_from" => {
                tracing::debug!("git: create branch from requested: not yet implemented, needs branch-name and source-ref input UI");
            }
            "cmd_delete_remote_branch" => {
                tracing::warn!("git: delete remote branch has no backend implementation yet, needs operon-diff support");
            }
            "cmd_publish_branch" => {
                tracing::warn!("git: publish branch has no backend implementation yet, needs operon-diff support for setting upstream tracking config");
            }
            "cmd_delete_branch" | "cmd_rename_branch" | "cmd_merge" | "cmd_rebase_branch" => {
                tracing::warn!("git: menu action '{id}' has no backend implementation yet");
            }

            // --- Remote Submenu ---
            "cmd_add_remote" | "cmd_remove_remote" => {
                tracing::warn!("git: menu action '{id}' has no backend implementation yet");
            }

            // --- Graph Heading Menu ---
            "graph_refresh" => {
                if let Some(win) = win_w.upgrade() {
                    refresh_git_graph(&win, workspace);
                }
            }
            "graph_center_head" => {
                tracing::debug!("git: graph_center_head requested (scroll position control not exposed in UI)");
            }
            "graph_filter_branch" => {
                tracing::debug!("git: graph_filter_branch requested (branch filter UI state not yet implemented)");
            }
            "graph_show_remote" => {
                tracing::debug!("git: graph_show_remote requested, needs UI state toggle in commit-graph - not yet implemented");
            }
            "graph_settings" => {
                tracing::debug!("git: graph_settings requested, needs graph settings dialog UI - not yet implemented");
            }

            // --- Submenu Parent Headers (Opened in UI) ---
            "commit_sub" | "changes_sub" | "pull_push_sub" | "branch_sub"
            | "remote_sub" | "stash_sub" | "tags_sub" | "worktrees_sub" => {
                tracing::debug!("git: submenu parent opened: {id}");
            }

            // --- Stash, Tags, Worktrees & Remaining Categories ---
            other => {
                if other.starts_with("stash_") || other.starts_with("cmd_stash")
                    || other.starts_with("tags_") || (other.starts_with("cmd_") && other.contains("tag"))
                    || other.starts_with("worktrees_") || (other.starts_with("cmd_") && other.contains("worktree")) {
                    tracing::warn!("git: menu action '{id}' has no backend implementation yet");
                } else {
                    tracing::debug!("git: unhandled menu item: {id}");
                }
            }
        }
    });

    // 21. Periodic background sync timer (refreshes stats every 2 seconds when sidebar is open or project session active)
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
    let (workspace, is_project_session) = get_active_repo_root(&state);
    sync_workspace_repos(window, workspace.clone());
    refresh_git_workspace(window, workspace.clone(), is_project_session);
    refresh_git_graph(window, workspace);
}

/// Discovers repositories in workspace, updates registry, and populates Slint UI repository list.
pub fn sync_workspace_repos(window: &crate::OperonWindow, workspace_root: PathBuf) {
    let window_weak = window.as_weak();
    tokio::spawn(async move {
        let repos_res = operon_rs::diff::discover_workspace_repos_async(workspace_root).await;
        let repos = match repos_res {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("git: failed to discover workspace repos: {e}");
                return;
            }
        };

        // Update registry thread-safely
        {
            if let Ok(mut reg) = get_repo_registry().lock() {
                for repo in &repos {
                    let _ = reg.add_repo(&repo.root);
                }
            }
        }

        let mut repo_entries = {
            if let Ok(reg) = get_repo_registry().lock() {
                reg.list_repos()
            } else {
                repos
            }
        };

        match REPO_SORT_MODE.load(std::sync::atomic::Ordering::Relaxed) {
            1 => repo_entries.sort_by_key(|a| a.name.to_lowercase()),
            2 => repo_entries.sort_by_key(|a| a.root.clone()),
            _ => {}
        }

        let mut slint_repos = Vec::new();
        for r in repo_entries {
            let branch_res = operon_rs::diff::current_branch_async(r.root.clone()).await;
            let branch_name = branch_res
                .map(|b| b.name)
                .unwrap_or_else(|_| "main".to_string());

            slint_repos.push(crate::GitRepositoryInfo {
                name: r.name.into(),
                branch: branch_name.into(),
                is_active: r.is_active,
                has_changes: r.has_changes,
            });
        }

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = window_weak.upgrade() {
                ui.set_git_repositories(ModelRc::from(Rc::new(VecModel::from(slint_repos))));
            }
        });
    });
}

/// Asynchronously fetches commit graph history for a workspace path and updates Slint UI properties.
pub fn refresh_git_graph(window: &crate::OperonWindow, workspace_root: PathBuf) {
    let window_weak = window.as_weak();
    tokio::spawn(async move {
        let graph_res = operon_rs::diff::get_commit_graph_async(workspace_root, 50, 0).await;
        if let Ok(commits) = graph_res {
            let slint_commits: Vec<crate::GitGraphCommit> = commits
                .into_iter()
                .map(|c| crate::GitGraphCommit {
                    hash: c.hash.into(),
                    short_hash: c.short_hash.into(),
                    message: c.message.into(),
                    author: c.author.into(),
                    branch_tag: c.branch_tag.into(),
                    is_head: c.is_head,
                    is_local: c.is_local,
                })
                .collect();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = window_weak.upgrade() {
                    ui.set_git_commits(ModelRc::from(Rc::new(VecModel::from(slint_commits))));
                }
            });
        }
    });
}

/// Executes git sync (pull then push in sequence) asynchronously and refreshes UI on completion.
pub fn execute_git_sync(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();
    let (workspace, is_project) = get_active_repo_root(&state);
    tokio::spawn(async move {
        let branch_res = operon_rs::diff::current_branch_async(workspace.clone()).await;
        let branch_name = match branch_res {
            Ok(b) => b.name,
            Err(e) => {
                tracing::error!("git: sync failed to get current branch: {e}");
                return;
            }
        };

        match operon_rs::diff::pull_async(
            workspace.clone(),
            "origin".to_string(),
            branch_name.clone(),
        )
        .await
        {
            Ok(()) => {
                tracing::info!("git: sync pull successful");
            }
            Err(operon_rs::diff::DiffError::MergeConflict(msg)) => {
                tracing::warn!("git sync stopped: pull merge conflict encountered: {msg}");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        refresh_git_workspace(&win, workspace.clone(), is_project);
                        refresh_git_graph(&win, workspace);
                    }
                });
                return;
            }
            Err(operon_rs::diff::DiffError::RemoteAuth(msg)) => {
                tracing::error!("git sync pull auth error: {msg}");
                return;
            }
            Err(e) => {
                tracing::error!("git sync pull failed: {e}");
                return;
            }
        }

        match operon_rs::diff::push_async(workspace.clone(), "origin".to_string(), branch_name)
            .await
        {
            Ok(()) => {
                tracing::info!("git: sync push successful");
            }
            Err(operon_rs::diff::DiffError::RemoteAuth(msg)) => {
                tracing::error!("git sync push auth error: {msg}");
            }
            Err(e) => {
                tracing::error!("git sync push failed: {e}");
            }
        }

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(win) = window_weak.upgrade() {
                refresh_git_workspace(&win, workspace.clone(), is_project);
                refresh_git_graph(&win, workspace);
            }
        });
    });
}

/// Executes git pull operation asynchronously and refreshes UI on completion.
pub fn execute_git_pull(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();
    let (workspace, is_project) = get_active_repo_root(&state);
    tokio::spawn(async move {
        let branch_res = operon_rs::diff::current_branch_async(workspace.clone()).await;
        let branch_name = match branch_res {
            Ok(b) => b.name,
            Err(e) => {
                tracing::error!("git: pull failed to get current branch: {e}");
                return;
            }
        };

        match operon_rs::diff::pull_async(workspace.clone(), "origin".to_string(), branch_name)
            .await
        {
            Ok(()) => {
                tracing::info!("git: pull successful");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        refresh_git_workspace(&win, workspace.clone(), is_project);
                        refresh_git_graph(&win, workspace);
                    }
                });
            }
            Err(operon_rs::diff::DiffError::MergeConflict(msg)) => {
                tracing::warn!("git pull warning: merge conflict encountered: {msg}");
            }
            Err(operon_rs::diff::DiffError::RemoteAuth(msg)) => {
                tracing::error!("git pull auth error: {msg}");
            }
            Err(e) => {
                tracing::error!("git pull failed: {e}");
            }
        }
    });
}

/// Executes git push operation asynchronously and refreshes UI on completion.
pub fn execute_git_push(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();
    let (workspace, is_project) = get_active_repo_root(&state);
    tokio::spawn(async move {
        let branch_res = operon_rs::diff::current_branch_async(workspace.clone()).await;
        let branch_name = match branch_res {
            Ok(b) => b.name,
            Err(e) => {
                tracing::error!("git: push failed to get current branch: {e}");
                return;
            }
        };

        match operon_rs::diff::push_async(workspace.clone(), "origin".to_string(), branch_name)
            .await
        {
            Ok(()) => {
                tracing::info!("git: push successful");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        refresh_git_workspace(&win, workspace.clone(), is_project);
                        refresh_git_graph(&win, workspace);
                    }
                });
            }
            Err(operon_rs::diff::DiffError::RemoteAuth(msg)) => {
                tracing::error!("git push auth error: {msg}");
            }
            Err(e) => {
                tracing::error!("git push failed: {e}");
            }
        }
    });
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
pub fn refresh_git_workspace(
    window: &crate::OperonWindow,
    workspace: PathBuf,
    is_project_session: bool,
) {
    let window_weak = window.as_weak();

    tokio::spawn(async move {
        let details_res = operon_rs::diff::get_diff_details_async(workspace.clone()).await;

        let details = match details_res {
            Ok(d) => d,
            Err(_) => {
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
                let convert_file = |f: operon_rs::diff::FileDiff| -> crate::GitFileDiff {
                    let path_obj = std::path::Path::new(&f.path);
                    let file_name = path_obj
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&f.path)
                        .to_string();

                    let dir_path = path_obj
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("")
                        .to_string();

                    let is_expanded = expanded_cache.contains(&f.path);

                    let slint_hunks: Vec<crate::GitDiffHunk> = f
                        .hunks
                        .into_iter()
                        .map(|h| {
                            let slint_lines: Vec<crate::GitDiffLine> = h
                                .lines
                                .into_iter()
                                .map(|l| crate::GitDiffLine {
                                    line_type: l.line_type.to_string().into(),
                                    content: highlight_diff_line(&l.content, &f.path, l.line_type)
                                        .into(),
                                    old_line_num: l
                                        .old_line_num
                                        .map(|n| n.to_string())
                                        .unwrap_or_default()
                                        .into(),
                                    new_line_num: l
                                        .new_line_num
                                        .map(|n| n.to_string())
                                        .unwrap_or_default()
                                        .into(),
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
                        dir_path: dir_path.into(),
                        status: f.status.into(),
                        insertions: f.insertions as i32,
                        deletions: f.deletions as i32,
                        hunks: ModelRc::from(Rc::new(VecModel::from(slint_hunks))),
                        is_expanded,
                    }
                };

                let unstaged: Vec<crate::GitFileDiff> = details
                    .unstaged_files
                    .into_iter()
                    .map(convert_file)
                    .collect();
                let staged: Vec<crate::GitFileDiff> =
                    details.staged_files.into_iter().map(convert_file).collect();

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
        state
            .borrow_mut()
            .set_current_project_dir(Some("/path/to/project".to_string()));

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
