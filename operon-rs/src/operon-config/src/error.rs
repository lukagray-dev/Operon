// error.rs — Error types for operon-config.
//
// All errors returned from `load()` and related public functions use this type.
// Each variant tells the caller exactly what went wrong and where, without
// requiring them to downcast from a generic anyhow::Error.
//
// The `#[from]` derive on Io and PolicyValidation means callers can use `?`
// when those foreign errors appear — the conversions are generated automatically.

use thiserror::Error;

/// All errors that can occur during config loading or path resolution.
#[derive(Debug, Error)]
pub enum ConfigError {
    // ── Filesystem / environment ───────────────────────────────────────────────
    /// `dirs::home_dir()` returned `None` — running in a context without a HOME.
    ///
    /// This is extremely rare (headless CI, certain containers). The user must
    /// set HOME or USERPROFILE explicitly in that environment.
    #[error(
        "cannot determine the home directory; set the HOME or USERPROFILE environment variable"
    )]
    NoHomeDir,

    /// Any I/O error — file not found, permission denied, disk full, etc.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // ── TOML parsing ──────────────────────────────────────────────────────────
    /// The config file exists but contains invalid TOML or doesn't match the schema.
    ///
    /// `path` is the absolute path to the file, `source` is the toml parse error.
    /// The error message from toml includes the line/column of the problem.
    #[error("could not parse config file at {path}: {source}")]
    TomlParse {
        path: String,
        source: toml::de::Error,
    },

    // ── Provider / credentials ────────────────────────────────────────────────
    /// The `[provider]` section contains an unrecognised provider name.
    ///
    /// `name` is the literal string from the config, `valid` lists the accepted
    /// values so the user knows exactly what to fix.
    #[error("unknown provider '{name}' in config; valid values are: {valid}")]
    UnknownProvider { name: String, valid: String },

    /// No API key was found for the configured provider.
    ///
    /// The key may come from the `[credentials]` section or from the env var
    /// named in `env_var`. If both are empty/absent, this error is returned.
    ///
    /// Ollama never triggers this — it doesn't require an API key.
    #[error(
        "no API key for provider '{provider}': set the {env_var} environment variable or add it to [credentials] in config.toml"
    )]
    MissingApiKey { provider: String, env_var: String },

    // ── Policy validation ─────────────────────────────────────────────────────
    /// A directory path listed in `[[directories]]` could not be canonicalized.
    ///
    /// This wraps `crate::policy::PolicyError`, which carries the path and the
    /// OS error in its own message.
    #[error("policy validation failed: {0}")]
    PolicyValidation(#[from] crate::policy::PolicyError),

    // ── Internal consistency ──────────────────────────────────────────────────
    /// A logic error that should not occur in correct code — indicates a bug.
    /// Used for unreachable branches guarded by invariants we maintain.
    #[error("internal config error: {0}")]
    Internal(String),
}
