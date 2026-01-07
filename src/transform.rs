//! Value transformation system for response extraction.
//!
//! This module provides a flexible transformation pipeline for normalizing
//! values extracted from API responses. It supports:
//!
//! - Value mapping (e.g., `STOP` -> `stop`)
//! - Regex replacement
//! - Case transformation (lowercase/uppercase)
//! - Numeric parsing (int/float)
//! - Custom transformation functions
//!
//! # Example
//!
//! ```rust
//! use cllient::transform::{TransformBuilder, TransformEngine};
//! use serde_json::json;
//!
//! // Create transforms for normalizing Google's finish_reason
//! let transform = TransformBuilder::field("finish_reason")
//!     .map("STOP", "stop")
//!     .map("MAX_TOKENS", "length")
//!     .map("SAFETY", "content_filter")
//!     .build();
//!
//! let engine = TransformEngine::new(vec![transform]);
//! let mut value = json!({"finish_reason": "STOP"});
//! engine.apply(&mut value).unwrap();
//!
//! assert_eq!(value["finish_reason"], "stop");
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::error::{ClientError, Result};

/// A transformation rule to apply to a specific field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueTransform {
    /// Field path to transform (supports dot notation for nested fields)
    field: String,
    /// The transformation to apply
    transform: TransformType,
}

impl ValueTransform {
    /// Create a new value transform for a specific field.
    pub fn new(field: impl Into<String>, transform: TransformType) -> Self {
        Self {
            field: field.into(),
            transform,
        }
    }

    /// Get the field path this transform applies to.
    pub fn field(&self) -> &str {
        &self.field
    }

    /// Get the transform type.
    pub fn transform(&self) -> &TransformType {
        &self.transform
    }

    /// Apply this transform to a value.
    pub fn apply(&self, root: &mut Value) -> Result<()> {
        if let Some(value) = get_value_mut(root, &self.field) {
            *value = self.transform.apply(value.take())?;
        }
        Ok(())
    }
}

/// Types of transformations that can be applied to values.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformType {
    /// Map specific values to different values.
    ///
    /// If the input value matches a key, it's replaced with the corresponding value.
    /// If no match is found, the original value is returned unchanged.
    Map(HashMap<String, String>),

    /// Apply regex-based find and replace.
    ///
    /// The pattern is applied to string values, with matches replaced by the replacement string.
    /// Supports capture group references ($1, $2, etc.) in the replacement.
    Regex {
        pattern: String,
        replacement: String,
    },

    /// Convert string values to lowercase.
    Lowercase,

    /// Convert string values to uppercase.
    Uppercase,

    /// Parse string values as integers.
    ///
    /// Returns null if parsing fails.
    ParseInt,

    /// Parse string values as floating-point numbers.
    ///
    /// Returns null if parsing fails.
    ParseFloat,

    /// Custom transformation function.
    ///
    /// This variant cannot be serialized/deserialized and is intended for programmatic use.
    #[serde(skip)]
    Custom(Arc<dyn Fn(Value) -> Value + Send + Sync>),
}

impl fmt::Debug for TransformType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransformType::Map(m) => f.debug_tuple("Map").field(m).finish(),
            TransformType::Regex {
                pattern,
                replacement,
            } => f
                .debug_struct("Regex")
                .field("pattern", pattern)
                .field("replacement", replacement)
                .finish(),
            TransformType::Lowercase => write!(f, "Lowercase"),
            TransformType::Uppercase => write!(f, "Uppercase"),
            TransformType::ParseInt => write!(f, "ParseInt"),
            TransformType::ParseFloat => write!(f, "ParseFloat"),
            TransformType::Custom(_) => write!(f, "Custom(<fn>)"),
        }
    }
}

