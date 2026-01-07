//! Tool/function call extraction for LLM provider responses.
//!
//! This module provides a unified, configuration-driven approach to extracting
//! tool calls from different LLM providers (OpenAI, Anthropic, Google). Each
//! provider has a distinct format for representing function calls in responses:
//!
//! - **OpenAI**: `{"tool_calls": [{"id": "...", "type": "function", "function": {"name": "...", "arguments": "..."}}]}`
//! - **Anthropic**: Content blocks of type `"tool_use"`: `{"type": "tool_use", "id": "...", "name": "...", "input": {...}}`
//! - **Google**: `{"functionCall": {"name": "...", "args": {...}}}`
//!
//! # Example
//!
//! ```rust
//! use cllient::tool_call::{ToolCallExtractor, ToolCallExtractorBuilder};
//! use serde_json::json;
//!
//! // Using a preset for OpenAI
//! let extractor = ToolCallExtractorBuilder::openai().build();
//!
//! let response = json!({
//!     "choices": [{
//!         "message": {
//!             "tool_calls": [{
//!                 "id": "call_123",
//!                 "type": "function",
//!                 "function": {
//!                     "name": "get_weather",
//!                     "arguments": "{\"location\": \"San Francisco\"}"
//!                 }
//!             }]
//!         }
//!     }]
//! });
//!
//! let tool_calls = extractor.extract(&response);
//! assert_eq!(tool_calls.len(), 1);
//! assert_eq!(tool_calls[0].name, "get_weather");
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Normalized tool call representation.
///
/// This struct provides a unified format for tool calls across all providers,
/// abstracting away the differences in how each provider represents function calls.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    /// For providers that don't provide IDs (like Google), a synthetic ID is generated.
    pub id: String,
    /// The name of the function/tool being called.
    pub name: String,
    /// The arguments for the function call.
    /// This is always a JSON object, even if the provider sends arguments as a string.
    pub arguments: Value,
}

impl ToolCall {
    /// Create a new ToolCall with the given id, name, and arguments.
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    /// Parse arguments from a JSON string, returning the parsed object or the original string as a value.
    pub fn parse_arguments(args_str: &str) -> Value {
        serde_json::from_str(args_str).unwrap_or_else(|_| Value::String(args_str.to_string()))
    }
}

/// Configuration for extracting tool calls from provider-specific response formats.
///
/// This configuration uses simplified JSONPath-like paths to navigate the response structure
/// and extract tool call information. Different providers structure their responses differently,
/// so this configuration allows adapting to each format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallExtractorConfig {
    /// JSONPath-like path to the tool calls array or object within the response.
    /// Examples:
    /// - OpenAI: `"choices[0].message.tool_calls"`
    /// - Anthropic: `"content[?(@.type=='tool_use')]"` or just `"content"` with type filtering
    /// - Google: `"candidates[0].content.parts[0].functionCall"`
    pub path: String,

    /// Path to the tool call ID within each tool call object.
    /// Examples:
    /// - OpenAI: `"id"`
    /// - Anthropic: `"id"`
    /// - Google: (none, synthetic ID generated)
    pub id_path: String,

    /// Path to the function/tool name within each tool call object.
    /// Examples:
    /// - OpenAI: `"function.name"`
    /// - Anthropic: `"name"`
    /// - Google: `"name"`
    pub name_path: String,

    /// Path to the arguments/input within each tool call object.
    /// Examples:
    /// - OpenAI: `"function.arguments"`
    /// - Anthropic: `"input"`
    /// - Google: `"args"`
    pub arguments_path: String,

    /// Whether the arguments are a JSON string (true) or already an object (false).
    /// - OpenAI: `true` (arguments are stringified JSON)
    /// - Anthropic: `false` (input is a JSON object)
    /// - Google: `false` (args is a JSON object)
    pub arguments_is_string: bool,

    /// Optional type field to filter content blocks (used by Anthropic).
    /// If set, only items where this field equals the expected type are processed.
    pub type_filter: Option<TypeFilter>,
}

/// Filter configuration for type-based content block filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeFilter {
    /// Path to the type field within each item.
    pub path: String,
    /// Expected value for the type field.
    pub value: String,
}

