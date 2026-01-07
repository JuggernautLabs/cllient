//! End condition system for SSE stream termination
//!
//! This module provides a flexible, config-driven system for detecting when SSE streams
//! should terminate. It supports multiple condition types that can be deserialized from
//! YAML configuration files.
//!
//! ## Supported Conditions
//!
//! - `RawLine`: Match exact raw SSE line content (e.g., "[DONE]")
//! - `FieldExists`: Check if a JSON field exists
//! - `FieldNotNull`: Check if a JSON field exists and is not null
//! - `FieldEquals`: Check if a JSON field equals a specific value
//! - `EventType`: Match Anthropic-style typed SSE events
//!
//! ## Configuration Examples
//!
//! ```yaml
//! end_conditions:
//!   # OpenAI style
//!   - raw_line: "[DONE]"
//!   - field: choices[0].finish_reason
//!     not_null: true
//!
//!   # Anthropic style
//!   - field: type
//!     equals: message_stop
//!   - event_type: message_stop
//!
//!   # Google style
//!   - field: candidates[0].finishReason
//!     not_null: true
//!   - field: error
//!     exists: true
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Conditions that signal end of SSE stream.
///
/// Each variant represents a different way to detect stream termination.
/// These can be loaded from YAML configuration files using `#[serde(untagged)]`
/// to automatically detect the appropriate variant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EndCondition {
    /// Raw line matches exactly (e.g., "[DONE]")
    ///
    /// Example YAML: `raw_line: "[DONE]"`
    RawLine {
        /// The exact raw line content to match (before JSON parsing)
        raw_line: String,
    },

    /// Field equals specific value
    ///
    /// Example YAML:
    /// ```yaml
    /// field: type
    /// equals: message_stop
    /// ```
    FieldEquals {
        /// JSON path to the field (e.g., "choices[0].finish_reason")
        field: String,
        /// Value the field must equal
        equals: Value,
    },

    /// Field exists and is not null
    ///
    /// Example YAML:
    /// ```yaml
    /// field: choices[0].finish_reason
    /// not_null: true
    /// ```
    FieldNotNull {
        /// JSON path to the field
        field: String,
        /// Must be true to enable this condition
        not_null: bool,
    },

    /// Field exists in JSON (may be null)
    ///
    /// Example YAML:
    /// ```yaml
    /// field: error
    /// exists: true
    /// ```
    FieldExists {
        /// JSON path to the field
        field: String,
        /// Must be true to enable this condition
        exists: bool,
    },

    /// Event type matches (for Anthropic-style typed events)
    ///
    /// Example YAML: `event_type: message_stop`
    EventType {
        /// The SSE event type to match
        event_type: String,
    },
}

impl EndCondition {
    /// Check if this condition is met.
    ///
    /// # Arguments
    ///
    /// * `raw_line` - The raw SSE line content (before any parsing)
    /// * `parsed` - The parsed JSON value, if available
    /// * `event_type` - The SSE event type (from "event:" prefix), if available
    ///
    /// # Returns
    ///
    /// `true` if the end condition is met and the stream should terminate
    pub fn is_met(&self, raw_line: &str, parsed: Option<&Value>, event_type: Option<&str>) -> bool {
        match self {
            EndCondition::RawLine { raw_line: expected } => {
                raw_line.trim() == expected
            }

            EndCondition::FieldEquals { field, equals } => {
                parsed
                    .and_then(|json| navigate_json_path(json, field))
                    .is_some_and(|v| v == equals)
            }

            EndCondition::FieldNotNull { field, not_null } => {
                if !*not_null {
                    return false;
                }
                parsed
                    .and_then(|json| navigate_json_path(json, field))
                    .is_some_and(|v| !v.is_null())
            }

            EndCondition::FieldExists { field, exists } => {
                if !*exists {
                    return false;
                }
                parsed
                    .and_then(|json| navigate_json_path(json, field))
                    .is_some()
            }

            EndCondition::EventType { event_type: expected } => {
                event_type.is_some_and(|et| et == expected)
            }
        }
    }

    /// Get a human-readable description of this condition
    pub fn description(&self) -> String {
        match self {
            EndCondition::RawLine { raw_line } => {
                format!("raw line equals \"{}\"", raw_line)
            }
            EndCondition::FieldEquals { field, equals } => {
                format!("field {} equals {}", field, equals)
            }
            EndCondition::FieldNotNull { field, .. } => {
                format!("field {} is not null", field)
            }
            EndCondition::FieldExists { field, .. } => {
                format!("field {} exists", field)
            }
            EndCondition::EventType { event_type } => {
                format!("event type is \"{}\"", event_type)
            }
        }
    }
}

