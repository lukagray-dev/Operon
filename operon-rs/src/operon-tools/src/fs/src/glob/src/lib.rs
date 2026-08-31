//! # operon-tools-fs-glob
//!
//! Fast, `.gitignore`-aware file and directory path search matching glob patterns.
//!
//! ## Capabilities
//! - Glob pattern matching (`**/*.rs`, `src/**/*.ts*`, `tests/*.py`).
//! - Automatic `.gitignore` and hidden file respect via ripgrep's `ignore` engine.
//! - Normalized forward-slash relative path output sorted alphabetically.
//! - Configurable result limit (`max_results`).

pub mod args;
pub mod error;
pub mod executor;
pub mod output;

#[cfg(test)]
mod tests;

use crate::args::GlobArgs;
use crate::error::GlobToolError;
use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolProgress, ToolProgressEmitter};
use serde_json::json;

/// Returns the canonical tool definition for the `glob` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`pattern`).
/// - Standard parameters for `pattern`, `path`, `max_results`, and `include_hidden`.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the JSON Schema parameters for the glob search tool here.
    // It enables the model to find files across large repositories using wildcards.
    let parameters = json!({
        "type": "object",
        "properties": {
            "pattern": {
                "type": "string",
                "description": "Glob pattern to match file or directory paths against (e.g. '**/*.rs', 'src/**/*.ts', 'tests/*.py', '*.json')."
            },
            "path": {
                "type": "string",
                "description": "Base directory path to search within. Must be an absolute path. Defaults to current workspace directory if omitted."
            },
            "max_results": {
                "type": "integer",
                "minimum": 1,
                "maximum": 1000,
                "description": "Maximum number of matching file paths to return. Default: 100, Max: 1000."
            },
            "include_hidden": {
                "type": "boolean",
                "description": "Whether to include hidden files and directories (dotfiles). Default: false."
            }
        },
        "required": ["pattern"]
    });

    ToolDefinition {
        name: "glob".to_string(),
        description: "Fast file and directory search matching glob patterns across the workspace. \
                      Pass `pattern` (e.g. `**/*.rs`, `src/**/*.tsx`) and optional `path` (base directory). \
                      Respects .gitignore by default and returns sorted relative paths."
            .to_string(),
        parameters,
    }
}

/// Deserializes `args_json` and executes the glob tool.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
) -> Result<ToolResult, GlobToolError> {
    execute_with_progress(call_id, args_json, None).await
}

/// Deserializes `args_json` and executes the glob tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, GlobToolError> {
    let args: GlobArgs = serde_json::from_value(args_json)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(
            call_id.clone(),
            "glob",
            args.path.clone(),
            format!("Matching pattern '{}'", args.pattern),
        ),
    );

    Ok(executor::execute(call_id, args).await)
}

