//! Pre-built TUI components for rendering agent interfaces

#[cfg(feature = "tui")]
use crate::message::TodoItem;
#[cfg(feature = "tui")]
use crate::tui::app::AppState;
#[cfg(feature = "tui")]
use crate::tui::app::Message;
#[cfg(feature = "tui")]
use crate::tui::app::MessageRole;
#[cfg(feature = "tui")]
use ratatui::Frame;
#[cfg(feature = "tui")]
use ratatui::layout::Constraint;
#[cfg(feature = "tui")]
use ratatui::layout::Direction;
#[cfg(feature = "tui")]
use ratatui::layout::Layout;
#[cfg(feature = "tui")]
use ratatui::layout::Rect;
#[cfg(feature = "tui")]
use ratatui::style::Color;
#[cfg(feature = "tui")]
use ratatui::style::Style;
#[cfg(feature = "tui")]
use ratatui::text::Line;
#[cfg(feature = "tui")]
use ratatui::text::Span;
#[cfg(feature = "tui")]
use ratatui::widgets::Block;
#[cfg(feature = "tui")]
use ratatui::widgets::Borders;
#[cfg(feature = "tui")]
use ratatui::widgets::List;
#[cfg(feature = "tui")]
use ratatui::widgets::ListItem;
#[cfg(feature = "tui")]
use ratatui::widgets::Paragraph;
#[cfg(feature = "tui")]
use ratatui::widgets::Wrap;

/// Render the default layout with all components
#[cfg(feature = "tui")]
pub fn render_default_layout(frame: &mut Frame, area: Rect, state: &AppState, _title: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Status bar
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Input field
        ])
        .split(area);
    
    // Render status bar
    render_status(frame, chunks[0], &state.status, state.is_processing);
    
    // Split main content area
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Messages
            Constraint::Percentage(40), // Todos and output
        ])
        .split(chunks[1]);
    
    // Render messages
    render_chat(frame, main_chunks[0], &state.messages);
    
    // Split right side
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Todos
            Constraint::Percentage(60), // Tool output
        ])
        .split(main_chunks[1]);
    
    // Render todos
    render_todos(frame, right_chunks[0], &state.todos);
    
    // Render tool output
    render_output(frame, right_chunks[1], &state.tool_output);
    
    // Render input
    render_input(frame, chunks[2], &state.input);
}

/// Render the status bar
#[cfg(feature = "tui")]
pub fn render_status(frame: &mut Frame, area: Rect, status: &str, is_processing: bool) {
    let status = Paragraph::new(status)
        .style(Style::default().fg(if is_processing {
            Color::Yellow
        } else {
            Color::Green
        }))
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(status, area);
}

/// Render the chat messages
#[cfg(feature = "tui")]
pub fn render_chat(frame: &mut Frame, area: Rect, messages: &[Message]) {
    let all_messages: Vec<ListItem> = messages
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
            
            // Simple line wrapping
            let width = area.width.saturating_sub(4) as usize;
            let wrapped = wrap_text(&msg.content, width);
            
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
    
    // Show only the most recent messages that fit
    let visible_height = area.height.saturating_sub(2) as usize;
    let messages_to_show: Vec<ListItem> = if all_messages.len() > visible_height {
        let skip_count = all_messages.len() - visible_height;
        all_messages.into_iter().skip(skip_count).collect()
    } else {
        all_messages
    };
    
    let messages_list = List::new(messages_to_show)
        .block(Block::default().borders(Borders::ALL).title("Chat"));
    frame.render_widget(messages_list, area);
}

/// Render the todo list
#[cfg(feature = "tui")]
pub fn render_todos(frame: &mut Frame, area: Rect, todos: &[TodoItem]) {
    let todos: Vec<ListItem> = todos
        .iter()
        .map(|todo| {
            let status_icon = match todo.status {
                crate::message::TodoStatus::Pending => "⏳",
                crate::message::TodoStatus::InProgress => "🔄",
                crate::message::TodoStatus::Completed => "✅",
                crate::message::TodoStatus::Blocked => "🚫",
            };
            let content = format!("{} {}", status_icon, todo.content);
            let style = match todo.status {
                crate::message::TodoStatus::Completed => Style::default().fg(Color::Green),
                crate::message::TodoStatus::InProgress => Style::default().fg(Color::Yellow),
                crate::message::TodoStatus::Blocked => Style::default().fg(Color::Red),
                _ => Style::default(),
            };
            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();
    
    let todos_list = List::new(todos)
        .block(Block::default().borders(Borders::ALL).title("Tasks"));
    frame.render_widget(todos_list, area);
}

/// Render the tool output
#[cfg(feature = "tui")]
pub fn render_output(frame: &mut Frame, area: Rect, output: &str) {
    let output_lines: Vec<Line> = output
        .lines()
        .map(|line| {
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
    
    // Show only the most recent output
    let visible_height = area.height.saturating_sub(2) as usize;
    let output_to_show: Vec<Line> = if output_lines.len() > visible_height {
        let skip_count = output_lines.len() - visible_height;
        output_lines.into_iter().skip(skip_count).collect()
    } else {
        output_lines
    };
    
    let tool_output = Paragraph::new(output_to_show)
        .block(Block::default().borders(Borders::ALL).title("Output"))
        .wrap(Wrap { trim: false });
    frame.render_widget(tool_output, area);
}

/// Render the input field
#[cfg(feature = "tui")]
pub fn render_input(frame: &mut Frame, area: Rect, input: &str) {
    let input = Paragraph::new(input)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input (Enter to send, Ctrl+C to quit)"),
        );
    frame.render_widget(input, area);
}

// Helper function for text wrapping
#[cfg(feature = "tui")]
fn wrap_text(text: &str, width: usize) -> Vec<String> {
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
    
    if result.is_empty() {
        vec![String::new()]
    } else {
        result
    }
}