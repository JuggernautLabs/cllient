//! Event action system for SSE parsing
//!
//! This module defines what happens when different events are extracted from
//! the SSE stream. It provides a configuration-driven approach to mapping
//! SSE events to semantic actions that can be handled uniformly across providers.
//!
//! ## Architecture
//!
//! The event action system consists of:
//! - `EventAction` - Enumeration of all possible actions (content, tool_call, etc.)
//! - `EventHandler` - Maps an SSE event pattern to an action with extraction rules
//! - `EventHandlerBuilder` - Fluent builder for constructing event handlers
//!
//! ## Usage
//!
//! ```rust,ignore
//! use cllient::streaming::sse::actions::{EventAction, EventHandler};
//!
//! // OpenAI-style content extraction
//! let handler = EventHandler::builder()
//!     .extract_path("choices[0].delta.content")
//!     .filter_path("choices[0].delta.content")
//!     .action(EventAction::Content)
//!     .build();
//!
//! // Anthropic-style with event type matching
//! let handler = EventHandler::builder()
//!     .event_type("content_block_delta")
//!     .extract_path("delta.text")
//!     .action(EventAction::Content)
//!     .build();
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::Arc;

use super::extractors::{JsonPath, JsonPathSegment};

// =============================================================================
// EventAction - Semantic actions that can be taken when an SSE event matches
// =============================================================================

/// Actions that can be taken when an SSE event matches a handler pattern.
///
/// These represent semantic operations in the streaming response lifecycle,
/// unified across different provider formats. The action determines how
/// the extracted data should be processed and emitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventAction {
    /// Emit content token - the primary text output from the model.
    /// Extracted value should be a string token.
    Content,

    /// Start of a new message with role information.
    /// Extracted value should be the role string (e.g., "assistant").
    RoleStart,

    /// Tool/function call data.
    /// Extracted value contains tool call information (id, name, arguments).
    ToolCall,

    /// Stream finished normally.
    /// May include finish reason in extracted value.
    Finish,

    /// Usage statistics update.
    /// Extracted value should contain token counts.
    UsageUpdate,

    /// Error occurred in the stream.
    /// Extracted value should contain error details.
    Error,

    /// Message start (Anthropic-specific).
    /// Contains initial message metadata including model, usage, etc.
    MessageStart,

    /// Message delta (Anthropic-specific).
    /// Contains message-level updates like stop_reason and usage.
    MessageDelta,

    /// Content block start (Anthropic-specific).
    /// Signals beginning of a new content block with type information.
    ContentBlockStart,

    /// Content block delta (Anthropic-specific).
    /// Contains incremental content updates within a block.
    ContentBlockDelta,

    /// Content block stop (Anthropic-specific).
    /// Signals end of the current content block.
    ContentBlockStop,

    /// Ping/keepalive event (ignore for processing).
    /// Used by some providers to maintain connection.
    Ping,

    /// Stream start event (generic).
    /// Used when message_start isn't appropriate.
    Start,

    /// Stream stop event (generic).
    /// Used when finish isn't appropriate.
    Stop,

    /// Custom action with a user-defined name.
    /// For provider-specific events not covered by standard actions.
    Custom(String),
}

impl Default for EventAction {
    fn default() -> Self {
        EventAction::Content
    }
}

impl fmt::Display for EventAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventAction::Content => write!(f, "content"),
            EventAction::RoleStart => write!(f, "role_start"),
            EventAction::ToolCall => write!(f, "tool_call"),
            EventAction::Finish => write!(f, "finish"),
            EventAction::UsageUpdate => write!(f, "usage_update"),
            EventAction::Error => write!(f, "error"),
            EventAction::MessageStart => write!(f, "message_start"),
            EventAction::MessageDelta => write!(f, "message_delta"),
            EventAction::ContentBlockStart => write!(f, "content_block_start"),
            EventAction::ContentBlockDelta => write!(f, "content_block_delta"),
            EventAction::ContentBlockStop => write!(f, "content_block_stop"),
            EventAction::Ping => write!(f, "ping"),
            EventAction::Start => write!(f, "start"),
            EventAction::Stop => write!(f, "stop"),
            EventAction::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

impl EventAction {
    /// Returns true if this action represents content that should be emitted.
    pub fn is_content_action(&self) -> bool {
        matches!(
            self,
            EventAction::Content | EventAction::ContentBlockDelta
        )
    }

