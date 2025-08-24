//! Event handling for TUI applications

#[cfg(feature = "tui")]
use crossterm::event::{self, Event, KeyEvent};
#[cfg(feature = "tui")]
use std::time::Duration;

/// Event types for the TUI
#[cfg(feature = "tui")]
#[derive(Debug, Clone)]
pub enum TuiEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
}

/// Event handler for the TUI
#[cfg(feature = "tui")]
pub struct EventHandler {
    tick_rate: Duration,
}

#[cfg(feature = "tui")]
impl EventHandler {
    /// Create a new event handler with the given tick rate
    pub fn new(tick_rate: Duration) -> Self {
        Self { tick_rate }
    }
    
    /// Poll for the next event
    pub fn next(&self) -> Result<TuiEvent, std::io::Error> {
        if event::poll(self.tick_rate)? {
            match event::read()? {
                Event::Key(key) => Ok(TuiEvent::Key(key)),
                Event::Resize(width, height) => Ok(TuiEvent::Resize(width, height)),
                _ => Ok(TuiEvent::Tick),
            }
        } else {
            Ok(TuiEvent::Tick)
        }
    }
}

#[cfg(feature = "tui")]
impl Default for EventHandler {
    fn default() -> Self {
        Self::new(Duration::from_millis(50))
    }
}