# Config-Driven SSE Parser System Design

## Problem Statement

The current SSE parsing system in `src/streaming/compat.rs` uses hardcoded provider dispatch:

```rust
fn parse_sse_line(config: &StreamingConfig, line: &str) -> Option<StreamEvent> {
    use crate::config::SseParser;
    match &config.parser {
        SseParser::AnthropicSse => parse_anthropic_sse(line),
        SseParser::OpenAiSse => parse_openai_sse(config, line),
        SseParser::GoogleSse => parse_generic_sse(line),
        SseParser::Custom(_) => parse_generic_sse(line),
    }
}
```

This violates the library's core principle: **configuration should fully define behavior**. Each new provider requires:
1. Adding a new `SseParser` enum variant
2. Writing a new Rust parsing function
3. Recompiling the library

## Goal

Replace the hardcoded dispatch with a fully config-driven approach where:
- YAML defines all extraction patterns, event mappings, and end conditions
- Rust code is **provider-agnostic** - it interprets config, not provider names
- Adding a new provider requires only a new YAML file

---

## Current State Analysis

### Existing YAML Schema (Partially Used)

The YAML configs already define streaming metadata that is **not fully utilized**:

```yaml
# config/service/anthropic.yaml
streaming:
  format: text/event-stream
  parser: anthropic_sse           # <-- Triggers hardcoded Rust function
  events:
    - type: message_start
      extract: message
      action: start
    - type: content_block_delta
      extract: delta.text         # <-- NOT USED by current parser
      action: content
    - type: message_stop
      extract: message
      action: stop
    - type: error
      extract: error
      action: error
```

The `events` array defines the mapping, but Rust ignores it and uses hardcoded logic.

### Existing Rust Infrastructure

The `src/streaming/sse/extractors/mod.rs` module already has the building blocks:

```rust
// JsonPath navigation
pub struct JsonPath(pub Vec<JsonPathSegment>);

pub enum JsonPathSegment {
    Key(&'static str),
    Index(usize),
}

// End condition detection
pub enum EndCondition {
    JsonPathEquals { path: JsonPath, value: &'static str },
    JsonPathExists(JsonPath),
    PayloadEquals(&'static str),
    None,
}

// Configuration-driven extraction
pub struct ExtractorConfig {
    pub token_path: JsonPath,
    pub json_end_condition: EndCondition,
    pub end_payload: Option<&'static str>,
}
```

The `JsonPathExtractor` implements the `TokenExtractor` trait but is constructed with **compile-time** configs (`openai_config()`, `claude_config()`).

---

## Proposed Solution

### Phase 1: Extended YAML Schema

Define a comprehensive streaming configuration that fully specifies parsing behavior:

```yaml
streaming:
  format: text/event-stream

  # Line-level preprocessing
  line_handling:
    prefix: "data: "              # Strip this prefix from data lines
    done_marker: "[DONE]"         # Raw string indicating stream end
    skip_prefixes:                # Lines to ignore entirely
      - "event: "
      - ":"                       # SSE comments

  # Event type detection and routing
  event_routing:
    # How to determine event type from JSON payload
    type_source: "$.type"         # JSONPath to event type field

    # Fallback for providers without event type field (OpenAI-style)
    type_inference:
      - condition: "$.choices[0].delta.content exists"
        type: content
      - condition: "$.choices[0].delta.role exists"
        type: role_start
      - condition: "$.choices[0].finish_reason exists"
        type: finish
      - condition: "$.usage exists"
        type: usage

  # Event type -> StreamEvent mapping
  events:
    message_start:
      action: start

    content_block_delta:
      action: content
      extract: "$.delta.text"

    content:
      action: content
      extract: "$.choices[0].delta.content"

    role_start:
      action: role
      extract: "$.choices[0].delta.role"

    message_delta:
      action: usage
      extract:
        input_tokens: "$.usage.input_tokens"
        output_tokens: "$.usage.output_tokens"

    message_stop:
      action: finish
      extract_finish_reason: "$.stop_reason"

    finish:
      action: finish
      extract_finish_reason: "$.choices[0].finish_reason"

    usage:
      action: usage
      extract:
        input_tokens: "$.usage.prompt_tokens"
        output_tokens: "$.usage.completion_tokens"
        total_tokens: "$.usage.total_tokens"

    error:
      action: error
      extract_error:
        message: "$.error.message"
        type: "$.error.type"
        code: "$.error.code"

  # Stream completion conditions
  end_conditions:
    - type: payload_equals
      value: "[DONE]"
    - type: json_path_equals
      path: "$.type"
      value: "message_stop"
    - type: json_path_exists
      path: "$.choices[0].finish_reason"
```

