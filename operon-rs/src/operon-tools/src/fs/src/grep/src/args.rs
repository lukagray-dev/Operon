/// Argument types for the grep tool.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PathOrPaths {
    Single(String),
    Multiple(Vec<String>),
}

/// Top-level args the model sends when calling the `grep` tool.
#[derive(Debug, Deserialize)]
pub struct GrepArgs {
    /// Regex pattern to search for.
    pub pattern: String,

    /// Target path(s) to search. Accepts a single string path or array of paths.
    #[serde(alias = "path", default)]
    paths: Option<PathOrPaths>,

    /// Optional glob pattern to filter files by name (e.g. "*.rs").
    #[serde(default)]
    pub include: Option<String>,

    /// Case-insensitive matching. Default: false.
    #[serde(default)]
    pub case_insensitive: Option<bool>,

    /// Number of context lines before and after matches. Default: 2.
    #[serde(default = "default_context_lines")]
    pub context_lines: usize,
}

fn default_context_lines() -> usize {
    2
}

impl GrepArgs {
    /// Returns the target paths as a vector.
    pub fn get_paths(&self) -> Vec<String> {
        match &self.paths {
            Some(PathOrPaths::Single(s)) => vec![s.clone()],
            Some(PathOrPaths::Multiple(v)) => v.clone(),
            None => vec![],
        }
    }
}