    /// Returns true if this action indicates the stream has finished.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EventAction::Finish | EventAction::Stop | EventAction::Error
        )
    }

    /// Returns true if this action should be ignored for content processing.
    pub fn is_ignorable(&self) -> bool {
        matches!(self, EventAction::Ping)
    }

    /// Parse an action from a string (for YAML config parsing).
    pub fn from_str(s: &str) -> Self {
        match s {
            "content" => EventAction::Content,
            "role_start" => EventAction::RoleStart,
            "tool_call" => EventAction::ToolCall,
            "finish" => EventAction::Finish,
            "usage_update" => EventAction::UsageUpdate,
            "error" => EventAction::Error,
            "message_start" => EventAction::MessageStart,
            "message_delta" => EventAction::MessageDelta,
            "content_block_start" | "content_start" => EventAction::ContentBlockStart,
            "content_block_delta" => EventAction::ContentBlockDelta,
            "content_block_stop" | "content_stop" => EventAction::ContentBlockStop,
            "ping" => EventAction::Ping,
            "start" => EventAction::Start,
            "stop" => EventAction::Stop,
            other => EventAction::Custom(other.to_string()),
        }
    }
}

// =============================================================================
// Transform function type
// =============================================================================

/// A transform function that can modify extracted values before emission.
///
/// This allows for value transformations like:
/// - Extracting nested fields
/// - Type conversions
/// - Filtering or mapping arrays
/// - Combining multiple fields
pub type TransformFn = Arc<dyn Fn(Value) -> Value + Send + Sync>;

/// Wrapper for TransformFn that provides Debug implementation
#[derive(Clone)]
pub struct Transform(pub TransformFn);

impl fmt::Debug for Transform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Transform(<fn>)")
    }
}

impl Transform {
    /// Create a new transform from a function
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Value) -> Value + Send + Sync + 'static,
    {
        Transform(Arc::new(f))
    }

    /// Apply the transform to a value
    pub fn apply(&self, value: Value) -> Value {
        (self.0)(value)
    }
}

// =============================================================================
// EventHandler - Maps SSE event patterns to actions
// =============================================================================

/// Handler for a specific SSE event type.
///
/// An EventHandler defines:
/// - How to identify matching events (event_type, filter_path)
/// - What data to extract (extract_path)
/// - What action to take (action)
/// - Optional transformation of extracted data (transform)
///
/// ## Example Configuration (from YAML)
///
/// ```yaml
/// events:
///   - type: content_block_delta
///     extract: delta.text
///     action: content
///     filter: delta.text
///
///   - extract: choices[0].delta.content
///     action: content
///     filter: choices[0].delta.content
/// ```
#[derive(Debug, Clone)]
pub struct EventHandler {
    /// Path to extract data from the SSE payload.
    /// Uses dot notation with array index support: "choices[0].delta.content"
    extract_path: JsonPath,

    /// Optional path that must exist/be non-null for this handler to match.
    /// Used when multiple handlers need to discriminate based on presence of fields.
    filter_path: Option<JsonPath>,

    /// Optional event type that must match (e.g., "content_block_delta" for Anthropic).
    /// If None, the handler matches any event type.
    event_type: Option<String>,

    /// The action to take when this handler matches.
    action: EventAction,

    /// Optional transform to apply to the extracted value.
    transform: Option<Transform>,
}

impl EventHandler {
    /// Create a new builder for constructing an EventHandler
    pub fn builder() -> EventHandlerBuilder {
        EventHandlerBuilder::new()
    }

    /// Get the extraction path
    pub fn extract_path(&self) -> &JsonPath {
        &self.extract_path
    }

    /// Get the filter path if present
    pub fn filter_path(&self) -> Option<&JsonPath> {
        self.filter_path.as_ref()
    }

    /// Get the event type filter if present
    pub fn event_type(&self) -> Option<&str> {
        self.event_type.as_deref()
    }

    /// Get the action for this handler
    pub fn action(&self) -> &EventAction {
        &self.action
    }

    /// Check if this handler matches the given event
    ///
    /// Returns true if:
    /// 1. event_type matches (or handler has no event_type filter)
    /// 2. filter_path exists in the payload (or handler has no filter_path)
    pub fn matches(&self, event_type: Option<&str>, payload: &Value) -> bool {
        // Check event type if specified
        if let Some(expected_type) = &self.event_type {
            match event_type {
                Some(actual_type) if actual_type == expected_type => {}
                _ => return false,
            }
        }

        // Check filter path if specified
        if let Some(filter) = &self.filter_path {
            if filter.navigate(payload).is_none() {
                return false;
            }
        }

        true
    }