### Phase 2: Rust Types

#### 2.1 Config Deserialization Types

```rust
// src/config/streaming.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root streaming configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamingConfig {
    pub format: StreamingFormat,
    #[serde(default)]
    pub line_handling: LineHandlingConfig,
    #[serde(default)]
    pub event_routing: EventRoutingConfig,
    #[serde(default)]
    pub events: HashMap<String, EventActionConfig>,
    #[serde(default)]
    pub end_conditions: Vec<EndConditionConfig>,
}

/// Line-level preprocessing rules
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LineHandlingConfig {
    /// Prefix to strip from data lines (e.g., "data: ")
    #[serde(default)]
    pub prefix: Option<String>,
    /// Raw string that indicates stream end (e.g., "[DONE]")
    #[serde(default)]
    pub done_marker: Option<String>,
    /// Line prefixes to skip entirely
    #[serde(default)]
    pub skip_prefixes: Vec<String>,
}

/// How to route payloads to event handlers
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EventRoutingConfig {
    /// JSONPath to the event type field (e.g., "$.type")
    #[serde(default)]
    pub type_source: Option<String>,
    /// Inference rules when type_source is absent
    #[serde(default)]
    pub type_inference: Vec<TypeInferenceRule>,
}

/// Infer event type based on payload structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TypeInferenceRule {
    /// Condition expression (e.g., "$.choices[0].delta.content exists")
    pub condition: String,
    /// Event type to assign if condition matches
    #[serde(rename = "type")]
    pub event_type: String,
}

/// Actions and extraction for a specific event type
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventActionConfig {
    /// The StreamEvent action to emit
    pub action: EventAction,
    /// JSONPath for content extraction
    #[serde(default)]
    pub extract: Option<ExtractionTarget>,
    /// JSONPath for finish reason extraction
    #[serde(default)]
    pub extract_finish_reason: Option<String>,
    /// Paths for usage extraction
    #[serde(default)]
    pub extract_usage: Option<UsageExtractionConfig>,
    /// Paths for error extraction
    #[serde(default)]
    pub extract_error: Option<ErrorExtractionConfig>,
}

/// Target for the action (what StreamEvent to emit)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventAction {
    Start,
    Content,
    Role,
    Finish,
    Usage,
    Error,
    Raw,
    Skip,  // Explicitly ignore this event type
}

/// What to extract from the payload
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ExtractionTarget {
    /// Single JSONPath string (e.g., "$.delta.text")
    Single(String),
    /// Multiple named extractions
    Multiple(HashMap<String, String>),
}

/// Usage token extraction paths
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UsageExtractionConfig {
    pub input_tokens: String,
    pub output_tokens: String,
    #[serde(default)]
    pub total_tokens: Option<String>,
}

/// Error field extraction paths
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorExtractionConfig {
    pub message: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

/// Stream end condition specification
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EndConditionConfig {
    /// Raw payload string equals value
    PayloadEquals { value: String },
    /// JSONPath exists and is non-null
    JsonPathExists { path: String },
    /// JSONPath equals specific value
    JsonPathEquals { path: String, value: String },
}
```

#### 2.2 Runtime Parsing Types