/// Navigate a JSON value using a path string.
///
/// Supports dot notation for object keys and bracket notation for array indices.
///
/// # Examples
///
/// - `"choices[0].delta.content"` navigates to `json["choices"][0]["delta"]["content"]`
/// - `"error.message"` navigates to `json["error"]["message"]`
/// - `"candidates[0].finishReason"` navigates to `json["candidates"][0]["finishReason"]`
fn navigate_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;

    // Parse path segments: split by '.' but handle bracket notation
    let segments = parse_path_segments(path);

    for segment in segments {
        current = match segment {
            PathSegment::Key(key) => current.get(key)?,
            PathSegment::Index(idx) => current.get(idx)?,
        };
    }

    Some(current)
}

/// A segment in a JSON path
#[derive(Debug, Clone)]
enum PathSegment<'a> {
    Key(&'a str),
    Index(usize),
}

/// Parse a path string into segments.
///
/// Handles both dot notation and bracket notation:
/// - `"foo.bar"` -> [Key("foo"), Key("bar")]
/// - `"arr[0]"` -> [Key("arr"), Index(0)]
/// - `"arr[0].name"` -> [Key("arr"), Index(0), Key("name")]
fn parse_path_segments(path: &str) -> Vec<PathSegment<'_>> {
    let mut segments = Vec::new();
    let mut remaining = path;

    while !remaining.is_empty() {
        // Skip leading dots
        remaining = remaining.trim_start_matches('.');

        if remaining.is_empty() {
            break;
        }

        // Check for bracket notation at start
        if remaining.starts_with('[') {
            if let Some(end) = remaining.find(']') {
                let idx_str = &remaining[1..end];
                if let Ok(idx) = idx_str.parse::<usize>() {
                    segments.push(PathSegment::Index(idx));
                }
                remaining = &remaining[end + 1..];
                continue;
            }
        }

        // Find end of key (either '.', '[', or end of string)
        let key_end = remaining
            .find(|c| c == '.' || c == '[')
            .unwrap_or(remaining.len());

        if key_end > 0 {
            let key = &remaining[..key_end];
            segments.push(PathSegment::Key(key));
            remaining = &remaining[key_end..];
        } else {
            // Shouldn't happen, but avoid infinite loop
            break;
        }
    }

    segments
}

/// Builder for creating `EndCondition` instances programmatically
pub struct EndConditionBuilder;

impl EndConditionBuilder {
    /// Create a raw line match condition
    ///
    /// # Example
    ///
    /// ```
    /// use cllient::streaming::end_condition::{EndConditionBuilder, EndCondition};
    ///
    /// let condition = EndConditionBuilder::raw_line("[DONE]");
    /// assert!(condition.is_met("[DONE]", None, None));
    /// ```
    pub fn raw_line(line: impl Into<String>) -> EndCondition {
        EndCondition::RawLine {
            raw_line: line.into(),
        }
    }

    /// Create a field not null condition
    ///
    /// # Example
    ///
    /// ```
    /// use cllient::streaming::end_condition::{EndConditionBuilder, EndCondition};
    /// use serde_json::json;
    ///
    /// let condition = EndConditionBuilder::field_not_null("finish_reason");
    /// let json = json!({"finish_reason": "stop"});
    /// assert!(condition.is_met("", Some(&json), None));
    /// ```
    pub fn field_not_null(path: impl Into<String>) -> EndCondition {
        EndCondition::FieldNotNull {
            field: path.into(),
            not_null: true,
        }
    }

    /// Create a field exists condition
    ///
    /// # Example
    ///
    /// ```
    /// use cllient::streaming::end_condition::{EndConditionBuilder, EndCondition};
    /// use serde_json::json;
    ///
    /// let condition = EndConditionBuilder::field_exists("error");
    /// let json = json!({"error": null});
    /// assert!(condition.is_met("", Some(&json), None));
    /// ```
    pub fn field_exists(path: impl Into<String>) -> EndCondition {
        EndCondition::FieldExists {
            field: path.into(),
            exists: true,
        }
    }

