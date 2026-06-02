// Permissions screen module
// Access control configuration

pub mod add_directory;
pub mod directory_list;
pub mod global_panel;
pub mod permissions;
pub mod rule_editor;
pub mod section_tabs;
pub mod state;
pub mod tool_table;

pub use permissions::render_permissions_screen;