impl TransformType {
    /// Apply this transformation to a JSON value.
    pub fn apply(&self, value: Value) -> Result<Value> {
        match self {
            TransformType::Map(mappings) => {
                if let Value::String(s) = &value {
                    if let Some(mapped) = mappings.get(s) {
                        return Ok(Value::String(mapped.clone()));
                    }
                }
                Ok(value)
            }

            TransformType::Regex {
                pattern,
                replacement,
            } => {
                if let Value::String(s) = value {
                    let re = Regex::new(pattern).map_err(|e| {
                        ClientError::ValidationError(format!("Invalid regex pattern: {}", e))
                    })?;
                    let result = re.replace_all(&s, replacement.as_str());
                    Ok(Value::String(result.into_owned()))
                } else {
                    Ok(value)
                }
            }

            TransformType::Lowercase => {
                if let Value::String(s) = value {
                    Ok(Value::String(s.to_lowercase()))
                } else {
                    Ok(value)
                }
            }

            TransformType::Uppercase => {
                if let Value::String(s) = value {
                    Ok(Value::String(s.to_uppercase()))
                } else {
                    Ok(value)
                }
            }

            TransformType::ParseInt => {
                if let Value::String(s) = &value {
                    if let Ok(n) = s.parse::<i64>() {
                        return Ok(Value::Number(n.into()));
                    }
                }
                // Already a number, return as-is
                if value.is_number() {
                    return Ok(value);
                }
                Ok(Value::Null)
            }

            TransformType::ParseFloat => {
                if let Value::String(s) = &value {
                    if let Ok(n) = s.parse::<f64>() {
                        if let Some(num) = serde_json::Number::from_f64(n) {
                            return Ok(Value::Number(num));
                        }
                    }
                }
                // Already a number, return as-is
                if value.is_number() {
                    return Ok(value);
                }
                Ok(Value::Null)
            }

            TransformType::Custom(f) => Ok(f(value)),
        }
    }
}

/// Builder for creating `ValueTransform` instances with a fluent API.
///
/// # Example
///
/// ```rust
/// use cllient::transform::TransformBuilder;
///
/// let transform = TransformBuilder::field("status")
///     .map("ACTIVE", "active")
///     .map("INACTIVE", "inactive")
///     .build();
/// ```
pub struct TransformBuilder {
    field: String,
    transform_type: TransformBuilderType,
}

enum TransformBuilderType {
    Map(HashMap<String, String>),
    Regex { pattern: String, replacement: String },
    Lowercase,
    Uppercase,
    ParseInt,
    ParseFloat,
    Custom(Arc<dyn Fn(Value) -> Value + Send + Sync>),
}

impl TransformBuilder {
    /// Start building a transform for the specified field.
    pub fn field(name: impl Into<String>) -> Self {
        Self {
            field: name.into(),
            transform_type: TransformBuilderType::Map(HashMap::new()),
        }
    }

    /// Add a value mapping.
    ///
    /// Multiple mappings can be chained together.
    pub fn map(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        if let TransformBuilderType::Map(ref mut mappings) = self.transform_type {
            mappings.insert(from.into(), to.into());
        } else {
            // Convert to map if not already
            let mut mappings = HashMap::new();
            mappings.insert(from.into(), to.into());
            self.transform_type = TransformBuilderType::Map(mappings);
        }
        self
    }

    /// Set a regex transformation.
    ///
    /// This will replace any previous transformation type.
    pub fn regex(mut self, pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        self.transform_type = TransformBuilderType::Regex {
            pattern: pattern.into(),
            replacement: replacement.into(),
        };
        self
    }

    /// Set a lowercase transformation.
    ///
    /// This will replace any previous transformation type.
    pub fn lowercase(mut self) -> Self {
        self.transform_type = TransformBuilderType::Lowercase;
        self
    }

    /// Set an uppercase transformation.
    ///
    /// This will replace any previous transformation type.
    pub fn uppercase(mut self) -> Self {
        self.transform_type = TransformBuilderType::Uppercase;
        self
    }

    /// Set a parse-int transformation.
    ///
    /// This will replace any previous transformation type.
    pub fn parse_int(mut self) -> Self {
        self.transform_type = TransformBuilderType::ParseInt;
        self
    }

    /// Set a parse-float transformation.
    ///
    /// This will replace any previous transformation type.
    pub fn parse_float(mut self) -> Self {
        self.transform_type = TransformBuilderType::ParseFloat;
        self
    }

    /// Set a custom transformation function.
    ///
    /// This will replace any previous transformation type.
    pub fn custom<F>(mut self, f: F) -> Self
    where
        F: Fn(Value) -> Value + Send + Sync + 'static,
    {
        self.transform_type = TransformBuilderType::Custom(Arc::new(f));
        self
    }

