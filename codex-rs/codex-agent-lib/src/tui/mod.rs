//! TUI components for building interactive agent applications

#[cfg(feature = "tui")]
pub mod app;

#[cfg(feature = "tui")]
pub mod components;

#[cfg(feature = "tui")]
pub mod event;

#[cfg(feature = "tui")]
pub use app::AgentTui;

#[cfg(feature = "tui")]
pub use app::AppState;

#[cfg(feature = "tui")]
pub use app::Message;

#[cfg(feature = "tui")]
pub use app::MessageRole;