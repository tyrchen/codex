//! Integration tests for the Codex Agent Library

use codex_agent_lib::Agent;
use codex_agent_lib::AgentConfig;
use codex_agent_lib::AgentState;
use codex_agent_lib::OutputData;
use codex_agent_lib::PlanMessage;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_agent_creation() {
    let config = AgentConfig::builder()
        .model("gpt-5-mini".to_string())
        .build();

    let agent = Agent::new(config);
    assert!(agent.is_ok());
}

#[tokio::test]
async fn test_agent_execution() {
    let config = AgentConfig::builder()
        .model("gpt-5-mini".to_string())
        .max_turns(1)
        .build();

    let agent = Agent::new(config).unwrap();

    // Create channels
    let (_input_tx, input_rx) = mpsc::channel(10);
    let (plan_tx, _plan_rx) = mpsc::channel::<PlanMessage>(10);
    let (output_tx, _output_rx) = mpsc::channel(10);

    // Start the agent
    let handle = agent.execute(input_rx, plan_tx, output_tx).await;
    assert!(handle.is_ok());
    let handle = handle.unwrap();

    // Stop the agent
    handle.controller().stop().await;
}

#[tokio::test]
async fn test_message_conversion() {
    use codex_agent_lib::InputMessage;

    // Test From<String>
    let msg1: InputMessage = "Hello".to_string().into();
    assert_eq!(msg1.message, "Hello");
    assert!(msg1.images.is_empty());

    // Test From<&str>
    let msg2: InputMessage = "World".into();
    assert_eq!(msg2.message, "World");
    assert!(msg2.images.is_empty());
}

#[tokio::test]
async fn test_output_data_helpers() {
    let completed = OutputData::Completed;
    assert!(completed.is_terminal());
    assert!(!completed.is_error());

    let error = OutputData::Error(codex_agent_lib::OutputError::Interrupted);
    assert!(error.is_terminal());
    assert!(error.is_error());

    let primary = OutputData::Primary("test".to_string());
    assert!(!primary.is_terminal());
    assert!(!primary.is_error());
}

#[tokio::test]
async fn test_controller_operations() {
    let config = AgentConfig::builder()
        .model("gpt-5-mini".to_string())
        .build();

    let agent = Agent::new(config).unwrap();

    let (_input_tx, input_rx) = mpsc::channel(10);
    let (plan_tx, _plan_rx) = mpsc::channel::<PlanMessage>(10);
    let (output_tx, _output_rx) = mpsc::channel(10);

    // Start the agent
    let handle = agent.execute(input_rx, plan_tx, output_tx).await.unwrap();

    let controller = handle.controller();

    // Initial turn count should be 0
    assert_eq!(controller.turn_count(), 0);

    // Test state
    assert_eq!(controller.state().await, AgentState::Running);

    // Pause the agent
    controller.pause().await;
    assert_eq!(controller.state().await, AgentState::Paused);

    // Resume the agent
    controller.resume().await;
    assert_eq!(controller.state().await, AgentState::Running);

    // Stop the agent
    controller.stop().await;
}

#[tokio::test]
async fn test_plan_channel() {
    let config = AgentConfig::builder()
        .model("gpt-5-mini".to_string())
        .max_turns(1)
        .build();

    let agent = Agent::new(config).unwrap();

    // Create channels
    let (_input_tx, input_rx) = mpsc::channel(10);
    let (plan_tx, mut plan_rx) = mpsc::channel::<PlanMessage>(10);
    let (output_tx, _output_rx) = mpsc::channel(10);

    // Start the agent
    let handle = agent.execute(input_rx, plan_tx, output_tx).await.unwrap();

    // Test that we can receive on plan channel (with timeout to not hang)
    let plan_result =
        tokio::time::timeout(std::time::Duration::from_millis(100), plan_rx.recv()).await;

    // Even if no plan is sent, the channel should be set up correctly
    assert!(plan_result.is_err() || plan_result.unwrap().is_none());

    // Stop the agent
    handle.controller().stop().await;
}

#[tokio::test]
async fn test_todo_status_serialization() {
    use codex_agent_lib::TodoStatus;

    // Test serialization
    let pending = TodoStatus::Pending;
    let serialized = serde_json::to_string(&pending).unwrap();
    assert_eq!(serialized, "\"pending\"");

    let in_progress = TodoStatus::InProgress;
    let serialized = serde_json::to_string(&in_progress).unwrap();
    assert_eq!(serialized, "\"in_progress\"");

    let completed = TodoStatus::Completed;
    let serialized = serde_json::to_string(&completed).unwrap();
    assert_eq!(serialized, "\"completed\"");

    let blocked = TodoStatus::Blocked;
    let serialized = serde_json::to_string(&blocked).unwrap();
    assert_eq!(serialized, "\"blocked\"");
}
