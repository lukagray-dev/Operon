// Permissions screen state
// Manages all UI state for the permissions configuration screen
// This is pure UI state — no business logic, no persistence
// Real permission data will be loaded/saved via AgentBridge in the future

use std::path::PathBuf;
use tui_textarea::TextArea;

// ============================================================================
// ENUMS
// ============================================================================

/// Which top-level section is currently active in the permissions screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionsSection {
    /// Global tools section (full-width table)
    Global,
    /// Directory-scoped tools section (split layout: dir list + tool table)
    Directory,
}

/// Permission mode for a single tool/group × role cell
/// Determines what happens when the tool is invoked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// Tool can be executed without prompting
    Allow,
    /// User will be prompted before tool execution
    Ask,
    /// Tool execution is blocked
    Deny,
}

impl PermissionMode {
    /// Get the display label for this permission mode
    pub fn label(&self) -> &'static str {
        match self {
            PermissionMode::Allow => "Allow",
            PermissionMode::Ask => "Ask",
            PermissionMode::Deny => "Deny",
        }
    }

    /// Get the ratatui style for this permission mode
    /// Uses theme constants for consistent coloring
    pub fn style(&self) -> ratatui::style::Style {
        use crate::ui::theme::{COLOR_ERROR, COLOR_SUCCESS, COLOR_WARNING};
        use ratatui::style::Style;

        match self {
            PermissionMode::Allow => Style::default().fg(COLOR_SUCCESS),
            PermissionMode::Ask => Style::default().fg(COLOR_WARNING),
            PermissionMode::Deny => Style::default().fg(COLOR_ERROR),
        }
    }

    /// Cycle to the next permission mode (for quick toggle with Space)
    /// Allow → Ask → Deny → Allow
    pub fn cycle(&self) -> Self {
        match self {
            PermissionMode::Allow => PermissionMode::Ask,
            PermissionMode::Ask => PermissionMode::Deny,
            PermissionMode::Deny => PermissionMode::Allow,
        }
    }
}

/// Which column is being edited in the rule editor modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // External variant will be used when implementing column selection
pub enum EditRole {
    /// Editing the Owner column
    Owner,
    /// Editing the External column
    External,
}

impl EditRole {
    /// Get the display label for this role
    #[allow(dead_code)] // Reserved for future use
    pub fn label(&self) -> &'static str {
        match self {
            EditRole::Owner => "Owner",
            EditRole::External => "External",
        }
    }
}

/// Which panel has focus in the Directory section
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    /// Directory list (left panel)
    DirList,
    /// Tool table (right panel)
    ToolTable,
}

// ============================================================================
// TOOL DATA STRUCTURES
// ============================================================================

/// A single tool (leaf node in the tool tree)
/// Represents an individual tool like "read_file" or "web_search"
#[derive(Debug, Clone)]
#[allow(dead_code)] // id field reserved for future use (e.g., persistence, lookup)
pub struct ToolEntry {
    /// Unique identifier for this tool (e.g., "read_file")
    pub id: &'static str,
    /// Display label for this tool (e.g., "read_file")
    pub label: &'static str,
    /// Permission mode for Owner role
    pub owner: PermissionMode,
    /// Permission mode for External role
    pub external: PermissionMode,
}

impl ToolEntry {
    /// Create a new tool entry with the given permissions
    pub fn new(
        id: &'static str,
        label: &'static str,
        owner: PermissionMode,
        external: PermissionMode,
    ) -> Self {
        Self {
            id,
            label,
            owner,
            external,
        }
    }
}

/// A tool group (parent node in the tool tree)
/// Represents a category of tools like "File System" or "Web"
/// Can be expanded to show individual ToolEntry children
#[derive(Debug, Clone)]
#[allow(dead_code)] // id field reserved for future use (e.g., persistence, lookup)
pub struct ToolGroup {
    /// Unique identifier for this group (e.g., "file_system")
    pub id: &'static str,
    /// Display label for this group (e.g., "File System")
    pub label: &'static str,
    /// Child tools in this group
    pub tools: Vec<ToolEntry>,
    /// Whether this group is currently expanded to show children
    pub expanded: bool,
    /// Derived permission mode for Owner role
    /// If all children have the same mode, shows that mode
    /// If children differ, this is used to display "Custom" in the UI
    pub owner: PermissionMode,
    /// Derived permission mode for External role
    /// If all children have the same mode, shows that mode
    /// If children differ, this is used to display "Custom" in the UI
    pub external: PermissionMode,
}

