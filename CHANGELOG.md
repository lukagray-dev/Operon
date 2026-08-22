# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to Semantic Versioning.

## [0.0.3-beta] - 2026-08-22

This pre-release distributes Operon on Windows across three unified frontends: the Graphical User Interface (GUI), the Terminal User Interface (TUI), and the new **VS Code Extension** powered by a high-performance native JSON-RPC bridge (`operon-vscode-bridge`), alongside major filesystem tool improvements, native desktop notifications, and a global SQLite-backed memory subsystem.

### Added
- **VS Code Extension & Native JSON-RPC Bridge (`operon-vscode-bridge`)**:
  - Introduced a standalone Visual Studio Code extension (`vscode/extension/`) providing the complete Operon AI agent experience directly within the editor.
  - Implemented high-performance native JSON-RPC 2.0 stdio bridge (`operon-vscode-bridge`) communicating with the core `operon-rs` runtime.
  - Full feature parity with the desktop interface:
    - **Interactive WorkGroup**: Visual tool timeline, collapsible tool execution inspection (displaying both input parameters and stdout results), and 60fps thinking orb animations.
    - **Real-Time Token Streaming**: RAF-batched 60fps markdown rendering with syntax highlighting and KaTeX math formatting.
    - **Human-in-the-Loop Clarification**: Embedded interactive `ask` question cards and dynamic tool authorization banners.
    - **Context Management**: Visual compaction pills, live token usage trackers, model reasoning level selector, and session task manager.
    - **Workspace & Chat Drawer**: Left overlay drawer for managing multi-workspace project folders, chat sessions, forks, and inline message editing.
    - **Settings Tab**: Modular settings editor with dedicated category controllers for Models, Channels, Permissions, Appearance, Memory, and General options.
  - Added turnkey compilation batch scripts: [`scripts/build-vscode.bat`](file:///d:/Operon/scripts/build-vscode.bat) (Development) and [`scripts/build-vscode-release.bat`](file:///d:/Operon/scripts/build-vscode-release.bat) (Release).
- **Terminal User Interface (`operon-tui`)**:
  - Introduced a complete, high-performance terminal interface built with `ratatui` and `crossterm`.
  - Supports live multi-block assistant message streaming, interactive syntax-highlighted markdown, tool execution timeline visualizations, and compaction pills.
  - Interactive human-in-the-loop permission request dialogs and multiple-choice `ask` tool question cards.
  - Real-time status bar with token usage tracker, active session context, and hotkey navigation.
- **Native Desktop Notifications (GUI)**:
  - Integrated native Windows desktop toast notifications via `tauri-plugin-notification` with explicit AppUserModelID (`com.operon.desktop`) registration and embedded brand logo icons.
  - Configurable notifications in the General Settings panel for **"Notify when response complete"** and **"Notify when asking permissions"**, synchronized in real time across the application.
- **Tool Suite Overhaul & Examples**:
  - Implemented automatic recursive parent directory creation in `operon-tools-fs-write` to eliminate redundant directory creation tool calls.
  - Optimized `operon-tools-fs-read` prompts and schemas to encourage single-turn batch reading of multiple files via the `paths` parameter.
  - Added 21 standalone, well-documented runnable basic usage examples across filesystem, shell, web, todo, memory, load, and ask tools.
- **Global Persistent Memory Subsystem**:
  - Added SQLite-backed persistent memory store (`operon-tools-memory-store`) supporting `memory_add`, `memory_edit`, `memory_delete`, `memory_retrieve`, and `memory_search` tools.
- **Channels Integration (WhatsApp & Telegram)**:
  - Centralized background channels service manager with auto-reconnect on startup, QR and phone code pairing, owner allowlists, and live policy coverage indicators.

### Changed
- **Absolute Path Enforcement**:
  - Enforced strict absolute path validation across all filesystem tools (`ls`, `read`, `write`, `edit`, `append`, `delete`, `grep`) and shell (`bash`), rejecting relative paths to maintain statelessness and prevent process-level working directory pollution.
- **Dual Workspace Routing in GUI**:
  - Preserved dual general chat (`~/.operon/workspace/`) vs project chat workspace routing without mutating the host process working directory.
- **WhatsApp Shared Workspace Directory & Policy Coverage**:
  - Migrated WhatsApp session workspace directory resolution from per-contact subdirectories (`~/.operon/channels/whatsapp/workspace/<number>/`) to a single shared workspace root (`WhatsAppConfig.workspace_dir`, defaulting to `~/.operon/workspace/`).
  - Added configuration option `workspace_dir` to `WhatsAppConfig` with GUI settings panel integration, including folder picker and real-time policy coverage indicator.
  - Role-specific `AGENTS.md` system prompt guidelines are now generated fresh in the shared workspace root prior to each turn based on the message sender's resolved role.
  - Per-contact session history storage remains fully isolated under `~/.operon/sessions/whatsapp/<number>/<session_id>.json`.
  - **Migration Note**: Legacy per-contact workspace folders under `~/.operon/channels/whatsapp/workspace/<number>/` are no longer used by WhatsApp session turns and can be safely removed manually.

### Fixed
- Fixed streaming provider test assertions in `operon-context-normalize-stream` for Gemini and tool call sequencers.
- Fixed non-existent test file paths and Windows drive letter resolution across filesystem tool test suites.
- Fixed Cargo workspace example binary target name collisions under MSVC on Windows.

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