```rust
// src/streaming/config_parser.rs

use serde_json::Value;
use crate::config::streaming::*;
use crate::streaming::compat::StreamEvent;

/// Runtime SSE parser built from config
pub struct ConfigDrivenParser {
    config: StreamingConfig,
    jsonpath_cache: JsonPathCache,
}

impl ConfigDrivenParser {
    /// Create parser from streaming config
    pub fn from_config(config: StreamingConfig) -> Result<Self, ConfigError> {
        // Pre-compile JSONPath expressions for performance
        let jsonpath_cache = Self::build_jsonpath_cache(&config)?;
        Ok(Self { config, jsonpath_cache })
    }

    /// Parse a single SSE line into a StreamEvent
    pub fn parse_line(&self, line: &str) -> Option<StreamEvent> {
        // 1. Check skip prefixes
        for prefix in &self.config.line_handling.skip_prefixes {
            if line.starts_with(prefix) {
                return None;
            }
        }

        // 2. Strip data prefix and extract payload
        let payload = self.extract_payload(line)?;

        // 3. Check for done marker
        if let Some(done) = &self.config.line_handling.done_marker {
            if payload.trim() == done {
                return Some(StreamEvent::Finish(None));
            }
        }

        // 4. Parse JSON
        let json: Value = serde_json::from_str(payload).ok()?;

        // 5. Check end conditions
        if self.check_end_conditions(&json, payload) {
            return Some(StreamEvent::Finish(None));
        }

        // 6. Determine event type
        let event_type = self.determine_event_type(&json)?;

        // 7. Route to action handler
        self.handle_event(&event_type, &json)
    }

    fn extract_payload<'a>(&self, line: &'a str) -> Option<&'a str> {
        match &self.config.line_handling.prefix {
            Some(prefix) => line.strip_prefix(prefix),
            None => Some(line),
        }
    }

    fn determine_event_type(&self, json: &Value) -> Option<String> {
        // Try explicit type field first
        if let Some(type_source) = &self.config.event_routing.type_source {
            if let Some(event_type) = self.extract_string(json, type_source) {
                return Some(event_type);
            }
        }

        // Fall back to type inference rules
        for rule in &self.config.event_routing.type_inference {
            if self.evaluate_condition(&rule.condition, json) {
                return Some(rule.event_type.clone());
            }
        }

        None
    }

    fn check_end_conditions(&self, json: &Value, payload: &str) -> bool {
        for condition in &self.config.end_conditions {
            match condition {
                EndConditionConfig::PayloadEquals { value } => {
                    if payload.trim() == value {
                        return true;
                    }
                }
                EndConditionConfig::JsonPathExists { path } => {
                    if self.path_exists(json, path) {
                        return true;
                    }
                }
                EndConditionConfig::JsonPathEquals { path, value } => {
                    if self.extract_string(json, path).as_deref() == Some(value) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn handle_event(&self, event_type: &str, json: &Value) -> Option<StreamEvent> {
        let action_config = self.config.events.get(event_type)?;

        match &action_config.action {
            EventAction::Start => Some(StreamEvent::Start),

            EventAction::Content => {
                let text = self.extract_content(&action_config.extract, json)?;
                Some(StreamEvent::Content(text))
            }

            EventAction::Role => {
                let role = self.extract_content(&action_config.extract, json)?;
                Some(StreamEvent::Role(role))
            }

            EventAction::Finish => {
                let reason = action_config.extract_finish_reason
                    .as_ref()
                    .and_then(|path| self.extract_string(json, path));
                Some(StreamEvent::Finish(reason))
            }

            EventAction::Usage => {
                let usage_config = action_config.extract_usage.as_ref()?;
                Some(StreamEvent::Usage {
                    input_tokens: self.extract_u32(json, &usage_config.input_tokens),
                    output_tokens: self.extract_u32(json, &usage_config.output_tokens),
                    total_tokens: usage_config.total_tokens
                        .as_ref()
                        .and_then(|p| self.extract_u32(json, p)),
                })
            }

            EventAction::Error => {
                let error_config = action_config.extract_error.as_ref()?;
                let message = self.extract_string(json, &error_config.message)
                    .unwrap_or_else(|| "Unknown error".to_string());
                Some(StreamEvent::Error(message))
            }

            EventAction::Raw => {
                Some(StreamEvent::Raw(serde_json::to_string(json).ok()?))
            }

            EventAction::Skip => None,
        }
    }

    // ... helper methods for JSONPath extraction
}
```

#### 2.3 JSONPath Implementation

