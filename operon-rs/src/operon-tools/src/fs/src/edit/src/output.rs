// Output for the edit tool is plain text (ToolContent::Text). No struct needed.
//
// Success format:   "{path} ({N} hunk(s) applied)"
// Error format:     "{path}\n{error description}"
//
// Both success and error results use ToolContent::Text with is_error = false,
// so the model reads the inline text for all outcomes.
