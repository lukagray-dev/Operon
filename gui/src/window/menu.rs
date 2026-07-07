//! Helpers for the titlebar's menu actions.
//!
//! Only the browser-opening helpers live here right now, but the module gives
//! us a clear place to grow the Files, View, and Help menu behavior later.

use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

/// The documentation page opened from the Help menu.
pub const DOCUMENTATION_URL: &str =
    "https://github.com/lukagray-dev/Operon/tree/main/docs";

/// The issue tracker opened from the Help menu.
pub const REPORT_BUG_URL: &str = "https://github.com/lukagray-dev/Operon/issues";

/// The creator profile opened from the Help menu.
pub const FOLLOW_CREATOR_URL: &str = "https://www.instagram.com/lukagray.official/";

/// The project repository opened from the Help menu.
pub const REPOSITORY_URL: &str = "https://github.com/lukagray-dev/Operon";

/// Small value object that records the command we will ask the operating
/// system to run when a Help menu link is opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl BrowserCommand {
    fn new(program: impl Into<String>, args: impl Into<Vec<String>>) -> Self {
        Self {
            program: program.into(),
            args: args.into(),
        }
    }
}

/// Returns the platform-specific browser launcher command for a URL.
///
/// The function is intentionally pure so it can be tested without launching a
/// real browser process.
pub fn browser_command_parts(url: &str) -> BrowserCommand {
    let url = url.trim().to_owned();

    #[cfg(target_os = "windows")]
    {
        return BrowserCommand::new(
            "cmd",
            vec!["/C".to_string(), "start".to_string(), String::new(), url],
        );
    }

    #[cfg(target_os = "macos")]
    {
        return BrowserCommand::new("open", vec![url]);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux and other Unix-like systems generally rely on xdg-open.
        BrowserCommand::new("xdg-open", vec![url])
    }
}

/// Launches the given URL in the user's default browser.
pub fn open_url(url: &str) -> Result<()> {
    let normalized_url = url.trim();

    if normalized_url.is_empty() {
        bail!("browser URL must not be empty");
    }

    let command = browser_command_parts(normalized_url);

    eprintln!(
        "[operon-gui][browser] Opening URL with command: {} {:?}",
        command.program, command.args
    );

    let status = Command::new(&command.program)
        .args(&command.args)
        .status()
        .with_context(|| format!("failed to spawn browser command for {normalized_url}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "browser command exited unsuccessfully for {normalized_url}: {status}"
        ))
    }
}

/// Opens the project documentation.
pub fn open_documentation() -> Result<()> {
    open_url(DOCUMENTATION_URL)
}

/// Opens the issue tracker.
pub fn open_report_bug() -> Result<()> {
    open_url(REPORT_BUG_URL)
}

/// Opens the creator's social profile.
pub fn open_follow_creator() -> Result<()> {
    open_url(FOLLOW_CREATOR_URL)
}

/// Opens the repository home page.
pub fn open_repository() -> Result<()> {
    open_url(REPOSITORY_URL)
}
