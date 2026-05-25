// Permissions screen module
// Access control configuration

pub mod permissions;
pub mod rule_editor;
pub mod state;
pub mod section_tabs;
pub mod global_panel;
pub mod directory_list;
pub mod tool_table;
pub mod add_directory;

pub use permissions::render_permissions_screen;
