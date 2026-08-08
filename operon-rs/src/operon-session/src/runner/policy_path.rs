// runner/policy_path.rs — Policy-facing path extraction for tool calls.
//
// Pure free functions that extract a representative filesystem path from
// a tool call's arguments. Used by the policy resolver to check permissions.
// No dependency on `SessionRunner` fields.

use operon_context::ToolCall;

/// Build the policy-facing path string for a tool call, if the tool uses one.
///
/// This helper extracts a representative filesystem path from the tool call's arguments.
/// The extracted path is used by the policy resolver to check whether the caller has
/// permission to access or operate on that specific path.
fn strip_range_suffix_str(s: &str) -> &str {
    if let Some(idx) = s.rfind(':') {
        let suffix = &s[idx + 1..];
        if suffix.eq_ignore_ascii_case("EOF")
            || suffix.parse::<usize>().is_ok()
            || (suffix.contains('-') && {
                let parts: Vec<&str> = suffix.split('-').collect();
                parts.len() == 2
                    && parts[0].parse::<usize>().is_ok()
                    && (parts[1].eq_ignore_ascii_case("EOF") || parts[1].parse::<usize>().is_ok())
            })
        {
            return &s[..idx];
        }
    }
    s
}

pub(super) fn policy_path_for_call(call: &ToolCall) -> Option<String> {
    let raw_str = match call.name.as_str() {
        "read" => call
            .arguments
            .get("path")
            .and_then(|v| v.as_str())
            .or_else(|| {
                call.arguments.get("paths").and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.as_str()),
                    serde_json::Value::Array(arr) => arr.first().and_then(|item| item.as_str()),
                    _ => None,
                })
            }),

        "grep" => call
            .arguments
            .get("path")
            .and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.as_str()),
                serde_json::Value::Array(arr) => arr.first().and_then(|item| item.as_str()),
                _ => None,
            })
            .or_else(|| {
                call.arguments.get("paths").and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.as_str()),
                    serde_json::Value::Array(arr) => arr.first().and_then(|item| item.as_str()),
                    _ => None,
                })
            }),

        "ls" => call
            .arguments
            .get("path")
            .or_else(|| call.arguments.get("dir"))
            .and_then(|v| v.as_str()),

        "bash" => call.arguments.get("cwd").and_then(|v| v.as_str()),

        "write" | "edit" | "append" | "delete" => {
            call.arguments.get("path").and_then(|v| v.as_str())
        }

        _ => None,
    };

    raw_str.map(|s| strip_range_suffix_str(s).to_string())
}