```rust
// src/streaming/jsonpath.rs

use serde_json::Value;

/// Compiled JSONPath expression for efficient repeated evaluation
#[derive(Debug, Clone)]
pub struct CompiledJsonPath {
    segments: Vec<PathSegment>,
}

#[derive(Debug, Clone)]
enum PathSegment {
    /// Object key: `.foo` or `["foo"]`
    Key(String),
    /// Array index: `[0]`
    Index(usize),
    /// Array wildcard: `[*]`
    Wildcard,
}

impl CompiledJsonPath {
    /// Parse a JSONPath expression
    ///
    /// Supported syntax:
    /// - `$.foo.bar` - dot notation
    /// - `$["foo"]["bar"]` - bracket notation
    /// - `$.array[0]` - array index
    /// - `$.choices[0].delta.content` - mixed
    pub fn parse(path: &str) -> Result<Self, JsonPathError> {
        let path = path.strip_prefix('$').unwrap_or(path);
        let mut segments = Vec::new();

        let mut chars = path.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '.' => {
                    // Dot notation: .key
                    let key: String = chars
                        .by_ref()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !key.is_empty() {
                        segments.push(PathSegment::Key(key));
                    }
                }
                '[' => {
                    // Bracket notation: ["key"] or [0] or [*]
                    let content: String = chars
                        .by_ref()
                        .take_while(|c| *c != ']')
                        .collect();

                    if content == "*" {
                        segments.push(PathSegment::Wildcard);
                    } else if let Ok(idx) = content.parse::<usize>() {
                        segments.push(PathSegment::Index(idx));
                    } else {
                        // Strip quotes if present
                        let key = content
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string();
                        segments.push(PathSegment::Key(key));
                    }
                }
                _ => continue,
            }
        }

        Ok(Self { segments })
    }

    /// Navigate JSON value following this path
    pub fn navigate<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let mut current = value;
        for segment in &self.segments {
            current = match segment {
                PathSegment::Key(key) => current.get(key)?,
                PathSegment::Index(idx) => current.get(*idx)?,
                PathSegment::Wildcard => {
                    // For existence checks, any element works
                    current.as_array()?.first()?
                }
            };
        }
        Some(current)
    }

    /// Check if path exists in value
    pub fn exists(&self, value: &Value) -> bool {
        self.navigate(value).is_some()
    }

    /// Extract string value at path
    pub fn extract_string(&self, value: &Value) -> Option<String> {
        self.navigate(value)?.as_str().map(String::from)
    }

    /// Extract u32 value at path
    pub fn extract_u32(&self, value: &Value) -> Option<u32> {
        self.navigate(value)?.as_u64().map(|v| v as u32)
    }
}

/// Cache of pre-compiled JSONPath expressions
pub struct JsonPathCache {
    paths: std::collections::HashMap<String, CompiledJsonPath>,
}

impl JsonPathCache {
    pub fn new() -> Self {
        Self { paths: std::collections::HashMap::new() }
    }

    pub fn get_or_compile(&mut self, path: &str) -> Result<&CompiledJsonPath, JsonPathError> {
        if !self.paths.contains_key(path) {
            let compiled = CompiledJsonPath::parse(path)?;
            self.paths.insert(path.to_string(), compiled);
        }
        Ok(self.paths.get(path).unwrap())
    }
}
```

#### 2.4 Condition Evaluator

```rust
// src/streaming/conditions.rs

use serde_json::Value;
use super::jsonpath::CompiledJsonPath;

/// Evaluate condition expressions like "$.path exists" or "$.path == 'value'"
pub fn evaluate_condition(condition: &str, value: &Value) -> bool {
    // Parse condition: "$.path exists" or "$.path == 'value'"
    if let Some(path_str) = condition.strip_suffix(" exists") {
        // Existence check
        if let Ok(path) = CompiledJsonPath::parse(path_str.trim()) {
            return path.exists(value);
        }
    }

    if let Some((lhs, rhs)) = condition.split_once(" == ") {
        // Equality check
        if let Ok(path) = CompiledJsonPath::parse(lhs.trim()) {
            let expected = rhs.trim().trim_matches('\'').trim_matches('"');
            if let Some(actual) = path.extract_string(value) {
                return actual == expected;
            }
        }
    }

    false
}
```

