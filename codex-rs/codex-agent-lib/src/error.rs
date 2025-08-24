//! Error types for the Codex Agent Library

use std::fmt;
use thiserror::Error;

/// Main error type for agent operations
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Tool execution error: {0}")]
    ToolError(String),

    #[error("Model error: {0}")]
    ModelError(String),

    #[error("Agent is already running")]
    AlreadyRunning,

    #[error("Agent is not running")]
    NotRunning,

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("MCP server error: {0}")]
    McpError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error(transparent)]
    CoreError(#[from] codex_core::error::CodexErr),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Error types that can be sent as output messages
#[derive(Debug, Clone)]
pub enum OutputError {
    /// Turn limit exceeded
    TurnLimitExceeded,

    /// Tool execution failed
    ToolError(String),

    /// Model API error
    ModelError(String),

    /// Network error
    NetworkError(String),

    /// Authentication failed
    AuthenticationError(String),

    /// Agent was interrupted
    Interrupted,

    /// Configuration error
    ConfigurationError(String),

    /// Unknown error
    Unknown(String),
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TurnLimitExceeded => write!(f, "Turn limit exceeded"),
            Self::ToolError(msg) => write!(f, "Tool error: {}", msg),
            Self::ModelError(msg) => write!(f, "Model error: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            Self::Interrupted => write!(f, "Agent was interrupted"),
            Self::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            Self::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl From<codex_core::error::CodexErr> for OutputError {
    fn from(err: codex_core::error::CodexErr) -> Self {
        use codex_core::error::CodexErr;
        match err {
            CodexErr::Interrupted => OutputError::Interrupted,
            CodexErr::InternalAgentDied => OutputError::Unknown("Internal agent died".to_string()),
            CodexErr::Stream(msg, _) => OutputError::NetworkError(msg),
            _ => OutputError::Unknown(err.to_string()),
        }
    }
}

/// Result type for agent operations
pub type Result<T> = std::result::Result<T, AgentError>;
