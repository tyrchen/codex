//! Simple agent example using the new simplified API

use codex_agent_lib::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("codex_agent_lib=debug")
        .init();
    
    // Method 1: Simple query-response
    println!("=== Simple Query Example ===");
    let mut agent = Agent::new(
        AgentConfig::builder()
            .model("gpt-5-mini".to_string())
            .api_key(std::env::var("OPENAI_API_KEY").ok())
            .build()
    )?;
    
    let response = agent.query("What is 2 + 2?").await?;
    println!("Response: {}", response);
    
    // Method 2: Using templates
    #[cfg(feature = "templates")]
    {
        println!("\n=== Template Example ===");
        let mut agent = Agent::from_template(templates::python_developer())?;
        let response = agent.query("Write a hello world in Python").await?;
        println!("Response: {}", response);
    }
    
    // Method 3: Streaming responses
    println!("\n=== Streaming Example ===");
    let agent = Agent::new(
        AgentConfig::builder()
            .model("gpt-5-mini".to_string())
            .api_key(std::env::var("OPENAI_API_KEY").ok())
            .build()
    )?;
    
    let mut stream = Box::pin(agent.stream("Tell me a short joke".to_string()));
    
    use tokio_stream::StreamExt;
    while let Some(result) = stream.next().await {
        match result {
            Ok(msg) => {
                match msg.data {
                    OutputData::Primary(text) | OutputData::PrimaryDelta(text) => {
                        print!("{}", text);
                    }
                    OutputData::Completed => {
                        println!("\n[Stream completed]");
                        break;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }
    
    // Method 4: Interactive session with callback
    println!("\n=== Interactive Example ===");
    let agent = Agent::new(
        AgentConfig::builder()
            .model("gpt-5-mini".to_string())
            .api_key(std::env::var("OPENAI_API_KEY").ok())
            .build()
    )?;
    
    let (input_tx, handle) = agent.interactive(|msg| {
        match msg.data {
            OutputData::Primary(text) => {
                println!("Assistant: {}", text);
            }
            OutputData::Completed => {
                println!("[Session ended]");
                return std::ops::ControlFlow::Break(());
            }
            _ => {}
        }
        std::ops::ControlFlow::Continue(())
    }).await?;
    
    // Send a message
    input_tx.send("What's the weather like?".into()).await
        .map_err(|_| AgentError::ChannelError)?;
    
    // Wait a bit for response
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    // Stop the agent
    handle.controller().stop().await;
    
    Ok(())
}