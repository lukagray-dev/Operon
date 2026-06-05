// markdown_commands.rs — Tauri command handlers for Markdown rendering.
//
// This module exposes the `render_markdown` IPC command to the GUI frontend.
// It acts as the bridge connecting the webview layer to the backend `operon-markdown` engine,
// allowing live and historical Markdown text (with tables, strikethroughs, tasklists, and LaTeX)
// to be compiled into HTML and typeset natively or via KaTeX.

/// Converts a raw markdown string into safe HTML representation.
///
/// This is called by the frontend (Tauri IPC invoke) during message streaming and 
/// chat history loading to ensure all formatting is rendered properly.
///
/// # Arguments
/// * `markdown` - A String containing the raw Markdown input.
///
/// # Returns
/// A String containing the parsed HTML representation.
#[tauri::command]
pub fn render_markdown(markdown: String) -> String {
    // Delegate the markdown parsing to the operon-rs markdown module.
    operon_rs::markdown::render(&markdown)
}
