# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to Semantic Versioning.

## [0.0.3-beta] - 2026-08-23

This release distributes Operon on Windows across three unified frontends: the Graphical User Interface (GUI), the Terminal User Interface (TUI), and the new standalone **VS Code Extension** (`.vsix`) powered by an optimized native JSON-RPC bridge (`operon-vscode-bridge`), alongside automated GitHub Releases update checking, desktop notifications, filesystem tool improvements, and a global SQLite-backed memory subsystem.

### Added
- **VS Code Extension Distribution (`operon-vscode-0.1.0.vsix`) & Native Bridge (`operon-vscode-bridge`)**:
  - Distributed standalone Visual Studio Code extension package (`.vsix`) bundling the pre-compiled, optimized Rust bridge binary (`bin/operon-vscode-bridge.exe`) and full TypeScript Webview frontend.
  - **Project-Scoped Session Architecture**:
    - Automatically links chat sessions to the active VS Code workspace folder without polluting user profile root paths.
    - Integrated "No Workspace Opened" disclaimer screen with single-click folder picker to prevent unattached session executions.
    - Automatically creates a fresh project-scoped session on startup so users can start typing and streaming immediately.
  - **IDE-Optimized Responsive Layout**:
    - Responsive input bar toolbar that collapses text labels (Auto-Approve, Model Name) into icon-only chips when the sidebar width is narrow.
    - Full feature parity with GUI: Interactive WorkGroup tool timeline, 60fps streaming markdown, thinking orb animations, multiple-choice `ask` cards, and floating permission approval cards.
    - Added native VS Code toast notifications for pending tool permission requests and turn completion.
    - Turnkey compilation and packaging scripts: [`scripts/build-vscode.bat`](file:///d:/Operon/scripts/build-vscode.bat) and [`scripts/build-vscode-release.bat`](file:///d:/Operon/scripts/build-vscode-release.bat).
- **Automated GitHub Release Update System (GUI & VS Code)**:
  - **Operon GUI**:
    - Added native updater service ([`gui/src-tauri/src/shared/updater.rs`](file:///d:/Operon/gui/src-tauri/src/shared/updater.rs)) querying GitHub Releases API (`lukagray-dev/Operon`) with semantic version comparison (`is_newer_version`).
    - Configurable background startup checking controlled via **"Automatic Update Checks"** in General Settings.
    - Prominent **"Update Ready (vX.Y.Z) — Relaunch"** badge in the left sidebar bottom drawer for single-click seamless restart and upgrade.
    - Integrated manual update check in Titlebar `Help → Check for update` with real-time feedback dialogs.
  - **VS Code Extension**:
    - Background startup check notifying developers of new Operon releases with direct action buttons to view release notes or update via the marketplace.
    - Registered Command Palette action: `Operon: Check for Updates`.
- **Terminal User Interface (`operon-tui`)**:
  - High-performance terminal interface built with `ratatui` and `crossterm`.
  - Supports live multi-block assistant message streaming, syntax-highlighted markdown, tool timeline visualizations, compaction pills, and interactive permission/ask dialogs.
- **Native Desktop Notifications (GUI)**:
  - Integrated native Windows desktop notifications via `tauri-plugin-notification` with explicit AppUserModelID (`com.operon.desktop`) registration and embedded brand logo icons.
- **Global Persistent Memory Subsystem**:
  - SQLite-backed persistent memory store (`operon-tools-memory-store`) supporting `memory_add`, `memory_edit`, `memory_delete`, `memory_retrieve`, and `memory_search` tools.

### Changed
- **VS Code Extension Specialization**:
  - Removed background WhatsApp and Telegram channel daemons from the VS Code extension and native bridge to keep the extension lightweight, fast, and focused purely on IDE workflows (channels remain fully supported in the standalone GUI).
- **Absolute Path Enforcement**:
  - Enforced strict absolute path validation across all filesystem tools (`ls`, `read`, `write`, `edit`, `append`, `delete`, `grep`) and shell (`bash`) to maintain statelessness and prevent process-level working directory pollution.

### Fixed
- Fixed message stream re-render race condition where in-memory assistant streaming tokens were wiped upon initial prompt session attachment.
- Fixed task/todo right sidebar pointer-events freeze and subscription re-render loops in VS Code extension.
- Fixed missing floating permission approval cards in VS Code Webview.
- Fixed Tauri v2 main thread async runtime panic in GUI background updater task.
- Fixed streaming provider test assertions in `operon-context-normalize-stream` for Gemini and tool call sequencers.

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
