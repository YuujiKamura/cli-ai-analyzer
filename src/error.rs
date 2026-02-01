//! Error types for gemini-analyzer

use std::path::PathBuf;
use thiserror::Error;

/// Result type for gemini-analyzer
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for gemini-analyzer
#[derive(Error, Debug)]
pub enum Error {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Process execution error
    #[error("Process error: {0}")]
    Process(String),

    /// Invalid file path
    #[error("Invalid file path: {0}")]
    InvalidPath(PathBuf),

    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Gemini CLI not found
    #[error("Gemini CLI not found. Install it with: npm install -g @anthropic/gemini-cli")]
    GeminiCliNotFound,

    /// Gemini CLI authentication error
    #[error("Gemini CLI authentication failed. Run: gemini auth")]
    AuthenticationFailed,

    /// Gemini CLI execution error
    #[error("Gemini CLI error: {0}")]
    GeminiError(String),

    /// Timeout error
    #[error("Operation timed out")]
    Timeout,
}

impl Error {
    /// Create a process error from a string
    pub fn process(msg: impl Into<String>) -> Self {
        Error::Process(msg.into())
    }

    /// Create a Gemini error from a string
    pub fn gemini(msg: impl Into<String>) -> Self {
        Error::GeminiError(msg.into())
    }
}
