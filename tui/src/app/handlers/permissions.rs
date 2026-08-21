// permissions.rs — Permissions screen action handlers for Operon TUI.
//
// ZERO BUSINESS LOGIC IN FRONTEND:
// The TUI queries `operon-rs` backend (`operon_config::get_permission_rows`, `operon_config::update_permission`,
// `operon_config::add_allowed_directory`, `operon_config::remove_allowed_directory`) to manage permissions.
// All configuration edits are persisted directly to `~/.operon/config.toml`.

use crate::events::action::Action;
use crate::state::AppState;
use crate::ui::screens::permissions::state::{EditRole, FocusedPanel, PermissionsSection};
use crate::ui::screens::permissions::tool_table::{count_tool_table_rows, get_row_indices};

/// Processes all permissions-related actions triggered by keyboard events or background updates.
pub fn handle(action: Action, state: &mut AppState) {
    match action {
        // ─────────────────────────────────────────────────────────────────────
        // Section & Panel Navigation
        // ─────────────────────────────────────────────────────────────────────
        Action::PermSwitchSection => match state.permissions.section {
            PermissionsSection::Global => {
                state.permissions.section = PermissionsSection::Directory;
                state.permissions.focused_panel = FocusedPanel::DirList;
                state.permissions.selected_row = 0;
            }
            PermissionsSection::Directory => {
                if state.permissions.directories.is_empty() {
                    state.permissions.section = PermissionsSection::Global;
                    state.permissions.selected_row = 0;
                } else {
                    match state.permissions.focused_panel {
                        FocusedPanel::DirList => {
                            state.permissions.focused_panel = FocusedPanel::ToolTable;
                            state.permissions.selected_row = 0;
                        }
                        FocusedPanel::ToolTable => {
                            state.permissions.section = PermissionsSection::Global;
                            state.permissions.selected_row = 0;
                        }
                    }
                }
            }
        },

        // ─────────────────────────────────────────────────────────────────────
        // Up Navigation
        // ─────────────────────────────────────────────────────────────────────
        Action::PermSelectUp => {
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.move_up();
            } else if state.permissions.add_dir.open {
                // No navigation inside add directory text input
            } else {
                match state.permissions.section {
                    PermissionsSection::Global => {
                        if state.permissions.selected_row > 0 {
                            state.permissions.selected_row -= 1;
                            if state.permissions.selected_row < state.permissions.tool_table_scroll
                            {
                                state.permissions.tool_table_scroll =
                                    state.permissions.selected_row;
                            }
                        }
                    }
                    PermissionsSection::Directory => match state.permissions.focused_panel {
                        FocusedPanel::DirList => {
                            if state.permissions.selected_dir > 0 {
                                state.permissions.selected_dir -= 1;
                                state.permissions.selected_row = 0;
                                if state.permissions.selected_dir
                                    < state.permissions.dir_list_scroll
                                {
                                    state.permissions.dir_list_scroll =
                                        state.permissions.selected_dir;
                                }
                            }
                        }
                        FocusedPanel::ToolTable => {
                            if state.permissions.selected_row > 0 {
                                state.permissions.selected_row -= 1;
                                if state.permissions.selected_row
                                    < state.permissions.tool_table_scroll
                                {
                                    state.permissions.tool_table_scroll =
                                        state.permissions.selected_row;
                                }
                            }
                        }
                    },
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Down Navigation
        // ─────────────────────────────────────────────────────────────────────
        Action::PermSelectDown => {
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.move_down();
            } else if state.permissions.add_dir.open {
                // No navigation inside add directory text input
            } else {
                match state.permissions.section {
                    PermissionsSection::Global => {
                        let max_row = count_tool_table_rows(&state.permissions.global_tools)
                            .saturating_sub(1);
                        if state.permissions.selected_row < max_row {
                            state.permissions.selected_row += 1;
                            let visible_height = 10;
                            if state.permissions.selected_row
                                >= state.permissions.tool_table_scroll + visible_height
                            {
                                state.permissions.tool_table_scroll = state
                                    .permissions
                                    .selected_row
                                    .saturating_sub(visible_height - 1);
                            }
                        }
                    }
                    PermissionsSection::Directory => match state.permissions.focused_panel {
                        FocusedPanel::DirList => {
                            let max_dir = state.permissions.directories.len().saturating_sub(1);
                            if state.permissions.selected_dir < max_dir {
                                state.permissions.selected_dir += 1;
                                state.permissions.selected_row = 0;
                                let visible_height = 10;
                                if state.permissions.selected_dir
                                    >= state.permissions.dir_list_scroll + visible_height
                                {
                                    state.permissions.dir_list_scroll = state
                                        .permissions
                                        .selected_dir
                                        .saturating_sub(visible_height - 1);
                                }
                            }
                        }
                        FocusedPanel::ToolTable => {
                            if !state.permissions.directories.is_empty() {
                                let tools = &state.permissions.directories
                                    [state.permissions.selected_dir]
                                    .tools;
                                let max_row = count_tool_table_rows(tools).saturating_sub(1);
                                if state.permissions.selected_row < max_row {
                                    state.permissions.selected_row += 1;
                                    let visible_height = 10;
                                    if state.permissions.selected_row
                                        >= state.permissions.tool_table_scroll + visible_height
                                    {
                                        state.permissions.tool_table_scroll = state
                                            .permissions
                                            .selected_row
                                            .saturating_sub(visible_height - 1);
                                    }
                                }
                            }
                        }
                    },
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Group Expansion (Enter)
        // ─────────────────────────────────────────────────────────────────────
        Action::PermToggleExpand => {
            if !state.permissions.rule_editor.open && !state.permissions.add_dir.open {
                let should_toggle = match state.permissions.section {
                    PermissionsSection::Global => true,
                    PermissionsSection::Directory => {
                        matches!(state.permissions.focused_panel, FocusedPanel::ToolTable)
                            && !state.permissions.directories.is_empty()
                    }
                };

                if should_toggle {
                    let selected_row = state.permissions.selected_row;
                    let target_group_key = {
                        let tools = state.permissions.active_tools();
                        if let Some((group_idx, tool_idx)) = get_row_indices(tools, selected_row) {
                            if tool_idx.is_none() {
                                Some(tools.groups[group_idx].key.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(key) = target_group_key {
                        state.permissions.toggle_group_expansion(&key);
                    }
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Open Rule Editor Modal (Space)
        // ─────────────────────────────────────────────────────────────────────
        Action::PermOpenEditor => {
            if !state.permissions.rule_editor.open && !state.permissions.add_dir.open {
                let should_open = match state.permissions.section {
                    PermissionsSection::Global => true,
                    PermissionsSection::Directory => {
                        matches!(state.permissions.focused_panel, FocusedPanel::ToolTable)
                            && !state.permissions.directories.is_empty()
                    }
                };

                if should_open {
                    let tools = state.permissions.active_tools();
                    if let Some((group_idx, tool_idx)) =
                        get_row_indices(tools, state.permissions.selected_row)
                    {
                        let current_mode = if let Some(tidx) = tool_idx {
                            tools.groups[group_idx].tools[tidx].owner_mode
                        } else {
                            tools.groups[group_idx].owner_mode
                        };

                        state.permissions.rule_editor.open(
                            group_idx,
                            tool_idx,
                            EditRole::Owner,
                            current_mode,
                        );
                    }
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Add Directory Trigger (+)
        // ─────────────────────────────────────────────────────────────────────
        Action::PermAddDirectory => {
            if !state.permissions.rule_editor.open
                && !state.permissions.add_dir.open
                && matches!(state.permissions.section, PermissionsSection::Directory)
            {
                state.permissions.add_dir.open();
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Delete Directory Trigger (-)
        // ─────────────────────────────────────────────────────────────────────
        Action::PermDeleteDirectory => {
            if !state.permissions.rule_editor.open
                && !state.permissions.add_dir.open
                && matches!(state.permissions.section, PermissionsSection::Directory)
                && matches!(state.permissions.focused_panel, FocusedPanel::DirList)
                && !state.permissions.directories.is_empty()
            {
                let selected_dir_entry =
                    &state.permissions.directories[state.permissions.selected_dir];
                // Do not delete workspace directory
                if !selected_dir_entry.is_workspace {
                    let path_to_remove = selected_dir_entry.path.clone();
                    let _ = operon_rs::remove_allowed_directory(&path_to_remove);
                    state.permissions.refresh_from_backend();
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Close Modal (Esc)
        // ─────────────────────────────────────────────────────────────────────
        Action::PermCloseModal => {
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.close();
            } else if state.permissions.add_dir.open {
                state.permissions.add_dir.close();
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Modal Navigation: Up / Down / Switch Role
        // ─────────────────────────────────────────────────────────────────────
        Action::PermEditorUp => {
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.move_up();
            }
        }
        Action::PermEditorDown => {
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.move_down();
            }
        }
        Action::PermEditorSwitchRole => {
            if state.permissions.rule_editor.open {
                let current_role = state.permissions.rule_editor.role;
                let new_role = match current_role {
                    EditRole::Owner => EditRole::External,
                    EditRole::External => EditRole::Owner,
                };

                let group_idx = state.permissions.rule_editor.group_idx;
                let tool_idx = state.permissions.rule_editor.tool_idx;

                let tools = state.permissions.active_tools();
                if group_idx < tools.groups.len() {
                    let group = &tools.groups[group_idx];
                    let new_mode = if let Some(tidx) = tool_idx {
                        if tidx < group.tools.len() {
                            match new_role {
                                EditRole::Owner => group.tools[tidx].owner_mode,
                                EditRole::External => group.tools[tidx].external_mode,
                            }
                        } else {
                            group.owner_mode
                        }
                    } else {
                        match new_role {
                            EditRole::Owner => group.owner_mode,
                            EditRole::External => group.external_mode,
                        }
                    };

                    state.permissions.rule_editor.role = new_role;
                    state.permissions.rule_editor.selected_mode = new_mode;
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Modal Confirmation (Enter)
        // ─────────────────────────────────────────────────────────────────────
        Action::PermEditorConfirm => {
            if state.permissions.rule_editor.open {
                let group_idx = state.permissions.rule_editor.group_idx;
                let tool_idx = state.permissions.rule_editor.tool_idx;
                let role = state.permissions.rule_editor.role;
                let new_mode = state.permissions.rule_editor.selected_mode;

                let (target_key, base_mode) = {
                    let tools = state.permissions.active_tools();
                    if group_idx < tools.groups.len() {
                        let group = &tools.groups[group_idx];
                        if let Some(tidx) = tool_idx {
                            if tidx < group.tools.len() {
                                let tool = &group.tools[tidx];
                                let base = match role {
                                    EditRole::Owner => tool.owner_base,
                                    EditRole::External => tool.external_base,
                                };
                                (tool.key.clone(), base)
                            } else {
                                (group.key.clone(), group.owner_base)
                            }
                        } else {
                            let base = match role {
                                EditRole::Owner => group.owner_base,
                                EditRole::External => group.external_base,
                            };
                            (group.key.clone(), base)
                        }
                    } else {
                        (
                            "".to_string(),
                            crate::ui::screens::permissions::state::PermissionMode::Deny,
                        )
                    }
                };

                let dir_param = match state.permissions.section {
                    PermissionsSection::Global => None,
                    PermissionsSection::Directory => {
                        if state.permissions.selected_dir < state.permissions.directories.len() {
                            Some(
                                state.permissions.directories[state.permissions.selected_dir]
                                    .path
                                    .clone(),
                            )
                        } else {
                            None
                        }
                    }
                };

                if !target_key.is_empty() {
                    // If target mode matches the default base mode, pass None to clear override
                    let target_mode_str = if new_mode == base_mode {
                        None
                    } else {
                        Some(new_mode.as_str())
                    };

                    let _ = operon_rs::update_permission(
                        role.as_scope_str(),
                        dir_param.as_deref(),
                        &target_key,
                        target_mode_str,
                    );

                    state.permissions.refresh_from_backend();
                }

                state.permissions.rule_editor.close();
            } else if state.permissions.add_dir.open {
                let path_str = state.permissions.add_dir.get_path();
                if !path_str.is_empty() {
                    let expanded_path = if let Some(stripped) = path_str.strip_prefix("~/") {
                        if let Some(home) = dirs::home_dir() {
                            home.join(stripped).to_string_lossy().to_string()
                        } else {
                            path_str
                        }
                    } else {
                        path_str
                    };

                    let _ = operon_rs::add_allowed_directory(&expanded_path);
                    state.permissions.refresh_from_backend();

                    // Select the newly added directory
                    if !state.permissions.directories.is_empty() {
                        state.permissions.selected_dir = state.permissions.directories.len() - 1;
                    }
                }

                state.permissions.add_dir.close();
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Forward Keystroke to Add Directory Input
        // ─────────────────────────────────────────────────────────────────────
        Action::PermForwardKeyToInput(key_event) if state.permissions.add_dir.open => {
            let _ = state.permissions.add_dir.input.input(key_event);
        }

        _ => {}
    }
}