impl ToolCallExtractorConfig {
    /// Create a new configuration for OpenAI-style tool calls.
    pub fn openai() -> Self {
        Self {
            path: "choices[0].message.tool_calls".to_string(),
            id_path: "id".to_string(),
            name_path: "function.name".to_string(),
            arguments_path: "function.arguments".to_string(),
            arguments_is_string: true,
            type_filter: None,
        }
    }

    /// Create a new configuration for Anthropic-style tool use blocks.
    pub fn anthropic() -> Self {
        Self {
            path: "content".to_string(),
            id_path: "id".to_string(),
            name_path: "name".to_string(),
            arguments_path: "input".to_string(),
            arguments_is_string: false,
            type_filter: Some(TypeFilter {
                path: "type".to_string(),
                value: "tool_use".to_string(),
            }),
        }
    }

    /// Create a new configuration for Google-style function calls.
    pub fn google() -> Self {
        Self {
            path: "candidates[0].content.parts".to_string(),
            id_path: "".to_string(), // Google doesn't provide IDs
            name_path: "functionCall.name".to_string(),
            arguments_path: "functionCall.args".to_string(),
            arguments_is_string: false,
            type_filter: Some(TypeFilter {
                path: "functionCall".to_string(),
                value: "".to_string(), // Just check existence, not value
            }),
        }
    }
}

/// Builder for creating ToolCallExtractorConfig instances.
///
/// Provides a fluent API for constructing extractors, with presets for common providers.
#[derive(Debug, Clone, Default)]
pub struct ToolCallExtractorBuilder {
    path: Option<String>,
    id_path: Option<String>,
    name_path: Option<String>,
    arguments_path: Option<String>,
    arguments_is_string: Option<bool>,
    type_filter: Option<TypeFilter>,
}

impl ToolCallExtractorBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder preset for OpenAI format.
    ///
    /// OpenAI format:
    /// ```json
    /// {"tool_calls": [{"id": "...", "type": "function", "function": {"name": "...", "arguments": "..."}}]}
    /// ```
    pub fn openai() -> Self {
        Self {
            path: Some("choices[0].message.tool_calls".to_string()),
            id_path: Some("id".to_string()),
            name_path: Some("function.name".to_string()),
            arguments_path: Some("function.arguments".to_string()),
            arguments_is_string: Some(true),
            type_filter: None,
        }
    }

    /// Create a builder preset for Anthropic format.
    ///
    /// Anthropic format (content blocks):
    /// ```json
    /// {"type": "tool_use", "id": "...", "name": "...", "input": {...}}
    /// ```
    pub fn anthropic() -> Self {
        Self {
            path: Some("content".to_string()),
            id_path: Some("id".to_string()),
            name_path: Some("name".to_string()),
            arguments_path: Some("input".to_string()),
            arguments_is_string: Some(false),
            type_filter: Some(TypeFilter {
                path: "type".to_string(),
                value: "tool_use".to_string(),
            }),
        }
    }

    /// Create a builder preset for Google format.
    ///
    /// Google format:
    /// ```json
    /// {"functionCall": {"name": "...", "args": {...}}}
    /// ```
    pub fn google() -> Self {
        Self {
            path: Some("candidates[0].content.parts".to_string()),
            id_path: Some("".to_string()),
            name_path: Some("functionCall.name".to_string()),
            arguments_path: Some("functionCall.args".to_string()),
            arguments_is_string: Some(false),
            type_filter: Some(TypeFilter {
                path: "functionCall".to_string(),
                value: "".to_string(),
            }),
        }
    }

    /// Set the path to the tool calls array/object.
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set the path to the tool call ID within each call.
    pub fn id_path(mut self, path: impl Into<String>) -> Self {
        self.id_path = Some(path.into());
        self
    }

    /// Set the path to the function/tool name.
    pub fn name_path(mut self, path: impl Into<String>) -> Self {
        self.name_path = Some(path.into());
        self
    }

    /// Set the path to the arguments/input.
    pub fn arguments_path(mut self, path: impl Into<String>) -> Self {
        self.arguments_path = Some(path.into());
        self
    }

    /// Set whether arguments are a JSON string (true) or object (false).
    pub fn arguments_is_string(mut self, is_string: bool) -> Self {
        self.arguments_is_string = Some(is_string);
        self
    }

    /// Set a type filter for filtering content blocks.
    pub fn type_filter(mut self, path: impl Into<String>, value: impl Into<String>) -> Self {
        self.type_filter = Some(TypeFilter {
            path: path.into(),
            value: value.into(),
        });
        self
    }

    /// Clear the type filter.
    pub fn no_type_filter(mut self) -> Self {
        self.type_filter = None;
        self
    }

    /// Build the extractor configuration.
    pub fn build(self) -> ToolCallExtractorConfig {
        ToolCallExtractorConfig {
            path: self.path.unwrap_or_default(),
            id_path: self.id_path.unwrap_or_default(),
            name_path: self.name_path.unwrap_or_default(),
            arguments_path: self.arguments_path.unwrap_or_default(),
            arguments_is_string: self.arguments_is_string.unwrap_or(false),
            type_filter: self.type_filter,
        }
    }

    /// Build and return a ToolCallExtractor.
    pub fn build_extractor(self) -> ToolCallExtractor {
        ToolCallExtractor::new(self.build())
    }
}

