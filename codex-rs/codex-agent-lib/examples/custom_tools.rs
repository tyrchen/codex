//! Example demonstrating custom tools with the Codex Agent Library

use codex_agent_lib::Agent;
use codex_agent_lib::AgentConfig;
use codex_agent_lib::OutputData;
use codex_agent_lib::PlanMessage;
use codex_agent_lib::ToolConfig;
use std::env;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Define custom tools
    let tools = vec![
        ToolConfig::Bash {
            allow_network: false,
        },
        ToolConfig::WebSearch,
        ToolConfig::FileRead,
        ToolConfig::FileWrite,
        ToolConfig::Custom {
            name: "calculator".to_string(),
            description: "Perform mathematical calculations".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "Mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
            handler: |args| {
                Box::pin(async move {
                    // Simple calculator implementation
                    let expr = args["expression"].as_str().unwrap_or("");
                    // In a real implementation, you'd evaluate the expression
                    Ok(format!("Result of '{}' = 42", expr))
                })
            },
        },
    ];

    // Build agent configuration with custom tools
    let config = AgentConfig::builder()
        .model("gpt-5-mini".to_string())
        .api_key(env::var("OPENAI_API_KEY").ok())
        .system_prompt(Some(
            "You are a helpful assistant with access to various tools including a calculator."
                .to_string(),
        ))
        .tools(tools)
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
                    println!("🚀 Agent started with custom tools");
                }
                OutputData::Primary(message) => {
                    println!("Assistant: {}", message);
                }
                OutputData::ToolStart {
                    tool_name,
                    arguments,
                } => {
                    println!("  🔧 Running {}: {:?}", tool_name, arguments);
                }
                OutputData::ToolComplete { tool_name, result } => {
                    println!("  ✅ {} result: {}", tool_name, result);
                }
                OutputData::Completed => {
                    println!("✨ Turn completed");
                }
                OutputData::Error(err) => {
                    eprintln!("❌ Error: {}", err);
                    break;
                }
                _ => {}
            }
        }
    });

    // Test the calculator tool
    println!("Testing custom calculator tool...");
    input_tx.send("What is 25 * 4 + 10?".into()).await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Test file operations
    println!("\nTesting file operations...");
    input_tx
        .send("Create a file called test.txt with 'Hello from Codex Agent!' content".into())
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Stop the agent
    println!("\nStopping agent...");
    handle.controller().stop().await;
    drop(input_tx);

    // Wait for completion
    handle.join().await?;
    output_task.await?;

    println!("Agent with custom tools stopped successfully");
    Ok(())
}
