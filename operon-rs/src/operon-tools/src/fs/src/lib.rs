//! # operon-tools-fs
//!
//! Filesystem tool group facade.
//! Re-exports all filesystem tool sub-crates as modules.
//!
//! ## Available tools
//! - `read`: Reads one or multiple files in a single call with optional line ranges.
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
