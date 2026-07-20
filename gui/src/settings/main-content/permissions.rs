//! Controller for the Permissions Settings page.
//!
//! This module binds user actions on the Permissions Settings view (such as toggling tab
//! switches, adding/removing allowed directories, expanding tool groups, and editing Allow/Ask/Deny
//! settings) to the `operon-rs` backend configuration system.

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::state::AppState;

/// Local state for tracking permissions view options.
struct PermissionsState {
    /// Holds the keys of the groups (e.g. "fs", "git") that are currently expanded in the tree view.
    expanded_groups: HashSet<String>,
    /// Cached copy of the loaded backend permission rows to determine inheritance and base modes.
    rows: Vec<operon_rs::PermissionRow>,
}

/// Helper function to clean Windows path suffixes/prefixes from backend directories.
fn clean_windows_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", stripped)
    } else if let Some(stripped) = path.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        path.to_string()
    }
}

/// Synchronizes the directories lists from the operon-rs config.toml back into Slint layout fields.
fn refresh_directories(window: &crate::SettingsWindow) {
    match operon_rs::get_allowed_directories_list() {
        Ok((directories, workspace_directory)) => {
            let mut dirs_list = directories;

            // Clean paths on Windows to avoid raw UNC path formatting in the UI
            #[cfg(target_os = "windows")]
            {
                dirs_list = dirs_list
                    .into_iter()
                    .map(|d| clean_windows_path(&d))
                    .collect();
            }

            let cleaned_workspace = {
                #[cfg(target_os = "windows")]
                {
                    clean_windows_path(&workspace_directory)
                }
                #[cfg(not(target_os = "windows"))]
                {
                    workspace_directory
                }
            };

            // Ensure workspace directory is included at the start of the list
            if !cleaned_workspace.is_empty() && !dirs_list.contains(&cleaned_workspace) {
                dirs_list.insert(0, cleaned_workspace.clone());
            }

            let slint_dirs: Vec<SharedString> =
                dirs_list.into_iter().map(SharedString::from).collect();
            window.set_allowed_directories(ModelRc::from(Rc::new(VecModel::from(slint_dirs))));
            window.set_workspace_directory(cleaned_workspace.into());
        }
        Err(e) => {
            eprintln!(
                "[operon-gui][settings] Failed to load allowed directories: {}",
                e
            );
        }
    }
}

/// Re-queries permission settings for the active tab (global/directory) and scope (owner/external),
/// flattens group-nested tool configurations based on expansion state, and refreshes the Slint model.
fn refresh_permissions(window: &crate::SettingsWindow, perm_state: &Rc<RefCell<PermissionsState>>) {
    let active_tab = window.get_permissions_active_tab();
    let configure_dir = window.get_configure_directory();
    let is_dir_view = !configure_dir.is_empty();

    // Check scope (0 = owner, 1 = external)
    let scope_index = window.get_permissions_active_scope();
    let scope = if scope_index == 0 {
        "owner"
    } else {
        "external"
    };

    // Permissions list is only visible in the Global tab (active-tab == 1) OR inside a directory configure view
    if active_tab == 1 || is_dir_view {
        let directory_param = if is_dir_view {
            Some(configure_dir.as_str())
        } else {
            None
        };

        match operon_rs::get_permission_rows(scope, directory_param) {
            Ok(rows) => {
                // Cache rows locally in state to refer back during updates
                perm_state.borrow_mut().rows = rows.clone();

                let mut flat_items = Vec::new();
                let expanded_groups = &perm_state.borrow().expanded_groups;

                // Categorize rows into groups and tools
                let mut groups = Vec::new();
                let mut tools = Vec::new();
                for r in rows {
                    if r.kind == "group" {
                        groups.push(r);
                    } else {
                        tools.push(r);
                    }
                }

                // Build a flattened view list. Group rows are followed immediately by their matching tools
                // if and only if that group's key is in the expanded groups set.
                for g in groups {
                    let has_tools = tools.iter().any(|t| t.group_key == g.key);
                    let is_expanded = expanded_groups.contains(&g.key);

                    flat_items.push(crate::PermissionItem {
                        key: g.key.clone().into(),
                        label: g.label.clone().into(),
                        subtitle: format!("group key: {} · default: {}", g.key, g.base_mode).into(),
                        mode: g.mode.clone().into(),
                        base_mode: g.base_mode.clone().into(),
                        is_explicit: g.is_explicit,
                        kind: g.kind.clone().into(),
                        group_key: g.group_key.clone().into(),
                        is_expanded,
                        has_tools,
                    });

                    if is_expanded {
                        for t in &tools {
                            if t.group_key == g.key {
                                flat_items.push(crate::PermissionItem {
                                    key: t.key.clone().into(),
                                    label: t.label.clone().into(),
                                    subtitle: format!(
                                        "tool key: {} · default: {}",
                                        t.key, t.base_mode
                                    )
                                    .into(),
                                    mode: t.mode.clone().into(),
                                    base_mode: t.base_mode.clone().into(),
                                    is_explicit: t.is_explicit,
                                    kind: t.kind.clone().into(),
                                    group_key: t.group_key.clone().into(),
                                    is_expanded: false,
                                    has_tools: false,
                                });
                            }
                        }
                    }
                }

                window.set_permission_items(ModelRc::from(Rc::new(VecModel::from(flat_items))));
            }
            Err(e) => {
                eprintln!(
                    "[operon-gui][settings] Failed to load permission rows: {}",
                    e
                );
                window.set_permission_items(ModelRc::from(Rc::new(VecModel::default())));
            }
        }
    }
}

