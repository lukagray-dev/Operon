// output.rs — Output types for the bash tool.
//
// Defines the structured result format returned by the bash tool on successful
// dispatch. On validation or spawn failures, ToolContent::Text is used directly
// (no struct needed for error paths).

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// BashOutput
// ─────────────────────────────────────────────────────────────────────────────

/// Structured output returned after a bash command executes.
///
/// Kept as an internal representation of process results. Converted to plain text
/// via `to_plain_text()` before being wrapped in `ToolContent::Text` in the `ToolResult`.
///
/// Returned even when the command exits with a non-zero code or times out —
/// the model sees the plain-text output and exit code and decides what to do next.
/// Only process spawn failures and validation errors use `is_error: true`
/// with `ToolContent::Text` directly instead.
#[derive(Debug, Serialize, Deserialize)]
pub struct BashOutput {
    /// The command that was executed, echoed back for correlation.
    /// Useful when the model issues multiple commands in one turn.
    pub command: String,

    /// The working directory the command was executed in.
    /// Matches the `cwd` argument that was passed in.
    /// Echoed back so the model can confirm execution context.
    pub cwd: String,

    /// Exit code of the process.
    /// - `0`  → success
    /// - `N`  (non-zero) → the command reported failure
    /// - `-1` → the process was killed due to timeout (`timed_out` will be true)
    ///
    /// A non-zero exit code is NOT a tool error — the model receives the output
    /// and decides how to respond (retry, adjust command, report to user, etc.).
    pub exit_code: i32,

    /// Merged stdout + stderr output, truncated to 10,000 characters if needed.
    ///
    /// Stdout appears first, then stderr — same order as `2>&1` shell redirection.
    /// When `truncated` is true, use more targeted commands to get the relevant
    /// portion: `| head -n 50`, `| tail -n 20`, `| grep "pattern"`, etc.
    pub output: String,

    /// True if the output was truncated at the 10,000 character limit.
    /// When true, the full output was NOT returned — important lines may be missing.
    pub truncated: bool,

    /// True if the command was killed because it exceeded the `timeout_ms` limit.
    /// When true, `exit_code` will be -1 and `output` contains whatever was
    /// buffered before the kill signal was sent.
    pub timed_out: bool,
}

impl BashOutput {
    /// Formats the bash execution result as raw plain text with header and summary lines.
    ///
    /// This format resembles a terminal transcript so language models can easily read
    /// multi-line stdout/stderr without JSON escaping overhead or `\n` literal pollution.
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();

        // 1. Header line: command executed and working directory
        out.push_str(&format!("=== {} (in {}) ===\n", self.command, self.cwd));

        // 2. Output content (stdout + stderr), verbatim with real line breaks
        if !self.output.is_empty() {
            out.push_str(&self.output);
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }

        // 3. Truncation note if output reached character limit
        if self.truncated {
            out.push_str("[Output truncated at 10,000 characters. Use head, tail, or grep to narrow output.]\n");
        }

        // 4. Summary line with exit code (and inline timeout note if applicable)
        if self.timed_out {
            out.push_str(&format!("Exit code: {} (timed out)", self.exit_code));
        } else {
            out.push_str(&format!("Exit code: {}", self.exit_code));
        }

        out
    }
}