---

## Phase 3: Migration Strategy

### 3.1 Updated Provider Configs

**Anthropic (config/service/anthropic.yaml):**

```yaml
streaming:
  format: text/event-stream

  line_handling:
    skip_prefixes:
      - "event: "
      - ":"

  event_routing:
    type_source: "$.type"

  events:
    message_start:
      action: start

    content_block_start:
      action: skip

    content_block_delta:
      action: content
      extract: "$.delta.text"

    content_block_stop:
      action: skip

    message_delta:
      action: usage
      extract_usage:
        input_tokens: "$.usage.input_tokens"
        output_tokens: "$.usage.output_tokens"

    message_stop:
      action: finish

    error:
      action: error
      extract_error:
        message: "$.error.message"
        type: "$.error.type"

  end_conditions:
    - type: json_path_equals
      path: "$.type"
      value: "message_stop"
```

**OpenAI (config/service/openai.yaml):**

```yaml
streaming:
  format: text/event-stream

  line_handling:
    prefix: "data: "
    done_marker: "[DONE]"
    skip_prefixes:
      - ":"

  event_routing:
    type_inference:
      - condition: "$.choices[0].delta.content exists"
        type: content
      - condition: "$.choices[0].delta.role exists"
        type: role
      - condition: "$.choices[0].finish_reason exists"
        type: finish
      - condition: "$.usage exists"
        type: usage

  events:
    content:
      action: content
      extract: "$.choices[0].delta.content"

    role:
      action: role
      extract: "$.choices[0].delta.role"

    finish:
      action: finish
      extract_finish_reason: "$.choices[0].finish_reason"

    usage:
      action: usage
      extract_usage:
        input_tokens: "$.usage.prompt_tokens"
        output_tokens: "$.usage.completion_tokens"
        total_tokens: "$.usage.total_tokens"

  end_conditions:
    - type: payload_equals
      value: "[DONE]"
```

**Google Gemini (config/service/google.yaml):**

```yaml
streaming:
  format: text/event-stream

  line_handling:
    prefix: "data: "
    skip_prefixes:
      - ":"

  event_routing:
    type_inference:
      - condition: "$.candidates[0].content.parts[0].text exists"
        type: content
      - condition: "$.candidates[0].finishReason exists"
        type: finish
      - condition: "$.usageMetadata exists"
        type: usage

  events:
    content:
      action: content
      extract: "$.candidates[0].content.parts[0].text"

    finish:
      action: finish
      extract_finish_reason: "$.candidates[0].finishReason"

    usage:
      action: usage
      extract_usage:
        input_tokens: "$.usageMetadata.promptTokenCount"
        output_tokens: "$.usageMetadata.candidatesTokenCount"
        total_tokens: "$.usageMetadata.totalTokenCount"

  end_conditions:
    - type: json_path_exists
      path: "$.candidates[0].finishReason"
```

### 3.2 Code Migration

**Step 1: Remove SseParser enum**

```rust
// BEFORE (src/config.rs)
impl_string_enum! {
    pub enum SseParser {
        OpenAiSse => "openai_sse",
        AnthropicSse => "anthropic_sse",
        GoogleSse => "google_sse",
    }
    default: OpenAiSse
}

// AFTER: Remove entirely - no longer needed
```

**Step 2: Update StreamProcessor**

```rust
// BEFORE (src/streaming/compat.rs)
fn parse_sse_line(config: &StreamingConfig, line: &str) -> Option<StreamEvent> {
    use crate::config::SseParser;
    match &config.parser {
        SseParser::AnthropicSse => parse_anthropic_sse(line),
        SseParser::OpenAiSse => parse_openai_sse(config, line),
        SseParser::GoogleSse => parse_generic_sse(line),
        SseParser::Custom(_) => parse_generic_sse(line),
    }
}

// AFTER
fn parse_sse_line(parser: &ConfigDrivenParser, line: &str) -> Option<StreamEvent> {
    parser.parse_line(line)
}
```