/// Registers the callback handlers on the Settings window for Permissions category settings.
pub fn wire_permissions_settings(window: &crate::SettingsWindow, _state: Rc<RefCell<AppState>>) {
    let weak_window = window.as_weak();
    let permissions_state = Rc::new(RefCell::new(PermissionsState {
        expanded_groups: HashSet::new(),
        rows: Vec::new(),
    }));

    // Populate directory records and loaded permission rows immediately on window open
    refresh_directories(window);
    refresh_permissions(window, &permissions_state);

    // Handler 1: Active tab changed (e.g. from allowed directories list to global permissions grid)
    window.on_permission_tab_changed({
        let weak_win = weak_window.clone();
        let perm_state = Rc::clone(&permissions_state);
        move |tab_idx| {
            if let Some(win) = weak_win.upgrade() {
                win.set_permissions_active_tab(tab_idx);
                refresh_permissions(&win, &perm_state);
            }
        }
    });

    // Handler 2: Scope changed (Owner vs External)
    window.on_permission_scope_changed({
        let weak_win = weak_window.clone();
        let perm_state = Rc::clone(&permissions_state);
        move |scope_idx| {
            if let Some(win) = weak_win.upgrade() {
                win.set_permissions_active_scope(scope_idx);
                refresh_permissions(&win, &perm_state);
            }
        }
    });

    // Handler 3: clicked to configure detailed tool permissions for a specific directory
    window.on_permission_configure_directory_clicked({
        let weak_win = weak_window.clone();
        let perm_state = Rc::clone(&permissions_state);
        move |dir| {
            if let Some(win) = weak_win.upgrade() {
                win.set_configure_directory(dir);
                // Clear expanded groups list so new directory views start collapsed/clean
                perm_state.borrow_mut().expanded_groups.clear();
                refresh_permissions(&win, &perm_state);
            }
        }
    });

    // Handler 4: Return from directory configuration view back to the main list
    window.on_permission_configure_directory_back({
        let weak_win = weak_window.clone();
        let perm_state = Rc::clone(&permissions_state);
        move || {
            if let Some(win) = weak_win.upgrade() {
                win.set_configure_directory("".into());
                refresh_permissions(&win, &perm_state);
            }
        }
    });

    // Handler 5: Add a new path to the list of allowed directories
    window.on_permission_add_directory_clicked({
        let weak_win = weak_window.clone();
        move |dir_path| {
            let clean_path = dir_path.trim();
            if !clean_path.is_empty() {
                match operon_rs::add_allowed_directory(clean_path) {
                    Ok(_) => {
                        println!("[operon-gui][settings] Added directory: {}", clean_path);
                        if let Some(win) = weak_win.upgrade() {
                            refresh_directories(&win);
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "[operon-gui][settings] Failed to add allowed directory: {}",
                            e
                        );
                    }
                }
            }
        }
    });

    // Handler 6: Remove a path from the list of allowed directories
    window.on_permission_remove_directory_clicked({
        let weak_win = weak_window.clone();
        move |dir_path| {
            let clean_path = dir_path.trim();
            if !clean_path.is_empty() {
                match operon_rs::remove_allowed_directory(clean_path) {
                    Ok(_) => {
                        println!("[operon-gui][settings] Removed directory: {}", clean_path);
                        if let Some(win) = weak_win.upgrade() {
                            refresh_directories(&win);
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "[operon-gui][settings] Failed to remove allowed directory: {}",
                            e
                        );
                    }
                }
            }
        }
    });

    // Handler 7: clicked expand/collapse chevron button for a specific group row
    window.on_permission_toggle_group_expanded({
        let weak_win = weak_window.clone();
        let perm_state = Rc::clone(&permissions_state);
        move |group_key| {
            {
                let mut state = perm_state.borrow_mut();
                if state.expanded_groups.contains(group_key.as_str()) {
                    state.expanded_groups.remove(group_key.as_str());
                } else {
                    state.expanded_groups.insert(group_key.to_string());
                }
            }
            if let Some(win) = weak_win.upgrade() {
                refresh_permissions(&win, &perm_state);
            }
        }
    });

    // Handler 8: Set a permission mode (Allow, Ask, Deny) for a group or tool row
    window.on_permission_set_mode({
        let weak_win = weak_window.clone();
        let perm_state = Rc::clone(&permissions_state);
        move |key, kind, mode| {
            if let Some(win) = weak_win.upgrade() {
                let active_scope = win.get_permissions_active_scope();
                let scope = if active_scope == 0 {
                    "owner"
                } else {
                    "external"
                };

                let configure_dir = win.get_configure_directory();
                let is_dir_view = !configure_dir.is_empty();
                let directory_param = if is_dir_view {
                    Some(configure_dir.as_str())
                } else {
                    None
                };

                // If user resets the mode back to the default inherited mode, we clear explicit override
                // by passing None to the backend update_permission call.
                let mut target_mode = Some(mode.as_str());
                if let Some(row) = perm_state
                    .borrow()
                    .rows
                    .iter()
                    .find(|r| r.key == key.as_str() && r.kind == kind.as_str())
                {
                    if row.base_mode == mode.as_str() {
                        target_mode = None;
                    }
                }

                match operon_rs::update_permission(
                    scope,
                    directory_param,
                    key.as_str(),
                    target_mode,
                ) {
                    Ok(_) => {
                        println!("[operon-gui][settings] Set mode successfully for: {}", key);
                        refresh_permissions(&win, &perm_state);
                    }
                    Err(e) => {
                        eprintln!(
                            "[operon-gui][settings] Failed to set permission mode: {}",
                            e
                        );
                    }
                }
            }
        }
    });
}