impl ToolGroup {
    /// Create a new tool group with the given tools
    /// Automatically syncs owner/external from children
    pub fn new(id: &'static str, label: &'static str, tools: Vec<ToolEntry>) -> Self {
        let mut group = Self {
            id,
            label,
            tools,
            expanded: false,
            owner: PermissionMode::Allow,
            external: PermissionMode::Deny,
        };
        group.sync_from_children();
        group
    }

    /// Recompute owner/external modes from children
    /// Call this after editing any child tool's permissions
    /// Sets the group mode to the common mode if all children match,
    /// otherwise leaves it as-is (UI will display "Custom")
    pub fn sync_from_children(&mut self) {
        if self.tools.is_empty() {
            return;
        }

        // Check if all children have the same owner mode
        let first_owner = self.tools[0].owner;
        let all_owner_same = self.tools.iter().all(|t| t.owner == first_owner);
        if all_owner_same {
            self.owner = first_owner;
        }
        // If not all same, leave self.owner as-is (will display as "Custom")

        // Check if all children have the same external mode
        let first_external = self.tools[0].external;
        let all_external_same = self.tools.iter().all(|t| t.external == first_external);
        if all_external_same {
            self.external = first_external;
        }
        // If not all same, leave self.external as-is (will display as "Custom")
    }

    /// Check if all children have the same owner mode
    pub fn is_owner_uniform(&self) -> bool {
        if self.tools.is_empty() {
            return true;
        }
        let first = self.tools[0].owner;
        self.tools.iter().all(|t| t.owner == first)
    }

    /// Check if all children have the same external mode
    pub fn is_external_uniform(&self) -> bool {
        if self.tools.is_empty() {
            return true;
        }
        let first = self.tools[0].external;
        self.tools.iter().all(|t| t.external == first)
    }

    /// Set all children to the given owner mode
    /// Used when editing the group as a whole
    pub fn set_all_owner(&mut self, mode: PermissionMode) {
        for tool in &mut self.tools {
            tool.owner = mode;
        }
        self.owner = mode;
    }

    /// Set all children to the given external mode
    /// Used when editing the group as a whole
    pub fn set_all_external(&mut self, mode: PermissionMode) {
        for tool in &mut self.tools {
            tool.external = mode;
        }
        self.external = mode;
    }
}

/// Full permissions state for one directory (or global)
/// Contains all tool groups and their current permission settings
#[derive(Debug, Clone)]
pub struct ToolTableData {
    /// All tool groups in this table
    pub groups: Vec<ToolGroup>,
}

impl ToolTableData {
    /// Create a new empty tool table
    pub fn new(groups: Vec<ToolGroup>) -> Self {
        Self { groups }
    }

    /// Create the default global tools table
    /// Contains: Web, Sub-agents, Ask Question, Task Management, Load Tools
    /// Default: Owner=Allow, External=Deny for all
    pub fn default_global() -> Self {
        Self::new(vec![
            ToolGroup::new(
                "web",
                "Web",
                vec![
                    ToolEntry::new(
                        "web_search",
                        "web_search",
                        PermissionMode::Allow,
                        PermissionMode::Deny,
                    ),
                    ToolEntry::new(
                        "web_fetch",
                        "web_fetch",
                        PermissionMode::Allow,
                        PermissionMode::Deny,
                    ),
                ],
            ),
            ToolGroup::new(
                "sub_agents",
                "Sub-agents",
                vec![ToolEntry::new(
                    "invoke_sub_agent",
                    "invoke_sub_agent",
                    PermissionMode::Allow,
                    PermissionMode::Deny,
                )],
            ),
            ToolGroup::new(
                "ask_question",
                "Ask Question",
                vec![ToolEntry::new(
                    "ask_user",
                    "ask_user",
                    PermissionMode::Allow,
                    PermissionMode::Deny,
                )],
            ),
            ToolGroup::new(
                "task_management",
                "Task Management",
                vec![
                    ToolEntry::new(
                        "create_task",
                        "create_task",
                        PermissionMode::Allow,
                        PermissionMode::Ask,
                    ),
                    ToolEntry::new(
                        "update_task",
                        "update_task",
                        PermissionMode::Allow,
                        PermissionMode::Ask,
                    ),
                ],
            ),
            ToolGroup::new(
                "load_tools",
                "Load Tools",
                vec![ToolEntry::new(
                    "load_mcp_tools",
                    "load_mcp_tools",
                    PermissionMode::Allow,
                    PermissionMode::Allow,
                )],
            ),
        ])
    }

