//! Interactive Python Agent Example
//!
//! This example demonstrates an interactive TUI agent that:
//! 1. Sets up a Python environment using uv
//! 2. Accepts user input for Python code generation
//! 3. Executes the generated code and displays results
//! 4. Shows real-time todo list and output updates

use codex_agent_lib::Agent;
use codex_agent_lib::AgentConfig;
use codex_agent_lib::OutputData;
use codex_agent_lib::PlanMessage;
use codex_agent_lib::SandboxPolicy;
use codex_agent_lib::TodoItem;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableMouseCapture;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::error::Error;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;

/// Application state
struct App {
    /// Input field content
    input: String,
    /// Chat history
    messages: Vec<Message>,
    /// Todo list
    todos: Vec<TodoItem>,
    /// Current agent status
    status: String,
    /// Tool output buffer
    tool_output: String,
    /// Whether the agent is processing
    is_processing: bool,
}

/// Message in the chat
#[derive(Clone)]
struct Message {
    role: MessageRole,
    content: String,
}

#[derive(Clone, PartialEq)]
enum MessageRole {
    User,
    Assistant,
    System,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            messages: vec![Message {
                role: MessageRole::System,
                content: "Welcome! I'll help you write and execute Python code. Let me set up the environment first...".to_string(),
            }],
            todos: Vec::new(),
            status: "Initializing...".to_string(),
            tool_output: String::new(),
            is_processing: false,
        }
    }

    fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(Message { role, content });
    }

    fn update_todos(&mut self, todos: Vec<TodoItem>) {
        self.todos = todos;
    }

    fn set_status(&mut self, status: String) {
        self.status = status;
    }

    fn append_tool_output(&mut self, output: String) {
        // Limit total output size to prevent memory issues
        if self.tool_output.len() > 10000 {
            // Keep only the last 5000 characters when we exceed limit
            let start = self.tool_output.len().saturating_sub(5000);
            self.tool_output = self.tool_output[start..].to_string();
            self.tool_output
                .insert_str(0, "... (output truncated) ...\n");
        }
        self.tool_output.push_str(&output);
    }

    fn clear_tool_output(&mut self) {
        self.tool_output.clear();
    }
}