/// Extractor for tool calls from provider responses.
///
/// Uses the configuration to navigate the response JSON and extract
/// normalized ToolCall instances.
#[derive(Debug, Clone)]
pub struct ToolCallExtractor {
    config: ToolCallExtractorConfig,
}

impl ToolCallExtractor {
    /// Create a new extractor with the given configuration.
    pub fn new(config: ToolCallExtractorConfig) -> Self {
        Self { config }
    }

    /// Create an extractor for OpenAI-style responses.
    pub fn openai() -> Self {
        Self::new(ToolCallExtractorConfig::openai())
    }

    /// Create an extractor for Anthropic-style responses.
    pub fn anthropic() -> Self {
        Self::new(ToolCallExtractorConfig::anthropic())
    }

    /// Create an extractor for Google-style responses.
    pub fn google() -> Self {
        Self::new(ToolCallExtractorConfig::google())
    }

    /// Extract tool calls from a response value.
    ///
    /// Returns a vector of normalized ToolCall instances.
    pub fn extract(&self, response: &Value) -> Vec<ToolCall> {
        let mut results = Vec::new();

        // Navigate to the tool calls location
        let tool_calls_value = match self.navigate_path(response, &self.config.path) {
            Some(v) => v,
            None => return results,
        };

        // Handle both array and single object cases
        let items: Vec<&Value> = if let Some(arr) = tool_calls_value.as_array() {
            arr.iter().collect()
        } else if tool_calls_value.is_object() {
            vec![tool_calls_value]
        } else {
            return results;
        };

        // Counter for synthetic IDs
        let mut id_counter = 0;

        for item in items {
            // Apply type filter if configured
            if let Some(ref filter) = self.config.type_filter {
                if !self.matches_type_filter(item, filter) {
                    continue;
                }
            }

            // Extract ID (generate synthetic if not present)
            let id = if self.config.id_path.is_empty() {
                id_counter += 1;
                format!("call_{}", id_counter)
            } else {
                self.navigate_path(item, &self.config.id_path)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        id_counter += 1;
                        format!("call_{}", id_counter)
                    })
            };

            // Extract name
            let name = match self.navigate_path(item, &self.config.name_path) {
                Some(v) => match v.as_str() {
                    Some(s) => s.to_string(),
                    None => continue, // Name is required
                },
                None => continue,
            };

            // Extract arguments
            let arguments = match self.navigate_path(item, &self.config.arguments_path) {
                Some(v) => {
                    if self.config.arguments_is_string {
                        // Parse stringified JSON
                        if let Some(s) = v.as_str() {
                            ToolCall::parse_arguments(s)
                        } else {
                            v.clone()
                        }
                    } else {
                        v.clone()
                    }
                }
                None => Value::Object(serde_json::Map::new()), // Default to empty object
            };

