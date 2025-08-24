//! Utility functions for working with agent output

#[cfg(feature = "utils")]
pub mod output {
    use crate::message::OutputData;
    use crate::message::OutputMessage;
    
    /// Strip ANSI escape codes from text
    #[cfg(feature = "utils")]
    pub fn clean_ansi(text: &str) -> String {
        let bytes = strip_ansi_escapes::strip(text);
        String::from_utf8_lossy(&bytes).to_string()
    }
    
    /// Extract shell commands from tool calls
    pub fn extract_commands(msg: &OutputMessage) -> Vec<String> {
        match &msg.data {
            OutputData::ToolStart {
                tool_name,
                arguments,
            } if tool_name == "shell" || tool_name == "bash" => {
                // Try to extract command from arguments
                if let Some(cmd) = arguments.get("command") {
                    if let Some(cmd_str) = cmd.as_str() {
                        return vec![cmd_str.to_string()];
                    } else if let Some(cmd_array) = cmd.as_array() {
                        let cmd_str = cmd_array
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        return vec![cmd_str];
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
    
    /// Format tool output for display with line limiting
    pub fn format_tool_output(output: &str, max_lines: usize) -> String {
        let lines: Vec<&str> = output.lines().collect();
        
        if lines.len() <= max_lines {
            output.to_string()
        } else {
            let mut result = String::new();
            
            // Show first half of allowed lines
            let head_count = max_lines / 2;
            for line in lines.iter().take(head_count) {
                result.push_str(line);
                result.push('\n');
            }
            
            // Add truncation indicator
            result.push_str(&format!(
                "\n... ({} lines omitted) ...\n\n",
                lines.len() - max_lines
            ));
            
            // Show last half of allowed lines
            let tail_count = max_lines - head_count;
            for line in lines.iter().rev().take(tail_count).rev() {
                result.push_str(line);
                result.push('\n');
            }
            
            result
        }
    }
    
    /// Smart text wrapping that preserves word boundaries
    pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
        if text.is_empty() || width == 0 {
            return vec![String::new()];
        }
        
        let mut result = Vec::new();
        
        for line in text.lines() {
            if line.len() <= width {
                result.push(line.to_string());
            } else {
                let mut current_line = String::new();
                let mut current_width = 0;
                
                for word in line.split_whitespace() {
                    let word_len = word.len();
                    
                    if current_width == 0 {
                        // First word on the line
                        if word_len > width {
                            // Word is longer than width, break it
                            let mut chars = word.chars();
                            while current_width < width {
                                if let Some(ch) = chars.next() {
                                    current_line.push(ch);
                                    current_width += 1;
                                } else {
                                    break;
                                }
                            }
                            result.push(current_line.clone());
                            current_line.clear();
                            current_width = 0;
                            
                            // Handle remaining characters
                            let remaining: String = chars.collect();
                            if !remaining.is_empty() {
                                for chunk in remaining.as_bytes().chunks(width) {
                                    result.push(String::from_utf8_lossy(chunk).to_string());
                                }
                            }
                        } else {
                            current_line.push_str(word);
                            current_width = word_len;
                        }
                    } else if current_width + 1 + word_len <= width {
                        // Word fits on current line with space
                        current_line.push(' ');
                        current_line.push_str(word);
                        current_width += 1 + word_len;
                    } else {
                        // Word doesn't fit, start new line
                        result.push(current_line.clone());
                        current_line.clear();
                        current_line.push_str(word);
                        current_width = word_len;
                    }
                }
                
                if !current_line.is_empty() {
                    result.push(current_line);
                }
            }
        }
        
        if result.is_empty() {
            vec![String::new()]
        } else {
            result
        }
    }
    
    /// Check if a message contains tool execution
    pub fn is_tool_message(msg: &OutputMessage) -> bool {
        matches!(
            msg.data,
            OutputData::ToolStart { .. }
                | OutputData::ToolOutput { .. }
                | OutputData::ToolComplete { .. }
        )
    }
    
    /// Extract tool name from a tool message
    pub fn get_tool_name(msg: &OutputMessage) -> Option<String> {
        match &msg.data {
            OutputData::ToolStart { tool_name, .. }
            | OutputData::ToolOutput { tool_name, .. }
            | OutputData::ToolComplete { tool_name, .. } => Some(tool_name.clone()),
            _ => None,
        }
    }
    
    /// Format a message for display
    pub fn format_message(msg: &OutputMessage) -> String {
        match &msg.data {
            OutputData::Primary(text) => text.clone(),
            OutputData::PrimaryDelta(delta) => delta.clone(),
            OutputData::ToolStart { tool_name, .. } => format!("🔧 Running: {}", tool_name),
            OutputData::ToolOutput { tool_name, output } => {
                format!("📤 {}: {}", tool_name, output)
            }
            OutputData::ToolComplete { tool_name, .. } => format!("✅ {} completed", tool_name),
            OutputData::Error(err) => format!("❌ Error: {:?}", err),
            OutputData::Completed => "✅ Completed".to_string(),
            OutputData::Start => "🚀 Starting...".to_string(),
            _ => String::new(),
        }
    }
}

#[cfg(not(feature = "utils"))]
pub mod output {
    use crate::message::OutputMessage;
    
    pub fn clean_ansi(text: &str) -> String {
        text.to_string()
    }
    
    pub fn extract_commands(_msg: &OutputMessage) -> Vec<String> {
        Vec::new()
    }
    
    pub fn format_tool_output(output: &str, _max_lines: usize) -> String {
        output.to_string()
    }
    
    pub fn wrap_text(text: &str, _width: usize) -> Vec<String> {
        text.lines().map(String::from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::output::*;
    
    #[test]
    fn test_wrap_text() {
        let text = "This is a long line that needs to be wrapped at a specific width";
        let wrapped = wrap_text(text, 20);
        assert!(wrapped.iter().all(|line| line.len() <= 20));
        
        let text = "Short";
        let wrapped = wrap_text(text, 20);
        assert_eq!(wrapped, vec!["Short"]);
        
        let text = "Verylongwordthatcannotbewrappednormally";
        let wrapped = wrap_text(text, 10);
        assert!(wrapped.iter().all(|line| line.len() <= 10));
    }
    
    #[test]
    fn test_format_tool_output() {
        let output = (0..20).map(|i| format!("Line {}", i)).collect::<Vec<_>>().join("\n");
        let formatted = format_tool_output(&output, 10);
        
        assert!(formatted.contains("Line 0"));
        assert!(formatted.contains("Line 19"));
        assert!(formatted.contains("omitted"));
    }
}