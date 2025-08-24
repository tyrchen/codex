//! TUI application state and management

#[cfg(feature = "tui")]
use crate::Agent;
#[cfg(feature = "tui")]
use crate::message::OutputData;
#[cfg(feature = "tui")]
use crate::message::TodoItem;
#[cfg(feature = "tui")]
use crate::Result;
#[cfg(feature = "tui")]
use crossterm::event::DisableMouseCapture;
#[cfg(feature = "tui")]
use crossterm::event::EnableMouseCapture;
#[cfg(feature = "tui")]
use crossterm::event::Event;
#[cfg(feature = "tui")]
use crossterm::event::KeyCode;
#[cfg(feature = "tui")]
use crossterm::event::KeyEventKind;
#[cfg(feature = "tui")]
use crossterm::event::{self};
#[cfg(feature = "tui")]
use crossterm::execute;
#[cfg(feature = "tui")]
use crossterm::terminal::EnterAlternateScreen;
#[cfg(feature = "tui")]
use crossterm::terminal::LeaveAlternateScreen;
#[cfg(feature = "tui")]
use crossterm::terminal::disable_raw_mode;
#[cfg(feature = "tui")]
use crossterm::terminal::enable_raw_mode;
#[cfg(feature = "tui")]
use ratatui::Terminal;
#[cfg(feature = "tui")]
use ratatui::backend::CrosstermBackend;
#[cfg(feature = "tui")]
use std::io;
#[cfg(feature = "tui")]
use std::sync::Arc;
#[cfg(feature = "tui")]
use std::sync::Mutex;
#[cfg(feature = "tui")]
use std::time::Duration;
#[cfg(feature = "tui")]
use tokio::sync::mpsc;

/// Message in the chat
#[cfg(feature = "tui")]
#[derive(Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[cfg(feature = "tui")]
#[derive(Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Application state
#[cfg(feature = "tui")]
pub struct AppState {
    /// Input field content
    pub input: String,
    /// Chat history
    pub messages: Vec<Message>,
    /// Todo list
    pub todos: Vec<TodoItem>,
    /// Current agent status
    pub status: String,
    /// Tool output buffer
    pub tool_output: String,
    /// Whether the agent is processing
    pub is_processing: bool,
    /// Custom data
    pub custom_data: Option<Box<dyn std::any::Any + Send + Sync>>,
}

#[cfg(feature = "tui")]
impl AppState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            messages: vec![Message {
                role: MessageRole::System,
                content: "Welcome! I'll help you with Python development. Let me set up the environment...".to_string(),
            }],
            todos: Vec::new(),
            status: "Ready".to_string(),
            tool_output: String::new(),
            is_processing: false,
            custom_data: None,
        }
    }
    
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(Message { role, content });
    }
    
    pub fn update_todos(&mut self, todos: Vec<TodoItem>) {
        self.todos = todos;
    }
    
    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }
    
    pub fn append_tool_output(&mut self, output: String) {
        // Limit total output size to prevent memory issues
        if self.tool_output.len() > 10000 {
            let start = self.tool_output.len().saturating_sub(5000);
            self.tool_output = self.tool_output[start..].to_string();
            self.tool_output.insert_str(0, "... (output truncated) ...\n");
        }
        self.tool_output.push_str(&output);
    }
    
    pub fn clear_tool_output(&mut self) {
        self.tool_output.clear();
    }
}

/// TUI application for running agents
#[cfg(feature = "tui")]
pub struct AgentTui {
    state: Arc<Mutex<AppState>>,
    title: String,
}

