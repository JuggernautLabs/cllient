//! Lightweight JSONPath extractor for cllient
//!
//! This module provides a compile-time parsed JSONPath implementation for efficiently
//! extracting values from `serde_json::Value`. It's designed to be used by both the SSE
//! parser and response extractors.
//!
//! # Features
//!
//! - **Compile-time parsing**: Paths are parsed once at construction time, not at runtime
//! - **Efficient navigation**: O(n) traversal where n is the path depth
//! - **Type-safe extraction**: Convenience methods for extracting typed values
//! - **Fluent builder API**: Construct paths programmatically when needed
//!
//! # Supported Path Syntax
//!
//! - `field` - Simple field access
//! - `field.nested` - Nested field access
//! - `field[0]` - Array index access
//! - `field[0].nested` - Chained access
//! - `choices[0].delta.content` - Complex nested paths
//!
//! # Examples
//!
//! ```rust
//! use cllient::jsonpath::JsonPath;
//! use serde_json::json;
//!
//! // Parse a path string
//! let path = JsonPath::parse("choices[0].delta.content").unwrap();
//! let json = json!({
//!     "choices": [{
//!         "delta": {
//!             "content": "Hello, world!"
//!         }
//!     }]
//! });
//!
//! assert_eq!(path.extract_str(&json), Some("Hello, world!"));
//!
//! // Use the builder API
//! let path = JsonPath::builder()
//!     .field("choices")
//!     .index(0)
//!     .field("delta")
//!     .field("content")
//!     .build();
//!
//! assert_eq!(path.extract_str(&json), Some("Hello, world!"));
//! ```

use serde_json::Value;
use std::fmt;

/// Error type for JSONPath parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonPathError {
    /// Empty path string provided
    EmptyPath,
    /// Invalid array index syntax (e.g., `field[abc]`)
    InvalidArrayIndex { segment: String, reason: String },
    /// Unclosed bracket in array access
    UnclosedBracket { position: usize },
    /// Empty segment (e.g., `field..nested`)
    EmptySegment { position: usize },
    /// Invalid character in path
    InvalidCharacter { char: char, position: usize },
}

impl fmt::Display for JsonPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonPathError::EmptyPath => write!(f, "empty path string"),
            JsonPathError::InvalidArrayIndex { segment, reason } => {
                write!(f, "invalid array index in '{}': {}", segment, reason)
            }
            JsonPathError::UnclosedBracket { position } => {
                write!(f, "unclosed bracket at position {}", position)
            }
            JsonPathError::EmptySegment { position } => {
                write!(f, "empty segment at position {}", position)
            }
            JsonPathError::InvalidCharacter { char, position } => {
                write!(f, "invalid character '{}' at position {}", char, position)
            }
        }
    }
}

impl std::error::Error for JsonPathError {}

/// A segment in a JSONPath expression
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Object key lookup: `{"key": value}` -> value
    Field(String),
    /// Array index lookup: `[value, ...]` -> value at index
    Index(usize),
}

impl Segment {
    /// Create a field segment
    pub const fn field(name: String) -> Self {
        Segment::Field(name)
    }

    /// Create an index segment
    pub const fn index(idx: usize) -> Self {
        Segment::Index(idx)
    }
}

/// A compiled JSONPath expression for efficient value extraction
///
/// The path is parsed once at construction time, making repeated extractions
/// very efficient. The path is stored as a vector of segments that are
/// traversed in order to navigate the JSON structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonPath {
    segments: Vec<Segment>,
    /// The original path string (for debugging/display)
    source: String,
}

impl JsonPath {
    /// Create a new empty JSONPath (returns the root value)
    pub fn root() -> Self {
        JsonPath {
            segments: Vec::new(),
            source: String::new(),
        }
    }

