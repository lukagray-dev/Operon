# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to Semantic Versioning.

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
