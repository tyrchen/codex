//! Tool definitions and interfaces

use serde::Deserialize;
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;

/// Represents a tool that can be called by the agent
#[derive(Debug, Clone)]
pub struct Tool {
    /// Unique name of the tool
    pub name: String,

    /// Description of what the tool does
    pub description: String,

    /// JSON schema for the tool's parameters
    pub parameters: serde_json::Value,

    /// Whether this tool requires approval before execution
    pub requires_approval: bool,
}

/// A tool call request from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// ID of the tool call
    pub id: String,

    /// Name of the tool to call
    pub tool_name: String,

    /// Arguments for the tool
    pub arguments: serde_json::Value,
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution was successful
    pub success: bool,

    /// The output from the tool
    pub output: String,

    /// Optional error message if the tool failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    /// Create a successful tool result
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
        }
    }

    /// Create a failed tool result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(error.into()),
        }
    }
}

/// Trait for custom tool implementations
///
/// This trait allows users to implement custom tools that can be used by the agent.
/// The example handlers below demonstrate how to implement this trait.
#[allow(dead_code)]
pub trait ToolHandler: Send + Sync {
    /// Execute the tool with the given arguments
    fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, crate::error::AgentError>> + Send>>;
}

/// Example: Built-in bash tool handler
///
/// This is an example implementation showing how to create a tool handler.
/// In production, this would integrate with codex_core's bash execution.
#[allow(dead_code)]
pub struct BashToolHandler {
    pub allow_network: bool,
}

#[allow(dead_code)]
impl ToolHandler for BashToolHandler {
    fn execute(
        &self,
        _arguments: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, crate::error::AgentError>> + Send>> {
        Box::pin(async move {
            // Implementation would integrate with codex_core's bash execution
            Ok(ToolResult::success("Bash command executed"))
        })
    }
}

/// Example: Built-in web search tool handler
///
/// This is an example implementation showing how to create a tool handler.
/// In production, this would integrate with web search functionality.
#[allow(dead_code)]
pub struct WebSearchToolHandler;

#[allow(dead_code)]
impl ToolHandler for WebSearchToolHandler {
    fn execute(
        &self,
        _arguments: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, crate::error::AgentError>> + Send>> {
        Box::pin(async move {
            // Implementation would integrate with web search functionality
            Ok(ToolResult::success("Web search completed"))
        })
    }
}
