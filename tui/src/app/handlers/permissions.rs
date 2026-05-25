// Permissions screen action handlers
// Handles: All Action::Perm* variants
// These actions manage the permissions configuration screen (tool permissions, directory management, modals)

use crate::events::action::Action;
use crate::state::AppState;

/// Handle permissions screen actions
/// Processes section switching, navigation, expand/collapse, modals, and permission editing
pub fn handle(action: Action, state: &mut AppState) {
    match action {
        Action::PermSwitchSection => {
            use crate::ui::screens::permissions::state::{PermissionsSection, FocusedPanel};
            
            match state.permissions.section {
                PermissionsSection::Global => {
                    // Switch from Global to Directory section
                    state.permissions.section = PermissionsSection::Directory;
                    state.permissions.focused_panel = FocusedPanel::DirList;
                    state.permissions.selected_row = 0; // Reset to first data row
                }
                PermissionsSection::Directory => {
                    if state.permissions.directories.is_empty() {
                        // No directories: switch back to Global
                        state.permissions.section = PermissionsSection::Global;
                        state.permissions.selected_row = 0; // Reset to first data row
                    } else {
                        // Switch focus between panels
                        match state.permissions.focused_panel {
                            FocusedPanel::DirList => {
                                state.permissions.focused_panel = FocusedPanel::ToolTable;
                                state.permissions.selected_row = 0; // Reset to first data row
                            }
                            FocusedPanel::ToolTable => {
                                // Switch back to Global section
                                state.permissions.section = PermissionsSection::Global;
                                state.permissions.selected_row = 0; // Reset to first data row
                            }
                        }
                    }
                }
            }
        }
        Action::PermSelectUp => {
            use crate::ui::screens::permissions::state::{PermissionsSection, FocusedPanel};
            
            // Check if modal is open
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.move_up();
            } else if state.permissions.add_dir.open {
                // No navigation in add directory modal
            } else {
                // Navigate in the active panel
                match state.permissions.section {
                    PermissionsSection::Global => {
                        // Navigate in global tool table
                        if state.permissions.selected_row > 0 {
                            state.permissions.selected_row -= 1;
                            
                            // Auto-scroll if needed
                            if state.permissions.selected_row < state.permissions.tool_table_scroll {
                                state.permissions.tool_table_scroll = state.permissions.selected_row;
                            }
                        }
                    }
                    PermissionsSection::Directory => {
                        match state.permissions.focused_panel {
                            FocusedPanel::DirList => {
                                // Navigate in directory list
                                if state.permissions.selected_dir > 0 {
                                    state.permissions.selected_dir -= 1;
                                    
                                    // Auto-scroll if needed
                                    if state.permissions.selected_dir < state.permissions.dir_list_scroll {
                                        state.permissions.dir_list_scroll = state.permissions.selected_dir;
                                    }
                                }
                            }
                            FocusedPanel::ToolTable => {
                                // Navigate in tool table
                                if state.permissions.selected_row > 0 {
                                    state.permissions.selected_row -= 1;
                                    
                                    // Auto-scroll if needed
                                    if state.permissions.selected_row < state.permissions.tool_table_scroll {
                                        state.permissions.tool_table_scroll = state.permissions.selected_row;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Action::PermSelectDown => {
            use crate::ui::screens::permissions::state::{PermissionsSection, FocusedPanel};
            use crate::ui::screens::permissions::tool_table::count_tool_table_rows;
            
            // Check if modal is open
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.move_down();
            } else if state.permissions.add_dir.open {
                // No navigation in add directory modal
            } else {
                // Navigate in the active panel
                match state.permissions.section {
                    PermissionsSection::Global => {
                        // Navigate in global tool table
                        let max_row = count_tool_table_rows(&state.permissions.global_tools).saturating_sub(1);
                        if state.permissions.selected_row < max_row {
                            state.permissions.selected_row += 1;
                            
                            // Auto-scroll if needed (simplified - assumes visible height of ~10)
                            let visible_height = 10;
                            if state.permissions.selected_row >= state.permissions.tool_table_scroll + visible_height {
                                state.permissions.tool_table_scroll = state.permissions.selected_row.saturating_sub(visible_height - 1);
                            }
                        }
                    }
                    PermissionsSection::Directory => {
                        match state.permissions.focused_panel {
                            FocusedPanel::DirList => {
                                // Navigate in directory list
                                let max_dir = state.permissions.directories.len().saturating_sub(1);
                                if state.permissions.selected_dir < max_dir {
                                    state.permissions.selected_dir += 1;
                                    
                                    // Auto-scroll if needed
                                    let visible_height = 10;
                                    if state.permissions.selected_dir >= state.permissions.dir_list_scroll + visible_height {
                                        state.permissions.dir_list_scroll = state.permissions.selected_dir.saturating_sub(visible_height - 1);
                                    }
                                }
                            }
                            FocusedPanel::ToolTable => {
                                // Navigate in tool table
                                if !state.permissions.directories.is_empty() {
                                    let tools = &state.permissions.directories[state.permissions.selected_dir].tools;
                                    let max_row = count_tool_table_rows(tools).saturating_sub(1);
                                    if state.permissions.selected_row < max_row {
                                        state.permissions.selected_row += 1;
                                        
                                        // Auto-scroll if needed
                                        let visible_height = 10;
                                        if state.permissions.selected_row >= state.permissions.tool_table_scroll + visible_height {
                                            state.permissions.tool_table_scroll = state.permissions.selected_row.saturating_sub(visible_height - 1);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Action::PermToggleExpand => {
            use crate::ui::screens::permissions::state::{PermissionsSection, FocusedPanel};
            use crate::ui::screens::permissions::tool_table::get_row_indices;
            
            // Only works in tool table, not in directory list or modals
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
                    let tools = state.permissions.active_tools_mut();
                    if let Some((group_idx, tool_idx)) = get_row_indices(tools, selected_row) {
                        if tool_idx.is_none() {
                            // This is a group row - toggle expansion
                            tools.groups[group_idx].expanded = !tools.groups[group_idx].expanded;
                        }
                    }
                }
            }
        }
        Action::PermOpenEditor => {
            use crate::ui::screens::permissions::state::{PermissionsSection, FocusedPanel, EditRole};
            use crate::ui::screens::permissions::tool_table::get_row_indices;
            
            // Only works in tool table, not in directory list or modals
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
                    if let Some((group_idx, tool_idx)) = get_row_indices(tools, state.permissions.selected_row) {
                        // Open editor for Owner column by default
                        let current_mode = if let Some(tidx) = tool_idx {
                            tools.groups[group_idx].tools[tidx].owner
                        } else {
                            tools.groups[group_idx].owner
                        };
                        
                        state.permissions.rule_editor.open(group_idx, tool_idx, EditRole::Owner, current_mode);
                    }
                }
            }
        }
        Action::PermAddDirectory => {
            use crate::ui::screens::permissions::state::PermissionsSection;
            
            // Only works in Directory section when no modal is open
            if !state.permissions.rule_editor.open 
                && !state.permissions.add_dir.open
                && matches!(state.permissions.section, PermissionsSection::Directory) {
                state.permissions.add_dir.open();
            }
        }
        Action::PermDeleteDirectory => {
            use crate::ui::screens::permissions::state::{PermissionsSection, FocusedPanel};
            
            // Only works in Directory section, DirList focused, when no modal is open
            if !state.permissions.rule_editor.open 
                && !state.permissions.add_dir.open
                && matches!(state.permissions.section, PermissionsSection::Directory)
                && matches!(state.permissions.focused_panel, FocusedPanel::DirList)
                && !state.permissions.directories.is_empty() {
                // Delete the selected directory
                state.permissions.directories.remove(state.permissions.selected_dir);
                
                // Adjust selection if needed
                if state.permissions.selected_dir >= state.permissions.directories.len() && state.permissions.selected_dir > 0 {
                    state.permissions.selected_dir -= 1;
                }
            }
        }
        Action::PermCloseModal => {
            // Close whichever modal is open
            if state.permissions.rule_editor.open {
                state.permissions.rule_editor.close();
            } else if state.permissions.add_dir.open {
                state.permissions.add_dir.close();
            }
        }
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
        Action::PermEditorConfirm => {
            use crate::ui::screens::permissions::state::EditRole;
            
            if state.permissions.rule_editor.open {
                let group_idx = state.permissions.rule_editor.group_idx;
                let tool_idx = state.permissions.rule_editor.tool_idx;
                let role = state.permissions.rule_editor.role;
                let new_mode = state.permissions.rule_editor.selected_mode;
                
                // Apply the change
                let tools = state.permissions.active_tools_mut();
                if let Some(tidx) = tool_idx {
                    // Editing a specific tool
                    match role {
                        EditRole::Owner => tools.groups[group_idx].tools[tidx].owner = new_mode,
                        EditRole::External => tools.groups[group_idx].tools[tidx].external = new_mode,
                    }
                    // Sync group from children
                    tools.groups[group_idx].sync_from_children();
                } else {
                    // Editing the group as a whole - set all children
                    match role {
                        EditRole::Owner => tools.groups[group_idx].set_all_owner(new_mode),
                        EditRole::External => tools.groups[group_idx].set_all_external(new_mode),
                    }
                }
                
                // Close the modal
                state.permissions.rule_editor.close();
            } else if state.permissions.add_dir.open {
                // Confirm add directory
                let path_str = state.permissions.add_dir.get_path();
                if !path_str.trim().is_empty() {
                    use std::path::PathBuf;
                    use crate::ui::screens::permissions::state::DirectoryEntry;
                    
                    // Expand ~ to home directory
                    let path = if let Some(stripped) = path_str.strip_prefix("~/") {
                        if let Some(home) = dirs::home_dir() {
                            home.join(stripped)
                        } else {
                            PathBuf::from(path_str)
                        }
                    } else {
                        PathBuf::from(path_str)
                    };
                    
                    // Add the directory
                    state.permissions.directories.push(DirectoryEntry::new(path));
                    
                    // Select the new directory
                    state.permissions.selected_dir = state.permissions.directories.len() - 1;
                }
                
                // Close the modal
                state.permissions.add_dir.close();
            }
        }
        Action::PermEditorSwitchRole => {
            if state.permissions.rule_editor.open {
                // Extract the necessary data to avoid borrow conflicts
                let group_idx = state.permissions.rule_editor.group_idx;
                let tool_idx = state.permissions.rule_editor.tool_idx;
                let current_role = state.permissions.rule_editor.role;
                
                // Toggle the role
                let new_role = match current_role {
                    crate::ui::screens::permissions::state::EditRole::Owner => {
                        crate::ui::screens::permissions::state::EditRole::External
                    }
                    crate::ui::screens::permissions::state::EditRole::External => {
                        crate::ui::screens::permissions::state::EditRole::Owner
                    }
                };
                
                // Get the current permission for the new role
                let tools = state.permissions.active_tools();
                let group = &tools.groups[group_idx];
                let new_mode = if let Some(tidx) = tool_idx {
                    // Editing a specific tool
                    match new_role {
                        crate::ui::screens::permissions::state::EditRole::Owner => group.tools[tidx].owner,
                        crate::ui::screens::permissions::state::EditRole::External => group.tools[tidx].external,
                    }
                } else {
                    // Editing a group
                    match new_role {
                        crate::ui::screens::permissions::state::EditRole::Owner => group.owner,
                        crate::ui::screens::permissions::state::EditRole::External => group.external,
                    }
                };
                
                // Update the rule editor state
                state.permissions.rule_editor.role = new_role;
                state.permissions.rule_editor.selected_mode = new_mode;
            }
        }
        Action::PermForwardKeyToInput(key_event) => {
            if state.permissions.add_dir.open {
                // Forward key to the path input TextArea in add directory modal
                let _ = state.permissions.add_dir.input.input(key_event);
            }
        }
        _ => {
            // Catch-all for safety (should never hit due to dispatch routing)
        }
    }
}
