//! Message types for agent communication

use serde::Deserialize;
use serde::Serialize;

/// Input message sent to the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMessage {
    /// The message content
    pub message: String,

    /// Optional images to include with the message
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub images: Vec<ImageInput>,
}

impl From<String> for InputMessage {
    fn from(message: String) -> Self {
        Self {
            message,
            images: Vec::new(),
        }
    }
}

impl From<&str> for InputMessage {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
            images: Vec::new(),
        }
    }
}

/// Image input for the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageInput {
    /// Base64 encoded image data
    Base64(String),

    /// Path to an image file
    Path(std::path::PathBuf),

    /// URL to an image
    Url(String),
}

/// Output message from the agent
#[derive(Debug, Clone)]
pub struct OutputMessage {
    /// Unique turn ID
    pub turn_id: u64,

    /// The output data
    pub data: OutputData,
}

/// Different types of output data from the agent
#[derive(Debug, Clone)]
pub enum OutputData {
    /// Turn has started
    Start,

    /// Primary message content (e.g., assistant's response)
    Primary(String),

    /// Streaming message delta (partial content)
    PrimaryDelta(String),

    /// Detailed information (e.g., tool execution details)
    Detail(String),

    /// Tool execution started
    ToolStart {
        tool_name: String,
        arguments: serde_json::Value,
    },

    /// Tool execution completed
    ToolComplete { tool_name: String, result: String },

    /// Tool output streaming (e.g., command output)
    ToolOutput { tool_name: String, output: String },

    /// Reasoning content (for models that support reasoning)
    Reasoning(String),

    /// Todo list update
    TodoUpdate { todos: Vec<TodoItem> },

    /// Turn completed successfully
    Completed,

    /// An error occurred
    Error(crate::error::OutputError),
}

/// Represents a todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// The task description
    pub content: String,

    /// The task status
    pub status: TodoStatus,
}

/// Todo item status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    /// Task is pending
    Pending,

    /// Task is in progress
    InProgress,

    /// Task is completed
    Completed,

    /// Task is blocked
    Blocked,
}

/// Plan update message sent through the dedicated plan channel
#[derive(Debug, Clone)]
pub struct PlanMessage {
    /// The updated todo list
    pub todos: Vec<TodoItem>,

    /// Optional metadata about the update
    pub metadata: Option<PlanMetadata>,
}

/// Metadata about a plan update
#[derive(Debug, Clone)]
pub struct PlanMetadata {
    /// The turn ID when this plan was updated
    pub turn_id: u64,

    /// Optional description of the update
    pub description: Option<String>,
}

impl OutputData {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Error(_))
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}