    /// Build the `ValueTransform`.
    pub fn build(self) -> ValueTransform {
        let transform = match self.transform_type {
            TransformBuilderType::Map(mappings) => TransformType::Map(mappings),
            TransformBuilderType::Regex {
                pattern,
                replacement,
            } => TransformType::Regex {
                pattern,
                replacement,
            },
            TransformBuilderType::Lowercase => TransformType::Lowercase,
            TransformBuilderType::Uppercase => TransformType::Uppercase,
            TransformBuilderType::ParseInt => TransformType::ParseInt,
            TransformBuilderType::ParseFloat => TransformType::ParseFloat,
            TransformBuilderType::Custom(f) => TransformType::Custom(f),
        };
        ValueTransform {
            field: self.field,
            transform,
        }
    }
}

/// Engine for applying multiple transforms to JSON values.
///
/// The engine applies transforms in order, allowing for chained transformations.
#[derive(Debug, Clone, Default)]
pub struct TransformEngine {
    transforms: Vec<ValueTransform>,
}

impl TransformEngine {
    /// Create a new transform engine with the given transforms.
    pub fn new(transforms: Vec<ValueTransform>) -> Self {
        Self { transforms }
    }

    /// Create an empty transform engine.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Add a transform to the engine.
    pub fn add(&mut self, transform: ValueTransform) {
        self.transforms.push(transform);
    }

    /// Apply all transforms to the given value.
    pub fn apply(&self, value: &mut Value) -> Result<()> {
        for transform in &self.transforms {
            transform.apply(value)?;
        }
        Ok(())
    }

    /// Check if the engine has any transforms.
    pub fn is_empty(&self) -> bool {
        self.transforms.is_empty()
    }

    /// Get the number of transforms in the engine.
    pub fn len(&self) -> usize {
        self.transforms.len()
    }

    /// Iterate over the transforms.
    pub fn iter(&self) -> impl Iterator<Item = &ValueTransform> {
        self.transforms.iter()
    }
}

impl FromIterator<ValueTransform> for TransformEngine {
    fn from_iter<I: IntoIterator<Item = ValueTransform>>(iter: I) -> Self {
        Self {
            transforms: iter.into_iter().collect(),
        }
    }
}

/// Parse transforms from YAML config format.
///
/// The YAML format supports:
/// ```yaml
/// transforms:
///   field_name:
///     map:
///       SOURCE_VALUE: target_value
///     # or
///     regex:
///       pattern: "\\d+"
///       replacement: "NUMBER"
///     # or
///     lowercase: true
///     # or
///     uppercase: true
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransformConfig {
    #[serde(flatten)]
    pub fields: HashMap<String, FieldTransformConfig>,
}

/// Configuration for a single field's transform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldTransformConfig {
    /// Simple map transform
    Map { map: HashMap<String, String> },
    /// Regex transform
    Regex { pattern: String, replacement: String },
    /// Lowercase transform
    Lowercase { lowercase: bool },
    /// Uppercase transform
    Uppercase { uppercase: bool },
    /// Parse as integer
    ParseInt { parse_int: bool },
    /// Parse as float
    ParseFloat { parse_float: bool },
}

impl TransformConfig {
    /// Convert the config to a `TransformEngine`.
    pub fn to_engine(&self) -> TransformEngine {
        let transforms: Vec<ValueTransform> = self
            .fields
            .iter()
            .map(|(field, config)| {
                let transform_type = match config {
                    FieldTransformConfig::Map { map } => TransformType::Map(map.clone()),
                    FieldTransformConfig::Regex {
                        pattern,
                        replacement,
                    } => TransformType::Regex {
                        pattern: pattern.clone(),
                        replacement: replacement.clone(),
                    },
                    FieldTransformConfig::Lowercase { lowercase: true } => TransformType::Lowercase,
                    FieldTransformConfig::Lowercase { lowercase: false } => {
                        TransformType::Map(HashMap::new())
                    }
                    FieldTransformConfig::Uppercase { uppercase: true } => TransformType::Uppercase,
                    FieldTransformConfig::Uppercase { uppercase: false } => {
                        TransformType::Map(HashMap::new())
                    }
                    FieldTransformConfig::ParseInt { parse_int: true } => TransformType::ParseInt,
                    FieldTransformConfig::ParseInt { parse_int: false } => {
                        TransformType::Map(HashMap::new())
                    }
                    FieldTransformConfig::ParseFloat { parse_float: true } => {
                        TransformType::ParseFloat
                    }
                    FieldTransformConfig::ParseFloat { parse_float: false } => {
                        TransformType::Map(HashMap::new())
                    }
                };
                ValueTransform::new(field.clone(), transform_type)
            })
            .collect();
        TransformEngine::new(transforms)
    }
}

