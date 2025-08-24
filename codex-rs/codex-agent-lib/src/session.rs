//! Session management for agents

#[cfg(feature = "session")]
use crate::Agent;
#[cfg(feature = "session")]
use crate::Result;
#[cfg(feature = "session")]
use crate::error::AgentError;
#[cfg(feature = "session")]
use crate::message::InputMessage;
#[cfg(feature = "session")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "session")]
use std::collections::VecDeque;
#[cfg(feature = "session")]
use std::path::Path;
#[cfg(feature = "session")]
use std::sync::Arc;
#[cfg(feature = "session")]
use tokio::sync::RwLock;
#[cfg(feature = "session")]
use tokio::sync::mpsc;

/// Session state that can be saved and loaded
#[cfg(feature = "session")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Message history
    pub messages: Vec<SerializedMessage>,
    /// Current turn count
    pub turn_count: u64,
    /// Session metadata
    pub metadata: SessionMetadata,
}

/// Serializable message format
#[cfg(feature = "session")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMessage {
    pub role: String,
    pub content: String,
    pub timestamp: u64,
}

/// Session metadata
#[cfg(feature = "session")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Session ID
    pub session_id: String,
    /// Creation timestamp
    pub created_at: u64,
    /// Last updated timestamp
    pub updated_at: u64,
    /// Model used
    pub model: String,
    /// Custom metadata
    pub custom: serde_json::Value,
}

/// Message history manager
#[cfg(feature = "session")]
pub struct MessageHistory {
    messages: VecDeque<SerializedMessage>,
    max_size: usize,
}

