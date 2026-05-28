use thiserror::Error;

#[derive(Debug, Error)]
pub enum SanitizerError {
    #[error("Failed to serialize message content: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Message array is empty")]
    EmptyMessages,
}