#[cfg(feature = "tui")]
impl AgentTui {
    /// Create a new TUI application
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppState::new())),
            title: "Agent TUI".to_string(),
        }
    }
    
    /// Set the title of the application
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
    
    /// Set initial messages
    pub fn with_messages(self, messages: Vec<Message>) -> Self {
        self.state.lock().unwrap().messages = messages;
        self
    }
    
    /// Run the TUI application with the given agent
    pub async fn run(
        &mut self,
        agent: Agent,
        initial_prompt: Option<String>,
    ) -> Result<()> {
        // Setup terminal
        enable_raw_mode().map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)
            .map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?;
        
        // Create channels
        let (input_tx, input_rx) = mpsc::channel(100);
        let (plan_tx, mut plan_rx) = mpsc::channel(100);
        let (output_tx, mut output_rx) = mpsc::channel(100);
        
        // Start the agent
        let handle = agent.execute(input_rx, plan_tx, output_tx).await?;
        let controller = handle.controller().clone();
        
        // Clone state for handlers
        let state_plan_clone = self.state.clone();
        
        // Spawn plan handler
        let _plan_task = tokio::spawn(async move {
            while let Some(plan_msg) = plan_rx.recv().await {
                let mut state = state_plan_clone.lock().unwrap();
                state.update_todos(plan_msg.todos);
            }
        });
        
        // Spawn output handler
        let state_output = self.state.clone();
        tokio::spawn(async move {
            while let Some(output) = output_rx.recv().await {
                let mut state = state_output.lock().unwrap();
                
                match output.data {
                    OutputData::Start => {
                        state.set_status("Agent started".to_string());
                    }
                    OutputData::Primary(msg) => {
                        state.add_message(MessageRole::Assistant, msg);
                        state.clear_tool_output();
                    }
                    OutputData::PrimaryDelta(delta) => {
                        if let Some(last_msg) = state.messages.last_mut() {
                            if last_msg.role == MessageRole::Assistant {
                                last_msg.content.push_str(&delta);
                            }
                        } else {
                            state.add_message(MessageRole::Assistant, delta);
                        }
                    }
                    OutputData::ToolStart { tool_name, arguments } => {
                        state.set_status(format!("Running: {}", tool_name));
                        state.clear_tool_output();
                        
                        if tool_name == "shell" || tool_name == "bash" {
                            if let Some(cmd) = arguments.get("command").and_then(|v| v.as_str()) {
                                let display_cmd = if cmd.len() > 100 {
                                    format!("{}...", &cmd[..100])
                                } else {
                                    cmd.to_string()
                                };
                                state.append_tool_output(format!("$ {}\n", display_cmd));
                            }
                        } else {
                            state.append_tool_output(format!("🔧 {}\n", tool_name));
                        }
                    }
                    OutputData::ToolOutput { output, .. } => {
                        #[cfg(feature = "utils")]
                        let cleaned = crate::utils::output::clean_ansi(&output);
                        #[cfg(not(feature = "utils"))]
                        let cleaned = output;
                        
                        let lines: Vec<&str> = cleaned.lines().take(10).collect();
                        for line in lines {
                            let truncated = if line.len() > 100 {
                                format!("{}...", &line[..100.min(line.len())])
                            } else {
                                line.to_string()
                            };
                            
                            if !truncated.trim().is_empty() {
                                state.append_tool_output(format!("{}\n", truncated));
                            }
                        }
                    }
                    OutputData::ToolComplete { tool_name, .. } => {
                        state.append_tool_output(format!("✓ {} completed\n\n", tool_name));
                    }
                    OutputData::Completed => {
                        state.set_status("Ready".to_string());
                        state.is_processing = false;
                    }
                    OutputData::Error(err) => {
                        state.add_message(MessageRole::System, format!("Error: {:?}", err));
                        state.set_status("Error occurred".to_string());
                        state.is_processing = false;
                    }
                    _ => {}
                }
            }
        });
        
        // Send initial prompt if provided
        if let Some(prompt) = initial_prompt {
            // Add the prompt as a user message to the UI
            self.state.lock().unwrap().add_message(MessageRole::User, prompt.clone());
            self.state.lock().unwrap().is_processing = true;
            self.state.lock().unwrap().set_status("Processing...".to_string());
            
            // Send the prompt to the agent
            input_tx.send(prompt.into()).await
                .map_err(|_| crate::error::AgentError::ChannelError)?;
        }
        
        // Main UI loop
        let state_ui = self.state.clone();
        let title = self.title.clone();
        
        loop {
            // Draw UI
            terminal.draw(|f| {
                let state = state_ui.lock().unwrap();
                crate::tui::components::render_default_layout(f, f.area(), &state, &title);
            }).map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?;
            
            // Handle input
            if event::poll(Duration::from_millis(50))
                .map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?
            {
                if let Event::Key(key) = event::read()
                    .map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?
                {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                                break;
                            }
                            KeyCode::Enter => {
                                let mut state = state_ui.lock().unwrap();
                                if !state.input.is_empty() && !state.is_processing {
                                    let msg = state.input.clone();
                                    state.input.clear();
                                    state.add_message(MessageRole::User, msg.clone());
                                    state.is_processing = true;
                                    state.set_status("Processing...".to_string());
                                    drop(state);
                                    
                                    let input_tx = input_tx.clone();
                                    tokio::spawn(async move {
                                        let _ = input_tx.send(msg.into()).await;
                                    });
                                }
                            }
                            KeyCode::Char(c) => {
                                let mut state = state_ui.lock().unwrap();
                                state.input.push(c);
                            }
                            KeyCode::Backspace => {
                                let mut state = state_ui.lock().unwrap();
                                state.input.pop();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        
        // Stop the agent
        controller.stop().await;
        
        // Restore terminal
        disable_raw_mode().map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        ).map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?;
        terminal.show_cursor()
            .map_err(|e| crate::error::AgentError::InternalError(e.to_string()))?;
        
        Ok(())
    }
}

#[cfg(feature = "tui")]
impl Default for AgentTui {
    fn default() -> Self {
        Self::new()
    }
}