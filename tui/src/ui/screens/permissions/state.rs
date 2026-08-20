// state.rs — Permissions screen state management for Operon TUI.
//
// DESIGN PHILOSOPHY:
// 1. Zero Business Logic in Frontend:
//    - The TUI permissions screen is a presentation layer over `operon-rs` policy engine.
//    - All permissions (group/tool hierarchy, owner vs external modes, default bases, overrides)
//      are loaded directly via `operon_rs::get_permission_rows` and `operon_rs::get_allowed_directories_list`.
//    - Updates are persisted directly to `~/.operon/config.toml` via `operon_rs::update_permission`.
// 2. Real-Time Dynamic Synchronization:
//    - Seamlessly supports adding, removing, and switching between allowed directory scopes.
//    - Accurately tracks group expand/collapse states and explicit configuration override badges.

use std::collections::HashSet;
use tui_textarea::TextArea;

/// Cleans raw Windows UNC path prefixes (e.g. `\\?\UNC\` or `\\?\`).
pub fn clean_windows_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", stripped)
    } else if let Some(stripped) = path.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        path.to_string()
    }
}

/// Compares two filesystem paths in a cross-platform, slash-normalized, case-insensitive way.
pub fn is_same_path(a: &str, b: &str) -> bool {
    let clean_a = clean_windows_path(a).replace('/', "\\").to_lowercase();
    let clean_b = clean_windows_path(b).replace('/', "\\").to_lowercase();
    clean_a.trim_end_matches('\\') == clean_b.trim_end_matches('\\')
}

/// Top-level section active in the permissions screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionsSection {
    /// Global tools section (network, subagents, etc.).
    Global,
    /// Directory-scoped tools section (filesystem, bash execution).
    Directory,
}

/// Permission mode for a tool or group under a specific caller role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// Tool can be executed without prompting.
    Allow,
    /// Tool execution requires user confirmation.
    Ask,
    /// Tool execution is completely blocked.
    Deny,
    /// Group has heterogeneous permissions across child tools.
    Custom,
}

impl PermissionMode {
    /// Parse permission mode from backend string identifier.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "allow" => PermissionMode::Allow,
            "ask" => PermissionMode::Ask,
            "deny" => PermissionMode::Deny,
            "custom" => PermissionMode::Custom,
            _ => PermissionMode::Deny,
        }
    }

    /// Convert mode to canonical backend string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::Allow => "allow",
            PermissionMode::Ask => "ask",
            PermissionMode::Deny => "deny",
            PermissionMode::Custom => "custom",
        }
    }

    /// User-facing display label for tables and badges.
    pub fn label(&self) -> &'static str {
        match self {
            PermissionMode::Allow => "Allow",
            PermissionMode::Ask => "Ask",
            PermissionMode::Deny => "Deny",
            PermissionMode::Custom => "Custom",
        }
    }

    /// Style corresponding to this permission mode.
    pub fn style(&self) -> ratatui::style::Style {
        use crate::ui::theme::{COLOR_ACCENT, COLOR_ERROR, COLOR_SUCCESS, COLOR_WARNING};
        use ratatui::style::Style;

        match self {
            PermissionMode::Allow => Style::default().fg(COLOR_SUCCESS),
            PermissionMode::Ask => Style::default().fg(COLOR_WARNING),
            PermissionMode::Deny => Style::default().fg(COLOR_ERROR),
            PermissionMode::Custom => Style::default().fg(COLOR_ACCENT),
        }
    }

    /// Cycle to next permission mode for quick toggle shortcut (Allow → Ask → Deny → Allow).
    #[allow(dead_code)]
    pub fn cycle(&self) -> Self {
        match self {
            PermissionMode::Allow => PermissionMode::Ask,
            PermissionMode::Ask => PermissionMode::Deny,
            PermissionMode::Deny => PermissionMode::Allow,
            PermissionMode::Custom => PermissionMode::Allow,
        }
    }
}

/// Caller role being edited in the rule editor modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditRole {
    /// Owner role (trusted user / operator).
    Owner,
    /// External role (untrusted callers / messaging channels).
    External,
}

impl EditRole {
    /// User-facing display label for the role.
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            EditRole::Owner => "Owner",
            EditRole::External => "External",
        }
    }

    /// Backend scope identifier.
    pub fn as_scope_str(&self) -> &'static str {
        match self {
            EditRole::Owner => "owner",
            EditRole::External => "external",
        }
    }
}

