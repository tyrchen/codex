//! Core agent implementation

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::info;

use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::config::Config;
use codex_core::plan_tool::StepStatus;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_login::CodexAuth;

use crate::config::AgentConfig;
use crate::error::AgentError;
use crate::error::OutputError;
use crate::error::Result;
use crate::message::InputMessage;
use crate::message::OutputData;
use crate::message::OutputMessage;
use crate::message::PlanMessage;
use crate::message::PlanMetadata;
use std::ops::ControlFlow;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

/// Current state of the agent
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// Agent is initialized but not running
    Initialized,

    /// Agent is currently running
    Running,

    /// Agent is paused
    Paused,

    /// Agent has stopped
    Stopped,

    /// Agent encountered an error
    Error,
}

/// Handle to control a running agent execution
pub struct AgentExecutionHandle {
    task_handle: JoinHandle<Result<()>>,
    controller: AgentController,
}

impl AgentExecutionHandle {
    /// Wait for the agent to complete
    pub async fn join(self) -> Result<()> {
        self.task_handle
            .await
            .map_err(|e| AgentError::InternalError(e.to_string()))?
    }

    /// Get the controller for this execution
    pub fn controller(&self) -> &AgentController {
        &self.controller
    }
}

/// Controller for managing a running agent
#[derive(Clone)]
pub struct AgentController {
    state: Arc<RwLock<AgentState>>,
    should_stop: Arc<AtomicBool>,
    turn_counter: Arc<AtomicU64>,
}

impl AgentController {
    /// Get the current state of the agent
    pub async fn state(&self) -> AgentState {
        *self.state.read().await
    }

    /// Stop the agent
    pub async fn stop(&self) {
        self.should_stop.store(true, Ordering::SeqCst);
        *self.state.write().await = AgentState::Stopped;
    }

    /// Pause the agent
    pub async fn pause(&self) {
        *self.state.write().await = AgentState::Paused;
    }

    /// Resume the agent
    pub async fn resume(&self) {
        let mut state = self.state.write().await;
        if *state == AgentState::Paused {
            *state = AgentState::Running;
        }
    }

    /// Get the current turn count
    pub fn turn_count(&self) -> u64 {
        self.turn_counter.load(Ordering::SeqCst)
    }
}

/// The main agent struct
#[derive(Clone)]
pub struct Agent {
    pub(crate) config: AgentConfig,
    conversation_manager: Arc<ConversationManager>,
    controller: AgentController,
}

impl Agent {
    /// Create a new agent with the given configuration
    pub fn new(config: AgentConfig) -> Result<Self> {
        let conversation_manager = Arc::new(ConversationManager::default());
        let controller = AgentController {
            state: Arc::new(RwLock::new(AgentState::Initialized)),
            should_stop: Arc::new(AtomicBool::new(false)),
            turn_counter: Arc::new(AtomicU64::new(0)),
        };

        Ok(Self {
            config,
            conversation_manager,
            controller,
        })
    }
    
    /// Create an agent from a template configuration
    #[cfg(feature = "templates")]
    pub fn from_template(config: AgentConfig) -> Result<Self> {
        Self::new(config)
    }
    
    /// Simple request-response pattern - sends a prompt and collects the complete response
    pub async fn query(&mut self, prompt: &str) -> Result<String> {
        let (input_tx, input_rx) = mpsc::channel(1);
        let (plan_tx, _plan_rx) = mpsc::channel(100);
        let (output_tx, mut output_rx) = mpsc::channel(100);
        
        // Clone self for the execution
        let agent = Self {
            config: self.config.clone(),
            conversation_manager: self.conversation_manager.clone(),
            controller: AgentController {
                state: Arc::new(RwLock::new(AgentState::Initialized)),
                should_stop: Arc::new(AtomicBool::new(false)),
                turn_counter: Arc::new(AtomicU64::new(0)),
            },
        };
        
        let handle = agent.execute(input_rx, plan_tx, output_tx).await?;
        
        // Send the prompt
        input_tx.send(prompt.into()).await.map_err(|_| AgentError::ChannelError)?;
        
        // Collect the response
        let mut response = String::new();
        while let Some(output) = output_rx.recv().await {
            match output.data {
                OutputData::Primary(text) | OutputData::PrimaryDelta(text) => {
                    response.push_str(&text);
                }
                OutputData::Completed => break,
                OutputData::Error(err) => return Err(AgentError::OutputError(err)),
                _ => {}
            }
        }
        
        // Stop the agent
        handle.controller().stop().await;
        let _ = handle.join().await;
        
        Ok(response)
    }
    
