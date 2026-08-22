//! Main Content backend root module.

#[path = "empty-state/mod.rs"]
pub mod empty_state;
pub mod input;
pub mod markdown;
pub mod messages;
pub mod terminal;
pub mod topbar;
#[path = "work-group/mod.rs"]
pub mod work_group;
