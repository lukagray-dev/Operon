//! Executor for the glob tool — walks the filesystem respecting .gitignore and matches patterns.
//!
//! Hey friend! This module implements fast directory traversal using the ripgrep `ignore` engine
//! and wildcard pattern matching via `globset`.

use crate::args::GlobArgs;
use crate::output::GlobOutput;
use globset::{GlobBuilder, GlobMatcher};
use ignore::WalkBuilder;
use operon_context_normalize_tools::{ToolCallId, ToolContent, ToolResult};
use std::path::Path;

/// Executes the glob search with the given arguments.
pub async fn execute(call_id: ToolCallId, args: GlobArgs) -> ToolResult {
    let raw_path = args.path.as_deref().unwrap_or(".");
    let base_path = Path::new(raw_path);

    // Enforce absolute path policy (unless "." / default workspace root)
    if raw_path != "." && !base_path.is_absolute() {
        return ToolResult {
            call_id,
            name: "glob".to_string(),
            content: ToolContent::Text(format!(
                "Error: Path must be an absolute path. Got: {}",
                raw_path
            )),
            is_error: true,
        };
    }

    if !base_path.exists() {
        return ToolResult {
            call_id,
            name: "glob".to_string(),
            content: ToolContent::Text(format!(
                "Error: Base directory does not exist: {}",
                raw_path
            )),
            is_error: true,
        };
    }

    // Build glob matcher
    let glob_pattern = args.pattern.trim();
    if glob_pattern.is_empty() {
        return ToolResult {
            call_id,
            name: "glob".to_string(),
            content: ToolContent::Text("Error: Glob pattern cannot be empty.".to_string()),
            is_error: true,
        };
    }

    let matcher: GlobMatcher = match GlobBuilder::new(glob_pattern)
        .literal_separator(false)
        .case_insensitive(cfg!(windows))
        .build()
    {
        Ok(g) => g.compile_matcher(),
        Err(e) => {
            return ToolResult {
                call_id,
                name: "glob".to_string(),
                content: ToolContent::Text(format!(
                    "Error: Invalid glob pattern '{}': {}",
                    glob_pattern, e
                )),
                is_error: true,
            };
        }
    };

    // Build a filename-only matcher for simple flat patterns (e.g., "*.rs", "Cargo.*")
    let filename_matcher: Option<GlobMatcher> =
        if !glob_pattern.contains('/') && !glob_pattern.contains('\\') {
            GlobBuilder::new(glob_pattern)
                .literal_separator(false)
                .case_insensitive(cfg!(windows))
                .build()
                .ok()
                .map(|g| g.compile_matcher())
        } else {
            None
        };

    // Use ignore::WalkBuilder for fast, gitignore-aware directory traversal
    let mut builder = WalkBuilder::new(base_path);
    builder
        .hidden(!args.include_hidden)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .parents(true);

    let canonical_base = base_path
        .canonicalize()
        .unwrap_or_else(|_| base_path.to_path_buf());

    let mut all_matches: Vec<String> = Vec::new();

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        // Skip the root base directory itself
        if path == base_path || path == canonical_base {
            continue;
        }

        // Relative path normalized with forward slashes for cross-platform consistency
        let rel_path = match path.strip_prefix(base_path) {
            Ok(p) => p,
            Err(_) => path,
        };

        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let is_match = matcher.is_match(&rel_str)
            || matcher.is_match(path)
            || filename_matcher.as_ref().map_or(false, |m| m.is_match(file_name));

        if is_match {
            all_matches.push(rel_str);
        }
    }

    all_matches.sort();
    let total_matches = all_matches.len();
    let truncated = total_matches > args.max_results;
    if truncated {
        all_matches.truncate(args.max_results);
    }

    let output = GlobOutput {
        pattern: glob_pattern.to_string(),
        base_path: raw_path.to_string(),
        matches: all_matches,
        total_matches,
        truncated,
    };

    ToolResult {
        call_id,
        name: "glob".to_string(),
        content: ToolContent::Text(output.to_formatted_text()),
        is_error: false,
    }
}

