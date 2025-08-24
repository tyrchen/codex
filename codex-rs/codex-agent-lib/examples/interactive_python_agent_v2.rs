//! Interactive Python Agent - Simplified Version
//!
//! This example shows how the original 700+ line interactive_python_agent.rs
//! can be reduced to just a few lines using the new library improvements.

#[cfg(all(feature = "tui", feature = "templates"))]
use codex_agent_lib::prelude::*;

#[cfg(all(feature = "tui", feature = "templates"))]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to file
    if let Ok(log_file) = std::fs::File::create("interactive_python_agent_v2.log") {
        tracing_subscriber::fmt()
            .with_env_filter("codex_agent_lib=debug")
            .with_writer(log_file)
            .with_ansi(false)
            .init();
    }
    
    // Create Python developer agent from template
    let agent = Agent::from_template(templates::python_developer())?;
    
    // Run interactive TUI with initial setup prompt
    AgentTui::new()
        .with_title("Python Development Assistant")
        .run(
            agent,
            Some("Please set up a Python environment using uv. First check if uv is installed, then initialize a project with uv init, create a virtual environment with uv venv. Then create a simple hello.py script that calculates and prints the first 20 prime numbers, and run it using 'uv run python hello.py' to verify everything works.".to_string())
        )
        .await?;
    
    Ok(())
}

#[cfg(not(all(feature = "tui", feature = "templates")))]
fn main() {
    eprintln!("This example requires both 'tui' and 'templates' features. Run with:");
    eprintln!("cargo run --example interactive_python_agent_v2 --features tui,templates");
}

// Original: 700+ lines of code
// New version: ~30 lines of code
// Reduction: 95%+ in code size
//
// Benefits:
// 1. No manual TUI implementation needed
// 2. No manual message handling
// 3. No manual state management
// 4. Pre-configured Python developer prompt
// 5. Built-in output processing and formatting
// 6. Automatic todo list tracking
// 7. Clean separation of concerns