/// Helper function to get a mutable reference to a nested value by path.
///
/// Supports dot notation: "foo.bar.baz" will navigate to `value["foo"]["bar"]["baz"]`.
fn get_value_mut<'a>(root: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for part in parts {
        // Try array index first
        if let Ok(index) = part.parse::<usize>() {
            current = current.get_mut(index)?;
        } else {
            current = current.get_mut(part)?;
        }
    }

    Some(current)
}

/// Get a value by path (immutable).
///
/// Supports dot notation for nested access: "foo.bar.baz" navigates to `value["foo"]["bar"]["baz"]`.
/// Also supports array indices: "items.0.name" navigates to `value["items"][0]["name"]`.
pub fn get_value<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for part in parts {
        // Try array index first
        if let Ok(index) = part.parse::<usize>() {
            current = current.get(index)?;
        } else {
            current = current.get(part)?;
        }
    }

    Some(current)
}

/// Set a value at a path, creating intermediate objects as needed.
#[allow(dead_code)]
fn set_value(root: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.is_empty() {
        return;
    }

    let mut current = root;
    for part in &parts[..parts.len() - 1] {
        // Ensure intermediate objects exist
        if !current.get(part).is_some() {
            current[*part] = Value::Object(serde_json::Map::new());
        }
        current = current.get_mut(part).unwrap();
    }

    let last_part = parts[parts.len() - 1];
    current[last_part] = value;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_map_transform() {
        let transform = TransformBuilder::field("finish_reason")
            .map("STOP", "stop")
            .map("MAX_TOKENS", "length")
            .map("SAFETY", "content_filter")
            .build();

        let mut value = json!({"finish_reason": "STOP"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["finish_reason"], "stop");

        let mut value = json!({"finish_reason": "MAX_TOKENS"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["finish_reason"], "length");

        // Unknown value should pass through unchanged
        let mut value = json!({"finish_reason": "UNKNOWN"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["finish_reason"], "UNKNOWN");
    }

    #[test]
    fn test_map_transform_google_role() {
        // Test the Google role transform example from the YAML
        let transform = TransformBuilder::field("role")
            .map("model", "assistant")
            .build();

        let mut value = json!({"role": "model"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["role"], "assistant");

        // User role should pass through
        let mut value = json!({"role": "user"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["role"], "user");
    }

    #[test]
    fn test_regex_transform() {
        let transform = TransformBuilder::field("text")
            .regex(r"\d+", "NUMBER")
            .build();

        let mut value = json!({"text": "I have 42 apples and 7 oranges"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["text"], "I have NUMBER apples and NUMBER oranges");
    }

    #[test]
    fn test_regex_transform_with_capture_groups() {
        let transform = TransformBuilder::field("text")
            .regex(r"(\w+)@(\w+)\.com", "[$1 at $2]")
            .build();

        let mut value = json!({"text": "Contact: user@example.com"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["text"], "Contact: [user at example]");
    }

    #[test]
    fn test_lowercase_transform() {
        let transform = TransformBuilder::field("status").lowercase().build();

        let mut value = json!({"status": "ACTIVE"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["status"], "active");
    }

    #[test]
    fn test_uppercase_transform() {
        let transform = TransformBuilder::field("status").uppercase().build();

        let mut value = json!({"status": "pending"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["status"], "PENDING");
    }

    #[test]
    fn test_parse_int_transform() {
        let transform = TransformBuilder::field("count").parse_int().build();

        let mut value = json!({"count": "42"});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["count"], 42);

        // Invalid number returns null
        let mut value = json!({"count": "not_a_number"});
        transform.apply(&mut value).unwrap();
        assert!(value["count"].is_null());

        // Already a number passes through
        let mut value = json!({"count": 42});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["count"], 42);
    }

    #[test]
    fn test_parse_float_transform() {
        let transform = TransformBuilder::field("ratio").parse_float().build();

        let mut value = json!({"ratio": "3.14159"});
        transform.apply(&mut value).unwrap();
        assert!((value["ratio"].as_f64().unwrap() - 3.14159).abs() < 0.0001);

        // Invalid number returns null
        let mut value = json!({"ratio": "not_a_float"});
        transform.apply(&mut value).unwrap();
        assert!(value["ratio"].is_null());
    }

    #[test]
    fn test_custom_transform() {
        let transform = TransformBuilder::field("data")
            .custom(|v| {
                if let Value::Array(arr) = v {
                    Value::Number(arr.len().into())
                } else {
                    v
                }
            })
            .build();

        let mut value = json!({"data": [1, 2, 3, 4, 5]});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["data"], 5);
    }

    #[test]
    fn test_nested_field_transform() {
        let transform = TransformBuilder::field("response.status.code")
            .map("OK", "success")
            .build();

        let mut value = json!({
            "response": {
                "status": {
                    "code": "OK"
                }
            }
        });
        transform.apply(&mut value).unwrap();
        assert_eq!(value["response"]["status"]["code"], "success");
    }

    #[test]
    fn test_transform_engine() {
        let engine = TransformEngine::new(vec![
            TransformBuilder::field("finish_reason")
                .map("STOP", "stop")
                .map("MAX_TOKENS", "length")
                .build(),
            TransformBuilder::field("role")
                .map("model", "assistant")
                .build(),
        ]);

        let mut value = json!({
            "finish_reason": "STOP",
            "role": "model",
            "content": "Hello!"
        });

        engine.apply(&mut value).unwrap();

        assert_eq!(value["finish_reason"], "stop");
        assert_eq!(value["role"], "assistant");
        assert_eq!(value["content"], "Hello!");
    }

    #[test]
    fn test_transform_engine_from_iter() {
        let transforms = vec![
            TransformBuilder::field("a").lowercase().build(),
            TransformBuilder::field("b").uppercase().build(),
        ];

        let engine: TransformEngine = transforms.into_iter().collect();
        assert_eq!(engine.len(), 2);
    }

    #[test]
    fn test_transform_config_parsing() {
        let yaml = r#"
finish_reason:
  map:
    STOP: stop
    MAX_TOKENS: length
role:
  map:
    model: assistant
"#;

        let config: TransformConfig = serde_yaml::from_str(yaml).unwrap();
        let engine = config.to_engine();
        assert_eq!(engine.len(), 2);

        let mut value = json!({
            "finish_reason": "STOP",
            "role": "model"
        });
        engine.apply(&mut value).unwrap();
        assert_eq!(value["finish_reason"], "stop");
        assert_eq!(value["role"], "assistant");
    }

    #[test]
    fn test_transform_missing_field() {
        let transform = TransformBuilder::field("nonexistent")
            .map("a", "b")
            .build();

        let mut value = json!({"other_field": "value"});
        // Should not error when field doesn't exist
        transform.apply(&mut value).unwrap();
        assert_eq!(value, json!({"other_field": "value"}));
    }

    #[test]
    fn test_transform_non_string_value() {
        // Map transform on non-string should pass through
        let transform = TransformBuilder::field("count")
            .map("42", "forty-two")
            .build();

        let mut value = json!({"count": 42});
        transform.apply(&mut value).unwrap();
        assert_eq!(value["count"], 42); // Should remain number
    }

    #[test]
    fn test_get_value_array_index() {
        let value = json!({
            "items": [
                {"name": "first"},
                {"name": "second"}
            ]
        });

        assert_eq!(get_value(&value, "items.0.name").unwrap(), "first");
        assert_eq!(get_value(&value, "items.1.name").unwrap(), "second");
    }

    #[test]
    fn test_transform_array_element() {
        let transform = TransformBuilder::field("items.0.status")
            .map("ACTIVE", "active")
            .build();

        let mut value = json!({
            "items": [
                {"status": "ACTIVE"},
                {"status": "INACTIVE"}
            ]
        });
        transform.apply(&mut value).unwrap();
        assert_eq!(value["items"][0]["status"], "active");
        assert_eq!(value["items"][1]["status"], "INACTIVE");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let transform = ValueTransform::new(
            "finish_reason",
            TransformType::Map({
                let mut m = HashMap::new();
                m.insert("STOP".to_string(), "stop".to_string());
                m.insert("MAX_TOKENS".to_string(), "length".to_string());
                m
            }),
        );

        let json = serde_json::to_string(&transform).unwrap();
        let deserialized: ValueTransform = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.field, "finish_reason");
        if let TransformType::Map(map) = &deserialized.transform {
            assert_eq!(map.get("STOP"), Some(&"stop".to_string()));
        } else {
            panic!("Expected Map transform type");
        }
    }

    #[test]
    fn test_yaml_serialization() {
        let transform = ValueTransform::new(
            "status",
            TransformType::Regex {
                pattern: r"\d+".to_string(),
                replacement: "NUM".to_string(),
            },
        );

        let yaml = serde_yaml::to_string(&transform).unwrap();
        let deserialized: ValueTransform = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.field, "status");
        if let TransformType::Regex {
            pattern,
            replacement,
        } = &deserialized.transform
        {
            assert_eq!(pattern, r"\d+");
            assert_eq!(replacement, "NUM");
        } else {
            panic!("Expected Regex transform type");
        }
    }

    #[test]
    fn test_google_finish_reason_transforms() {
        // Test the exact transforms from google.v2.yaml
        let engine = TransformEngine::new(vec![
            TransformBuilder::field("finish_reason")
                .map("STOP", "stop")
                .map("MAX_TOKENS", "length")
                .map("SAFETY", "content_filter")
                .map("RECITATION", "content_filter")
                .map("OTHER", "other")
                .build(),
            TransformBuilder::field("role")
                .map("model", "assistant")
                .build(),
        ]);

        let test_cases = vec![
            ("STOP", "stop"),
            ("MAX_TOKENS", "length"),
            ("SAFETY", "content_filter"),
            ("RECITATION", "content_filter"),
            ("OTHER", "other"),
        ];

        for (input, expected) in test_cases {
            let mut value = json!({"finish_reason": input, "role": "model"});
            engine.apply(&mut value).unwrap();
            assert_eq!(
                value["finish_reason"], expected,
                "Failed for input: {}",
                input
            );
            assert_eq!(value["role"], "assistant");
        }
    }

    #[test]
    fn test_openai_finish_reason_transforms() {
        // Test the transforms from openai.v2.yaml (mostly pass-through)
        let engine = TransformEngine::new(vec![TransformBuilder::field("finish_reason")
            .map("stop", "stop")
            .map("length", "length")
            .map("tool_calls", "tool_calls")
            .map("content_filter", "content_filter")
            .build()]);

        // OpenAI already uses normalized values, so they pass through
        let mut value = json!({"finish_reason": "stop"});
        engine.apply(&mut value).unwrap();
        assert_eq!(value["finish_reason"], "stop");

        let mut value = json!({"finish_reason": "tool_calls"});
        engine.apply(&mut value).unwrap();
        assert_eq!(value["finish_reason"], "tool_calls");
    }

    #[test]
    fn test_chained_transforms() {
        // Apply multiple transforms to the same field
        let engine = TransformEngine::new(vec![
            TransformBuilder::field("text").lowercase().build(),
            TransformBuilder::field("text")
                .regex(r"\s+", "_")
                .build(),
        ]);

        let mut value = json!({"text": "Hello World"});
        engine.apply(&mut value).unwrap();
        assert_eq!(value["text"], "hello_world");
    }

    #[test]
    fn test_empty_engine() {
        let engine = TransformEngine::empty();
        assert!(engine.is_empty());
        assert_eq!(engine.len(), 0);

        let mut value = json!({"test": "value"});
        engine.apply(&mut value).unwrap();
        assert_eq!(value["test"], "value");
    }

    #[test]
    fn test_engine_add() {
        let mut engine = TransformEngine::empty();
        engine.add(TransformBuilder::field("test").lowercase().build());
        assert_eq!(engine.len(), 1);
    }
}
