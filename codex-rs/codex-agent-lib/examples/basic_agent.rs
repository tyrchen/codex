//! Basic example of using the Codex Agent Library

use codex_agent_lib::Agent;
use codex_agent_lib::AgentConfig;
use codex_agent_lib::OutputData;
use codex_agent_lib::PlanMessage;
use std::env;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Build agent configuration
    let config = AgentConfig::builder()
        .model("gpt-5-mini".to_string())
        .api_key(env::var("OPENAI_API_KEY").ok())
        .system_prompt(Some("You are a helpful assistant that can execute commands and help with programming tasks.".to_string()))
        .max_turns(10)
        .build();

    // Create the agent
    let agent = Agent::new(config)?;

    // Create channels for communication
    let (input_tx, input_rx) = mpsc::channel(100);
    let (plan_tx, mut plan_rx) = mpsc::channel::<PlanMessage>(100);
    let (output_tx, mut output_rx) = mpsc::channel(100);

    // Start the agent execution
    let handle = agent.execute(input_rx, plan_tx, output_tx).await?;

    // Spawn a task to handle plan updates
    let plan_task = tokio::spawn(async move {
        while let Some(plan_msg) = plan_rx.recv().await {
            println!("📋 Plan updated: {} tasks", plan_msg.todos.len());
            for todo in &plan_msg.todos {
                println!(
                    "  - [{}] {}",
                    match todo.status {
                        codex_agent_lib::TodoStatus::Pending => "⏳",
                        codex_agent_lib::TodoStatus::InProgress => "🔄",
                        codex_agent_lib::TodoStatus::Completed => "✅",
                        codex_agent_lib::TodoStatus::Blocked => "🚧",
                    },
                    todo.content
                );
            }
        }
    });

    // Spawn a task to handle output
    let output_task = tokio::spawn(async move {
        while let Some(output) = output_rx.recv().await {
            match output.data {
                OutputData::Start => {
                    println!("🚀 Agent started");
                }
                OutputData::Primary(message) => {
                    println!("Assistant: {}", message);
                }
                OutputData::PrimaryDelta(delta) => {
                    print!("{}", delta); // Stream output without newline
                }
                OutputData::Detail(detail) => {
                    println!("  Detail: {}", detail);
                }
                OutputData::ToolStart { tool_name, .. } => {
                    println!("  🔧 Running tool: {}", tool_name);
                }
                OutputData::ToolOutput { tool_name, output } => {
                    println!("  📝 [{}]: {}", tool_name, output);
                }
                OutputData::ToolComplete { tool_name, result } => {
                    println!("  ✅ Tool {} completed: {}", tool_name, result);
                }
                OutputData::TodoUpdate { todos } => {
                    println!("  📋 Todo list updated: {} items", todos.len());
                }
                OutputData::Reasoning(reasoning) => {
                    println!("  💭 Reasoning: {}", reasoning);
                }
                OutputData::Completed => {
                    println!("✨ Turn completed");
                }
                OutputData::Error(err) => {
                    eprintln!("❌ Error: {}", err);
                    break;
                }
            }
        }
    });

    // Send some messages
    println!("Sending message to agent...");
    input_tx
        .send("Hello! Can you explain what you can do?".into())
        .await?;

    // Wait a bit for response
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Send another message
    println!("\nSending another message...");
    input_tx
        .send("Can you show me the current directory?".into())
        .await?;

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Stop the agent
    println!("\nStopping agent...");
    handle.controller().stop().await;

    // Drop the input sender to signal completion
    drop(input_tx);

    // Wait for the agent to finish
    handle.join().await?;
    output_task.await?;

    println!("Agent stopped successfully");
    Ok(())
}
