//! Message processing pipeline for filtering, transforming, and aggregating messages

use crate::message::OutputData;
use crate::message::OutputMessage;

/// Filter trait for message processing
pub trait MessageFilter: Send + Sync {
    /// Returns true if the message should be kept
    fn should_keep(&self, msg: &OutputMessage) -> bool;
}

/// Transformer trait for message processing
pub trait MessageTransformer: Send + Sync {
    /// Transform a message
    fn transform(&self, msg: OutputMessage) -> OutputMessage;
}

/// Aggregator trait for combining multiple messages
pub trait MessageAggregator: Send + Sync {
    /// Process a message and potentially return an aggregated result
    fn process(&mut self, msg: OutputMessage) -> Option<OutputMessage>;
    
    /// Flush any remaining messages
    fn flush(&mut self) -> Vec<OutputMessage>;
}

/// Message processor that applies filters, transformers, and aggregators
pub struct MessageProcessor {
    filters: Vec<Box<dyn MessageFilter>>,
    transformers: Vec<Box<dyn MessageTransformer>>,
    aggregators: Vec<Box<dyn MessageAggregator>>,
}

impl MessageProcessor {
    /// Create a new builder
    pub fn builder() -> MessageProcessorBuilder {
        MessageProcessorBuilder::default()
    }
    
    /// Process a single message
    pub fn process(&mut self, mut msg: OutputMessage) -> Vec<OutputMessage> {
        // Apply filters
        for filter in &self.filters {
            if !filter.should_keep(&msg) {
                return Vec::new();
            }
        }
        
        // Apply transformers
        for transformer in &self.transformers {
            msg = transformer.transform(msg);
        }
        
        // Apply aggregators
        if self.aggregators.is_empty() {
            vec![msg]
        } else {
            let mut results = Vec::new();
            let mut current_msg = Some(msg);
            
            for aggregator in &mut self.aggregators {
                if let Some(msg) = current_msg.take() {
                    if let Some(aggregated) = aggregator.process(msg) {
                        current_msg = Some(aggregated);
                    }
                }
            }
            
            if let Some(msg) = current_msg {
                results.push(msg);
            }
            
            results
        }
    }
    
    /// Flush any remaining aggregated messages
    pub fn flush(&mut self) -> Vec<OutputMessage> {
        let mut results = Vec::new();
        for aggregator in &mut self.aggregators {
            results.extend(aggregator.flush());
        }
        results
    }
}

/// Builder for MessageProcessor
#[derive(Default)]
pub struct MessageProcessorBuilder {
    filters: Vec<Box<dyn MessageFilter>>,
    transformers: Vec<Box<dyn MessageTransformer>>,
    aggregators: Vec<Box<dyn MessageAggregator>>,
}

impl MessageProcessorBuilder {
    /// Add a custom filter
    pub fn filter<F>(mut self, filter: F) -> Self
    where
        F: MessageFilter + 'static,
    {
        self.filters.push(Box::new(filter));
        self
    }
    
    /// Add a custom transformer
    pub fn transform<T>(mut self, transformer: T) -> Self
    where
        T: MessageTransformer + 'static,
    {
        self.transformers.push(Box::new(transformer));
        self
    }
    
    /// Add a custom aggregator
    pub fn aggregate<A>(mut self, aggregator: A) -> Self
    where
        A: MessageAggregator + 'static,
    {
        self.aggregators.push(Box::new(aggregator));
        self
    }
    
    /// Filter out tool output messages
    pub fn filter_tool_output(mut self) -> Self {
        self.filters.push(Box::new(ToolOutputFilter));
        self
    }
    
    /// Filter messages by type
    pub fn filter_by_type(mut self, types: Vec<&str>) -> Self {
        self.filters.push(Box::new(TypeFilter::new(types)));
        self
    }
    
    /// Strip ANSI codes from messages
    #[cfg(feature = "utils")]
    pub fn strip_ansi_codes(mut self) -> Self {
        self.transformers.push(Box::new(AnsiStripper));
        self
    }
    
    /// Truncate long lines
    pub fn truncate_lines(mut self, max_length: usize) -> Self {
        self.transformers
            .push(Box::new(LineTruncator { max_length }));
        self
    }
    
    /// Aggregate delta messages
    pub fn aggregate_deltas(mut self) -> Self {
        self.aggregators.push(Box::new(DeltaAggregator::new()));
        self
    }
    
    /// Remove duplicate consecutive messages
    pub fn remove_duplicates(mut self) -> Self {
        self.aggregators.push(Box::new(DuplicateRemover::new()));
        self
    }
    