    /// Create a field equals condition
    ///
    /// # Example
    ///
    /// ```
    /// use cllient::streaming::end_condition::{EndConditionBuilder, EndCondition};
    /// use serde_json::json;
    ///
    /// let condition = EndConditionBuilder::field_equals("type", "message_stop");
    /// let json = json!({"type": "message_stop"});
    /// assert!(condition.is_met("", Some(&json), None));
    /// ```
    pub fn field_equals(path: impl Into<String>, value: impl Into<Value>) -> EndCondition {
        EndCondition::FieldEquals {
            field: path.into(),
            equals: value.into(),
        }
    }

    /// Create an event type match condition
    ///
    /// # Example
    ///
    /// ```
    /// use cllient::streaming::end_condition::{EndConditionBuilder, EndCondition};
    ///
    /// let condition = EndConditionBuilder::event_type("message_stop");
    /// assert!(condition.is_met("", None, Some("message_stop")));
    /// ```
    pub fn event_type(typ: impl Into<String>) -> EndCondition {
        EndCondition::EventType {
            event_type: typ.into(),
        }
    }
}

/// Collection of end conditions that can be checked together.
///
/// Returns `true` if any condition is met (OR logic).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EndConditions(pub Vec<EndCondition>);

impl EndConditions {
    /// Create an empty set of conditions
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Add a condition
    pub fn add(&mut self, condition: EndCondition) {
        self.0.push(condition);
    }

    /// Check if any condition is met
    pub fn any_met(&self, raw_line: &str, parsed: Option<&Value>, event_type: Option<&str>) -> bool {
        self.0.iter().any(|c| c.is_met(raw_line, parsed, event_type))
    }

    /// Check which condition was met (if any)
    pub fn which_met(&self, raw_line: &str, parsed: Option<&Value>, event_type: Option<&str>) -> Option<&EndCondition> {
        self.0.iter().find(|c| c.is_met(raw_line, parsed, event_type))
    }