/// Focused panel within the Directory section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    /// Directory list (left panel).
    DirList,
    /// Tool permission table (right panel).
    ToolTable,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool & Table Data Structures
// ─────────────────────────────────────────────────────────────────────────────

/// A single tool leaf node in the permission tree.
#[derive(Debug, Clone)]
pub struct ToolEntry {
    /// Canonical tool identifier (e.g. "fs_read", "web_search").
    pub key: String,
    /// Human-friendly display label (e.g. "File Read", "Web Search").
    pub label: String,
    /// Parent group key (e.g. "fs", "web").
    #[allow(dead_code)]
    pub group_key: String,

    /// Effective permission mode for Owner role.
    pub owner_mode: PermissionMode,
    /// Default base permission mode for Owner role.
    pub owner_base: PermissionMode,
    /// Whether this tool has an explicit configuration override for Owner.
    pub owner_explicit: bool,

    /// Effective permission mode for External role.
    pub external_mode: PermissionMode,
    /// Default base permission mode for External role.
    pub external_base: PermissionMode,
    /// Whether this tool has an explicit configuration override for External.
    pub external_explicit: bool,
}

/// A tool category group node in the permission tree.
#[derive(Debug, Clone)]
pub struct ToolGroup {
    /// Canonical group identifier (e.g. "fs", "bash", "web").
    pub key: String,
    /// Human-friendly display label (e.g. "Filesystem", "Shell Execution", "Web").
    pub label: String,
    /// Child tools belonging to this group.
    pub tools: Vec<ToolEntry>,
    /// Whether this group is expanded in the table view to show child tools.
    pub expanded: bool,

    /// Effective permission mode for Owner role.
    pub owner_mode: PermissionMode,
    /// Default base permission mode for Owner role.
    pub owner_base: PermissionMode,
    /// Whether this group has an explicit configuration override for Owner.
    pub owner_explicit: bool,

    /// Effective permission mode for External role.
    pub external_mode: PermissionMode,
    /// Default base permission mode for External role.
    pub external_base: PermissionMode,
    /// Whether this group has an explicit configuration override for External.
    pub external_explicit: bool,
}

/// Table container holding groups and tools for a single scope (Global or Directory).
#[derive(Debug, Clone, Default)]
pub struct ToolTableData {
    /// All tool groups belonging to this table.
    pub groups: Vec<ToolGroup>,
}

impl ToolTableData {
    /// Constructs a `ToolTableData` from backend `PermissionRow` data for Owner and External roles.
    pub fn from_backend_rows(
        owner_rows: Vec<operon_rs::PermissionRow>,
        external_rows: Vec<operon_rs::PermissionRow>,
        expanded_groups: &HashSet<String>,
    ) -> Self {
        let mut groups = Vec::new();

        // Extract groups
        let owner_groups: Vec<_> = owner_rows.iter().filter(|r| r.kind == "group").collect();
        let owner_tools: Vec<_> = owner_rows.iter().filter(|r| r.kind == "tool").collect();

        for g in owner_groups {
            let ext_g = external_rows
                .iter()
                .find(|r| r.kind == "group" && r.key == g.key);

            let ext_mode = ext_g.map_or(PermissionMode::Deny, |r| PermissionMode::from_str(&r.mode));
            let ext_base = ext_g.map_or(PermissionMode::Deny, |r| PermissionMode::from_str(&r.base_mode));
            let ext_explicit = ext_g.map_or(false, |r| r.is_explicit);

            let mut group_tools = Vec::new();
            for t in owner_tools.iter().filter(|t| t.group_key == g.key) {
                let ext_t = external_rows
                    .iter()
                    .find(|r| r.kind == "tool" && r.key == t.key);

                let t_ext_mode = ext_t.map_or(PermissionMode::Deny, |r| PermissionMode::from_str(&r.mode));
                let t_ext_base = ext_t.map_or(PermissionMode::Deny, |r| PermissionMode::from_str(&r.base_mode));
                let t_ext_explicit = ext_t.map_or(false, |r| r.is_explicit);

                group_tools.push(ToolEntry {
                    key: t.key.clone(),
                    label: t.label.clone(),
                    group_key: t.group_key.clone(),
                    owner_mode: PermissionMode::from_str(&t.mode),
                    owner_base: PermissionMode::from_str(&t.base_mode),
                    owner_explicit: t.is_explicit,
                    external_mode: t_ext_mode,
                    external_base: t_ext_base,
                    external_explicit: t_ext_explicit,
                });
            }

            let is_expanded = expanded_groups.contains(&g.key);

            groups.push(ToolGroup {
                key: g.key.clone(),
                label: g.label.clone(),
                tools: group_tools,
                expanded: is_expanded,
                owner_mode: PermissionMode::from_str(&g.mode),
                owner_base: PermissionMode::from_str(&g.base_mode),
                owner_explicit: g.is_explicit,
                external_mode: ext_mode,
                external_base: ext_base,
                external_explicit: ext_explicit,
            });
        }

        Self { groups }
    }
}