    /// Extract data from the payload using this handler's configuration
    ///
    /// Returns None if the extraction path doesn't exist in the payload
    pub fn extract(&self, payload: &Value) -> Option<Value> {
        let extracted = self.extract_path.navigate(payload)?.clone();

        // Apply transform if present
        match &self.transform {
            Some(transform) => Some(transform.apply(extracted)),
            None => Some(extracted),
        }
    }

    /// Execute this handler on a payload, returning the action and extracted value
    ///
    /// Returns None if the handler doesn't match or extraction fails
    pub fn execute(&self, event_type: Option<&str>, payload: &Value) -> Option<(EventAction, Value)> {
        if !self.matches(event_type, payload) {
            return None;
        }

        self.extract(payload).map(|value| (self.action.clone(), value))
    }
}

// =============================================================================
// EventHandlerBuilder - Fluent builder for EventHandler
// =============================================================================

/// Builder for constructing EventHandler instances.
///
/// Provides a fluent API for building handlers:
///
/// ```rust,ignore
/// let handler = EventHandler::builder()
///     .event_type("content_block_delta")
///     .extract_path("delta.text")
///     .filter_path("delta.text")
///     .action(EventAction::Content)
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct EventHandlerBuilder {
    extract_path: Option<JsonPath>,
    filter_path: Option<JsonPath>,
    event_type: Option<String>,
    action: EventAction,
    transform: Option<Transform>,
}

impl EventHandlerBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the extraction path from a string.
    ///
    /// Supports dot notation and array indexing:
    /// - "choices[0].delta.content"
    /// - "delta.text"
    /// - "usage.input_tokens"
    pub fn extract_path(mut self, path: &str) -> Self {
        self.extract_path = Some(parse_json_path(path));
        self
    }

    /// Set the extraction path from an existing JsonPath
    pub fn extract_path_raw(mut self, path: JsonPath) -> Self {
        self.extract_path = Some(path);
        self
    }

    /// Set the filter path from a string.
    ///
    /// The handler will only match if this path exists in the payload.
    pub fn filter_path(mut self, path: &str) -> Self {
        self.filter_path = Some(parse_json_path(path));
        self
    }

    /// Set the filter path from an existing JsonPath
    pub fn filter_path_raw(mut self, path: JsonPath) -> Self {
        self.filter_path = Some(path);
        self
    }

    /// Set the event type filter.
    ///
    /// Used for providers like Anthropic that send typed events.
    pub fn event_type(mut self, event_type: &str) -> Self {
        self.event_type = Some(event_type.to_string());
        self
    }

    /// Set the action to take when this handler matches.
    pub fn action(mut self, action: EventAction) -> Self {
        self.action = action;
        self
    }

    /// Set a transform function for the extracted value.
    pub fn transform<F>(mut self, f: F) -> Self
    where
        F: Fn(Value) -> Value + Send + Sync + 'static,
    {
        self.transform = Some(Transform::new(f));
        self
    }

    /// Build the EventHandler.
    ///
    /// # Panics
    ///
    /// Panics if `extract_path` was not set.
    pub fn build(self) -> EventHandler {
        EventHandler {
            extract_path: self.extract_path.expect("extract_path is required"),
            filter_path: self.filter_path,
            event_type: self.event_type,
            action: self.action,
            transform: self.transform,
        }
    }

    /// Build the EventHandler, returning None if required fields are missing.
    pub fn try_build(self) -> Option<EventHandler> {
        Some(EventHandler {
            extract_path: self.extract_path?,
            filter_path: self.filter_path,
            event_type: self.event_type,
            action: self.action,
            transform: self.transform,
        })
    }
}

// =============================================================================
// EventHandlerSet - Collection of handlers for a provider
// =============================================================================

/// A collection of event handlers that can be applied to SSE events.
///
/// Handlers are evaluated in order; the first matching handler wins.
#[derive(Debug, Clone, Default)]
pub struct EventHandlerSet {
    handlers: Vec<EventHandler>,
}

impl EventHandlerSet {
    /// Create a new empty handler set
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a handler to the set
    pub fn add(&mut self, handler: EventHandler) {
        self.handlers.push(handler);
    }

    /// Add a handler and return self for chaining
    pub fn with(mut self, handler: EventHandler) -> Self {
        self.add(handler);
        self
    }

