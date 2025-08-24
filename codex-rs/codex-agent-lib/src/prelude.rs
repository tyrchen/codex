//! Prelude module for convenient imports

// Core agent types
pub use crate::Agent;
pub use crate::AgentController;
pub use crate::AgentExecutionHandle;
pub use crate::AgentState;

// Configuration
pub use crate::AgentConfig;
pub use crate::McpServerConfig;
pub use crate::SandboxPolicy;
pub use crate::ToolConfig;

// Messages
pub use crate::InputMessage;
pub use crate::OutputData;
pub use crate::OutputMessage;
pub use crate::PlanMessage;
pub use crate::TodoItem;
pub use crate::TodoStatus;

// Error handling
pub use crate::AgentError;
pub use crate::OutputError;
pub use crate::Result;

// Templates (if enabled)
#[cfg(feature = "templates")]
pub use crate::templates::templates;

// Processing (if enabled)
#[cfg(feature = "utils")]
pub use crate::processing::MessageProcessor;
#[cfg(feature = "utils")]
pub use crate::processing::MessageProcessorBuilder;

// Utils (if enabled)
#[cfg(feature = "utils")]
pub use crate::utils::output;

// Session management (if enabled)
#[cfg(feature = "session")]
pub use crate::session::AgentSession;
#[cfg(feature = "session")]
pub use crate::session::MessageHistory;
#[cfg(feature = "session")]
pub use crate::session::SessionMetrics;
#[cfg(feature = "session")]
pub use crate::session::SessionState;

// TUI (if enabled)
#[cfg(feature = "tui")]
pub use crate::tui::AgentTui;
#[cfg(feature = "tui")]
pub use crate::tui::AppState;
#[cfg(feature = "tui")]
pub use crate::tui::Message;
#[cfg(feature = "tui")]
pub use crate::tui::MessageRole;