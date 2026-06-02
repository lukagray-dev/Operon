//! # operon-config
//!
//! Configuration loading for Operon: reads `~/.operon/config.toml`, applies
//! environment variable overrides, creates first-run defaults, and returns
//! strongly-typed [`AppConfig`].
//!
//! ## Usage
//!
//! ```no_run
//! use operon_config::load;
//!
//! fn main() -> Result<(), operon_config::ConfigError> {
//!     let config = load()?;
//!     println!("Provider: {:?}", config.provider.provider);
//!     println!("Workspace: {}", config.paths.workspace_dir.display());
//!     Ok(())
//! }
//! ```
//!
//! ## Config file location
//!
//! ```text
//! ~/.operon/
//! ├── config.toml     ← main config (created with defaults on first run)
//! ├── workspace/      ← default agent workspace (Direction 1, always accessible)
//! │   └── AGENTS.md   ← global agent instructions
//! └── sessions/       ← per-session SQLite databases
//! ```
//!
//! ## Directory model
//!
//! Operon uses a three-directional directory system:
//!
//! 1. **Default workspace** (`~/.operon/workspace/`) — always accessible to the agent.
//!    Injected automatically into the policy regardless of config.toml contents.
//!    The owner cannot remove this from the allowed list.
//!
//! 2. **Allowed directories** — listed in `[[directories]]` in config.toml.
//!    Per-directory, per-role tool permissions (filesystem + shell).
//!
//! 3. **Project directory** — opened VS Code-style at session startup.
//!    Session-scoped, NOT stored in config.toml. Passed to `SessionRunner::new()`
//!    as an optional argument. Permissions start at `Ask` for everything.
//!
//! ## Snapshot / AGENTS.md loading
//!
//! - **Normal open** (no project): snapshot builder uses `~/.operon/workspace/`
//!   as root — loads `workspace/AGENTS.md`, workspace tree, workspace git status.
//! - **Project open**: snapshot builder uses the project directory as root —
//!   loads `<project>/AGENTS.md`, project tree, project git status.
//!   The default workspace AGENTS.md is NOT loaded in this mode.
//!
//! ## Environment variable overrides
//!
//! If `[credentials] api_key` is empty in config.toml, the loader falls back
//! to the provider-specific environment variable:
//!
//! | Provider    | Environment variable |
//! |-------------|----------------------|
//! | anthropic   | `ANTHROPIC_API_KEY`  |
//! | open_ai     | `OPENAI_API_KEY`     |
//! | gemini      | `GEMINI_API_KEY`     |
//! | deep_seek   | `DEEPSEEK_API_KEY`   |
//! | open_router | `OPENROUTER_API_KEY` |
//! | groq        | `GROQ_API_KEY`       |
//! | mistral     | `MISTRAL_API_KEY`    |
//! | xai         | `XAI_API_KEY`        |
//! | cohere      | `COHERE_API_KEY`     |
//! | ollama      | *(no key required)*  |

pub mod error;
mod loader;
pub mod paths;
pub mod schema;

// ─────────────────────────────────────────────────────────────────────────────
// Public re-exports
// ─────────────────────────────────────────────────────────────────────────────

/// Load and validate the full Operon configuration.
///
/// This is the primary entry point. Call once at startup.
/// See [`crate`] module docs for full behavior description.
pub use loader::load;

/// All error variants that can occur during config loading.
pub use error::ConfigError;

/// Platform paths used at runtime (`~/.operon/`, workspace, sessions, etc.).
pub use paths::OperonPaths;

/// The fully resolved, validated runtime configuration.
///
/// Returned by [`load()`]. Contains:
/// - [`AppConfig::provider`] — `ProviderConfig` for HTTP requests
/// - [`AppConfig::policy`]   — `PolicyConfig` for the resolver
/// - [`AppConfig::paths`]    — filesystem paths for snapshot + persistence
pub use schema::AppConfig;
