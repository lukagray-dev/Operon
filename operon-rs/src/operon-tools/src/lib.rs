//! # operon-tools
//!
//! All Operon agent tool groups and their runtime implementations.
//!
//! ## Structure
//!
//! - `fs` — filesystem tools (read, write, edit, grep, …)
//! - `shell` — shell tools (bash, …)
//! - `todo` — todo tools (create, list, update, delete)
//! - `dispatcher` — routes model tool calls to implementations; manages tiered
//!   descriptions per session.
//!
//! ## Session usage
//!
//! ```rust
//! use operon_tools::dispatcher::Dispatcher;
//!
//! let mut d = Dispatcher::new();
//! d.register_fs_tools();
//! d.register_shell_tools();
//! d.register_todo_tools();
//!
//! // Definitions to send to the model:
//! let defs: Vec<_> = d.definitions().collect();
//!
//! // After the model calls a tool:
//! // let result = d.dispatch(tool_call).await;
//! ```

pub mod dispatcher;

#[cfg(test)]
mod tests;

pub use operon_tools_fs as fs;
pub use operon_tools_shell as shell;
pub use operon_tools_todo as todo;