    /// Find the first matching handler and execute it
    pub fn process(&self, event_type: Option<&str>, payload: &Value) -> Option<(EventAction, Value)> {
        for handler in &self.handlers {
            if let Some(result) = handler.execute(event_type, payload) {
                return Some(result);
            }
        }
        None
    }

    /// Get all handlers
    pub fn handlers(&self) -> &[EventHandler] {
        &self.handlers
    }

    /// Get the number of handlers
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Check if the handler set is empty
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

// =============================================================================
// Pre-built handler sets for common providers
// =============================================================================

/// Create the OpenAI event handler set
///
/// OpenAI SSE format uses a single event type with different content fields:
/// - Content: choices[0].delta.content
/// - Role: choices[0].delta.role
/// - Finish: choices[0].finish_reason
/// - Tool calls: choices[0].delta.tool_calls
/// - Usage: usage (with stream_options.include_usage=true)
pub fn openai_handlers() -> EventHandlerSet {
    EventHandlerSet::new()
        .with(
            EventHandler::builder()
                .extract_path("choices[0].delta.content")
                .filter_path("choices[0].delta.content")
                .action(EventAction::Content)
                .build(),
        )
        .with(
            EventHandler::builder()
                .extract_path("choices[0].delta.role")
                .filter_path("choices[0].delta.role")
                .action(EventAction::RoleStart)
                .build(),
        )
        .with(
            EventHandler::builder()
                .extract_path("choices[0].finish_reason")
                .filter_path("choices[0].finish_reason")
                .action(EventAction::Finish)
                .build(),
        )
        .with(
            EventHandler::builder()
                .extract_path("choices[0].delta.tool_calls")
                .filter_path("choices[0].delta.tool_calls")
                .action(EventAction::ToolCall)
                .build(),
        )
        .with(
            EventHandler::builder()
                .extract_path("usage")
                .filter_path("usage")
                .action(EventAction::UsageUpdate)
                .build(),
        )
}

/// Create the Anthropic/Claude event handler set
///
/// Anthropic SSE format uses typed events with different payloads:
/// - message_start: Initial message with model/usage info
/// - content_block_start: New content block beginning
/// - content_block_delta: Incremental content (text or tool input)
/// - content_block_stop: Content block finished
/// - message_delta: Message-level updates (stop_reason, usage)
/// - message_stop: Stream complete
/// - error: Error occurred
/// - ping: Keepalive
pub fn anthropic_handlers() -> EventHandlerSet {
    EventHandlerSet::new()
        .with(
            EventHandler::builder()
                .event_type("message_start")
                .extract_path("message")
                .action(EventAction::MessageStart)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("content_block_start")
                .extract_path("content_block")
                .action(EventAction::ContentBlockStart)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("content_block_delta")
                .extract_path("delta.text")
                .filter_path("delta.text")
                .action(EventAction::Content)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("content_block_delta")
                .extract_path("delta.partial_json")
                .filter_path("delta.partial_json")
                .action(EventAction::ToolCall)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("content_block_stop")
                .extract_path("index")
                .action(EventAction::ContentBlockStop)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("message_delta")
                .extract_path("delta")
                .action(EventAction::MessageDelta)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("message_stop")
                .extract_path("type")
                .action(EventAction::Stop)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("error")
                .extract_path("error")
                .action(EventAction::Error)
                .build(),
        )
        .with(
            EventHandler::builder()
                .event_type("ping")
                .extract_path("type")
                .action(EventAction::Ping)
                .build(),
        )
}

// =============================================================================
// Path parsing utilities
// =============================================================================

/// Parse a JSON path string into a JsonPath structure.
///
/// Supports:
/// - Dot notation: "foo.bar.baz"
/// - Array indexing: "choices[0]"
/// - Combined: "choices[0].delta.content"
pub fn parse_json_path(path: &str) -> JsonPath {
    let mut segments = Vec::new();

    for part in path.split('.') {
        if part.is_empty() {
            continue;
        }

        // Check for array index notation: "choices[0]"
        if let Some(bracket_pos) = part.find('[') {
            // Key part before bracket
            let key = &part[..bracket_pos];
            if !key.is_empty() {
                segments.push(JsonPathSegment::Key(leak_str(key)));
            }

            // Parse index
            if let Some(close_pos) = part.find(']') {
                let index_str = &part[bracket_pos + 1..close_pos];
                if let Ok(index) = index_str.parse::<usize>() {
                    segments.push(JsonPathSegment::Index(index));
                }
            }
        } else {
            segments.push(JsonPathSegment::Key(leak_str(part)));
        }
    }

    JsonPath(segments)
}

/// Leak a string to get a static reference.
/// This is safe because these paths are typically created once at startup.
fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

// =============================================================================
// Serialization support for EventHandler
// =============================================================================

/// Serializable representation of an EventHandler for YAML config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventHandlerConfig {
    /// The extraction path (required)
    pub extract: String,

