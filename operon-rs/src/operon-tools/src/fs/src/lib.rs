//! # operon-tools-fs
//!
//! Filesystem tool group facade.
//! Re-exports all filesystem tool sub-crates as modules.
//!
//! ## Available tools
//! - `read`: Reads one or multiple files in a single call with optional line ranges.
//! - `edit`: Edits an existing file by replacing exact text with atomic writes.
//! - `write`: Creates a new file or fully overwrites an existing file with atomic writes.
//! - `append`: Appends text to the end of an existing file without modifying existing content.
//!
//! ## Usage
//! ```rust
//! use operon_tools_fs::read;
//!
//! # async fn example() {
//! // Get the tool definition
//! let def = read::definition();
//!
//! // Execute the tool
//! // let result = read::execute(call_id, args).await;
//! # }
//! ```

pub use operon_tools_fs_read as read;
pub use operon_tools_fs_edit as edit;
pub use operon_tools_fs_write as write;
pub use operon_tools_fs_append as append;