/// Draw the UI
fn draw_ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Input field
        ])
        .split(frame.area());

    // Status bar
    let status = Paragraph::new(app.status.clone())
        .style(Style::default().fg(if app.is_processing {
            Color::Yellow
        } else {
            Color::Green
        }))
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(status, chunks[0]);

    // Main content area - split horizontally
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Messages
            Constraint::Percentage(40), // Todos and output
        ])
        .split(chunks[1]);

    // Messages area - show only the most recent messages that fit
    let all_messages: Vec<ListItem> = app
        .messages
        .iter()
        .flat_map(|msg| {
            let style = match msg.role {
                MessageRole::User => Style::default().fg(Color::Cyan),
                MessageRole::Assistant => Style::default().fg(Color::White),
                MessageRole::System => Style::default().fg(Color::Yellow),
            };

            let prefix = match msg.role {
                MessageRole::User => "You: ",
                MessageRole::Assistant => "Assistant: ",
                MessageRole::System => "System: ",
            };

            // Wrap long messages
            let width = main_chunks[0].width.saturating_sub(4) as usize;
            let wrapped = textwrap::wrap(&msg.content, width);

            wrapped
                .into_iter()
                .enumerate()
                .map(move |(i, line)| {
                    let content = if i == 0 {
                        format!("{}{}", prefix, line)
                    } else {
                        format!("     {}", line)
                    };
                    ListItem::new(Line::from(Span::styled(content, style)))
                })
                .collect::<Vec<_>>()
        })
        .collect();

    // Show only the most recent messages that fit in the viewport
    let visible_height = main_chunks[0].height.saturating_sub(2) as usize; // Subtract borders
    let messages_to_show: Vec<ListItem> = if all_messages.len() > visible_height {
        let skip_count = all_messages.len() - visible_height;
        all_messages.into_iter().skip(skip_count).collect()
    } else {
        all_messages
    };

    let messages_list =
        List::new(messages_to_show).block(Block::default().borders(Borders::ALL).title("Chat"));
    frame.render_widget(messages_list, main_chunks[0]);

    // Right side - split vertically
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Todos
            Constraint::Percentage(60), // Tool output
        ])
        .split(main_chunks[1]);

    // Todo list
    let todos: Vec<ListItem> = app
        .todos
        .iter()
        .map(|todo| {
            let status_icon = match todo.status {
                codex_agent_lib::TodoStatus::Pending => "⏳",
                codex_agent_lib::TodoStatus::InProgress => "🔄",
                codex_agent_lib::TodoStatus::Completed => "✅",
                codex_agent_lib::TodoStatus::Blocked => "🚫",
            };
            let content = format!("{} {}", status_icon, todo.content);
            let style = match todo.status {
                codex_agent_lib::TodoStatus::Completed => Style::default().fg(Color::Green),
                codex_agent_lib::TodoStatus::InProgress => Style::default().fg(Color::Yellow),
                codex_agent_lib::TodoStatus::Blocked => Style::default().fg(Color::Red),
                _ => Style::default(),
            };
            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

    let todos_list = List::new(todos).block(Block::default().borders(Borders::ALL).title("Tasks"));
    frame.render_widget(todos_list, right_chunks[0]);

    // Tool output - scroll to bottom automatically
    let output_lines: Vec<Line> = app
        .tool_output
        .lines()
        .map(|line| {
            // Highlight specific output types
            if line.starts_with('$') {
                Line::from(Span::styled(line, Style::default().fg(Color::Cyan)))
            } else if line.starts_with('✓') {
                Line::from(Span::styled(line, Style::default().fg(Color::Green)))
            } else if line.starts_with("🔧") {
                Line::from(Span::styled(line, Style::default().fg(Color::Yellow)))
            } else if line.contains("error") || line.contains("Error") {
                Line::from(Span::styled(line, Style::default().fg(Color::Red)))
            } else {
                Line::from(Span::styled(line, Style::default().fg(Color::Gray)))
            }
        })
        .collect();

    // Calculate scroll position to show most recent output
    let visible_height = right_chunks[1].height.saturating_sub(2) as usize;
    let output_to_show: Vec<Line> = if output_lines.len() > visible_height {
        let skip_count = output_lines.len() - visible_height;
        output_lines.into_iter().skip(skip_count).collect()
    } else {
        output_lines
    };

    let tool_output = Paragraph::new(output_to_show)
        .block(Block::default().borders(Borders::ALL).title("Output"))
        .wrap(Wrap { trim: false });
    frame.render_widget(tool_output, right_chunks[1]);

    // Input field
    let input = Paragraph::new(app.input.clone())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input (Enter to send, Ctrl+C to quit)"),
        );
    frame.render_widget(input, chunks[2]);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Initialize logging to file (optional)
    if let Ok(log_file) = std::fs::File::create("interactive_python_agent.log") {
        tracing_subscriber::fmt()
            .with_env_filter("codex_agent_lib=debug")
            .with_writer(log_file)
            .with_ansi(false)
            .init();
    } else {
        // If we can't create a log file, just use stdout
        tracing_subscriber::fmt()
            .with_env_filter("codex_agent_lib=debug")
            .with_ansi(false)
            .init();
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let app = Arc::new(Mutex::new(App::new()));

    // Configure the agent with system prompt for Python development
    let config = AgentConfig::builder()
        .model("gpt-5-mini".to_string())
        .api_key(std::env::var("OPENAI_API_KEY").ok())
        .system_prompt(Some(
            r#"You are a Python development assistant running in an interactive terminal-based interface. You help users write and execute Python code using `uv` for environment management.

## CRITICAL REQUIREMENT
**YOU MUST ALWAYS START BY CALLING update_plan TO CREATE A TASK LIST BEFORE DOING ANY OTHER WORK!**

## Your Capabilities

- Execute shell commands to set up Python environments and run scripts
- Create and edit Python files using apply_patch
- Track your progress with update_plan for multi-step tasks
- Provide clear, concise updates about your actions

## Available Tools

- **shell**: Execute commands - IMPORTANT: Always use bash syntax, NOT nu or other shells
- **apply_patch**: Create and edit files with precise patches
- **update_plan**: Track task progress with step-by-step plans

## CRITICAL: Shell Command Requirements

**ALWAYS prefix commands with bash -c when creating files or using shell features:**
- Use: `shell(["bash", "-c", "echo 'content' > file.py"])`
- NOT: `shell(["echo", "content", ">", "file.py"])` (this won't work!)
- The shell tool needs explicit bash invocation for redirects and pipes

## How You Work

### Planning (MANDATORY)
**ALWAYS start by creating a plan with update_plan before doing any work!**
- Create your plan immediately when you receive a request
- Break tasks into 3-7 meaningful steps (5-7 words each)
- Mark steps as: pending, in_progress, or completed
- Always have exactly one in_progress step
- Update the plan as you complete each step

Example plan for Python setup:
```json
{
  "plan": [
    {"step": "Check uv installation", "status": "in_progress"},
    {"step": "Initialize Python project", "status": "pending"},
    {"step": "Create virtual environment", "status": "pending"},
    {"step": "Create hello.py script", "status": "pending"},
    {"step": "Run the script", "status": "pending"}
  ]
}
```

### Preambles
Before tool calls, send brief updates (8-12 words) about what you're doing:
- "Setting up Python environment with uv..."
- "Installing required packages for data analysis..."
- "Creating script to calculate prime numbers..."

### Testing Your Work
Always verify your Python scripts work correctly:
- Run the script after creating it
- Check for errors and fix them
- Show the output to the user

## Essential uv Commands and Usage

### Initial Setup (do this ONCE at the start)
1. Check if uv is installed: Run `uv --version`
2. Initialize a Python project: Run `uv init` in the current directory
   - This creates a pyproject.toml file and src/ directory structure
3. Create/activate virtual environment: Run `uv venv`
   - This creates a .venv directory with an isolated Python environment
   - uv automatically uses this environment for all subsequent commands

### Installing Packages
- Install a package: `uv pip install package_name`
- Install multiple packages: `uv pip install pandas numpy matplotlib`
- Install from requirements.txt: `uv pip install -r requirements.txt`
- Show installed packages: `uv pip list`

### Running Python Scripts with uv
IMPORTANT: Always use `uv run` to execute Python scripts to ensure the correct environment is used:
- Run a script: `uv run python script_name.py`
- Run with arguments: `uv run python script.py arg1 arg2`
- Interactive Python: `uv run python`
- Run a module: `uv run python -m module_name`

### File Organization and Operations
- Place all Python scripts in the current directory or src/ subdirectory
- Name files descriptively: `data_analysis.py`, `web_scraper.py`, etc.
- For simple scripts, current directory is fine
- For larger projects, use src/ directory structure

### Creating and Managing Files

**Using apply_patch (preferred for complex files):**
```bash
apply_patch << 'EOF'
*** Begin Patch
*** Create File: hello.py
def calculate_primes(n):
    primes = []
    for num in range(2, n + 1):
        is_prime = True
        for i in range(2, int(num ** 0.5) + 1):
            if num % i == 0:
                is_prime = False
                break
        if is_prime:
            primes.append(num)
    return primes

print(calculate_primes(20))
*** End Patch
EOF
```

**Using bash for simple files (IMPORTANT: Use bash -c):**
```json
{"command": ["bash", "-c", "echo 'print(\"Hello from Python!\")' > hello.py"]}
```

**Using bash with heredoc for multi-line files:**
```json
{"command": ["bash", "-c", "cat > script.py << 'EOF'\nimport math\nprint(f\"Pi: {math.pi}\")\nEOF"]}
```

**Alternative: Let apply_patch handle file creation to avoid shell issues**

**Other file operations:**
- Read a file: `cat filename.py`
- List files: `ls -la`
- Create directory: `mkdir dirname`
- Check if file exists: `test -f filename.py && echo "exists" || echo "not found"`

### Your Workflow

1. **FIRST STEP - ALWAYS**: Call update_plan to create your task list
2. **Initial setup** (once per session) - USE BASH EXPLICITLY:
   ```json
   {"command": ["bash", "-c", "uv --version"]}  # Check if uv is installed
   {"command": ["bash", "-c", "uv init"]}       # Initialize Python project
   {"command": ["bash", "-c", "uv venv"]}       # Create virtual environment
   ```

3. **For each user request**:
   - Send a brief preamble about what you're doing
   - Analyze package requirements
   - Install packages: `uv pip install <packages>`
   - Create Python script using apply_patch or shell
   - Execute: `uv run python script.py`
   - Show output and verify correctness
   - Update plan to mark steps completed

### Example Workflow
For a data analysis request:
```bash
# Step 1: Install packages
uv pip install pandas matplotlib numpy

# Step 2: Create analysis script
apply_patch << 'EOF'
*** Begin Patch
*** Create File: analysis.py
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt

# Generate sample data
data = pd.DataFrame({
    'x': np.linspace(0, 10, 100),
    'y': np.sin(np.linspace(0, 10, 100))
})

print(f"Data shape: {data.shape}")
print(f"Summary:\n{data.describe()}")
*** End Patch
EOF

# Step 3: Run and verify
uv run python analysis.py
```

## Key Principles

### Environment Management
- **Always use** `uv run python` not bare `python` - ensures correct environment
- Virtual environment (.venv) is managed automatically by uv
- No manual activation/deactivation needed
- uv is faster than pip with better dependency resolution

### Error Handling
- **Package errors**: Verify spelling, suggest alternatives
- **Script errors**: Show full output, fix iteratively (max 3 attempts)
- **uv not found**: Guide user to install: `curl -LsSf https://astral.sh/uv/install.sh | sh`

### Quality Guidelines
- Keep code simple and readable
- Test your scripts before marking tasks complete
- Fix issues at root cause, not with surface patches
- Provide concise progress updates (8-12 words)
- Group related commands in single preambles

## Final Notes

- Be precise, safe, and helpful
- Complete tasks fully before yielding to user
- Show command outputs clearly
- Suggest logical next steps when appropriate"#
                .to_string(),
        ))
        .max_turns(100)
        // Use DangerFullAccess to allow full file system access for uv operations
        .sandbox_policy(SandboxPolicy::DangerFullAccess)
        // Note: File operations are done through the shell/bash tool
        // The Bash tool with DangerFullAccess allows all file operations
        .build();

    // Create the agent
    let agent = Agent::new(config)?;

    // Create channels
    let (input_tx, input_rx) = mpsc::channel(100);
    let (plan_tx, mut plan_rx) = mpsc::channel::<PlanMessage>(100);
    let (output_tx, mut output_rx) = mpsc::channel(100);

    // Start the agent
    let handle = agent.execute(input_rx, plan_tx, output_tx).await?;
    let controller = handle.controller().clone();

    // Clone app for output handler
    let app_clone = app.clone();

    // Clone app for plan handler
    let app_plan_clone = app.clone();

    // Spawn plan handler
    let _plan_task = tokio::spawn(async move {
        while let Some(plan_msg) = plan_rx.recv().await {
            let mut app = app_plan_clone.lock().unwrap_or_else(|e| e.into_inner());
            app.update_todos(plan_msg.todos);
        }
    });

    // Spawn output handler
    tokio::spawn(async move {
        while let Some(output) = output_rx.recv().await {
            let mut app = app_clone.lock().unwrap_or_else(|e| e.into_inner());

            match output.data {
                OutputData::Start => {
                    app.set_status("Agent started".to_string());
                }
                OutputData::Primary(msg) => {
                    // Check if this is a duplicate of the last message
                    let should_add = if let Some(last_msg) = app.messages.last() {
                        last_msg.role != MessageRole::Assistant || !last_msg.content.contains(&msg)
                    } else {
                        true
                    };

                    if should_add {
                        app.add_message(MessageRole::Assistant, msg);
                        app.clear_tool_output();
                    }
                }
                OutputData::PrimaryDelta(delta) => {
                    // Only append to existing assistant message, don't create new ones
                    if let Some(last_msg) = app.messages.last_mut() {
                        if last_msg.role == MessageRole::Assistant {
                            last_msg.content.push_str(&delta);
                        }
                    }
                    // If there's no assistant message yet, create one
                    else {
                        app.add_message(MessageRole::Assistant, delta);
                    }
                }
                OutputData::ToolStart {
                    tool_name,
                    arguments,
                } => {
                    app.set_status(format!("Running: {}", tool_name));
                    app.clear_tool_output();
                    if tool_name == "shell" || tool_name == "bash" {
                        // Show the command being executed
                        if let Some(cmd) = arguments.get("command").and_then(|v| v.as_array()) {
                            let cmd_str = cmd
                                .iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            // Truncate long commands for display
                            let display_cmd = if cmd_str.len() > 100 {
                                format!("{}...", &cmd_str[..100])
                            } else {
                                cmd_str
                            };
                            app.append_tool_output(format!("$ {}\n", display_cmd));
                        } else if let Some(cmd) = arguments.get("command").and_then(|v| v.as_str())
                        {
                            // Truncate long commands for display
                            let display_cmd = if cmd.len() > 100 {
                                format!("{}...", &cmd[..100])
                            } else {
                                cmd.to_string()
                            };
                            app.append_tool_output(format!("$ {}\n", display_cmd));
                        }
                    } else {
                        app.append_tool_output(format!("🔧 {}\n", tool_name));
                    }
                }
                OutputData::ToolOutput { output, .. } => {
                    // Clean and add the output
                    let cleaned = strip_ansi_escapes::strip(&output);
                    let cleaned_str = String::from_utf8_lossy(&cleaned);

                    // Add each line, limiting total lines
                    let lines: Vec<&str> = cleaned_str.lines().take(10).collect();
                    for line in lines {
                        // Truncate long lines properly at char boundaries
                        let truncated = if line.len() > 100 {
                            let mut end = 100;
                            while !line.is_char_boundary(end) && end > 0 {
                                end -= 1;
                            }
                            format!("{}...", &line[..end])
                        } else {
                            line.to_string()
                        };

                        if !truncated.trim().is_empty() {
                            app.append_tool_output(format!("{}\n", truncated));
                        }
                    }
                }
                OutputData::ToolComplete { tool_name, .. } => {
                    app.append_tool_output(format!("✓ {} completed\n\n", tool_name));
                }
                OutputData::Completed => {
                    app.set_status("Ready".to_string());
                    app.is_processing = false;
                }
                OutputData::Error(err) => {
                    eprintln!("Agent error: {:?}", err); // Debug output
                    app.add_message(MessageRole::System, format!("Error: {:?}", err));
                    app.set_status("Error occurred".to_string());
                    app.is_processing = false;
                }
                _ => {}
            }
        }
    });

    // Send initial message to set up Python environment
    input_tx.send("Please set up a Python environment using uv. First check if uv is installed, then initialize a project with uv init, create a virtual environment with uv venv. Then create a simple hello.py script that calculates and prints the first 20 prime numbers, and run it using 'uv run python hello.py' to verify everything works.".into()).await?;
    app.lock().unwrap_or_else(|e| e.into_inner()).is_processing = true;

    // Main UI loop
    let app_ui = app.clone();
    let input_tx_clone = input_tx.clone();

    loop {
        // Draw UI
        terminal.draw(|f| {
            let app = app_ui.lock().unwrap_or_else(|e| e.into_inner());
            draw_ui(f, &app);
        })?;

        // Handle input - reduce polling frequency for better performance
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            break;
                        }
                        KeyCode::Enter => {
                            let mut app = app_ui.lock().unwrap_or_else(|e| e.into_inner());
                            if !app.input.is_empty() && !app.is_processing {
                                let msg = app.input.clone();
                                app.input.clear();
                                app.add_message(MessageRole::User, msg.clone());
                                app.is_processing = true;
                                app.set_status("Processing...".to_string());
                                drop(app); // Release lock before sending

                                // Send message through channel
                                let input_tx = input_tx_clone.clone();
                                tokio::spawn(async move {
                                    let _ = input_tx.send(msg.into()).await;
                                });
                            }
                        }
                        KeyCode::Char(c) => {
                            let mut app = app_ui.lock().unwrap_or_else(|e| e.into_inner());
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            let mut app = app_ui.lock().unwrap_or_else(|e| e.into_inner());
                            app.input.pop();
                        }
                        KeyCode::Up => {
                            // Could implement scrolling later
                        }
                        KeyCode::Down => {
                            // Could implement scrolling later
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
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

// Add textwrap for message wrapping
mod textwrap {
    pub fn wrap(text: &str, width: usize) -> Vec<String> {
        if text.is_empty() {
            return vec![String::new()];
        }

        let mut result = Vec::new();
        let mut current_line = String::new();

        for word in text.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + word.len() + 1 < width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                result.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            result.push(current_line);
        }

        result
    }
}
