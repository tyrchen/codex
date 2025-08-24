//! Configuration types for the agent

use std::path::PathBuf;
use typed_builder::TypedBuilder;

/// Type alias for custom tool handler function
pub type CustomToolHandler =
    fn(
        serde_json::Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>;

/// Main configuration for the agent
#[derive(Debug, Clone, TypedBuilder)]
pub struct AgentConfig {
    /// Model to use (e.g., "gpt-5-mini", "o3")
    #[builder(default = "gpt-5-mini".to_string())]
    pub model: String,

    /// API key for authentication
    #[builder(setter(into), default)]
    pub api_key: Option<String>,

    /// Model provider (e.g., "openai", "azure", "ollama")
    #[builder(default = "openai".to_string())]
    pub model_provider: String,

    /// System prompt for the agent
    #[builder(setter(into), default)]
    pub system_prompt: Option<String>,

    /// Base instructions for the agent
    #[builder(setter(into), default)]
    pub base_instructions: Option<String>,

    /// Tools available to the agent
    #[builder(default)]
    pub tools: Vec<ToolConfig>,

    /// MCP servers to connect to
    #[builder(default)]
    pub mcp_servers: Vec<McpServerConfig>,

    /// Maximum number of turns before stopping
    #[builder(default = 100)]
    pub max_turns: usize,

    /// Working directory for the agent
    #[builder(default = std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))]
    pub working_directory: PathBuf,

    /// Enable reasoning mode (for supported models)
    #[builder(default = false)]
    pub enable_reasoning: bool,

    /// Sandbox policy for tool execution
    #[builder(default = SandboxPolicy::WorkspaceWrite)]
    pub sandbox_policy: SandboxPolicy,

    /// Approval policy for tool execution
    #[builder(default = ApprovalPolicy::Never)]
    pub approval_policy: ApprovalPolicy,

    /// Custom Codex home directory
    #[builder(setter(into), default)]
    pub codex_home: Option<PathBuf>,

    /// Disable response storage (for zero data retention)
    #[builder(default = false)]
    pub disable_response_storage: bool,

    /// Show raw agent reasoning (for supported models)
    #[builder(default = false)]
    pub show_raw_reasoning: bool,
}

/// Tool configuration
#[derive(Debug, Clone)]
pub enum ToolConfig {
    /// Built-in bash/shell tool
    Bash {
        /// Whether to allow network access
        allow_network: bool,
    },

    /// Built-in web search tool
    WebSearch,

    /// Built-in file reading tool
    FileRead,

    /// Built-in file writing tool
    FileWrite,

    /// Built-in apply patch tool
    ApplyPatch,

    /// Custom tool with a callback
    Custom {
        name: String,
        description: String,
        parameters: serde_json::Value,
        handler: CustomToolHandler,
    },
}

/// MCP server configuration
#[derive(Debug, Clone, TypedBuilder)]
pub struct McpServerConfig {
    /// Name of the MCP server
    pub name: String,

    /// Command to run the server
    pub command: String,

    /// Arguments for the command
    #[builder(default)]
    pub args: Vec<String>,

    /// Environment variables for the server
    #[builder(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// Sandbox policy for tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPolicy {
    /// No restrictions (dangerous!)
    DangerFullAccess,

    /// Read-only access to filesystem
    ReadOnly,

    /// Read-write access to workspace only
    WorkspaceWrite,
}

impl From<SandboxPolicy> for codex_protocol::config_types::SandboxMode {
    fn from(policy: SandboxPolicy) -> Self {
        match policy {
            SandboxPolicy::DangerFullAccess => {
                codex_protocol::config_types::SandboxMode::DangerFullAccess
            }
            SandboxPolicy::ReadOnly => codex_protocol::config_types::SandboxMode::ReadOnly,
            SandboxPolicy::WorkspaceWrite => {
                codex_protocol::config_types::SandboxMode::WorkspaceWrite
            }
        }
    }
}

/// Approval policy for tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalPolicy {
    /// Never ask for approval (fully autonomous)
    Never,

    /// Ask on failure
    OnFailure,

    /// Let the model decide when to ask
    OnRequest,

    /// Always ask for approval except for safe commands
    UnlessTrusted,
}

impl From<ApprovalPolicy> for codex_protocol::protocol::AskForApproval {
    fn from(policy: ApprovalPolicy) -> Self {
        match policy {
            ApprovalPolicy::Never => codex_protocol::protocol::AskForApproval::Never,
            ApprovalPolicy::OnFailure => codex_protocol::protocol::AskForApproval::OnFailure,
            ApprovalPolicy::OnRequest => codex_protocol::protocol::AskForApproval::OnRequest,
            ApprovalPolicy::UnlessTrusted => {
                codex_protocol::protocol::AskForApproval::UnlessTrusted
            }
        }
    }
}
