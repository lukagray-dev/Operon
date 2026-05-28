use std::fs;
use std::path::Path;

use crate::error::SnapshotError;

/// Reads `AGENTS.md` from the configured root when present.
pub(crate) fn read_agents_md(root: &Path) -> Result<Option<String>, SnapshotError> {
    let agents_path = root.join("AGENTS.md");
    if !agents_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(agents_path)?;
    Ok(Some(content))
}