    /// Interactive session with callback for each message
    pub async fn interactive<F>(
        self,
        mut handler: F,
    ) -> Result<(mpsc::Sender<InputMessage>, AgentExecutionHandle)>
    where
        F: FnMut(OutputMessage) -> ControlFlow<()> + Send + 'static,
    {
        let (input_tx, input_rx) = mpsc::channel(100);
        let (plan_tx, _plan_rx) = mpsc::channel(100);
        let (output_tx, mut output_rx) = mpsc::channel(100);
        
        let handle = self.execute(input_rx, plan_tx, output_tx).await?;
        
        // Spawn handler task
        tokio::spawn(async move {
            while let Some(msg) = output_rx.recv().await {
                if let ControlFlow::Break(()) = handler(msg) {
                    break;
                }
            }
        });
        
        Ok((input_tx, handle))
    }
    
    /// Stream responses as they arrive
    pub fn stream(
        self,
        prompt: String,
    ) -> impl Stream<Item = Result<OutputMessage>> {
        let (input_tx, input_rx) = mpsc::channel(1);
        let (plan_tx, _plan_rx) = mpsc::channel(100);
        let (output_tx, output_rx) = mpsc::channel(100);
        
        // Create the stream
        let stream = tokio_stream::wrappers::ReceiverStream::new(output_rx)
            .map(Ok);
        
        // Start the agent
        tokio::spawn(async move {
            match self.execute(input_rx, plan_tx, output_tx).await {
                Ok(handle) => {
                    // Send the prompt
                    let _ = input_tx.send(prompt.into()).await;
                    
                    // Wait for completion
                    let _ = handle.join().await;
                }
                Err(e) => {
                    error!("Failed to start agent: {}", e);
                }
            }
        });
        
        stream
    }

    /// Execute the agent with the given input, plan, and output channels
    pub async fn execute(
        self,
        input_rx: mpsc::Receiver<InputMessage>,
        plan_tx: mpsc::Sender<PlanMessage>,
        output_tx: mpsc::Sender<OutputMessage>,
    ) -> Result<AgentExecutionHandle> {
        let controller = self.controller.clone();

        // Check if already running
        {
            let mut state = controller.state.write().await;
            if *state == AgentState::Running {
                return Err(AgentError::AlreadyRunning);
            }
            *state = AgentState::Running;
        }

        let task_handle =
            tokio::spawn(async move { self.run_agent_loop(input_rx, plan_tx, output_tx).await });

        Ok(AgentExecutionHandle {
            task_handle,
            controller,
        })
    }

    /// Main agent execution loop
    async fn run_agent_loop(
        self,
        mut input_rx: mpsc::Receiver<InputMessage>,
        plan_tx: mpsc::Sender<PlanMessage>,
        output_tx: mpsc::Sender<OutputMessage>,
    ) -> Result<()> {
        // Convert AgentConfig to codex_core::config::Config
        let core_config = self.build_core_config()?;

        // Create a new conversation
        let auth = if let Some(api_key) = &self.config.api_key {
            Some(CodexAuth::from_api_key(api_key))
        } else {
            CodexAuth::from_codex_home(&core_config.codex_home, core_config.preferred_auth_method)
                .ok()
                .flatten()
        };

        let NewConversation {
            conversation_id,
            conversation,
            session_configured: _,
        } = self
            .conversation_manager
            .new_conversation_with_auth(core_config, auth)
            .await
            .map_err(AgentError::CoreError)?;

        info!("Started conversation {}", conversation_id);

        // Start the event processing task
        let conversation_clone = conversation.clone();
        let plan_tx_clone = plan_tx.clone();
        let output_tx_clone = output_tx.clone();
        let controller_clone = self.controller.clone();

        let event_task = tokio::spawn(async move {
            Self::process_events(
                conversation_clone,
                plan_tx_clone,
                output_tx_clone,
                controller_clone,
            )
            .await
        });

        // Process input messages
        while let Some(input_msg) = input_rx.recv().await {
            // Check if we should stop
            if self.controller.should_stop.load(Ordering::SeqCst) {
                break;
            }

            // Check if paused
            while *self.controller.state.read().await == AgentState::Paused {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                if self.controller.should_stop.load(Ordering::SeqCst) {
                    break;
                }
            }

            // Check turn limit
            let turn_count = self.controller.turn_counter.load(Ordering::SeqCst);
            if turn_count >= self.config.max_turns as u64 {
                let _ = output_tx
                    .send(OutputMessage {
                        turn_id: turn_count,
                        data: OutputData::Error(OutputError::TurnLimitExceeded),
                    })
                    .await;
                break;
            }

            // Submit the input to the conversation
            let input_items = vec![InputItem::Text {
                text: input_msg.message,
            }];

            let op = Op::UserInput { items: input_items };

            if let Err(e) = conversation.submit(op).await {
                error!("Failed to submit input: {}", e);
                let _ = output_tx
                    .send(OutputMessage {
                        turn_id: turn_count,
                        data: OutputData::Error(OutputError::from(e)),
                    })
                    .await;
            }

            // Increment turn counter
            self.controller.turn_counter.fetch_add(1, Ordering::SeqCst);
        }

        // Send shutdown signal
        let _ = conversation.submit(Op::Shutdown).await;

        // Wait for event task to complete
        let _ = event_task.await;

        // Update state
        *self.controller.state.write().await = AgentState::Stopped;

        Ok(())
    }

