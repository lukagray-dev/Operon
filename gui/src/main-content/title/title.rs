//! Session Title controller.
//!
//! Hey friend! This file manages updating the active session title inside the Slint GUI,
//! as well as formatting and determining the title from user message text.

/// Sets the current session title in the Slint GUI window.
pub fn set_session_title(window: &crate::OperonWindow, title: &str) {
    println!("[operon-gui][title] Setting session title to: {}", title);
    window.set_session_title(title.into());
}

/// Determines the conversation title based on the first user message.
///
/// Hey friend! This takes the raw first user message and formats it into a clean,
/// truncated title string. If no message is present, it returns a default fallback.
pub fn determine_session_title(first_message: Option<&str>, default_fallback: &str) -> String {
    match first_message {
        Some(msg) => {
            let mut clean_title = msg.replace('\n', " ").trim().to_string();
            if clean_title.len() > 40 {
                clean_title = format!("{}...", &clean_title[..40]);
            }
            clean_title
        }
        None => default_fallback.to_string(),
    }
}