    /// Parse a JSONPath string into a compiled path expression
    ///
    /// # Supported syntax
    ///
    /// - `field` - Simple field access
    /// - `field.nested` - Nested field access
    /// - `field[0]` - Array index access
    /// - `field[0].nested[1].value` - Chained access
    ///
    /// # Errors
    ///
    /// Returns `JsonPathError` if the path string is malformed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cllient::jsonpath::JsonPath;
    ///
    /// let path = JsonPath::parse("choices[0].delta.content").unwrap();
    /// let path = JsonPath::parse("error.message").unwrap();
    /// let path = JsonPath::parse("data[0][1].value").unwrap();
    /// ```
    pub fn parse(path: &str) -> Result<Self, JsonPathError> {
        if path.is_empty() {
            return Err(JsonPathError::EmptyPath);
        }

        let mut segments = Vec::new();
        let mut chars = path.char_indices().peekable();
        let mut current_field = String::new();
        let mut in_bracket = false;
        let mut bracket_content = String::new();
        let mut bracket_start = 0;

        while let Some((pos, ch)) = chars.next() {
            if in_bracket {
                if ch == ']' {
                    // Parse the bracket content as an index
                    if bracket_content.is_empty() {
                        return Err(JsonPathError::InvalidArrayIndex {
                            segment: "[]".to_string(),
                            reason: "empty index".to_string(),
                        });
                    }

                    let index: usize = bracket_content.parse().map_err(|_| {
                        JsonPathError::InvalidArrayIndex {
                            segment: format!("[{}]", bracket_content),
                            reason: "not a valid integer".to_string(),
                        }
                    })?;

                    // If we have a pending field, push it first
                    if !current_field.is_empty() {
                        segments.push(Segment::Field(std::mem::take(&mut current_field)));
                    }

                    segments.push(Segment::Index(index));
                    bracket_content.clear();
                    in_bracket = false;
                } else if ch.is_ascii_digit() {
                    bracket_content.push(ch);
                } else {
                    return Err(JsonPathError::InvalidArrayIndex {
                        segment: format!("[{}{}...", bracket_content, ch),
                        reason: format!("unexpected character '{}'", ch),
                    });
                }
            } else {
                match ch {
                    '.' => {
                        // End of current field
                        if current_field.is_empty() && segments.is_empty() {
                            return Err(JsonPathError::EmptySegment { position: pos });
                        }
                        if !current_field.is_empty() {
                            segments.push(Segment::Field(std::mem::take(&mut current_field)));
                        }
                        // Check for empty segment (consecutive dots)
                        if chars.peek().map(|(_, c)| *c == '.').unwrap_or(false) {
                            return Err(JsonPathError::EmptySegment { position: pos + 1 });
                        }
                    }
                    '[' => {
                        in_bracket = true;
                        bracket_start = pos;
                    }
                    ']' => {
                        return Err(JsonPathError::InvalidCharacter {
                            char: ']',
                            position: pos,
                        });
                    }
                    _ if ch.is_alphanumeric() || ch == '_' || ch == '-' => {
                        current_field.push(ch);
                    }
                    _ => {
                        return Err(JsonPathError::InvalidCharacter {
                            char: ch,
                            position: pos,
                        });
                    }
                }
            }
        }

        // Handle unclosed bracket
        if in_bracket {
            return Err(JsonPathError::UnclosedBracket {
                position: bracket_start,
            });
        }

        // Push final field if any
        if !current_field.is_empty() {
            segments.push(Segment::Field(current_field));
        }

        // Ensure we have at least one segment
        if segments.is_empty() {
            return Err(JsonPathError::EmptyPath);
        }

        Ok(JsonPath {
            segments,
            source: path.to_string(),
        })
    }

    /// Create a builder for constructing paths programmatically
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cllient::jsonpath::JsonPath;
    ///
    /// let path = JsonPath::builder()
    ///     .field("choices")
    ///     .index(0)
    ///     .field("delta")
    ///     .field("content")
    ///     .build();
    /// ```
    pub fn builder() -> JsonPathBuilder {
        JsonPathBuilder::new()
    }