    /// The filter path (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,

    /// The event type to match (optional, for Anthropic-style events)
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,

    /// The action to take
    pub action: String,
}

impl From<EventHandlerConfig> for EventHandler {
    fn from(config: EventHandlerConfig) -> Self {
        let mut builder = EventHandler::builder()
            .extract_path(&config.extract)
            .action(EventAction::from_str(&config.action));

        if let Some(filter) = config.filter {
            builder = builder.filter_path(&filter);
        }

        if let Some(event_type) = config.event_type {
            builder = builder.event_type(&event_type);
        }

        builder.build()
    }
}

impl From<&EventHandler> for EventHandlerConfig {
    fn from(handler: &EventHandler) -> Self {
        EventHandlerConfig {
            extract: json_path_to_string(handler.extract_path()),
            filter: handler.filter_path().map(json_path_to_string),
            event_type: handler.event_type().map(String::from),
            action: handler.action().to_string(),
        }
    }
}

/// Convert a JsonPath back to a string representation
fn json_path_to_string(path: &JsonPath) -> String {
    let mut result = String::new();

    for (i, segment) in path.0.iter().enumerate() {
        match segment {
            JsonPathSegment::Key(key) => {
                if i > 0 && !result.ends_with(']') {
                    result.push('.');
                }
                result.push_str(key);
            }
            JsonPathSegment::Index(idx) => {
                result.push('[');
                result.push_str(&idx.to_string());
                result.push(']');
            }
        }
    }

    result
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    mod event_action_tests {
        use super::*;

        #[test]
        fn test_event_action_serialization() {
            assert_eq!(
                serde_json::to_string(&EventAction::Content).unwrap(),
                "\"content\""
            );
            assert_eq!(
                serde_json::to_string(&EventAction::ToolCall).unwrap(),
                "\"tool_call\""
            );
            assert_eq!(
                serde_json::to_string(&EventAction::UsageUpdate).unwrap(),
                "\"usage_update\""
            );
            assert_eq!(
                serde_json::to_string(&EventAction::Custom("my_action".to_string())).unwrap(),
                "{\"custom\":\"my_action\"}"
            );
        }

        #[test]
        fn test_event_action_deserialization() {
            assert_eq!(
                serde_json::from_str::<EventAction>("\"content\"").unwrap(),
                EventAction::Content
            );
            assert_eq!(
                serde_json::from_str::<EventAction>("\"message_start\"").unwrap(),
                EventAction::MessageStart
            );
            assert_eq!(
                serde_json::from_str::<EventAction>("{\"custom\":\"foo\"}").unwrap(),
                EventAction::Custom("foo".to_string())
            );
        }

        #[test]
        fn test_event_action_from_str() {
            assert_eq!(EventAction::from_str("content"), EventAction::Content);
            assert_eq!(EventAction::from_str("tool_call"), EventAction::ToolCall);
            assert_eq!(EventAction::from_str("message_start"), EventAction::MessageStart);
            assert_eq!(EventAction::from_str("content_start"), EventAction::ContentBlockStart);
            assert_eq!(
                EventAction::from_str("my_custom"),
                EventAction::Custom("my_custom".to_string())
            );
        }

        #[test]
        fn test_event_action_display() {
            assert_eq!(EventAction::Content.to_string(), "content");
            assert_eq!(EventAction::MessageDelta.to_string(), "message_delta");
            assert_eq!(
                EventAction::Custom("test".to_string()).to_string(),
                "custom:test"
            );
        }

        #[test]
        fn test_event_action_predicates() {
            assert!(EventAction::Content.is_content_action());
            assert!(EventAction::ContentBlockDelta.is_content_action());
            assert!(!EventAction::ToolCall.is_content_action());

            assert!(EventAction::Finish.is_terminal());
            assert!(EventAction::Stop.is_terminal());
            assert!(EventAction::Error.is_terminal());
            assert!(!EventAction::Content.is_terminal());

            assert!(EventAction::Ping.is_ignorable());
            assert!(!EventAction::Content.is_ignorable());
        }
    }

    mod json_path_tests {
        use super::*;

        #[test]
        fn test_parse_simple_path() {
            let path = parse_json_path("delta.text");
            assert_eq!(path.0.len(), 2);

            let payload = json!({"delta": {"text": "hello"}});
            let result = path.navigate(&payload);
            assert_eq!(result, Some(&json!("hello")));
        }

        #[test]
        fn test_parse_array_path() {
            let path = parse_json_path("choices[0].delta.content");
            assert_eq!(path.0.len(), 4);

            let payload = json!({
                "choices": [{"delta": {"content": "token"}}]
            });
            let result = path.navigate(&payload);
            assert_eq!(result, Some(&json!("token")));
        }

        #[test]
        fn test_parse_nested_array_path() {
            let path = parse_json_path("data[0].items[2].value");

            let payload = json!({
                "data": [
                    {"items": ["a", "b", {"value": "found"}]}
                ]
            });
            let result = path.navigate(&payload);
            assert_eq!(result, Some(&json!("found")));
        }

        #[test]
        fn test_path_navigation_missing() {
            let path = parse_json_path("foo.bar.baz");
            let payload = json!({"foo": {"other": 1}});
            assert_eq!(path.navigate(&payload), None);
        }

        #[test]
        fn test_json_path_to_string() {
            let path = parse_json_path("choices[0].delta.content");
            let string = json_path_to_string(&path);
            assert_eq!(string, "choices[0].delta.content");
        }
    }

    mod event_handler_tests {
        use super::*;

        #[test]
        fn test_builder_basic() {
            let handler = EventHandler::builder()
                .extract_path("delta.text")
                .action(EventAction::Content)
                .build();

            assert_eq!(handler.action(), &EventAction::Content);
            assert!(handler.filter_path().is_none());
            assert!(handler.event_type().is_none());
        }

        #[test]
        fn test_builder_with_filter() {
            let handler = EventHandler::builder()
                .extract_path("choices[0].delta.content")
                .filter_path("choices[0].delta.content")
                .action(EventAction::Content)
                .build();

            assert!(handler.filter_path().is_some());
        }

        #[test]
        fn test_builder_with_event_type() {
            let handler = EventHandler::builder()
                .event_type("content_block_delta")
                .extract_path("delta.text")
                .action(EventAction::Content)
                .build();

            assert_eq!(handler.event_type(), Some("content_block_delta"));
        }

        #[test]
        fn test_builder_with_transform() {
            let handler = EventHandler::builder()
                .extract_path("value")
                .action(EventAction::Content)
                .transform(|v| {
                    if let Some(s) = v.as_str() {
                        Value::String(s.to_uppercase())
                    } else {
                        v
                    }
                })
                .build();

            let payload = json!({"value": "hello"});
            let extracted = handler.extract(&payload);
            assert_eq!(extracted, Some(json!("HELLO")));
        }

        #[test]
        #[should_panic(expected = "extract_path is required")]
        fn test_builder_missing_path() {
            EventHandler::builder()
                .action(EventAction::Content)
                .build();
        }

        #[test]
        fn test_try_build_missing_path() {
            let result = EventHandler::builder()
                .action(EventAction::Content)
                .try_build();
            assert!(result.is_none());
        }

        #[test]
        fn test_handler_matches_no_filters() {
            let handler = EventHandler::builder()
                .extract_path("content")
                .action(EventAction::Content)
                .build();

            let payload = json!({"content": "test"});
            assert!(handler.matches(None, &payload));
            assert!(handler.matches(Some("any_event"), &payload));
        }

        #[test]
        fn test_handler_matches_event_type() {
            let handler = EventHandler::builder()
                .event_type("content_block_delta")
                .extract_path("delta.text")
                .action(EventAction::Content)
                .build();

            let payload = json!({"delta": {"text": "hi"}});
            assert!(handler.matches(Some("content_block_delta"), &payload));
            assert!(!handler.matches(Some("message_start"), &payload));
            assert!(!handler.matches(None, &payload));
        }

        #[test]
        fn test_handler_matches_filter_path() {
            let handler = EventHandler::builder()
                .extract_path("choices[0].delta.content")
                .filter_path("choices[0].delta.content")
                .action(EventAction::Content)
                .build();

            let matching = json!({"choices": [{"delta": {"content": "token"}}]});
            let non_matching = json!({"choices": [{"delta": {"role": "assistant"}}]});

            assert!(handler.matches(None, &matching));
            assert!(!handler.matches(None, &non_matching));
        }

        #[test]
        fn test_handler_extract() {
            let handler = EventHandler::builder()
                .extract_path("choices[0].delta.content")
                .action(EventAction::Content)
                .build();

            let payload = json!({"choices": [{"delta": {"content": "hello world"}}]});
            let extracted = handler.extract(&payload);
            assert_eq!(extracted, Some(json!("hello world")));
        }

        #[test]
        fn test_handler_extract_missing() {
            let handler = EventHandler::builder()
                .extract_path("nonexistent.path")
                .action(EventAction::Content)
                .build();

            let payload = json!({"other": "data"});
            let extracted = handler.extract(&payload);
            assert_eq!(extracted, None);
        }

        #[test]
        fn test_handler_execute() {
            let handler = EventHandler::builder()
                .event_type("content_block_delta")
                .extract_path("delta.text")
                .filter_path("delta.text")
                .action(EventAction::Content)
                .build();

            let payload = json!({"delta": {"text": "token"}});

            // Should match and extract
            let result = handler.execute(Some("content_block_delta"), &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::Content);
            assert_eq!(value, json!("token"));

            // Should not match wrong event type
            let result = handler.execute(Some("message_start"), &payload);
            assert!(result.is_none());
        }
    }

    mod event_handler_set_tests {
        use super::*;

        #[test]
        fn test_empty_set() {
            let set = EventHandlerSet::new();
            assert!(set.is_empty());
            assert_eq!(set.len(), 0);
        }

        #[test]
        fn test_add_handlers() {
            let mut set = EventHandlerSet::new();
            set.add(
                EventHandler::builder()
                    .extract_path("content")
                    .action(EventAction::Content)
                    .build(),
            );
            assert_eq!(set.len(), 1);
        }

        #[test]
        fn test_with_chaining() {
            let set = EventHandlerSet::new()
                .with(
                    EventHandler::builder()
                        .extract_path("a")
                        .action(EventAction::Content)
                        .build(),
                )
                .with(
                    EventHandler::builder()
                        .extract_path("b")
                        .action(EventAction::ToolCall)
                        .build(),
                );

            assert_eq!(set.len(), 2);
        }

        #[test]
        fn test_process_first_match_wins() {
            let set = EventHandlerSet::new()
                .with(
                    EventHandler::builder()
                        .extract_path("content")
                        .filter_path("content")
                        .action(EventAction::Content)
                        .build(),
                )
                .with(
                    EventHandler::builder()
                        .extract_path("role")
                        .filter_path("role")
                        .action(EventAction::RoleStart)
                        .build(),
                );

            // First handler matches
            let payload = json!({"content": "test", "role": "assistant"});
            let result = set.process(None, &payload);
            assert!(result.is_some());
            let (action, _) = result.unwrap();
            assert_eq!(action, EventAction::Content);

            // Only second handler matches
            let payload = json!({"role": "assistant"});
            let result = set.process(None, &payload);
            assert!(result.is_some());
            let (action, _) = result.unwrap();
            assert_eq!(action, EventAction::RoleStart);

            // No handler matches
            let payload = json!({"other": "data"});
            let result = set.process(None, &payload);
            assert!(result.is_none());
        }
    }

    mod provider_handler_tests {
        use super::*;

        #[test]
        fn test_openai_content_handler() {
            let handlers = openai_handlers();

            let payload = json!({
                "choices": [{
                    "delta": {"content": "Hello"}
                }]
            });

            let result = handlers.process(None, &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::Content);
            assert_eq!(value, json!("Hello"));
        }

        #[test]
        fn test_openai_role_handler() {
            let handlers = openai_handlers();

            let payload = json!({
                "choices": [{
                    "delta": {"role": "assistant"}
                }]
            });

            let result = handlers.process(None, &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::RoleStart);
            assert_eq!(value, json!("assistant"));
        }

        #[test]
        fn test_openai_finish_handler() {
            let handlers = openai_handlers();

            let payload = json!({
                "choices": [{
                    "delta": {},
                    "finish_reason": "stop"
                }]
            });

            let result = handlers.process(None, &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::Finish);
            assert_eq!(value, json!("stop"));
        }

        #[test]
        fn test_openai_usage_handler() {
            let handlers = openai_handlers();

            let payload = json!({
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 20,
                    "total_tokens": 30
                }
            });

            let result = handlers.process(None, &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::UsageUpdate);
            assert_eq!(value["prompt_tokens"], 10);
        }

        #[test]
        fn test_anthropic_message_start() {
            let handlers = anthropic_handlers();

            let payload = json!({
                "type": "message_start",
                "message": {
                    "id": "msg_123",
                    "model": "claude-3-opus",
                    "usage": {"input_tokens": 100}
                }
            });

            let result = handlers.process(Some("message_start"), &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::MessageStart);
            assert_eq!(value["id"], "msg_123");
        }

        #[test]
        fn test_anthropic_content_delta() {
            let handlers = anthropic_handlers();

            let payload = json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "text_delta", "text": "Hello"}
            });

            let result = handlers.process(Some("content_block_delta"), &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::Content);
            assert_eq!(value, json!("Hello"));
        }

        #[test]
        fn test_anthropic_tool_delta() {
            let handlers = anthropic_handlers();

            let payload = json!({
                "type": "content_block_delta",
                "index": 1,
                "delta": {"type": "input_json_delta", "partial_json": "{\"key\":"}
            });

            let result = handlers.process(Some("content_block_delta"), &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::ToolCall);
            assert_eq!(value, json!("{\"key\":"));
        }

        #[test]
        fn test_anthropic_message_stop() {
            let handlers = anthropic_handlers();

            let payload = json!({"type": "message_stop"});

            let result = handlers.process(Some("message_stop"), &payload);
            assert!(result.is_some());
            let (action, _) = result.unwrap();
            assert_eq!(action, EventAction::Stop);
        }

        #[test]
        fn test_anthropic_error() {
            let handlers = anthropic_handlers();

            let payload = json!({
                "type": "error",
                "error": {
                    "type": "rate_limit_error",
                    "message": "Too many requests"
                }
            });

            let result = handlers.process(Some("error"), &payload);
            assert!(result.is_some());
            let (action, value) = result.unwrap();
            assert_eq!(action, EventAction::Error);
            assert_eq!(value["message"], "Too many requests");
        }

        #[test]
        fn test_anthropic_ping() {
            let handlers = anthropic_handlers();

            let payload = json!({"type": "ping"});

            let result = handlers.process(Some("ping"), &payload);
            assert!(result.is_some());
            let (action, _) = result.unwrap();
            assert_eq!(action, EventAction::Ping);
            assert!(action.is_ignorable());
        }
    }

    mod config_serialization_tests {
        use super::*;

        #[test]
        fn test_config_to_handler() {
            let config = EventHandlerConfig {
                extract: "choices[0].delta.content".to_string(),
                filter: Some("choices[0].delta.content".to_string()),
                event_type: None,
                action: "content".to_string(),
            };

            let handler: EventHandler = config.into();
            assert_eq!(handler.action(), &EventAction::Content);
            assert!(handler.filter_path().is_some());
        }

        #[test]
        fn test_handler_to_config() {
            let handler = EventHandler::builder()
                .event_type("content_block_delta")
                .extract_path("delta.text")
                .filter_path("delta.text")
                .action(EventAction::Content)
                .build();

            let config: EventHandlerConfig = (&handler).into();
            assert_eq!(config.extract, "delta.text");
            assert_eq!(config.filter, Some("delta.text".to_string()));
            assert_eq!(config.event_type, Some("content_block_delta".to_string()));
            assert_eq!(config.action, "content");
        }

        #[test]
        fn test_config_yaml_roundtrip() {
            let yaml = r#"
                extract: choices[0].delta.content
                filter: choices[0].delta.content
                action: content
            "#;

            let config: EventHandlerConfig = serde_yaml::from_str(yaml).unwrap();
            assert_eq!(config.extract, "choices[0].delta.content");
            assert_eq!(config.action, "content");

            let yaml_out = serde_yaml::to_string(&config).unwrap();
            let config2: EventHandlerConfig = serde_yaml::from_str(&yaml_out).unwrap();
            assert_eq!(config.extract, config2.extract);
        }

        #[test]
        fn test_config_with_event_type() {
            let yaml = r#"
                type: content_block_delta
                extract: delta.text
                action: content
            "#;

            let config: EventHandlerConfig = serde_yaml::from_str(yaml).unwrap();
            assert_eq!(config.event_type, Some("content_block_delta".to_string()));
        }
    }
}
