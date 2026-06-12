//! Argument types for the `load_tools` tool.
//!
//! Parsing is infallible: a missing or empty `group` simply means "list all groups".
//! There is no error case here.

/// Arguments parsed from the `load_tools` tool call.
pub struct LoadToolsArgs {
    /// Name of the tool group to load.
    ///
    /// `None` means the model called `load_tools` with no group argument, which
    /// triggers the "list all available groups" mode instead of loading a specific one.
    pub group: Option<String>,
}

impl LoadToolsArgs {
    /// Parses args from the raw JSON value injected by the dispatcher.
    ///
    /// This function is infallible. A missing, null, or empty `group` field
    /// returns `group: None`, which means "list all groups".
    ///
    /// # Examples
    ///
    /// - `{"group": "fs"}` → `LoadToolsArgs { group: Some("fs") }`
    /// - `{}`              → `LoadToolsArgs { group: None }`
    /// - `{"group": ""}`   → `LoadToolsArgs { group: None }` (empty filtered out)
    pub fn parse(args_json: &serde_json::Value) -> Self {
        // Extract "group" as a non-empty string, or None if missing/empty.
        let group = args_json
            .get("group")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        LoadToolsArgs { group }
    }
}