    /// Process events from the conversation
    async fn process_events(
        conversation: Arc<codex_core::CodexConversation>,
        plan_tx: mpsc::Sender<PlanMessage>,
        output_tx: mpsc::Sender<OutputMessage>,
        controller: AgentController,
    ) -> Result<()> {
        let mut current_turn_id = 0u64;

        loop {
            // Check if we should stop
            if controller.should_stop.load(Ordering::SeqCst) {
                break;
            }

            // Get next event
            let event = match conversation.next_event().await {
                Ok(event) => event,
                Err(e) => {
                    error!("Failed to get next event: {}", e);
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::Error(OutputError::from(e)),
                        })
                        .await;
                    break;
                }
            };

            // Process the event
            match event.msg {
                EventMsg::AgentMessage(msg) => {
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::Primary(msg.message),
                        })
                        .await;
                }

                EventMsg::AgentMessageDelta(delta) => {
                    // Send streaming delta
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::PrimaryDelta(delta.delta),
                        })
                        .await;
                }

                EventMsg::AgentReasoning(reasoning) => {
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::Reasoning(reasoning.text),
                        })
                        .await;
                }

                EventMsg::McpToolCallBegin(tool_call) => {
                    // Check if this is update_plan and send PlanMessage immediately
                    if tool_call.invocation.tool == "update_plan"
                        && let Some(args) = &tool_call.invocation.arguments
                        && let Some(plan_array) = args.get("plan").and_then(|v| v.as_array())
                    {
                        let todos: Vec<crate::message::TodoItem> = plan_array
                            .iter()
                            .filter_map(|item| {
                                let step = item.get("step")?.as_str()?;
                                let status = item.get("status")?.as_str()?;
                                Some(crate::message::TodoItem {
                                    content: step.to_string(),
                                    status: match status {
                                        "pending" => crate::message::TodoStatus::Pending,
                                        "in_progress" => crate::message::TodoStatus::InProgress,
                                        "completed" => crate::message::TodoStatus::Completed,
                                        _ => crate::message::TodoStatus::Pending,
                                    },
                                })
                            })
                            .collect();

                        let _ = plan_tx
                            .send(PlanMessage {
                                todos,
                                metadata: Some(PlanMetadata {
                                    turn_id: current_turn_id,
                                    description: Some(
                                        "Plan updated via update_plan tool".to_string(),
                                    ),
                                }),
                            })
                            .await;
                    }

                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::ToolStart {
                                tool_name: tool_call.invocation.tool.clone(),
                                arguments: tool_call
                                    .invocation
                                    .arguments
                                    .clone()
                                    .unwrap_or_default(),
                            },
                        })
                        .await;
                }

                EventMsg::McpToolCallEnd(tool_call) => {
                    let result = match &tool_call.result {
                        Ok(result) => {
                            // Extract text from ContentBlock
                            let text =
                                if let Some(mcp_types::ContentBlock::TextContent(text_content)) =
                                    result.content.first()
                                {
                                    text_content.text.clone()
                                } else {
                                    String::new()
                                };

                            // Check if this is a update_plan tool
                            if tool_call.invocation.tool == "update_plan" {
                                // Parse plan from the arguments (not the result)
                                if let Some(args) = &tool_call.invocation.arguments
                                    && let Some(plan_array) =
                                        args.get("plan").and_then(|v| v.as_array())
                                {
                                    let todos: Vec<crate::message::TodoItem> = plan_array
                                        .iter()
                                        .filter_map(|item| {
                                            Some(crate::message::TodoItem {
                                                content: item.get("step")?.as_str()?.to_string(),
                                                status: match item.get("status")?.as_str()? {
                                                    "pending" => {
                                                        crate::message::TodoStatus::Pending
                                                    }
                                                    "in_progress" => {
                                                        crate::message::TodoStatus::InProgress
                                                    }
                                                    "completed" => {
                                                        crate::message::TodoStatus::Completed
                                                    }
                                                    _ => crate::message::TodoStatus::Pending,
                                                },
                                            })
                                        })
                                        .collect();

                                    let _ = plan_tx
                                        .send(PlanMessage {
                                            todos,
                                            metadata: Some(PlanMetadata {
                                                turn_id: current_turn_id,
                                                description: Some(
                                                    "Plan completed via update_plan tool"
                                                        .to_string(),
                                                ),
                                            }),
                                        })
                                        .await;
                                }
                            }
                            text
                        }
                        Err(e) => format!("Error: {}", e),
                    };

                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::ToolComplete {
                                tool_name: tool_call.invocation.tool.clone(),
                                result,
                            },
                        })
                        .await;
                }

                EventMsg::ExecCommandBegin(exec) => {
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::ToolStart {
                                tool_name: "bash".to_string(),
                                arguments: serde_json::json!({ "command": exec.command }),
                            },
                        })
                        .await;
                }

                EventMsg::ExecCommandOutputDelta(output) => {
                    // Convert ByteBuf to String (best effort, may contain invalid UTF-8)
                    let output_str = String::from_utf8_lossy(&output.chunk).to_string();
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::ToolOutput {
                                tool_name: "bash".to_string(),
                                output: output_str,
                            },
                        })
                        .await;
                }

                EventMsg::ExecCommandEnd(exec) => {
                    let result = format!("Exit code: {}", exec.exit_code);
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::ToolComplete {
                                tool_name: "bash".to_string(),
                                result,
                            },
                        })
                        .await;
                }

                EventMsg::TaskComplete(_) => {
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::Completed,
                        })
                        .await;
                    current_turn_id += 1;
                }

                EventMsg::Error(err) => {
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::Error(OutputError::Unknown(err.message)),
                        })
                        .await;
                }

                EventMsg::TurnAborted(_abort) => {
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::Error(OutputError::Interrupted),
                        })
                        .await;
                }

                EventMsg::SessionConfigured(_) => {
                    // Session start - send a start message
                    let _ = output_tx
                        .send(OutputMessage {
                            turn_id: current_turn_id,
                            data: OutputData::Start,
                        })
                        .await;
                }

                EventMsg::PlanUpdate(plan_update) => {
                    // Convert Codex's internal plan update to our PlanMessage
                    let todos: Vec<crate::message::TodoItem> = plan_update
                        .plan
                        .iter()
                        .map(|item| crate::message::TodoItem {
                            content: item.step.clone(),
                            status: match item.status {
                                StepStatus::Pending => crate::message::TodoStatus::Pending,
                                StepStatus::InProgress => crate::message::TodoStatus::InProgress,
                                StepStatus::Completed => crate::message::TodoStatus::Completed,
                            },
                        })
                        .collect();

                    let _ = plan_tx
                        .send(PlanMessage {
                            todos,
                            metadata: Some(PlanMetadata {
                                turn_id: current_turn_id,
                                description: plan_update.explanation,
                            }),
                        })
                        .await;
                }

                _ => {
                    // Other events we can ignore or log
                    debug!("Received event: {:?}", event);
                }
            }
        }

        Ok(())
    }

    /// Build core config from agent config
    fn build_core_config(&self) -> Result<Config> {
        // Build overrides for Config
        let overrides = codex_core::config::ConfigOverrides {
            model: Some(self.config.model.clone()),
            model_provider: Some(self.config.model_provider.clone()),
            cwd: Some(self.config.working_directory.clone()),
            approval_policy: Some(self.config.approval_policy.into()),
            sandbox_mode: Some(self.config.sandbox_policy.into()),
            disable_response_storage: Some(self.config.disable_response_storage),
            base_instructions: self.config.system_prompt.clone(),
            include_plan_tool: Some(true), // Enable plan tool for task tracking
            // Always enable apply_patch tool for file operations
            include_apply_patch_tool: Some(true),
            codex_linux_sandbox_exe: None,
            config_profile: None,
            show_raw_agent_reasoning: Some(self.config.show_raw_reasoning),
        };

        // Load config with overrides
        let config = Config::load_with_cli_overrides(Vec::new(), overrides)
            .map_err(|e| AgentError::ConfigError(e.to_string()))?;

        Ok(config)
    }
}
