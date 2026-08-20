<div align="center">

# **Operon TUI**

### *Keyboard-first, lightning-fast terminal frontend for the Operon Agent Platform*

[![Rust](https://img.shields.io/badge/Rust-2021-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Ratatui](https://img.shields.io/badge/Ratatui-v0.29-blue?style=flat-square)](https://ratatui.rs/)
[![Crossterm](https://img.shields.io/badge/Crossterm-v0.28-purple?style=flat-square)](https://github.com/crossterm-rs/crossterm)
[![License](https://img.shields.io/badge/License-AGPL--3.0-green?style=flat-square)](../LICENSE)

</div>

---

## ⚡ Overview

**Operon TUI** is the official terminal interface for the Operon agentic ecosystem. Built with [Ratatui](https://ratatui.rs/) and [Crossterm](https://github.com/crossterm-rs/crossterm), it provides an ultra-responsive, zero-overhead terminal workspace for interacting with autonomous agents, configuring AI models, managing permission boundaries, and resuming past sessions without ever leaving your command line.

Like all Operon frontends, the TUI is designed as a **thin client** over the unified `operon-rs` backend harness—ensuring zero business logic duplication and 100% feature parity with the GUI desktop app.

---

## ✨ Features

- 💬 **Live Streaming Agent Loop**: Real-time text generation and thinking/reasoning streaming deltas powered by `operon_rs::session::SessionRunner`.
- 🛑 **In-Flight Cancellation**: Cleanly interrupt prompt execution anytime with `Esc` or `Ctrl+C` without closing your terminal.
- 🔄 **Conversation Resumption**: Switch to `/ -> Resume` to discover, browse, and seamlessly continue previous conversations stored in the workspace.
- 🤖 **Dynamic Model & Provider Configuration**: Configure API keys, base URLs, and run live auto-discovery over OpenAI, Anthropic, Google Gemini, Groq, Ollama, DeepSeek, and custom providers.
- 🛡️ **Role-Based Permission Rules**: View and toggle granular filesystem, shell, web, and memory tool permissions for `Owner` and `External` roles, with directory authorization lists.
- 📋 **Seamless Clipboard & Mouse Integration**: Native `Ctrl+V` pasting with bracketed paste support and `Ctrl+Shift` + drag text selection for terminal copying.
- 🎨 **Modern Minimalist Aesthetics**: Clean box-drawing borders, high-contrast accessible color tokens, live status-bar gauges, and zero distracting emojis.

---

## 🏗️ Architecture

Operon TUI operates as a pure event-driven terminal consumer. All model calling, provider routing, context compaction, tool execution, and database persistence are handled by `operon-rs`.

```text
┌────────────────────────────────────────────────────────┐
│                      Operon TUI                        │
│                                                        │
│  [ Chat Screen ]   [ Models ]   [ Permissions ]   ...  │
│  ┌──────────────────────────────────────────────────┐  │
│  │ Messages View / Streaming Deltas & Thinking      │  │
│  ├──────────────────────────────────────────────────┤  │
│  │ Input Box (Multi-line tui-textarea / Ctrl+Enter) │  │
│  └──────────────────────────────────────────────────┘  │
│  [ Status Bar: Model • Context Usage • Git Branch ]    │
└───────────────────────────┬────────────────────────────┘
                            │ (AgentBridge Trait)
                            ▼
┌────────────────────────────────────────────────────────┐
│                   operon-rs Facade                     │
│                                                        │
│  ┌──────────────────┐  ┌────────────────────────────┐  │
│  │  SessionRunner   │  │  Context & Compaction      │  │
│  ├──────────────────┤  ├────────────────────────────┤  │
│  │  Policy Engine   │  │  JSON Session Persistence  │  │
│  └──────────────────┘  └────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

---

## ⌨️ Keybindings Cheat Sheet

### Global Navigation
| Key | Action | Description |
| :--- | :--- | :--- |
| `/` | **Screen Selector** | Open inline screen picker (`Chat`, `Resume`, `Models`, `Permissions`, `Help`) |
| `Esc` | **Back / Cancel** | Return to Chat screen, close modals, or cancel in-flight agent generation |
| `Ctrl+Q` | **Quit** | Exit Operon TUI safely, restoring terminal state |
| `Ctrl+V` / `Shift+Insert` | **Paste** | Paste text from system clipboard into the active focused field |

### Chat Screen
| Key | Action | Description |
| :--- | :--- | :--- |
| `Enter` / `Ctrl+Enter` | **Send Prompt** | Submit current input message to the agent |
| `Shift+Enter` | **Newline** | Insert a line break into the multi-line input box |
| `Esc` / `Ctrl+C` | **Cancel Prompt** | Abort active generation turn cleanly |
| `Ctrl+Z` / `Ctrl+Shift+Z` | **Undo / Redo** | History undo and redo in the text input box |
| `PageUp` / `PageDown` | **Scroll Chat** | Scroll message history up or down |

### Resume Screen (`/ -> Resume`)
| Key | Action | Description |
| :--- | :--- | :--- |
| `↑` / `↓` (or `k` / `j`) | **Navigate** | Highlight previous conversation sessions in the current workspace |
| `Enter` | **Resume Session** | Load selected conversation history into the chat panel and continue |
| `Esc` | **Back** | Cancel and return to Chat screen |

### Models Configuration Screen (`/ -> Models`)
| Key | Action | Description |
| :--- | :--- | :--- |
| `↑` / `↓` | **Navigate** | Browse provider list or navigate form fields |
| `Tab` / `Shift+Tab` | **Cycle Fields** | Move focus to next/previous input field |
| `Ctrl+F` | **Fetch Models** | Trigger auto-discovery request to fetch available models from provider |
| `F2` | **Toggle Visibility** | Show or mask the API Key input |
| `Ctrl+S` | **Save & Activate** | Persist credentials and activate the selected model to `~/.operon/config.toml` |

### Permissions Screen (`/ -> Permissions`)
| Key | Action | Description |
| :--- | :--- | :--- |
| `Tab` | **Switch Panel** | Toggle focus between Section tabs, Allowed Directories, and Tools Table |
| `↑` / `↓` | **Select Rule** | Navigate tool groups or directories |
| `Enter` | **Expand / Collapse** | Toggle tool group expansion |
| `Space` | **Edit Rule** | Open interactive `Allow` / `Ask` / `Deny` modal for highlighted tool |
| `+` | **Add Directory** | Open modal to authorize an additional filesystem directory |
| `-` | **Remove Directory** | Remove highlighted directory from the whitelist |

---

## 🚀 Running Operon TUI

### From Repository Root
Using the development launcher scripts:
```bash
# Windows
scripts\run-tui.bat

# Or release build
scripts\run-tui-release.bat
```

### Directly with Cargo
```bash
# Run development binary
cargo run -p operon-tui

# Run optimized release binary
cargo run --release -p operon-tui
```

---

## ⚙️ Configuration

Operon TUI reads and writes to the centralized configuration file at:
```text
~/.operon/config.toml
```

All credentials, model parameters, and permission policies configured within the TUI are automatically synchronized across all frontends (GUI, TUI, and CLI).

---

## 📄 License

Operon TUI is distributed under the **GNU Affero General Public License v3.0 (AGPLv3)**. See [LICENSE](../LICENSE) for full details.