    /// Build the processor
    pub fn build(self) -> MessageProcessor {
        MessageProcessor {
            filters: self.filters,
            transformers: self.transformers,
            aggregators: self.aggregators,
        }
    }
}

// Built-in filters

struct ToolOutputFilter;

impl MessageFilter for ToolOutputFilter {
    fn should_keep(&self, msg: &OutputMessage) -> bool {
        !matches!(
            msg.data,
            OutputData::ToolOutput { .. } | OutputData::ToolStart { .. }
        )
    }
}

struct TypeFilter {
    allowed_types: Vec<String>,
}

impl TypeFilter {
    fn new(types: Vec<&str>) -> Self {
        Self {
            allowed_types: types.into_iter().map(String::from).collect(),
        }
    }
}

impl MessageFilter for TypeFilter {
    fn should_keep(&self, msg: &OutputMessage) -> bool {
        let type_name = match &msg.data {
            OutputData::Primary(_) => "primary",
            OutputData::PrimaryDelta(_) => "delta",
            OutputData::ToolStart { .. } => "tool_start",
            OutputData::ToolOutput { .. } => "tool_output",
            OutputData::ToolComplete { .. } => "tool_complete",
            OutputData::Completed => "completed",
            OutputData::Error(_) => "error",
            OutputData::Start => "start",
            _ => "unknown",
        };
        
        self.allowed_types.iter().any(|t| t == type_name)
    }
}

// Built-in transformers

#[cfg(feature = "utils")]
struct AnsiStripper;

#[cfg(feature = "utils")]
impl MessageTransformer for AnsiStripper {
    fn transform(&self, mut msg: OutputMessage) -> OutputMessage {
        use crate::utils::output::clean_ansi;
        
        match &mut msg.data {
            OutputData::Primary(text) | OutputData::PrimaryDelta(text) => {
                *text = clean_ansi(text);
            }
            OutputData::ToolOutput { output, .. } => {
                *output = clean_ansi(output);
            }
            _ => {}
        }
        
        msg
    }
}

struct LineTruncator {
    max_length: usize,
}

impl MessageTransformer for LineTruncator {
    fn transform(&self, mut msg: OutputMessage) -> OutputMessage {
        match &mut msg.data {
            OutputData::Primary(text) | OutputData::PrimaryDelta(text) => {
                *text = truncate_lines(text, self.max_length);
            }
            OutputData::ToolOutput { output, .. } => {
                *output = truncate_lines(output, self.max_length);
            }
            _ => {}
        }
        
        msg
    }
}

fn truncate_lines(text: &str, max_length: usize) -> String {
    text.lines()
        .map(|line| {
            if line.len() > max_length {
                format!("{}...", &line[..max_length.min(line.len())])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Built-in aggregators

struct DeltaAggregator {
    buffer: String,
}

impl DeltaAggregator {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }
}

impl MessageAggregator for DeltaAggregator {
    fn process(&mut self, msg: OutputMessage) -> Option<OutputMessage> {
        match msg.data {
            OutputData::PrimaryDelta(delta) => {
                self.buffer.push_str(&delta);
                None
            }
            _ => {
                if !self.buffer.is_empty() {
                    let aggregated = OutputMessage {
                        data: OutputData::Primary(std::mem::take(&mut self.buffer)),
                        turn_id: 0,
                    };
                    Some(aggregated)
                } else {
                    Some(msg)
                }
            }
        }
    }
    
    fn flush(&mut self) -> Vec<OutputMessage> {
        if !self.buffer.is_empty() {
            vec![OutputMessage {
                data: OutputData::Primary(std::mem::take(&mut self.buffer)),
                turn_id: 0,
            }]
        } else {
            Vec::new()
        }
    }
}

struct DuplicateRemover {
    last_message: Option<String>,
}

impl DuplicateRemover {
    fn new() -> Self {
        Self {
            last_message: None,
        }
    }
    
    fn message_content(msg: &OutputMessage) -> Option<String> {
        match &msg.data {
            OutputData::Primary(text) | OutputData::PrimaryDelta(text) => Some(text.clone()),
            _ => None,
        }
    }
}

impl MessageAggregator for DuplicateRemover {
    fn process(&mut self, msg: OutputMessage) -> Option<OutputMessage> {
        let content = Self::message_content(&msg);
        
        if let Some(ref current) = content {
            if self.last_message.as_ref() == Some(current) {
                // Duplicate, skip it
                return None;
            }
            self.last_message = Some(current.clone());
        }
        
        Some(msg)
    }
    
    fn flush(&mut self) -> Vec<OutputMessage> {
        Vec::new()
    }
}