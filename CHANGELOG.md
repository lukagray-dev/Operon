# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to Semantic Versioning.

## [Unreleased]

### Changed
- **WhatsApp Shared Workspace Directory & Policy Coverage**:
  - Migrated WhatsApp session workspace directory resolution from per-contact subdirectories (`~/.operon/channels/whatsapp/workspace/<number>/`) to a single shared workspace root (`WhatsAppConfig.workspace_dir`, defaulting to `~/.operon/workspace/`).
  - Added a configuration option `workspace_dir` to `WhatsAppConfig` with GUI settings panel integration, including a folder picker and real-time policy coverage indicator.
  - Role-specific `AGENTS.md` system prompt guidelines are now generated fresh in the shared workspace root prior to each turn based on the message sender's resolved role.
  - Per-contact session history storage remains fully isolated under `~/.operon/sessions/whatsapp/<number>/<session_id>.json`.
  - **Migration Note**: Legacy per-contact workspace folders under `~/.operon/channels/whatsapp/workspace/<number>/` are no longer used by WhatsApp session turns and can be safely removed manually.

## [0.0.2-beta] - 2026-07-21

This release marks the migration of the desktop GUI framework from Tauri v2 to Slint, achieving a truly native graphics-rendered desktop interface with zero browser/WebView overhead, alongside significant markdown rendering and UI improvements.

### Added
- **Slint GUI Framework Migration**:
  - Migrated the entire GUI application from Tauri v2/Web HTML/CSS to Slint, utilizing direct-to-GPU graphics rendering (Skia/FemtoVG) and dropping the memory footprint to ~70 MB idle / <90 MB under load.
  - Configured Windows executable builds to run in the `windows` subsystem for production release, hiding the console terminal window on startup.
  - Refactored GitHub Actions workflows (`ci.yml`, `pre-release.yml`, `stable-release.yml`) to support Slint system dependencies on Linux and standard cargo release builds.

### Fixed
- **Markdown Text Wrapping**:
  - Replaced the custom non-wrapping `InlineMarkdown` layout loop with the native `StyledText` element in paragraphs, bullet points, headers, blockquotes, and table cells to enable proper line wrapping of long formatted text.
  - Added support for dynamic blinking cursor rendering on the Slint side by compiling styled text variants with and without the cursor in Rust.
  - Fixed literal markdown heading hash (`#`) display by bolding heading strings on the Rust side rather than prepending hashes.
  - Styled inline code spans with a soft red color (`#e06c75`) within `StyledText` using HTML-style font tags.
- **User Message Bubble**:
  - Resolved user bubble layout constraints and vertical overflow by bounding and animating height constraints directly on the `text-container` element with explicit clipping.
  - Resolved UI height binding loops by placing the hover touch overlay as a sibling to the user message layout structure.
  - Propagated custom font settings to headings within user messages to ensure they correctly use the `"Google Sans"` application font rather than hardcoding serif fonts.

## [0.0.1-beta] - 2026-06-07

This is the initial pre-release of **Operon**, featuring the core Rust-based agent runtime and a high-performance Tauri-based desktop GUI interface.

### Added
- **Framework & Architecture**:
  - Restructured and migrated the GUI framework from Slint to **Tauri v2** for improved performance, standard web technologies support, and native OS integrations.
  - Implemented custom native titlebar with standard window controls (Minimize, Maximize, Close).
  - Developed collapsible and resizable left-sidebar with responsive UI, directory management, and collapsible project trees.
- **AI Backend (`operon-rs`)**:
  - Developed session runner, events handling, and configuration management infrastructure.
  - Implemented robust provider integration, supporting OpenAI, Anthropic, NVIDIA NIM, and custom OpenAI-compatible endpoints with fallback mechanisms.
  - Created automatic metadata and model discovery for dynamic model selection.
- **Agent Capabilities & Tools**:
  - Created custom tool loading framework with lazy-loading behavior to prevent large payload limitations.
  - Added native shell/bash execution tool with safety boundaries.
  - Implemented interactive `ask` multiple-choice tool enabling the agent to get direct human-in-the-loop decisions.
  - Added web tools (`web_search` and `web_fetch`) for live web access.
  - Integrated file-system tools (read, write, list, delete) with precise directory-scoped permissions.
- **User Experience (UX)**:
  - Built premium glassmorphism styling for model selectors, panels, and settings pages.
  - Implemented live context window status bar displaying token usage details.
  - Added support for manual response cancellation with toggleable Send/Stop control.
  - Added copy feedback cues and syntax highlighting using `highlight.js` with github-dark theme styling.
- **CI/CD & Testing**:
  - Set up automated GitHub Actions workflows for continuous integration (CI), pre-releases (on merge to main), and manual stable releases.
  - Added comprehensive test suites for providers, parser/cmark backend rendering, and snapshot test suites.