    /// Get the segments of this path
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Get the original source string (if parsed from string)
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Extract a value from a JSON structure, returning a reference
    ///
    /// Returns `None` if the path doesn't exist in the JSON.
    pub fn extract<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        let mut current = value;
        for segment in &self.segments {
            current = match segment {
                Segment::Field(key) => current.get(key)?,
                Segment::Index(idx) => current.get(*idx)?,
            };
        }
        Some(current)
    }

    /// Extract and clone a value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist in the JSON.
    pub fn extract_owned(&self, value: &Value) -> Option<Value> {
        self.extract(value).cloned()
    }

    /// Extract a string value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist or the value is not a string.
    pub fn extract_str<'a>(&self, value: &'a Value) -> Option<&'a str> {
        self.extract(value).and_then(Value::as_str)
    }

    /// Extract an i64 value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist or the value is not a number.
    pub fn extract_i64(&self, value: &Value) -> Option<i64> {
        self.extract(value).and_then(Value::as_i64)
    }

    /// Extract a u64 value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist or the value is not a number.
    pub fn extract_u64(&self, value: &Value) -> Option<u64> {
        self.extract(value).and_then(Value::as_u64)
    }

    /// Extract an f64 value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist or the value is not a number.
    pub fn extract_f64(&self, value: &Value) -> Option<f64> {
        self.extract(value).and_then(Value::as_f64)
    }

    /// Extract a boolean value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist or the value is not a boolean.
    pub fn extract_bool(&self, value: &Value) -> Option<bool> {
        self.extract(value).and_then(Value::as_bool)
    }

    /// Extract an array value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist or the value is not an array.
    pub fn extract_array<'a>(&self, value: &'a Value) -> Option<&'a Vec<Value>> {
        self.extract(value).and_then(Value::as_array)
    }

    /// Extract an object value from a JSON structure
    ///
    /// Returns `None` if the path doesn't exist or the value is not an object.
    pub fn extract_object<'a>(
        &self,
        value: &'a Value,
    ) -> Option<&'a serde_json::Map<String, Value>> {
        self.extract(value).and_then(Value::as_object)
    }

    /// Check if the path exists in the JSON value
    pub fn exists(&self, value: &Value) -> bool {
        self.extract(value).is_some()
    }

    /// Check if the path exists and the value is not null
    pub fn exists_non_null(&self, value: &Value) -> bool {
        self.extract(value).is_some_and(|v| !v.is_null())
    }

    /// Check if the path value equals the given string
    pub fn equals_str(&self, value: &Value, expected: &str) -> bool {
        self.extract_str(value).is_some_and(|s| s == expected)
    }
}

impl fmt::Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.source.is_empty() {
            write!(f, "{}", self.source)
        } else {
            // Reconstruct from segments
            let mut first = true;
            for segment in &self.segments {
                match segment {
                    Segment::Field(name) => {
                        if !first {
                            write!(f, ".")?;
                        }
                        write!(f, "{}", name)?;
                    }
                    Segment::Index(idx) => {
                        write!(f, "[{}]", idx)?;
                    }
                }
                first = false;
            }
            Ok(())
        }
    }
}

/// Builder for constructing JSONPath expressions programmatically
///
/// This is useful when you want to construct paths dynamically or
/// when you prefer a fluent API over parsing strings.
#[derive(Debug, Clone, Default)]
pub struct JsonPathBuilder {
    segments: Vec<Segment>,
}

impl JsonPathBuilder {
    /// Create a new empty builder
    pub fn new() -> Self {
        JsonPathBuilder {
            segments: Vec::new(),
        }
    }

    /// Add a field segment
    pub fn field(mut self, name: impl Into<String>) -> Self {
        self.segments.push(Segment::Field(name.into()));
        self
    }

    /// Add an array index segment
    pub fn index(mut self, idx: usize) -> Self {
        self.segments.push(Segment::Index(idx));
        self
    }

    /// Build the final JsonPath
    pub fn build(self) -> JsonPath {
        // Reconstruct source string
        let mut source = String::new();
        let mut first = true;
        for segment in &self.segments {
            match segment {
                Segment::Field(name) => {
                    if !first {
                        source.push('.');
                    }
                    source.push_str(name);
                }
                Segment::Index(idx) => {
                    source.push('[');
                    source.push_str(&idx.to_string());
                    source.push(']');
                }
            }
            first = false;
        }

        JsonPath {
            segments: self.segments,
            source,
        }
    }
}

/// A set of related JSONPath extractors for common extraction patterns
///
/// This is useful when you need to extract multiple values from the same
/// JSON structure, such as extracting both content and finish_reason from
/// an OpenAI response.
#[derive(Debug, Clone)]
pub struct JsonPathSet {
    paths: Vec<(String, JsonPath)>,
}

impl JsonPathSet {
    /// Create a new empty path set
    pub fn new() -> Self {
        JsonPathSet { paths: Vec::new() }
    }

    /// Add a named path to the set
    pub fn with(mut self, name: impl Into<String>, path: JsonPath) -> Self {
        self.paths.push((name.into(), path));
        self
    }

