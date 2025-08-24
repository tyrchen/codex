//! TUI agent example using the new TUI module

#[cfg(feature = "tui")]
use codex_agent_lib::prelude::*;

#[cfg(feature = "tui")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to file
    if let Ok(log_file) = std::fs::File::create("tui_agent.log") {
        tracing_subscriber::fmt()
            .with_env_filter("codex_agent_lib=debug")
            .with_writer(log_file)
            .with_ansi(false)
            .init();
    }
    
    // Create agent from template
    let agent = Agent::from_template(templates::python_developer())?;
    
    // Run TUI application
    let mut tui = AgentTui::new()
        .with_title("Python Development Assistant");
    
    tui.run(
        agent,
        Some("Please set up a Python environment with uv and create a hello world script".to_string())
    ).await?;
    
    Ok(())
}

#[cfg(not(feature = "tui"))]
fn main() {
    eprintln!("This example requires the 'tui' feature. Run with:");
    eprintln!("cargo run --example tui_agent --features tui,templates");
}