    /// Return the number of conditions
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if there are no conditions
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<EndCondition> for EndConditions {
    fn from_iter<I: IntoIterator<Item = EndCondition>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for EndConditions {
    type Item = EndCondition;
    type IntoIter = std::vec::IntoIter<EndCondition>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a EndConditions {
    type Item = &'a EndCondition;
    type IntoIter = std::slice::Iter<'a, EndCondition>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // Path parsing tests
    // ========================================================================

    #[test]
    fn test_parse_simple_path() {
        let segments = parse_path_segments("foo");
        assert_eq!(segments.len(), 1);
        assert!(matches!(segments[0], PathSegment::Key("foo")));
    }

    #[test]
    fn test_parse_dotted_path() {
        let segments = parse_path_segments("foo.bar.baz");
        assert_eq!(segments.len(), 3);
        assert!(matches!(segments[0], PathSegment::Key("foo")));
        assert!(matches!(segments[1], PathSegment::Key("bar")));
        assert!(matches!(segments[2], PathSegment::Key("baz")));
    }

    #[test]
    fn test_parse_array_index() {
        let segments = parse_path_segments("arr[0]");
        assert_eq!(segments.len(), 2);
        assert!(matches!(segments[0], PathSegment::Key("arr")));
        assert!(matches!(segments[1], PathSegment::Index(0)));
    }

    #[test]
    fn test_parse_complex_path() {
        let segments = parse_path_segments("choices[0].delta.content");
        assert_eq!(segments.len(), 4);
        assert!(matches!(segments[0], PathSegment::Key("choices")));
        assert!(matches!(segments[1], PathSegment::Index(0)));
        assert!(matches!(segments[2], PathSegment::Key("delta")));
        assert!(matches!(segments[3], PathSegment::Key("content")));
    }

    #[test]
    fn test_parse_nested_arrays() {
        let segments = parse_path_segments("arr[0][1]");
        assert_eq!(segments.len(), 3);
        assert!(matches!(segments[0], PathSegment::Key("arr")));
        assert!(matches!(segments[1], PathSegment::Index(0)));
        assert!(matches!(segments[2], PathSegment::Index(1)));
    }

    // ========================================================================
    // JSON navigation tests
    // ========================================================================

    #[test]
    fn test_navigate_simple_key() {
        let json = json!({"foo": "bar"});
        let result = navigate_json_path(&json, "foo");
        assert_eq!(result, Some(&json!("bar")));
    }

    #[test]
    fn test_navigate_nested_keys() {
        let json = json!({"foo": {"bar": {"baz": 42}}});
        let result = navigate_json_path(&json, "foo.bar.baz");
        assert_eq!(result, Some(&json!(42)));
    }

    #[test]
    fn test_navigate_array_index() {
        let json = json!({"arr": [1, 2, 3]});
        let result = navigate_json_path(&json, "arr[1]");
        assert_eq!(result, Some(&json!(2)));
    }

    #[test]
    fn test_navigate_openai_path() {
        let json = json!({
            "choices": [{
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        });
        let result = navigate_json_path(&json, "choices[0].delta.content");
        assert_eq!(result, Some(&json!("Hello")));
    }

    #[test]
    fn test_navigate_missing_key() {
        let json = json!({"foo": "bar"});
        let result = navigate_json_path(&json, "baz");
        assert_eq!(result, None);
    }

    #[test]
    fn test_navigate_out_of_bounds() {
        let json = json!({"arr": [1, 2]});
        let result = navigate_json_path(&json, "arr[5]");
        assert_eq!(result, None);
    }

    // ========================================================================
    // RawLine condition tests
    // ========================================================================

    #[test]
    fn test_raw_line_exact_match() {
        let condition = EndConditionBuilder::raw_line("[DONE]");
        assert!(condition.is_met("[DONE]", None, None));
    }

    #[test]
    fn test_raw_line_trimmed_match() {
        let condition = EndConditionBuilder::raw_line("[DONE]");
        assert!(condition.is_met("  [DONE]  ", None, None));
    }

    #[test]
    fn test_raw_line_no_match() {
        let condition = EndConditionBuilder::raw_line("[DONE]");
        assert!(!condition.is_met("[FINISHED]", None, None));
    }

    // ========================================================================
    // FieldEquals condition tests
    // ========================================================================

    #[test]
    fn test_field_equals_string() {
        let condition = EndConditionBuilder::field_equals("type", "message_stop");
        let json = json!({"type": "message_stop"});
        assert!(condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_equals_number() {
        let condition = EndConditionBuilder::field_equals("status", 200);
        let json = json!({"status": 200});
        assert!(condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_equals_nested() {
        let condition = EndConditionBuilder::field_equals("response.type", "complete");
        let json = json!({"response": {"type": "complete"}});
        assert!(condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_equals_wrong_value() {
        let condition = EndConditionBuilder::field_equals("type", "message_stop");
        let json = json!({"type": "content_block_delta"});
        assert!(!condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_equals_missing_field() {
        let condition = EndConditionBuilder::field_equals("type", "message_stop");
        let json = json!({"other": "value"});
        assert!(!condition.is_met("", Some(&json), None));
    }

    // ========================================================================
    // FieldNotNull condition tests
    // ========================================================================

    #[test]
    fn test_field_not_null_with_value() {
        let condition = EndConditionBuilder::field_not_null("finish_reason");
        let json = json!({"finish_reason": "stop"});
        assert!(condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_not_null_with_null() {
        let condition = EndConditionBuilder::field_not_null("finish_reason");
        let json = json!({"finish_reason": null});
        assert!(!condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_not_null_missing() {
        let condition = EndConditionBuilder::field_not_null("finish_reason");
        let json = json!({"other": "value"});
        assert!(!condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_not_null_nested() {
        let condition = EndConditionBuilder::field_not_null("choices[0].finish_reason");
        let json = json!({
            "choices": [{"finish_reason": "length"}]
        });
        assert!(condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_not_null_nested_null() {
        let condition = EndConditionBuilder::field_not_null("choices[0].finish_reason");
        let json = json!({
            "choices": [{"finish_reason": null}]
        });
        assert!(!condition.is_met("", Some(&json), None));
    }

    // ========================================================================
    // FieldExists condition tests
    // ========================================================================

    #[test]
    fn test_field_exists_present() {
        let condition = EndConditionBuilder::field_exists("error");
        let json = json!({"error": {"message": "test"}});
        assert!(condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_exists_null_value() {
        let condition = EndConditionBuilder::field_exists("error");
        let json = json!({"error": null});
        assert!(condition.is_met("", Some(&json), None));
    }

    #[test]
    fn test_field_exists_missing() {
        let condition = EndConditionBuilder::field_exists("error");
        let json = json!({"success": true});
        assert!(!condition.is_met("", Some(&json), None));
    }

    // ========================================================================
    // EventType condition tests
    // ========================================================================

    #[test]
    fn test_event_type_match() {
        let condition = EndConditionBuilder::event_type("message_stop");
        assert!(condition.is_met("", None, Some("message_stop")));
    }

    #[test]
    fn test_event_type_no_match() {
        let condition = EndConditionBuilder::event_type("message_stop");
        assert!(!condition.is_met("", None, Some("content_block_delta")));
    }

    #[test]
    fn test_event_type_none_provided() {
        let condition = EndConditionBuilder::event_type("message_stop");
        assert!(!condition.is_met("", None, None));
    }

    // ========================================================================
    // YAML deserialization tests
    // ========================================================================

    #[test]
    fn test_deserialize_raw_line() {
        let yaml = r#"raw_line: "[DONE]""#;
        let condition: EndCondition = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(condition, EndCondition::RawLine { raw_line } if raw_line == "[DONE]"));
    }

    #[test]
    fn test_deserialize_field_equals() {
        let yaml = r#"
field: type
equals: message_stop
"#;
        let condition: EndCondition = serde_yaml::from_str(yaml).unwrap();
        assert!(condition.is_met("", Some(&json!({"type": "message_stop"})), None));
    }

    #[test]
    fn test_deserialize_field_not_null() {
        let yaml = r#"
field: choices[0].finish_reason
not_null: true
"#;
        let condition: EndCondition = serde_yaml::from_str(yaml).unwrap();
        assert!(condition.is_met("", Some(&json!({"choices": [{"finish_reason": "stop"}]})), None));
    }

    #[test]
    fn test_deserialize_field_exists() {
        let yaml = r#"
field: error
exists: true
"#;
        let condition: EndCondition = serde_yaml::from_str(yaml).unwrap();
        assert!(condition.is_met("", Some(&json!({"error": null})), None));
    }

    #[test]
    fn test_deserialize_event_type() {
        let yaml = r#"event_type: message_stop"#;
        let condition: EndCondition = serde_yaml::from_str(yaml).unwrap();
        assert!(condition.is_met("", None, Some("message_stop")));
    }

    #[test]
    fn test_deserialize_openai_conditions() {
        let yaml = r#"
- raw_line: "[DONE]"
- field: choices[0].finish_reason
  not_null: true
"#;
        let conditions: Vec<EndCondition> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(conditions.len(), 2);

        // Test raw line condition
        assert!(conditions[0].is_met("[DONE]", None, None));

        // Test field not null condition
        let json = json!({"choices": [{"finish_reason": "stop"}]});
        assert!(conditions[1].is_met("", Some(&json), None));
    }

    #[test]
    fn test_deserialize_anthropic_conditions() {
        let yaml = r#"
- field: type
  equals: message_stop
- field: error
  exists: true
"#;
        let conditions: Vec<EndCondition> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(conditions.len(), 2);

        // Test field equals condition
        let json = json!({"type": "message_stop"});
        assert!(conditions[0].is_met("", Some(&json), None));

        // Test field exists condition
        let json = json!({"error": {"message": "test"}});
        assert!(conditions[1].is_met("", Some(&json), None));
    }

    #[test]
    fn test_deserialize_google_conditions() {
        let yaml = r#"
- field: candidates[0].finishReason
  not_null: true
- field: error
  exists: true
"#;
        let conditions: Vec<EndCondition> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(conditions.len(), 2);

        // Test finish reason
        let json = json!({"candidates": [{"finishReason": "STOP"}]});
        assert!(conditions[0].is_met("", Some(&json), None));
    }

    // ========================================================================
    // EndConditions collection tests
    // ========================================================================

    #[test]
    fn test_end_conditions_any_met() {
        let conditions = EndConditions(vec![
            EndConditionBuilder::raw_line("[DONE]"),
            EndConditionBuilder::field_not_null("finish_reason"),
        ]);

        // First condition met
        assert!(conditions.any_met("[DONE]", None, None));

        // Second condition met
        let json = json!({"finish_reason": "stop"});
        assert!(conditions.any_met("other", Some(&json), None));

        // Neither met
        let json = json!({"finish_reason": null});
        assert!(!conditions.any_met("other", Some(&json), None));
    }

    #[test]
    fn test_end_conditions_which_met() {
        let conditions = EndConditions(vec![
            EndConditionBuilder::raw_line("[DONE]"),
            EndConditionBuilder::field_not_null("finish_reason"),
        ]);

        let met = conditions.which_met("[DONE]", None, None);
        assert!(met.is_some());
        assert!(matches!(met.unwrap(), EndCondition::RawLine { .. }));
    }

    #[test]
    fn test_end_conditions_empty() {
        let conditions = EndConditions::new();
        assert!(conditions.is_empty());
        assert!(!conditions.any_met("[DONE]", None, None));
    }

    // ========================================================================
    // Real-world integration tests
    // ========================================================================

    #[test]
    fn test_openai_stream_end() {
        let yaml = r#"
- raw_line: "[DONE]"
- field: choices[0].finish_reason
  not_null: true
"#;
        let conditions: EndConditions = EndConditions(serde_yaml::from_str(yaml).unwrap());

        // Simulate OpenAI [DONE] message
        assert!(conditions.any_met("[DONE]", None, None));

        // Simulate finish_reason being set
        let json = json!({
            "id": "chatcmpl-123",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        assert!(conditions.any_met("", Some(&json), None));

        // Normal streaming chunk (not done)
        let json = json!({
            "id": "chatcmpl-123",
            "choices": [{
                "index": 0,
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        });
        assert!(!conditions.any_met("", Some(&json), None));
    }

    #[test]
    fn test_anthropic_stream_end() {
        let yaml = r#"
- field: type
  equals: message_stop
- field: error
  exists: true
"#;
        let conditions: EndConditions = EndConditions(serde_yaml::from_str(yaml).unwrap());

        // Simulate Anthropic message_stop
        let json = json!({"type": "message_stop"});
        assert!(conditions.any_met("", Some(&json), None));

        // Simulate error
        let json = json!({
            "type": "error",
            "error": {"type": "rate_limit", "message": "Too many requests"}
        });
        assert!(conditions.any_met("", Some(&json), None));

        // Normal content block (not done)
        let json = json!({
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "Hello"}
        });
        assert!(!conditions.any_met("", Some(&json), None));
    }

    #[test]
    fn test_google_stream_end() {
        let yaml = r#"
- field: candidates[0].finishReason
  not_null: true
- field: error
  exists: true
"#;
        let conditions: EndConditions = EndConditions(serde_yaml::from_str(yaml).unwrap());

        // Simulate Google finish
        let json = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hello"}]},
                "finishReason": "STOP"
            }]
        });
        assert!(conditions.any_met("", Some(&json), None));

        // Simulate error
        let json = json!({
            "error": {
                "code": 429,
                "message": "Resource exhausted"
            }
        });
        assert!(conditions.any_met("", Some(&json), None));

        // Normal streaming chunk (no finish reason yet)
        let json = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hello"}]}
            }]
        });
        assert!(!conditions.any_met("", Some(&json), None));
    }

    // ========================================================================
    // Description tests
    // ========================================================================

    #[test]
    fn test_descriptions() {
        let conditions = vec![
            EndConditionBuilder::raw_line("[DONE]"),
            EndConditionBuilder::field_equals("type", "stop"),
            EndConditionBuilder::field_not_null("finish_reason"),
            EndConditionBuilder::field_exists("error"),
            EndConditionBuilder::event_type("message_stop"),
        ];

        assert_eq!(conditions[0].description(), "raw line equals \"[DONE]\"");
        assert_eq!(conditions[1].description(), "field type equals \"stop\"");
        assert_eq!(conditions[2].description(), "field finish_reason is not null");
        assert_eq!(conditions[3].description(), "field error exists");
        assert_eq!(conditions[4].description(), "event type is \"message_stop\"");
    }

    // ========================================================================
    // Serialization round-trip tests
    // ========================================================================

    #[test]
    fn test_serialize_deserialize_round_trip() {
        let original = vec![
            EndConditionBuilder::raw_line("[DONE]"),
            EndConditionBuilder::field_equals("type", "message_stop"),
            EndConditionBuilder::field_not_null("choices[0].finish_reason"),
            EndConditionBuilder::field_exists("error"),
            EndConditionBuilder::event_type("message_stop"),
        ];

        let yaml = serde_yaml::to_string(&original).unwrap();
        let deserialized: Vec<EndCondition> = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(original.len(), deserialized.len());

        // Test each condition still works correctly
        assert!(deserialized[0].is_met("[DONE]", None, None));
        assert!(deserialized[1].is_met("", Some(&json!({"type": "message_stop"})), None));
        assert!(deserialized[2].is_met("", Some(&json!({"choices": [{"finish_reason": "stop"}]})), None));
        assert!(deserialized[3].is_met("", Some(&json!({"error": null})), None));
        assert!(deserialized[4].is_met("", None, Some("message_stop")));
    }
}
