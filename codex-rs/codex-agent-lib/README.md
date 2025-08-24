# Codex Agent Library

A Rust library for embedding Codex agent capabilities into your applications. This library provides a high-level API for creating and managing AI agents powered by OpenAI models with tool execution capabilities.

## Features

- **Easy Integration**: Simple API for embedding AI agents into your Rust applications
- **Tool Support**: Built-in tools (bash, web search, file operations) and custom tool support
- **Async/Await**: Fully asynchronous execution with Tokio
- **Control Flow**: Start, stop, pause, and resume agent execution
- **Streaming Output**: Real-time streaming of agent responses and tool executions
- **MCP Support**: Connect to Model Context Protocol servers
- **Sandboxing**: Configurable sandbox policies for safe tool execution
- **Type-Safe**: Strongly typed configuration with builder pattern

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
codex-agent-lib = { path = "../path/to/codex-agent-lib" }
tokio = { version = "1.42", features = ["full"] }
```

## Quick Start

```rust
use codex_agent_lib::{Agent, AgentConfig, OutputData};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the agent
    let config = AgentConfig::builder()
        .model("gpt-5-mini")
        .api_key(std::env::var("OPENAI_API_KEY")?)
        .system_prompt("You are a helpful assistant.")
        .build();

    // Create the agent
    let agent = Agent::new(config)?;

    // Set up communication channels
    let (input_tx, input_rx) = mpsc::channel(100);
    let (output_tx, mut output_rx) = mpsc::channel(100);

    // Start the agent
    let handle = agent.execute(input_rx, output_tx).await?;

    // Send a message
    input_tx.send("Hello, how are you?".into()).await?;

    // Process responses
    while let Some(output) = output_rx.recv().await {
        match output.data {
            OutputData::Primary(msg) => println!("Assistant: {}", msg),
            OutputData::Completed => break,
            _ => {}
        }
    }

    Ok(())
}
```

## Configuration Options

The `AgentConfig` builder supports many options:

```rust
let config = AgentConfig::builder()
    .model("gpt-5-mini")                    // Model to use
    .api_key("sk-...")                      // Optional API key
    .model_provider("openai")               // Provider: openai, azure, ollama
    .system_prompt("Instructions...")       // System instructions
    .tools(vec![...])                       // Available tools
    .mcp_servers(vec![...])                // MCP servers to connect
    .max_turns(100)                        // Maximum conversation turns
    .working_directory("/path/to/dir")    // Working directory
    .sandbox_policy(SandboxPolicy::WorkspaceWrite)  // Sandbox policy
    .approval_policy(ApprovalPolicy::Never)        // Approval policy
    .build();
```

## Custom Tools

You can add custom tools to extend the agent's capabilities:

```rust
use codex_agent_lib::ToolConfig;

let custom_tool = ToolConfig::Custom {
    name: "calculator".to_string(),
    description: "Perform calculations".to_string(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "expression": {
                "type": "string",
                "description": "Math expression"
            }
        }
    }),
    handler: |args| {
        Box::pin(async move {
            // Your tool implementation
            Ok("Result: 42".to_string())
        })
    },
};

let config = AgentConfig::builder()
    .tools(vec![custom_tool])
    .build();
```

## Agent Control

Control the agent during execution:

```rust
// Start the agent
let handle = agent.execute(input_rx, output_tx).await?;

// Get the controller
let controller = handle.controller();

// Check state
let state = controller.state().await;

// Pause execution
controller.pause().await;

// Resume execution
controller.resume().await;

// Stop the agent
controller.stop().await;

// Wait for completion
handle.join().await?;
```

## Output Types

The agent sends various types of output:

```rust
match output.data {
    OutputData::Start => {},                    // Agent started
    OutputData::Primary(msg) => {},             // Main response
    OutputData::Detail(detail) => {},           // Detailed info
    OutputData::ToolStart { .. } => {},        // Tool execution started
    OutputData::ToolComplete { .. } => {},     // Tool execution completed
    OutputData::Reasoning(text) => {},         // Model reasoning (if supported)
    OutputData::Completed => {},               // Turn completed
    OutputData::Error(err) => {},              // Error occurred
}
```

## Examples

See the `examples/` directory for more examples:

- `basic_agent.rs` - Simple agent with basic interaction
- `custom_tools.rs` - Agent with custom tools

Run examples with:

```bash
cargo run --example basic_agent
cargo run --example custom_tools
```

## License

Apache-2.0