    /// Create the default directory-scoped tools table
    /// Contains: File System, Shell
    /// Default: Owner=Allow, External=Ask for File System; Owner=Allow, External=Deny for Shell
    pub fn default_directory() -> Self {
        Self::new(vec![
            ToolGroup::new(
                "file_system",
                "File System",
                vec![
                    ToolEntry::new(
                        "read_file",
                        "read_file",
                        PermissionMode::Allow,
                        PermissionMode::Ask,
                    ),
                    ToolEntry::new(
                        "write_file",
                        "write_file",
                        PermissionMode::Allow,
                        PermissionMode::Ask,
                    ),
                    ToolEntry::new(
                        "list_dir",
                        "list_dir",
                        PermissionMode::Allow,
                        PermissionMode::Ask,
                    ),
                    ToolEntry::new(
                        "create_dir",
                        "create_dir",
                        PermissionMode::Allow,
                        PermissionMode::Ask,
                    ),
                    ToolEntry::new(
                        "delete_file",
                        "delete_file",
                        PermissionMode::Ask,
                        PermissionMode::Deny,
                    ),
                ],
            ),
            ToolGroup::new(
                "shell",
                "Shell",
                vec![
                    ToolEntry::new(
                        "run_command",
                        "run_command",
                        PermissionMode::Allow,
                        PermissionMode::Deny,
                    ),
                    ToolEntry::new(
                        "run_script",
                        "run_script",
                        PermissionMode::Allow,
                        PermissionMode::Deny,
                    ),
                ],
            ),
        ])
    }
}

// ============================================================================
// DIRECTORY ENTRY
// ============================================================================

/// A directory entry in the directory list
/// Each directory has its own set of tool permissions
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    /// Path to this directory
    pub path: PathBuf,
    /// Tool permissions for this directory
    pub tools: ToolTableData,
}

impl DirectoryEntry {
    /// Create a new directory entry with default permissions
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            tools: ToolTableData::default_directory(),
        }
    }
}

// ============================================================================
// MODAL STATES
// ============================================================================

/// State for the rule editor modal
/// Allows editing a single permission cell (tool × role)
#[derive(Debug, Clone)]
pub struct RuleEditorState {
    /// Whether the modal is currently open
    pub open: bool,
    /// Index of the group being edited
    pub group_idx: usize,
    /// Index of the tool within the group (None = editing group itself)
    pub tool_idx: Option<usize>,
    /// Which role column is being edited (Owner or External)
    pub role: EditRole,
    /// Currently selected permission mode in the modal
    pub selected_mode: PermissionMode,
}

impl RuleEditorState {
    /// Create a new closed rule editor
    pub fn new() -> Self {
        Self {
            open: false,
            group_idx: 0,
            tool_idx: None,
            role: EditRole::Owner,
            selected_mode: PermissionMode::Allow,
        }
    }

    /// Open the editor for a specific tool/group × role cell
    pub fn open(
        &mut self,
        group_idx: usize,
        tool_idx: Option<usize>,
        role: EditRole,
        current_mode: PermissionMode,
    ) {
        self.open = true;
        self.group_idx = group_idx;
        self.tool_idx = tool_idx;
        self.role = role;
        self.selected_mode = current_mode;
    }

    /// Close the editor without saving
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Move selection up in the modal (Allow ← Ask ← Deny)
    pub fn move_up(&mut self) {
        self.selected_mode = match self.selected_mode {
            PermissionMode::Allow => PermissionMode::Deny,
            PermissionMode::Ask => PermissionMode::Allow,
            PermissionMode::Deny => PermissionMode::Ask,
        };
    }

    /// Move selection down in the modal (Allow → Ask → Deny)
    pub fn move_down(&mut self) {
        self.selected_mode = self.selected_mode.cycle();
    }

    /// Switch between Owner and External roles
    /// Reloads the current mode for the new role
    #[allow(dead_code)] // Alternative implementation kept for API completeness
    pub fn switch_role(&mut self, tools: &ToolTableData) {
        // Toggle the role
        self.role = match self.role {
            EditRole::Owner => EditRole::External,
            EditRole::External => EditRole::Owner,
        };

        // Update selected_mode to reflect the current permission for the new role
        let group = &tools.groups[self.group_idx];
        self.selected_mode = if let Some(tool_idx) = self.tool_idx {
            // Editing a specific tool
            match self.role {
                EditRole::Owner => group.tools[tool_idx].owner,
                EditRole::External => group.tools[tool_idx].external,
            }
        } else {
            // Editing a group
            match self.role {
                EditRole::Owner => group.owner,
                EditRole::External => group.external,
            }
        };
    }
}

