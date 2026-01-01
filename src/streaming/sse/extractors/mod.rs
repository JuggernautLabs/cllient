//! Token extraction implementations for different AI providers
//!
//! This module provides a unified, configuration-driven approach to token extraction
//! from SSE payloads. The `JsonPathExtractor` handles the common extraction pattern,
//! while provider-specific configurations define the JSON paths and end conditions.

use super::core::types::{TokenExtractor, TokenExtractionResult};

/// JSON path for navigating SSE payload structure
#[derive(Debug, Clone)]
pub struct JsonPath(pub Vec<JsonPathSegment>);

/// Segment in a JSON path
#[derive(Debug, Clone)]
pub enum JsonPathSegment {
    /// Object key lookup: `{"key": value}` -> value
    Key(&'static str),
    /// Array index lookup: `[value, ...]` -> value at index
    Index(usize),
}

impl JsonPath {
    /// Navigate a JSON value following this path
    pub fn navigate<'a>(&self, value: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
        let mut current = value;
        for segment in &self.0 {
            current = match segment {
                JsonPathSegment::Key(key) => current.get(*key)?,
                JsonPathSegment::Index(idx) => current.get(*idx)?,
            };
        }
        Some(current)
    }
}

/// End condition detection strategy
#[derive(Debug, Clone)]
pub enum EndCondition {
    /// Check if a JSON path has a specific string value
    JsonPathEquals {
        path: JsonPath,
        value: &'static str,
    },
    /// Check if a JSON path exists and is non-null
    JsonPathExists(JsonPath),
    /// Check raw payload string equality
    PayloadEquals(&'static str),
    /// No end condition from JSON (only uses payload check)
    None,
}

/// Configuration for extracting tokens from provider-specific SSE formats
#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    /// Path to the token/content in the JSON payload
    pub token_path: JsonPath,
    /// How to detect stream end from JSON payload
    pub json_end_condition: EndCondition,
    /// Raw payload string that indicates end of stream (e.g., "[DONE]")
    pub end_payload: Option<&'static str>,
}

/// Generic JSON-path based token extractor
///
/// This extractor uses configuration to navigate provider-specific JSON structures,
/// eliminating the need for separate extractor implementations per provider.
#[derive(Debug, Clone)]
pub struct JsonPathExtractor {
    config: ExtractorConfig,
}

impl JsonPathExtractor {
    pub const fn new(config: ExtractorConfig) -> Self {
        Self { config }
    }

    fn check_json_end(&self, value: &serde_json::Value) -> bool {
        match &self.config.json_end_condition {
            EndCondition::JsonPathEquals { path, value: expected } => {
                path.navigate(value)
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s == *expected)
            }
            EndCondition::JsonPathExists(path) => {
                path.navigate(value)
                    .is_some_and(|v| !v.is_null())
            }
            EndCondition::PayloadEquals(_) | EndCondition::None => false,
        }
    }
}

impl TokenExtractor for JsonPathExtractor {
    fn extract_token(&self, json_value: &serde_json::Value) -> TokenExtractionResult {
        // Check for end condition first
        if self.check_json_end(json_value) {
            return TokenExtractionResult::EndStream;
        }

        // Extract token using configured path
        if let Some(token) = self.config.token_path.navigate(json_value).and_then(|v| v.as_str()) {
            TokenExtractionResult::Token(token.to_string())
        } else {
            TokenExtractionResult::NoToken
        }
    }

    fn is_end_payload(&self, payload: &str) -> bool {
        self.config.end_payload
            .is_some_and(|end| payload.trim() == end)
    }
}

// ============================================================================
// Provider-specific configurations
// ============================================================================

/// OpenAI extractor configuration
///
/// Format: `{"choices":[{"delta":{"content":"token"}, "finish_reason": null}]}`
pub fn openai_config() -> ExtractorConfig {
    use JsonPathSegment::*;
    ExtractorConfig {
        // choices[0].delta.content
        token_path: JsonPath(vec![Key("choices"), Index(0), Key("delta"), Key("content")]),
        // End when choices[0].finish_reason exists and is non-null
        json_end_condition: EndCondition::JsonPathExists(
            JsonPath(vec![Key("choices"), Index(0), Key("finish_reason")])
        ),
        end_payload: Some("[DONE]"),
    }
}

/// Claude/Anthropic extractor configuration
///
/// Format: `{"delta":{"text":"token"}, "type":"content_block_delta"}`
pub fn claude_config() -> ExtractorConfig {
    use JsonPathSegment::*;
    ExtractorConfig {
        // delta.text
        token_path: JsonPath(vec![Key("delta"), Key("text")]),
        // End when type == "message_stop"
        json_end_condition: EndCondition::JsonPathEquals {
            path: JsonPath(vec![Key("type")]),
            value: "message_stop",
        },
        end_payload: None,
    }
}

// ============================================================================
// Convenience type aliases for backwards compatibility
// ============================================================================

/// OpenAI-compatible token extractor
pub type OpenAITokenExtractor = JsonPathExtractor;

/// Claude/Anthropic token extractor
pub type ClaudeTokenExtractor = JsonPathExtractor;

/// Create an OpenAI token extractor
pub fn openai_extractor() -> OpenAITokenExtractor {
    JsonPathExtractor::new(openai_config())
}

/// Create a Claude token extractor
pub fn claude_extractor() -> ClaudeTokenExtractor {
    JsonPathExtractor::new(claude_config())
}