#[cfg(feature = "session")]
impl MessageHistory {
    /// Create a new message history with the given max size
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    /// Add a message to the history
    pub fn add(&mut self, role: String, content: String) {
        if self.messages.len() >= self.max_size {
            self.messages.pop_front();
        }
        
        self.messages.push_back(SerializedMessage {
            role,
            content,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
    }
    
    /// Get all messages
    pub fn get_all(&self) -> Vec<SerializedMessage> {
        self.messages.iter().cloned().collect()
    }
    
    /// Clear the history
    pub fn clear(&mut self) {
        self.messages.clear();
    }
    
    /// Get the number of messages
    pub fn len(&self) -> usize {
        self.messages.len()
    }
    
    /// Check if the history is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Session metrics
#[cfg(feature = "session")]
#[derive(Debug, Clone, Default)]
pub struct SessionMetrics {
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Total tokens used (estimated)
    pub tokens_used: u64,
    /// Total tool calls
    pub tool_calls: u64,
    /// Total errors
    pub errors: u64,
    /// Session duration in seconds
    pub duration_secs: u64,
}

/// Agent session with state management
#[cfg(feature = "session")]
pub struct AgentSession {
    agent: Agent,
    state: Arc<RwLock<SessionState>>,
    history: Arc<RwLock<MessageHistory>>,
    metrics: Arc<RwLock<SessionMetrics>>,
    input_tx: Option<mpsc::Sender<InputMessage>>,
    handle: Option<crate::agent::AgentExecutionHandle>,
}

#[cfg(feature = "session")]
impl AgentSession {
    /// Create a new session with the given agent
    pub fn new(agent: Agent) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let state = SessionState {
            messages: Vec::new(),
            turn_count: 0,
            metadata: SessionMetadata {
                session_id,
                created_at: now,
                updated_at: now,
                model: agent.config.model.clone(),
                custom: serde_json::Value::Object(serde_json::Map::new()),
            },
        };
        
        Self {
            agent,
            state: Arc::new(RwLock::new(state)),
            history: Arc::new(RwLock::new(MessageHistory::new(1000))),
            metrics: Arc::new(RwLock::new(SessionMetrics::default())),
            input_tx: None,
            handle: None,
        }
    }
    
    /// Start the session
    pub async fn start(&mut self) -> Result<()> {
        if self.handle.is_some() {
            return Err(AgentError::AlreadyRunning);
        }
        
        let (input_tx, input_rx) = mpsc::channel(100);
        let (plan_tx, mut plan_rx) = mpsc::channel(100);
        let (output_tx, mut output_rx) = mpsc::channel::<crate::message::OutputMessage>(100);
        
        // Clone for handlers
        let history = self.history.clone();
        let metrics = self.metrics.clone();
        let state = self.state.clone();
        
        // Spawn output handler
        tokio::spawn(async move {
            while let Some(output) = output_rx.recv().await {
                let mut metrics = metrics.write().await;
                metrics.messages_received += 1;
                
                match &output.data {
                    crate::message::OutputData::Primary(text) => {
                        let mut history = history.write().await;
                        history.add("assistant".to_string(), text.clone());
                        
                        let mut state = state.write().await;
                        state.messages.push(SerializedMessage {
                            role: "assistant".to_string(),
                            content: text.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        });
                    }
                    crate::message::OutputData::ToolStart { .. } => {
                        metrics.tool_calls += 1;
                    }
                    crate::message::OutputData::Error(_) => {
                        metrics.errors += 1;
                    }
                    _ => {}
                }
            }
        });
        
        // Spawn plan handler (just consume for now)
        tokio::spawn(async move {
            while let Some(_plan) = plan_rx.recv().await {
                // Could store plan state here if needed
            }
        });
        
        let handle = self.agent.clone().execute(input_rx, plan_tx, output_tx).await?;
        self.input_tx = Some(input_tx);
        self.handle = Some(handle);
        
        Ok(())
    }
    
    /// Send a message to the agent
    pub async fn send(&mut self, message: String) -> Result<()> {
        if let Some(tx) = &self.input_tx {
            // Update history
            {
                let mut history = self.history.write().await;
                history.add("user".to_string(), message.clone());
            }
            
            // Update metrics
            {
                let mut metrics = self.metrics.write().await;
                metrics.messages_sent += 1;
            }
            
            // Update state
            {
                let mut state = self.state.write().await;
                state.messages.push(SerializedMessage {
                    role: "user".to_string(),
                    content: message.clone(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                });
                state.turn_count += 1;
                state.metadata.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            }
            
            tx.send(message.into()).await
                .map_err(|_| AgentError::ChannelError)?;
            Ok(())
        } else {
            Err(AgentError::NotRunning)
        }
    }
    
    /// Get the message history
    pub async fn get_history(&self) -> Vec<SerializedMessage> {
        self.history.read().await.get_all()
    }
    
    /// Get the session metrics
    pub async fn get_metrics(&self) -> SessionMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Save the session to a file
    pub async fn save_session(&self, path: &Path) -> Result<()> {
        let state = self.state.read().await.clone();
        let json = serde_json::to_string_pretty(&state)
            .map_err(|e| AgentError::InternalError(e.to_string()))?;
        
        tokio::fs::write(path, json).await
            .map_err(|e| AgentError::InternalError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Load a session from a file
    pub async fn load_session(path: &Path, agent: Agent) -> Result<Self> {
        let json = tokio::fs::read_to_string(path).await
            .map_err(|e| AgentError::InternalError(e.to_string()))?;
        
        let state: SessionState = serde_json::from_str(&json)
            .map_err(|e| AgentError::InternalError(e.to_string()))?;
        
        let mut history = MessageHistory::new(1000);
        for msg in &state.messages {
            history.add(msg.role.clone(), msg.content.clone());
        }
        
        Ok(Self {
            agent,
            state: Arc::new(RwLock::new(state)),
            history: Arc::new(RwLock::new(history)),
            metrics: Arc::new(RwLock::new(SessionMetrics::default())),
            input_tx: None,
            handle: None,
        })
    }
    
    /// Stop the session
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.handle.take() {
            handle.controller().stop().await;
            let _ = handle.join().await;
        }
        self.input_tx = None;
        Ok(())
    }
}