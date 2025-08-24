//! # Codex Agent Library
//!
//! A library for embedding Codex agent capabilities into other Rust applications.
//!
//! This library provides a high-level API for creating and managing AI agents
//! powered by OpenAI models with tool execution capabilities.
//!
//! ## Example
//!
//! ```rust,no_run
//! use codex_agent_lib::{Agent, AgentConfig, OutputData};
//! use tokio::sync::mpsc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AgentConfig::builder()
//!         .model("gpt-5-mini".to_string())
//!         .api_key(std::env::var("OPENAI_API_KEY").ok())
//!         .system_prompt(Some("You are a helpful assistant.".to_string()))
//!         .build();
//!
//!     let agent = Agent::new(config)?;
//!
//!     let (input_tx, input_rx) = mpsc::channel(100);
//!     let (output_tx, mut output_rx) = mpsc::channel(100);
//!
//!     let handle = agent.execute(input_rx, output_tx).await?;
//!
//!     input_tx.send("Hello, how are you?".into()).await?;
//!
//!     while let Some(output) = output_rx.recv().await {
//!         match output.data {
//!             OutputData::Primary(msg) => println!("{}", msg),
//!             OutputData::Completed => break,
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

mod agent;
mod config;
mod error;
mod message;
mod tool;

// Feature-gated modules
#[cfg(feature = "utils")]
pub mod processing;
#[cfg(feature = "utils")]
pub mod utils;

#[cfg(feature = "templates")]
pub mod templates;

#[cfg(feature = "session")]
pub mod session;

#[cfg(feature = "tui")]
pub mod tui;

// Prelude for convenient imports
pub mod prelude;

// Core exports
pub use agent::Agent;
pub use agent::AgentController;
pub use agent::AgentExecutionHandle;
pub use agent::AgentState;
pub use config::AgentConfig;
pub use config::McpServerConfig;
pub use config::SandboxPolicy;
pub use config::ToolConfig;
pub use error::AgentError;
pub use error::OutputError;
pub use error::Result;
pub use message::InputMessage;
pub use message::OutputData;
pub use message::OutputMessage;
pub use message::PlanMessage;
pub use message::PlanMetadata;
pub use message::TodoItem;
pub use message::TodoStatus;
pub use tool::Tool;
pub use tool::ToolCall;
pub use tool::ToolResult;

// Re-export commonly used types
pub use typed_builder::TypedBuilder;