/// Directory item representing an allowed directory scope.
#[derive(Debug, Clone)]
pub struct DirectoryEntry {
    /// Canonical directory path string.
    pub path: String,
    /// Whether this entry is the primary workspace directory.
    pub is_workspace: bool,
    /// Permission table associated with this directory.
    pub tools: ToolTableData,
}

impl DirectoryEntry {
    /// Create a new directory entry with loaded permissions.
    pub fn new(path: String, is_workspace: bool, tools: ToolTableData) -> Self {
        Self {
            path,
            is_workspace,
            tools,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Modal State Containers
// ─────────────────────────────────────────────────────────────────────────────

/// State for the Rule Editor popup modal.
#[derive(Debug, Clone)]
pub struct RuleEditorState {
    /// Whether the editor modal is open.
    pub open: bool,
    /// Index of the target group in the active table.
    pub group_idx: usize,
    /// Index of the target tool (None if editing group).
    pub tool_idx: Option<usize>,
    /// Target caller role (Owner or External).
    pub role: EditRole,
    /// Selected permission mode (Allow, Ask, Deny).
    pub selected_mode: PermissionMode,
}

impl RuleEditorState {
    /// Create a new closed rule editor state.
    pub fn new() -> Self {
        Self {
            open: false,
            group_idx: 0,
            tool_idx: None,
            role: EditRole::Owner,
            selected_mode: PermissionMode::Allow,
        }
    }

    /// Open editor modal with initial parameters.
    pub fn open(
        &mut self,
        group_idx: usize,
        tool_idx: Option<usize>,
        role: EditRole,
        initial_mode: PermissionMode,
    ) {
        self.open = true;
        self.group_idx = group_idx;
        self.tool_idx = tool_idx;
        self.role = role;
        self.selected_mode = if initial_mode == PermissionMode::Custom {
            PermissionMode::Allow
        } else {
            initial_mode
        };
    }

    /// Close editor modal.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Move selection up in mode radio list.
    pub fn move_up(&mut self) {
        self.selected_mode = match self.selected_mode {
            PermissionMode::Allow => PermissionMode::Deny,
            PermissionMode::Ask => PermissionMode::Allow,
            PermissionMode::Deny => PermissionMode::Ask,
            PermissionMode::Custom => PermissionMode::Allow,
        };
    }

    /// Move selection down in mode radio list.
    pub fn move_down(&mut self) {
        self.selected_mode = match self.selected_mode {
            PermissionMode::Allow => PermissionMode::Ask,
            PermissionMode::Ask => PermissionMode::Deny,
            PermissionMode::Deny => PermissionMode::Allow,
            PermissionMode::Custom => PermissionMode::Allow,
        };
    }
}

impl Default for RuleEditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// State for the Add Directory popup modal.
pub struct AddDirectoryState {
    /// Whether the add directory modal is open.
    pub open: bool,
    /// Text input widget for typing the path.
    pub input: TextArea<'static>,
}

impl AddDirectoryState {
    /// Create a new closed add directory state.
    pub fn new() -> Self {
        Self {
            open: false,
            input: TextArea::default(),
        }
    }

    /// Open the add directory modal and clear input.
    pub fn open(&mut self) {
        self.open = true;
        self.input = TextArea::default();
    }

    /// Close the add directory modal.
    pub fn close(&mut self) {
        self.open = false;
        self.input = TextArea::default();
    }

    /// Retrieve the trimmed path string from the text area.
    pub fn get_path(&self) -> String {
        self.input.lines().join("").trim().to_string()
    }
}

impl Default for AddDirectoryState {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PermissionsScreen State Container
// ─────────────────────────────────────────────────────────────────────────────

/// Complete state container for the Permissions TUI screen.
pub struct PermissionsState {
    /// Currently active top-level section (Global or Directory).
    pub section: PermissionsSection,

    /// Which panel is focused in the Directory section.
    pub focused_panel: FocusedPanel,

    /// Global tools table loaded from operon-rs.
    pub global_tools: ToolTableData,

    /// List of allowed directories loaded from operon-rs.
    pub directories: Vec<DirectoryEntry>,

    /// Selected directory index in the directory list.
    pub selected_dir: usize,

    /// Selected row index in the active tool table (groups + visible children).
    pub selected_row: usize,

    /// Vertical scroll offset for the directory list panel.
    pub dir_list_scroll: usize,

    /// Vertical scroll offset for the tool table panel.
    pub tool_table_scroll: usize,

    /// State for the Rule Editor popup modal.
    pub rule_editor: RuleEditorState,

    /// State for the Add Directory popup modal.
    pub add_dir: AddDirectoryState,

    /// Set of expanded group keys to preserve accordion states across refreshes.
    expanded_groups: HashSet<String>,
}

impl PermissionsState {
    /// Constructs a new `PermissionsState` and loads all real permissions from operon-rs.
    pub fn new() -> Self {
        let mut state = Self {
            section: PermissionsSection::Global,
            focused_panel: FocusedPanel::DirList,
            global_tools: ToolTableData::default(),
            directories: Vec::new(),
            selected_dir: 0,
            selected_row: 0,
            dir_list_scroll: 0,
            tool_table_scroll: 0,
            rule_editor: RuleEditorState::new(),
            add_dir: AddDirectoryState::new(),
            expanded_groups: HashSet::new(),
        };

        state.refresh_from_backend();
        state
    }

    /// Queries `operon_rs` policy and config functions to refresh all permission tables.
    pub fn refresh_from_backend(&mut self) {
        // 1. Refresh Global tools
        let owner_global = operon_rs::get_permission_rows("owner", None).unwrap_or_default();
        let external_global = operon_rs::get_permission_rows("external", None).unwrap_or_default();
        self.global_tools = ToolTableData::from_backend_rows(
            owner_global,
            external_global,
            &self.expanded_groups,
        );

        // 2. Refresh Allowed Directories
        let (dirs_list, workspace_dir) = operon_rs::get_allowed_directories_list()
            .unwrap_or_else(|_| (Vec::new(), String::new()));

        let cleaned_workspace = clean_windows_path(&workspace_dir);

        let mut combined_dirs: Vec<String> = Vec::new();
        for d in dirs_list {
            let cleaned = clean_windows_path(&d);
            if !combined_dirs.iter().any(|existing| is_same_path(existing, &cleaned)) {
                combined_dirs.push(cleaned);
            }
        }

        if !cleaned_workspace.is_empty()
            && !combined_dirs.iter().any(|existing| is_same_path(existing, &cleaned_workspace))
        {
            combined_dirs.insert(0, cleaned_workspace.clone());
        }

        self.directories = combined_dirs
            .into_iter()
            .map(|dir_path| {
                let is_workspace = is_same_path(&dir_path, &cleaned_workspace)
                    || dir_path == "~/.operon/workspace"
                    || dir_path == "~\\.operon\\workspace";

                let owner_dir = operon_rs::get_permission_rows("owner", Some(&dir_path)).unwrap_or_default();
                let external_dir = operon_rs::get_permission_rows("external", Some(&dir_path)).unwrap_or_default();
                let tools = ToolTableData::from_backend_rows(
                    owner_dir,
                    external_dir,
                    &self.expanded_groups,
                );
                DirectoryEntry::new(dir_path, is_workspace, tools)
            })
            .collect();

        if self.selected_dir >= self.directories.len() && !self.directories.is_empty() {
            self.selected_dir = self.directories.len() - 1;
        }
    }

    /// Returns a reference to the active tool table (Global or selected Directory).
    pub fn active_tools(&self) -> &ToolTableData {
        match self.section {
            PermissionsSection::Global => &self.global_tools,
            PermissionsSection::Directory => {
                if self.directories.is_empty() {
                    &self.global_tools
                } else {
                    &self.directories[self.selected_dir].tools
                }
            }
        }
    }

    /// Returns a mutable reference to the active tool table.
    pub fn active_tools_mut(&mut self) -> &mut ToolTableData {
        match self.section {
            PermissionsSection::Global => &mut self.global_tools,
            PermissionsSection::Directory => {
                if self.directories.is_empty() {
                    &mut self.global_tools
                } else {
                    &mut self.directories[self.selected_dir].tools
                }
            }
        }
    }

    /// Toggles expansion of a group and updates the persisted expansion set.
    pub fn toggle_group_expansion(&mut self, group_key: &str) {
        let is_expanded = if self.expanded_groups.contains(group_key) {
            self.expanded_groups.remove(group_key);
            false
        } else {
            self.expanded_groups.insert(group_key.to_string());
            true
        };

        let tools = self.active_tools_mut();
        if let Some(g) = tools.groups.iter_mut().find(|g| g.key == group_key) {
            g.expanded = is_expanded;
        }
    }
}

impl Default for PermissionsState {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_windows_path_unc() {
        assert_eq!(clean_windows_path(r"\\?\C:\Users\test"), r"C:\Users\test");
        assert_eq!(clean_windows_path(r"\\?\UNC\server\share"), r"\\server\share");
        assert_eq!(clean_windows_path("C:/Users/test"), "C:/Users/test");
    }

    #[test]
    fn test_is_same_path_deduplication() {
        assert!(is_same_path(r"\\?\C:\Users\test\.operon\workspace", r"C:\Users\test\.operon\workspace"));
        assert!(is_same_path("C:/Users/test/.operon/workspace", r"C:\Users\test\.operon\workspace"));
        assert!(is_same_path(r"C:\Users\Test\.operon\workspace\", r"c:\users\test\.operon\workspace"));
        assert!(!is_same_path(r"C:\Users\test\other", r"C:\Users\test\workspace"));
    }

    #[test]
    fn test_permission_mode_from_str_and_as_str() {
        assert_eq!(PermissionMode::from_str("allow"), PermissionMode::Allow);
        assert_eq!(PermissionMode::from_str("ASK"), PermissionMode::Ask);
        assert_eq!(PermissionMode::from_str("deny"), PermissionMode::Deny);
        assert_eq!(PermissionMode::from_str("custom"), PermissionMode::Custom);
        assert_eq!(PermissionMode::from_str("unknown"), PermissionMode::Deny);

        assert_eq!(PermissionMode::Allow.as_str(), "allow");
        assert_eq!(PermissionMode::Ask.as_str(), "ask");
        assert_eq!(PermissionMode::Deny.as_str(), "deny");
        assert_eq!(PermissionMode::Custom.as_str(), "custom");
    }

    #[test]
    fn test_tool_table_data_from_backend_rows() {
        let owner_rows = vec![
            operon_rs::PermissionRow {
                key: "fs".to_string(),
                label: "Filesystem".to_string(),
                mode: "allow".to_string(),
                base_mode: "allow".to_string(),
                is_explicit: false,
                kind: "group".to_string(),
                group_key: "".to_string(),
            },
            operon_rs::PermissionRow {
                key: "fs_read".to_string(),
                label: "File Read".to_string(),
                mode: "allow".to_string(),
                base_mode: "allow".to_string(),
                is_explicit: false,
                kind: "tool".to_string(),
                group_key: "fs".to_string(),
            },
        ];

        let external_rows = vec![
            operon_rs::PermissionRow {
                key: "fs".to_string(),
                label: "Filesystem".to_string(),
                mode: "deny".to_string(),
                base_mode: "deny".to_string(),
                is_explicit: false,
                kind: "group".to_string(),
                group_key: "".to_string(),
            },
            operon_rs::PermissionRow {
                key: "fs_read".to_string(),
                label: "File Read".to_string(),
                mode: "deny".to_string(),
                base_mode: "deny".to_string(),
                is_explicit: false,
                kind: "tool".to_string(),
                group_key: "fs".to_string(),
            },
        ];

        let expanded = HashSet::new();
        let table = ToolTableData::from_backend_rows(owner_rows, external_rows, &expanded);

        assert_eq!(table.groups.len(), 1);
        assert_eq!(table.groups[0].key, "fs");
        assert_eq!(table.groups[0].owner_mode, PermissionMode::Allow);
        assert_eq!(table.groups[0].external_mode, PermissionMode::Deny);
        assert_eq!(table.groups[0].tools.len(), 1);
        assert_eq!(table.groups[0].tools[0].key, "fs_read");
        assert_eq!(table.groups[0].tools[0].owner_mode, PermissionMode::Allow);
        assert_eq!(table.groups[0].tools[0].external_mode, PermissionMode::Deny);
    }
}