    /// Add a named path by parsing a path string
    pub fn with_path(
        mut self,
        name: impl Into<String>,
        path: &str,
    ) -> Result<Self, JsonPathError> {
        self.paths.push((name.into(), JsonPath::parse(path)?));
        Ok(self)
    }

    /// Extract all values from a JSON structure
    pub fn extract_all<'a>(&self, value: &'a Value) -> Vec<(&str, Option<&'a Value>)> {
        self.paths
            .iter()
            .map(|(name, path)| (name.as_str(), path.extract(value)))
            .collect()
    }

    /// Get a path by name
    pub fn get(&self, name: &str) -> Option<&JsonPath> {
        self.paths
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p)
    }
}

impl Default for JsonPathSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-compiled paths for common LLM response patterns
pub mod patterns {
    use super::*;

    /// OpenAI chat completion streaming response paths
    pub fn openai_streaming() -> JsonPathSet {
        JsonPathSet::new()
            .with(
                "content",
                JsonPath::builder()
                    .field("choices")
                    .index(0)
                    .field("delta")
                    .field("content")
                    .build(),
            )
            .with(
                "finish_reason",
                JsonPath::builder()
                    .field("choices")
                    .index(0)
                    .field("finish_reason")
                    .build(),
            )
            .with(
                "role",
                JsonPath::builder()
                    .field("choices")
                    .index(0)
                    .field("delta")
                    .field("role")
                    .build(),
            )
    }

    /// OpenAI chat completion non-streaming response paths
    pub fn openai_completion() -> JsonPathSet {
        JsonPathSet::new()
            .with(
                "content",
                JsonPath::builder()
                    .field("choices")
                    .index(0)
                    .field("message")
                    .field("content")
                    .build(),
            )
            .with(
                "role",
                JsonPath::builder()
                    .field("choices")
                    .index(0)
                    .field("message")
                    .field("role")
                    .build(),
            )
            .with(
                "finish_reason",
                JsonPath::builder()
                    .field("choices")
                    .index(0)
                    .field("finish_reason")
                    .build(),
            )
            .with("prompt_tokens", JsonPath::parse("usage.prompt_tokens").unwrap())
            .with(
                "completion_tokens",
                JsonPath::parse("usage.completion_tokens").unwrap(),
            )
            .with("total_tokens", JsonPath::parse("usage.total_tokens").unwrap())
    }

    /// Anthropic Claude streaming response paths
    pub fn claude_streaming() -> JsonPathSet {
        JsonPathSet::new()
            .with(
                "text",
                JsonPath::builder().field("delta").field("text").build(),
            )
            .with("type", JsonPath::builder().field("type").build())
            .with("stop_reason", JsonPath::parse("delta.stop_reason").unwrap())
    }

    /// Google Gemini streaming response paths
    pub fn gemini_streaming() -> JsonPathSet {
        JsonPathSet::new()
            .with(
                "text",
                JsonPath::builder()
                    .field("candidates")
                    .index(0)
                    .field("content")
                    .field("parts")
                    .index(0)
                    .field("text")
                    .build(),
            )
            .with(
                "finish_reason",
                JsonPath::builder()
                    .field("candidates")
                    .index(0)
                    .field("finishReason")
                    .build(),
            )
    }