**Step 3: Create parser at StreamProcessor construction**

```rust
impl StreamProcessor {
    pub fn new(config: &StreamingConfig) -> Result<Self> {
        let parser = ConfigDrivenParser::from_config(config.clone())?;
        Ok(Self { parser })
    }
}
```

---

## Benefits

1. **Zero Rust changes for new providers** - Just add YAML
2. **Testable configs** - YAML can be validated independently
3. **Runtime flexibility** - Configs can be modified without recompilation
4. **Self-documenting** - YAML shows exactly what the parser does
5. **Unified interface** - All providers use the same parsing infrastructure
6. **Type safety preserved** - Config validation catches errors at load time

---

## Backwards Compatibility

The old `parser: openai_sse` field can be interpreted as a shorthand that expands to the full config. During migration:

```rust
impl StreamingConfig {
    /// Expand legacy parser field to full config if needed
    pub fn normalize(mut self) -> Self {
        if self.events.is_empty() && !self.parser.is_custom() {
            // Load default event config based on parser type
            self.events = Self::default_events_for(&self.parser);
        }
        self
    }
}
```

---

## Implementation Phases

### Phase 1: Foundation (Week 1)
- [ ] Add new config types to `src/config/streaming.rs`
- [ ] Implement `CompiledJsonPath` with tests
- [ ] Implement condition evaluator with tests

### Phase 2: Parser (Week 2)
- [ ] Implement `ConfigDrivenParser`
- [ ] Add comprehensive unit tests
- [ ] Integrate with `StreamProcessor`

### Phase 3: Migration (Week 3)
- [ ] Update Anthropic YAML config
- [ ] Update OpenAI YAML config
- [ ] Update Google YAML config
- [ ] Update DeepSeek YAML config (inherits OpenAI)

### Phase 4: Cleanup (Week 4)
- [ ] Remove hardcoded parser functions
- [ ] Remove `SseParser` enum
- [ ] Update documentation
- [ ] Add integration tests with real SSE samples

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_jsonpath_basic() {
    let path = CompiledJsonPath::parse("$.choices[0].delta.content").unwrap();
    let json = json!({"choices": [{"delta": {"content": "hello"}}]});
    assert_eq!(path.extract_string(&json), Some("hello".to_string()));
}

#[test]
fn test_anthropic_content_extraction() {
    let config = load_yaml("config/service/anthropic.yaml");
    let parser = ConfigDrivenParser::from_config(config.streaming).unwrap();

    let line = r#"data: {"type":"content_block_delta","delta":{"text":"Hello"}}"#;
    let event = parser.parse_line(line);

    assert_eq!(event, Some(StreamEvent::Content("Hello".to_string())));
}

#[test]
fn test_openai_end_detection() {
    let config = load_yaml("config/service/openai.yaml");
    let parser = ConfigDrivenParser::from_config(config.streaming).unwrap();

    assert_eq!(
        parser.parse_line("data: [DONE]"),
        Some(StreamEvent::Finish(None))
    );
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_anthropic_stream_parsing() {
    let sample_stream = include_str!("../fixtures/anthropic_stream.txt");
    let config = load_yaml("config/service/anthropic.yaml");
    let parser = ConfigDrivenParser::from_config(config.streaming).unwrap();

    let events: Vec<_> = sample_stream
        .lines()
        .filter_map(|line| parser.parse_line(line))
        .collect();

    assert!(matches!(events.first(), Some(StreamEvent::Start)));
    assert!(events.iter().any(|e| matches!(e, StreamEvent::Content(_))));
    assert!(matches!(events.last(), Some(StreamEvent::Finish(_))));
}
```

---

## Open Questions

1. **JSONPath library**: Use existing crate (`jsonpath_lib`) or custom implementation?
   - Recommendation: Custom, minimal implementation for performance and control

2. **Error handling**: How to report config validation errors clearly?
   - Recommendation: Custom error types with path to invalid config

3. **Hot reload**: Should config changes take effect without restart?
   - Recommendation: Out of scope for initial implementation

4. **Schema validation**: Generate JSON Schema for config validation?
   - Recommendation: Yes, derive from Rust types with `schemars`
