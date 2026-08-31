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
//! - `delete`: Deletes a file or directory, with options for trash or permanent deletion.
//! - `grep`: regex search across files and directories with gitignore-aware recursive walking, filename glob filtering, context lines, and per-file match reporting.
//! - `ls`: single-level directory listing with [FILE]/[DIR] type prefixes, metadata (size, modified), and glob-pattern exclusion.
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

pub use operon_tools_fs_append as append;
pub use operon_tools_fs_delete as delete;
pub use operon_tools_fs_edit as edit;
pub use operon_tools_fs_glob as glob;
pub use operon_tools_fs_grep as grep;
pub use operon_tools_fs_ls as ls;
pub use operon_tools_fs_read as read;
pub use operon_tools_fs_write as write;