            results.push(ToolCall { id, name, arguments });
        }

        results
    }

    /// Extract a single tool call (convenience method for responses with exactly one call).
    pub fn extract_one(&self, response: &Value) -> Option<ToolCall> {
        let calls = self.extract(response);
        if calls.len() == 1 {
            calls.into_iter().next()
        } else {
            None
        }
    }

    /// Navigate a path like "choices[0].message.tool_calls" through a JSON value.
    fn navigate_path<'a>(&self, value: &'a Value, path: &str) -> Option<&'a Value> {
        if path.is_empty() {
            return Some(value);
        }

        let mut current = value;

        for segment in PathSegmentIterator::new(path) {
            current = match segment {
                PathSegment::Key(key) => current.get(key)?,
                PathSegment::Index(idx) => current.get(idx)?,
            };
        }

        Some(current)
    }

    /// Check if an item matches the type filter.
    fn matches_type_filter(&self, item: &Value, filter: &TypeFilter) -> bool {
        if filter.value.is_empty() {
            // Just check existence
            self.navigate_path(item, &filter.path).is_some()
        } else {
            // Check for specific value
            self.navigate_path(item, &filter.path)
                .and_then(|v| v.as_str())
                .map(|s| s == filter.value)
                .unwrap_or(false)
        }
    }
}

/// Path segment for navigation.
#[derive(Debug)]
enum PathSegment<'a> {
    Key(&'a str),
    Index(usize),
}

/// Iterator over path segments.
struct PathSegmentIterator<'a> {
    remaining: &'a str,
}

impl<'a> PathSegmentIterator<'a> {
    fn new(path: &'a str) -> Self {
        Self { remaining: path }
    }
}

impl<'a> Iterator for PathSegmentIterator<'a> {
    type Item = PathSegment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        // Check for array index notation
        if self.remaining.starts_with('[') {
            let end = self.remaining.find(']')?;
            let index_str = &self.remaining[1..end];
            let index: usize = index_str.parse().ok()?;

            // Advance past the closing bracket and optional dot
            self.remaining = &self.remaining[end + 1..];
            if self.remaining.starts_with('.') {
                self.remaining = &self.remaining[1..];
            }

            return Some(PathSegment::Index(index));
        }

        // Find the next separator (. or [)
        let next_dot = self.remaining.find('.');
        let next_bracket = self.remaining.find('[');

        let end = match (next_dot, next_bracket) {
            (Some(d), Some(b)) => d.min(b),
            (Some(d), None) => d,
            (None, Some(b)) => b,
            (None, None) => self.remaining.len(),
        };

        let key = &self.remaining[..end];
        self.remaining = &self.remaining[end..];

        // Skip the dot if present
        if self.remaining.starts_with('.') {
            self.remaining = &self.remaining[1..];
        }

        if key.is_empty() {
            self.next()
        } else {
            Some(PathSegment::Key(key))
        }
    }
}

// ============================================================================
// Streaming Tool Call Accumulator
// ============================================================================

/// Accumulator for building tool calls from streaming responses.
///
/// Some providers send tool calls incrementally during streaming:
/// - OpenAI sends tool calls in `choices[0].delta.tool_calls` with incremental `arguments`
/// - Anthropic sends `content_block_start` with tool info, then `partial_json` deltas
///
/// This accumulator collects these incremental pieces and builds complete ToolCalls.
#[derive(Debug, Clone, Default)]
pub struct StreamingToolCallAccumulator {
    /// Tool calls being accumulated, keyed by index or ID.
    in_progress: Vec<PartialToolCall>,
}

/// A tool call being accumulated from streaming chunks.
#[derive(Debug, Clone, Default)]
struct PartialToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments_buffer: String,
}

impl StreamingToolCallAccumulator {
    /// Create a new empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an OpenAI-style streaming delta.
    ///
    /// OpenAI streaming format for tool calls:
    /// ```json
    /// {"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "...", "function": {"name": "...", "arguments": "..."}}]}}]}
    /// ```
    pub fn process_openai_delta(&mut self, delta: &Value) {
        let tool_calls = delta
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("tool_calls"))
            .and_then(|tc| tc.as_array());