impl Default for RuleEditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// State for the add directory modal
/// Allows user to input a new directory path
#[derive(Debug)]
pub struct AddDirState {
    /// Whether the modal is currently open
    pub open: bool,
    /// Text input widget for the directory path
    pub input: TextArea<'static>,
}

impl AddDirState {
    /// Create a new closed add directory modal
    pub fn new() -> Self {
        let mut input = TextArea::default();
        input.set_placeholder_text("~/");
        Self { open: false, input }
    }

    /// Open the modal and reset the input
    pub fn open(&mut self) {
        self.open = true;
        self.input = TextArea::default();
        self.input.set_placeholder_text("~/");
    }

    /// Close the modal without saving
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Get the current input text
    pub fn get_path(&self) -> String {
        self.input.lines().join("")
    }
}

impl Default for AddDirState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MAIN PERMISSIONS SCREEN STATE
// ============================================================================

/// Full state for the permissions screen
/// Manages all UI state including section selection, scroll, modals, etc.
#[derive(Debug)]
pub struct PermissionsScreenState {
    /// Which top-level section is active (Global or Directory)
    pub section: PermissionsSection,
    /// Global tools table (full-width, no directory scope)
    pub global_tools: ToolTableData,
    /// List of directories with their own tool permissions
    pub directories: Vec<DirectoryEntry>,
    /// Selected directory index in the directory list
    pub selected_dir: usize,
    /// Which panel has focus in Directory section (DirList or ToolTable)
    pub focused_panel: FocusedPanel,
    /// Selected row index in the currently focused panel
    pub selected_row: usize,
    /// Scroll offset for the directory list
    pub dir_list_scroll: usize,
    /// Scroll offset for the tool table
    pub tool_table_scroll: usize,
    /// Rule editor modal state
    pub rule_editor: RuleEditorState,
    /// Add directory modal state
    pub add_dir: AddDirState,
}

impl PermissionsScreenState {
    /// Create a new permissions screen state with default values
    /// Starts in Global section with empty directory list
    pub fn new() -> Self {
        Self {
            section: PermissionsSection::Global,
            global_tools: ToolTableData::default_global(),
            directories: Vec::new(),
            selected_dir: 0,
            focused_panel: FocusedPanel::DirList,
            selected_row: 0, // Start at first group row (row 0 in current_row counter, header is separate)
            dir_list_scroll: 0,
            tool_table_scroll: 0,
            rule_editor: RuleEditorState::new(),
            add_dir: AddDirState::new(),
        }
    }

    /// Get the currently active tool table based on section and selected directory
    pub fn active_tools(&self) -> &ToolTableData {
        match self.section {
            PermissionsSection::Global => &self.global_tools,
            PermissionsSection::Directory => {
                if self.directories.is_empty() {
                    // Return global as fallback (shouldn't happen in normal use)
                    &self.global_tools
                } else {
                    &self.directories[self.selected_dir].tools
                }
            }
        }
    }

    /// Get the currently active tool table (mutable) based on section and selected directory
    pub fn active_tools_mut(&mut self) -> &mut ToolTableData {
        match self.section {
            PermissionsSection::Global => &mut self.global_tools,
            PermissionsSection::Directory => {
                if self.directories.is_empty() {
                    // Return global as fallback (shouldn't happen in normal use)
                    &mut self.global_tools
                } else {
                    &mut self.directories[self.selected_dir].tools
                }
            }
        }
    }

    /// Get the current scroll offset based on section and focused panel
    #[allow(dead_code)] // Reserved for future scroll management features
    pub fn active_scroll(&self) -> usize {
        match self.section {
            PermissionsSection::Global => self.tool_table_scroll,
            PermissionsSection::Directory => match self.focused_panel {
                FocusedPanel::DirList => self.dir_list_scroll,
                FocusedPanel::ToolTable => self.tool_table_scroll,
            },
        }
    }

    /// Set the scroll offset for the currently active panel
    #[allow(dead_code)] // Reserved for future scroll management features
    pub fn set_active_scroll(&mut self, offset: usize) {
        match self.section {
            PermissionsSection::Global => self.tool_table_scroll = offset,
            PermissionsSection::Directory => match self.focused_panel {
                FocusedPanel::DirList => self.dir_list_scroll = offset,
                FocusedPanel::ToolTable => self.tool_table_scroll = offset,
            },
        }
    }
}

impl Default for PermissionsScreenState {
    fn default() -> Self {
        Self::new()
    }
}