    /// Common error message paths (works for multiple providers)
    pub fn error_paths() -> JsonPathSet {
        JsonPathSet::new()
            .with("error_message", JsonPath::parse("error.message").unwrap())
            .with("error_type", JsonPath::parse("error.type").unwrap())
            .with("error_code", JsonPath::parse("error.code").unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_simple_field() {
        let path = JsonPath::parse("field").unwrap();
        assert_eq!(path.segments(), &[Segment::Field("field".to_string())]);
    }

    #[test]
    fn test_parse_nested_fields() {
        let path = JsonPath::parse("field.nested.deep").unwrap();
        assert_eq!(
            path.segments(),
            &[
                Segment::Field("field".to_string()),
                Segment::Field("nested".to_string()),
                Segment::Field("deep".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_array_index() {
        let path = JsonPath::parse("items[0]").unwrap();
        assert_eq!(
            path.segments(),
            &[
                Segment::Field("items".to_string()),
                Segment::Index(0),
            ]
        );
    }

    #[test]
    fn test_parse_complex_path() {
        let path = JsonPath::parse("choices[0].delta.content").unwrap();
        assert_eq!(
            path.segments(),
            &[
                Segment::Field("choices".to_string()),
                Segment::Index(0),
                Segment::Field("delta".to_string()),
                Segment::Field("content".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_multiple_indices() {
        let path = JsonPath::parse("data[0][1].value").unwrap();
        assert_eq!(
            path.segments(),
            &[
                Segment::Field("data".to_string()),
                Segment::Index(0),
                Segment::Index(1),
                Segment::Field("value".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_gemini_path() {
        let path = JsonPath::parse("candidates[0].content.parts[0].text").unwrap();
        assert_eq!(
            path.segments(),
            &[
                Segment::Field("candidates".to_string()),
                Segment::Index(0),
                Segment::Field("content".to_string()),
                Segment::Field("parts".to_string()),
                Segment::Index(0),
                Segment::Field("text".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_error_empty() {
        assert_eq!(JsonPath::parse(""), Err(JsonPathError::EmptyPath));
    }

    #[test]
    fn test_parse_error_empty_segment() {
        let err = JsonPath::parse("field..nested").unwrap_err();
        assert!(matches!(err, JsonPathError::EmptySegment { .. }));
    }

    #[test]
    fn test_parse_error_invalid_index() {
        let err = JsonPath::parse("field[abc]").unwrap_err();
        assert!(matches!(err, JsonPathError::InvalidArrayIndex { .. }));
    }

    #[test]
    fn test_parse_error_unclosed_bracket() {
        let err = JsonPath::parse("field[0").unwrap_err();
        assert!(matches!(err, JsonPathError::UnclosedBracket { .. }));
    }

    #[test]
    fn test_extract_simple() {
        let path = JsonPath::parse("name").unwrap();
        let json = json!({"name": "test"});
        assert_eq!(path.extract_str(&json), Some("test"));
    }

    #[test]
    fn test_extract_nested() {
        let path = JsonPath::parse("user.name").unwrap();
        let json = json!({"user": {"name": "Alice"}});
        assert_eq!(path.extract_str(&json), Some("Alice"));
    }

    #[test]
    fn test_extract_array() {
        let path = JsonPath::parse("items[1]").unwrap();
        let json = json!({"items": ["a", "b", "c"]});
        assert_eq!(path.extract_str(&json), Some("b"));
    }

    #[test]
    fn test_extract_complex() {
        let path = JsonPath::parse("choices[0].delta.content").unwrap();
        let json = json!({
            "choices": [{
                "delta": {
                    "content": "Hello, world!"
                }
            }]
        });
        assert_eq!(path.extract_str(&json), Some("Hello, world!"));
    }

    #[test]
    fn test_extract_missing() {
        let path = JsonPath::parse("missing.field").unwrap();
        let json = json!({"other": "value"});
        assert_eq!(path.extract(&json), None);
    }

    #[test]
    fn test_extract_wrong_type() {
        let path = JsonPath::parse("value").unwrap();
        let json = json!({"value": 42});
        assert_eq!(path.extract_str(&json), None);
        assert_eq!(path.extract_i64(&json), Some(42));
    }

    #[test]
    fn test_extract_bool() {
        let path = JsonPath::parse("enabled").unwrap();
        let json = json!({"enabled": true});
        assert_eq!(path.extract_bool(&json), Some(true));
    }

    #[test]
    fn test_extract_f64() {
        let path = JsonPath::parse("price").unwrap();
        let json = json!({"price": 19.99});
        assert_eq!(path.extract_f64(&json), Some(19.99));
    }

    #[test]
    fn test_exists() {
        let path = JsonPath::parse("field").unwrap();
        let json = json!({"field": "value"});
        assert!(path.exists(&json));

        let json = json!({"other": "value"});
        assert!(!path.exists(&json));
    }

    #[test]
    fn test_exists_non_null() {
        let path = JsonPath::parse("field").unwrap();
        let json = json!({"field": "value"});
        assert!(path.exists_non_null(&json));

        let json = json!({"field": null});
        assert!(!path.exists_non_null(&json));

        let json = json!({"other": "value"});
        assert!(!path.exists_non_null(&json));
    }

    #[test]
    fn test_equals_str() {
        let path = JsonPath::parse("type").unwrap();
        let json = json!({"type": "message_stop"});
        assert!(path.equals_str(&json, "message_stop"));
        assert!(!path.equals_str(&json, "other"));
    }

    #[test]
    fn test_builder() {
        let path = JsonPath::builder()
            .field("choices")
            .index(0)
            .field("delta")
            .field("content")
            .build();

        let json = json!({
            "choices": [{
                "delta": {
                    "content": "Hello!"
                }
            }]
        });

        assert_eq!(path.extract_str(&json), Some("Hello!"));
    }

    #[test]
    fn test_display() {
        let path = JsonPath::parse("choices[0].delta.content").unwrap();
        assert_eq!(path.to_string(), "choices[0].delta.content");

        let path = JsonPath::builder()
            .field("items")
            .index(0)
            .field("name")
            .build();
        assert_eq!(path.to_string(), "items[0].name");
    }

    #[test]
    fn test_path_set() {
        let paths = JsonPathSet::new()
            .with("content", JsonPath::parse("delta.text").unwrap())
            .with("type", JsonPath::parse("type").unwrap());

        let json = json!({
            "type": "content_block_delta",
            "delta": {"text": "Hello"}
        });

        let results = paths.extract_all(&json);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1.and_then(|v| v.as_str()), Some("Hello"));
        assert_eq!(
            results[1].1.and_then(|v| v.as_str()),
            Some("content_block_delta")
        );
    }

    #[test]
    fn test_openai_streaming_patterns() {
        let paths = patterns::openai_streaming();
        let json = json!({
            "choices": [{
                "delta": {
                    "content": "Hello",
                    "role": "assistant"
                },
                "finish_reason": null
            }]
        });

        let content = paths.get("content").unwrap();
        assert_eq!(content.extract_str(&json), Some("Hello"));

        let role = paths.get("role").unwrap();
        assert_eq!(role.extract_str(&json), Some("assistant"));
    }

    #[test]
    fn test_claude_streaming_patterns() {
        let paths = patterns::claude_streaming();
        let json = json!({
            "type": "content_block_delta",
            "delta": {"text": "world"}
        });

        let text = paths.get("text").unwrap();
        assert_eq!(text.extract_str(&json), Some("world"));

        let type_path = paths.get("type").unwrap();
        assert_eq!(type_path.extract_str(&json), Some("content_block_delta"));
    }

    #[test]
    fn test_gemini_streaming_patterns() {
        let paths = patterns::gemini_streaming();
        let json = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "Gemini response"
                    }]
                },
                "finishReason": "STOP"
            }]
        });

        let text = paths.get("text").unwrap();
        assert_eq!(text.extract_str(&json), Some("Gemini response"));

        let finish = paths.get("finish_reason").unwrap();
        assert_eq!(finish.extract_str(&json), Some("STOP"));
    }

    #[test]
    fn test_error_paths() {
        let paths = patterns::error_paths();
        let json = json!({
            "error": {
                "message": "Rate limit exceeded",
                "type": "rate_limit_error",
                "code": "429"
            }
        });

        let msg = paths.get("error_message").unwrap();
        assert_eq!(msg.extract_str(&json), Some("Rate limit exceeded"));

        let error_type = paths.get("error_type").unwrap();
        assert_eq!(error_type.extract_str(&json), Some("rate_limit_error"));
    }

    #[test]
    fn test_extract_array_values() {
        let path = JsonPath::parse("items").unwrap();
        let json = json!({"items": [1, 2, 3]});

        let arr = path.extract_array(&json).unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], json!(1));
    }

    #[test]
    fn test_extract_object() {
        let path = JsonPath::parse("user").unwrap();
        let json = json!({"user": {"name": "Alice", "age": 30}});

        let obj = path.extract_object(&json).unwrap();
        assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("Alice"));
    }

    #[test]
    fn test_underscores_and_hyphens_in_field_names() {
        let path = JsonPath::parse("field_name.sub-field").unwrap();
        let json = json!({"field_name": {"sub-field": "value"}});
        assert_eq!(path.extract_str(&json), Some("value"));
    }

    #[test]
    fn test_numeric_field_names() {
        let path = JsonPath::parse("data.2023").unwrap();
        let json = json!({"data": {"2023": "year data"}});
        assert_eq!(path.extract_str(&json), Some("year data"));
    }
}