        if let Some(calls) = tool_calls {
            for call in calls {
                let index = call.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;

                // Ensure we have enough slots
                while self.in_progress.len() <= index {
                    self.in_progress.push(PartialToolCall::default());
                }

                let partial = &mut self.in_progress[index];

                // Update ID if present
                if let Some(id) = call.get("id").and_then(|i| i.as_str()) {
                    partial.id = Some(id.to_string());
                }

                // Update function info if present
                if let Some(function) = call.get("function") {
                    if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                        partial.name = Some(name.to_string());
                    }
                    if let Some(args) = function.get("arguments").and_then(|a| a.as_str()) {
                        partial.arguments_buffer.push_str(args);
                    }
                }
            }
        }
    }

    /// Process an Anthropic-style content block start.
    ///
    /// Anthropic streaming format:
    /// - `content_block_start`: `{"content_block": {"type": "tool_use", "id": "...", "name": "...", "input": {}}}`
    /// - `content_block_delta`: `{"delta": {"type": "input_json_delta", "partial_json": "..."}}`
    pub fn process_anthropic_block_start(&mut self, content_block: &Value) {
        if content_block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
            return;
        }

        let partial = PartialToolCall {
            id: content_block.get("id").and_then(|i| i.as_str()).map(String::from),
            name: content_block.get("name").and_then(|n| n.as_str()).map(String::from),
            arguments_buffer: String::new(),
        };

        self.in_progress.push(partial);
    }

    /// Process an Anthropic-style partial JSON delta.
    pub fn process_anthropic_delta(&mut self, delta: &Value) {
        if delta.get("type").and_then(|t| t.as_str()) != Some("input_json_delta") {
            return;
        }

        if let Some(partial_json) = delta.get("partial_json").and_then(|p| p.as_str()) {
            if let Some(last) = self.in_progress.last_mut() {
                last.arguments_buffer.push_str(partial_json);
            }
        }
    }

    /// Process a Google-style streaming chunk.
    ///
    /// Google sends complete function calls in streaming, not incrementally.
    pub fn process_google_chunk(&mut self, chunk: &Value) {
        let parts = chunk
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array());

        if let Some(parts) = parts {
            for part in parts {
                if let Some(function_call) = part.get("functionCall") {
                    let partial = PartialToolCall {
                        id: None, // Google doesn't provide IDs
                        name: function_call.get("name").and_then(|n| n.as_str()).map(String::from),
                        arguments_buffer: function_call
                            .get("args")
                            .map(|a| a.to_string())
                            .unwrap_or_default(),
                    };
                    self.in_progress.push(partial);
                }
            }
        }
    }

    /// Finalize and return all accumulated tool calls.
    ///
    /// This consumes the accumulator and returns complete ToolCall instances.
    pub fn finalize(self) -> Vec<ToolCall> {
        let mut id_counter = 0;
        self.in_progress
            .into_iter()
            .filter_map(|partial| {
                let name = partial.name?;

                let id = partial.id.unwrap_or_else(|| {
                    id_counter += 1;
                    format!("call_{}", id_counter)
                });

                let arguments = if partial.arguments_buffer.is_empty() {
                    Value::Object(serde_json::Map::new())
                } else {
                    ToolCall::parse_arguments(&partial.arguments_buffer)
                };

                Some(ToolCall { id, name, arguments })
            })
            .collect()
    }

    /// Check if there are any tool calls in progress.
    pub fn has_pending(&self) -> bool {
        !self.in_progress.is_empty()
    }

    /// Get the number of tool calls being accumulated.
    pub fn pending_count(&self) -> usize {
        self.in_progress.len()
    }

    /// Clear all accumulated data.
    pub fn clear(&mut self) {
        self.in_progress.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // OpenAI Tests
    // ========================================================================

    #[test]
    fn test_openai_single_tool_call() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\": \"San Francisco\", \"unit\": \"celsius\"}"
                        }
                    }]
                }
            }]
        });

        let calls = extractor.extract(&response);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_abc123");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["location"], "San Francisco");
        assert_eq!(calls[0].arguments["unit"], "celsius");
    }

    #[test]
    fn test_openai_multiple_tool_calls() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"location\": \"NYC\"}"
                            }
                        },
                        {
                            "id": "call_2",
                            "type": "function",
                            "function": {
                                "name": "get_time",
                                "arguments": "{\"timezone\": \"EST\"}"
                            }
                        }
                    ]
                }
            }]
        });

        let calls = extractor.extract(&response);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[1].name, "get_time");
    }

    #[test]
    fn test_openai_no_tool_calls() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello, how can I help you?"
                }
            }]
        });

        let calls = extractor.extract(&response);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_openai_malformed_arguments() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "test_func",
                            "arguments": "not valid json"
                        }
                    }]
                }
            }]
        });

        let calls = extractor.extract(&response);

        assert_eq!(calls.len(), 1);
        // Malformed JSON becomes a string value
        assert_eq!(calls[0].arguments, Value::String("not valid json".to_string()));
    }

    // ========================================================================
    // Anthropic Tests
    // ========================================================================

    #[test]
    fn test_anthropic_single_tool_use() {
        let extractor = ToolCallExtractor::anthropic();

        let response = json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "I'll check the weather for you."
                },
                {
                    "type": "tool_use",
                    "id": "toolu_abc123",
                    "name": "get_weather",
                    "input": {
                        "location": "San Francisco",
                        "unit": "celsius"
                    }
                }
            ],
            "stop_reason": "tool_use"
        });

        let calls = extractor.extract(&response);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "toolu_abc123");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["location"], "San Francisco");
        assert_eq!(calls[0].arguments["unit"], "celsius");
    }

    #[test]
    fn test_anthropic_multiple_tool_uses() {
        let extractor = ToolCallExtractor::anthropic();

        let response = json!({
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "get_weather",
                    "input": {"location": "NYC"}
                },
                {
                    "type": "text",
                    "text": "And also..."
                },
                {
                    "type": "tool_use",
                    "id": "toolu_2",
                    "name": "get_time",
                    "input": {"timezone": "EST"}
                }
            ]
        });

        let calls = extractor.extract(&response);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[1].name, "get_time");
    }

    #[test]
    fn test_anthropic_text_only() {
        let extractor = ToolCallExtractor::anthropic();

        let response = json!({
            "content": [
                {
                    "type": "text",
                    "text": "Hello, I'm Claude!"
                }
            ]
        });

        let calls = extractor.extract(&response);
        assert!(calls.is_empty());
    }

    // ========================================================================
    // Google Tests
    // ========================================================================

    #[test]
    fn test_google_function_call() {
        let extractor = ToolCallExtractor::google();

        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "get_weather",
                            "args": {
                                "location": "San Francisco",
                                "unit": "celsius"
                            }
                        }
                    }],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });

        let calls = extractor.extract(&response);

        assert_eq!(calls.len(), 1);
        assert!(calls[0].id.starts_with("call_")); // Synthetic ID
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["location"], "San Francisco");
    }

    #[test]
    fn test_google_text_only() {
        let extractor = ToolCallExtractor::google();

        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "Hello! How can I help you today?"
                    }],
                    "role": "model"
                }
            }]
        });

        let calls = extractor.extract(&response);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_google_multiple_function_calls() {
        let extractor = ToolCallExtractor::google();

        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {
                            "functionCall": {
                                "name": "get_weather",
                                "args": {"location": "NYC"}
                            }
                        },
                        {
                            "functionCall": {
                                "name": "get_time",
                                "args": {"timezone": "EST"}
                            }
                        }
                    ]
                }
            }]
        });

        let calls = extractor.extract(&response);

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[1].name, "get_time");
    }

    // ========================================================================
    // Builder Tests
    // ========================================================================

    #[test]
    fn test_builder_custom_config() {
        let config = ToolCallExtractorBuilder::new()
            .path("result.tools")
            .id_path("tool_id")
            .name_path("tool_name")
            .arguments_path("params")
            .arguments_is_string(false)
            .build();

        assert_eq!(config.path, "result.tools");
        assert_eq!(config.id_path, "tool_id");
        assert_eq!(config.name_path, "tool_name");
        assert_eq!(config.arguments_path, "params");
        assert!(!config.arguments_is_string);
    }

    #[test]
    fn test_builder_with_type_filter() {
        let config = ToolCallExtractorBuilder::new()
            .path("content")
            .type_filter("type", "function_call")
            .build();

        assert!(config.type_filter.is_some());
        let filter = config.type_filter.unwrap();
        assert_eq!(filter.path, "type");
        assert_eq!(filter.value, "function_call");
    }

    #[test]
    fn test_builder_presets() {
        let openai = ToolCallExtractorBuilder::openai().build();
        assert!(openai.arguments_is_string);
        assert!(openai.path.contains("tool_calls"));

        let anthropic = ToolCallExtractorBuilder::anthropic().build();
        assert!(!anthropic.arguments_is_string);
        assert!(anthropic.type_filter.is_some());

        let google = ToolCallExtractorBuilder::google().build();
        assert!(!google.arguments_is_string);
        assert!(google.name_path.contains("functionCall"));
    }

    // ========================================================================
    // Streaming Accumulator Tests
    // ========================================================================

    #[test]
    fn test_streaming_openai_accumulator() {
        let mut acc = StreamingToolCallAccumulator::new();

        // First chunk - starts the tool call
        acc.process_openai_delta(&json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_streaming_1",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"loc"
                        }
                    }]
                }
            }]
        }));

        // Second chunk - continues arguments
        acc.process_openai_delta(&json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": {
                            "arguments": "ation\": \"SF\"}"
                        }
                    }]
                }
            }]
        }));

        assert!(acc.has_pending());
        assert_eq!(acc.pending_count(), 1);

        let calls = acc.finalize();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_streaming_1");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["location"], "SF");
    }

    #[test]
    fn test_streaming_openai_multiple_parallel() {
        let mut acc = StreamingToolCallAccumulator::new();

        // Both tool calls start in the same chunk
        acc.process_openai_delta(&json!({
            "choices": [{
                "delta": {
                    "tool_calls": [
                        {
                            "index": 0,
                            "id": "call_1",
                            "function": {"name": "func_a", "arguments": "{\"x\": "}
                        },
                        {
                            "index": 1,
                            "id": "call_2",
                            "function": {"name": "func_b", "arguments": "{\"y\": "}
                        }
                    ]
                }
            }]
        }));

        // Continue both
        acc.process_openai_delta(&json!({
            "choices": [{
                "delta": {
                    "tool_calls": [
                        {"index": 0, "function": {"arguments": "1}"}},
                        {"index": 1, "function": {"arguments": "2}"}}
                    ]
                }
            }]
        }));

        let calls = acc.finalize();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].arguments["x"], 1);
        assert_eq!(calls[1].arguments["y"], 2);
    }

    #[test]
    fn test_streaming_anthropic_accumulator() {
        let mut acc = StreamingToolCallAccumulator::new();

        // content_block_start event
        acc.process_anthropic_block_start(&json!({
            "type": "tool_use",
            "id": "toolu_streaming_1",
            "name": "get_weather",
            "input": {}
        }));

        // Partial JSON deltas
        acc.process_anthropic_delta(&json!({
            "type": "input_json_delta",
            "partial_json": "{\"location\":"
        }));

        acc.process_anthropic_delta(&json!({
            "type": "input_json_delta",
            "partial_json": " \"Boston\"}"
        }));

        let calls = acc.finalize();

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "toolu_streaming_1");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["location"], "Boston");
    }

    #[test]
    fn test_streaming_google_accumulator() {
        let mut acc = StreamingToolCallAccumulator::new();

        // Google sends complete function calls in each chunk
        acc.process_google_chunk(&json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "search",
                            "args": {"query": "rust programming"}
                        }
                    }]
                }
            }]
        }));

        let calls = acc.finalize();

        assert_eq!(calls.len(), 1);
        assert!(calls[0].id.starts_with("call_")); // Synthetic ID
        assert_eq!(calls[0].name, "search");
        assert_eq!(calls[0].arguments["query"], "rust programming");
    }

    #[test]
    fn test_streaming_empty_accumulator() {
        let acc = StreamingToolCallAccumulator::new();
        assert!(!acc.has_pending());

        let calls = acc.finalize();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_streaming_accumulator_clear() {
        let mut acc = StreamingToolCallAccumulator::new();

        acc.process_openai_delta(&json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "function": {"name": "test", "arguments": "{}"}
                    }]
                }
            }]
        }));

        assert!(acc.has_pending());
        acc.clear();
        assert!(!acc.has_pending());
    }

    // ========================================================================
    // Path Navigation Tests
    // ========================================================================

    #[test]
    fn test_path_navigation_simple() {
        let extractor = ToolCallExtractor::openai();
        let value = json!({"a": {"b": {"c": 42}}});

        let result = extractor.navigate_path(&value, "a.b.c");
        assert_eq!(result, Some(&json!(42)));
    }

    #[test]
    fn test_path_navigation_with_array() {
        let extractor = ToolCallExtractor::openai();
        let value = json!({"items": [{"name": "first"}, {"name": "second"}]});

        let result = extractor.navigate_path(&value, "items[1].name");
        assert_eq!(result, Some(&json!("second")));
    }

    #[test]
    fn test_path_navigation_empty() {
        let extractor = ToolCallExtractor::openai();
        let value = json!({"test": 123});

        let result = extractor.navigate_path(&value, "");
        assert_eq!(result, Some(&value));
    }

    #[test]
    fn test_path_navigation_missing() {
        let extractor = ToolCallExtractor::openai();
        let value = json!({"a": 1});

        let result = extractor.navigate_path(&value, "b.c.d");
        assert!(result.is_none());
    }

    // ========================================================================
    // ToolCall Constructor Tests
    // ========================================================================

    #[test]
    fn test_tool_call_new() {
        let call = ToolCall::new("id_123", "my_function", json!({"param": "value"}));

        assert_eq!(call.id, "id_123");
        assert_eq!(call.name, "my_function");
        assert_eq!(call.arguments["param"], "value");
    }

    #[test]
    fn test_tool_call_parse_arguments_valid() {
        let parsed = ToolCall::parse_arguments("{\"key\": \"value\", \"num\": 42}");

        assert!(parsed.is_object());
        assert_eq!(parsed["key"], "value");
        assert_eq!(parsed["num"], 42);
    }

    #[test]
    fn test_tool_call_parse_arguments_invalid() {
        let parsed = ToolCall::parse_arguments("not json");

        assert!(parsed.is_string());
        assert_eq!(parsed.as_str(), Some("not json"));
    }

    // ========================================================================
    // Edge Cases
    // ========================================================================

    #[test]
    fn test_extract_one_single_call() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "test", "arguments": "{}"}
                    }]
                }
            }]
        });

        let call = extractor.extract_one(&response);
        assert!(call.is_some());
        assert_eq!(call.unwrap().name, "test");
    }

    #[test]
    fn test_extract_one_multiple_calls() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {
                    "tool_calls": [
                        {"id": "1", "type": "function", "function": {"name": "a", "arguments": "{}"}},
                        {"id": "2", "type": "function", "function": {"name": "b", "arguments": "{}"}}
                    ]
                }
            }]
        });

        // Returns None when there's more than one call
        let call = extractor.extract_one(&response);
        assert!(call.is_none());
    }

    #[test]
    fn test_extract_one_no_calls() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {"content": "Hello!"}
            }]
        });

        let call = extractor.extract_one(&response);
        assert!(call.is_none());
    }

    #[test]
    fn test_empty_arguments() {
        let extractor = ToolCallExtractor::openai();

        let response = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "no_args_func"
                            // arguments field is missing
                        }
                    }]
                }
            }]
        });

        let calls = extractor.extract(&response);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].arguments.is_object());
        assert!(calls[0].arguments.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_tool_call_serialization() {
        let call = ToolCall {
            id: "call_123".to_string(),
            name: "my_func".to_string(),
            arguments: json!({"x": 1, "y": 2}),
        };

        let serialized = serde_json::to_string(&call).unwrap();
        let deserialized: ToolCall = serde_json::from_str(&serialized).unwrap();

        assert_eq!(call, deserialized);
    }
}
