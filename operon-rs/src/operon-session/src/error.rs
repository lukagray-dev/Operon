// error.rs — Session-level error type for operon-session.
//
// All fallible operations in this crate surface a `SessionError`. Each variant
// wraps a specific subsystem failure so the caller can distinguish recoverable
// warnings from fatal errors, and so `?` propagation works cleanly across
// subsystem boundaries.
//
// Note: HTTP non-2xx responses from the provider are returned as
// `SessionError::Stream` (not `Http`) because `reqwest::Error` does not expose
// a public constructor for status errors. See `http.rs::send_streaming`.

use thiserror::Error;

/// Fatal and non-fatal errors produced by the operon-session agent loop.
///
/// All variants that wrap sub-crate error types use `#[from]` so that `?`
/// conversions are zero-boilerplate at call sites.
#[derive(Debug, Error)]
pub enum SessionError {
    /// Network-level failure returned by reqwest (connection refused, TLS
    /// handshake, timeout, etc.). Distinct from HTTP-level non-2xx errors.
    #[error("Provider HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// SSE stream parse error or HTTP non-2xx response from the provider.
    ///
    /// This variant carries the HTTP status + body text for non-2xx responses
    /// because `reqwest::Error` does not expose a constructor for status errors.
    #[error("Stream parse error: {0}")]
    Stream(String),

    /// Failure while normalizing or denormalizing message/tool wire formats.
    #[error("Message normalization error: {0}")]
    Normalize(String),

    /// Failure returned by the context sanitizer before building the request.
    #[error("Sanitizer error: {0}")]
    Sanitizer(#[from] operon_context_sanitizer::SanitizerError),

    /// Failure returned by the snapshot builder.
    #[error("Snapshot error: {0}")]
    Snapshot(#[from] operon_context_snapshot::SnapshotError),

    /// Failure returned by the compaction pipeline.
    #[error("Compaction error: {0}")]
    Compaction(#[from] operon_context_compaction::CompactionError),

    /// SQLite store failure during session creation or turn persistence.
    #[error("Store error: {0}")]
    Store(String),

    /// Attempted operation on a runner that is not in the correct lifecycle state.
    ///
    /// For example, calling `run()` on a `Done` runner, or `pause()` on an
    /// `Idle` runner.
    #[error("Session is not in a runnable state: {state:?}")]
    InvalidState { state: String },

    /// Configuration-level error during session startup.
    ///
    /// Occurs when the project directory (Direction 3) cannot be canonicalized
    /// by `PolicyConfig::validate()`, e.g. the directory does not exist yet,
    /// or the provider config is inconsistent.
    #[error("Session configuration error: {0}")]
    Config(String),
}
