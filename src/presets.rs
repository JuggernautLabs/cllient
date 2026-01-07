//! Preset builder configurations for major LLM providers.
//!
//! This module provides convenience functions that return pre-configured builders
//! matching the v2 YAML service configurations. These presets eliminate the need
//! to manually construct complex configurations for common providers.
//!
//! # Usage
//!
//! ```rust,ignore
//! use cllient::presets;
//!
//! // Message formatting presets
//! let openai_format = presets::message_format::openai();
//! let anthropic_format = presets::message_format::anthropic();
//! let google_format = presets::message_format::google();
//!
//! // SSE parser presets
//! let openai_parser = presets::sse_parser::openai();
//! let anthropic_parser = presets::sse_parser::anthropic();
//! let google_parser = presets::sse_parser::google();
//!
//! // Response extractor presets
//! let openai_extractor = presets::response_extractor::openai();
//! let anthropic_extractor = presets::response_extractor::anthropic();
//! let google_extractor = presets::response_extractor::google();
//! ```

use crate::config::{ResponseConfig, ResponseExtract, UsageExtract};
use crate::message_format::{
    ContentBlockConfig, ContentBlockType, MessageFormatBuilder, MessageFormatConfig,
};
use crate::streaming::sse::extractors::{
    EndCondition, ExtractorConfig, JsonPath, JsonPathExtractor, JsonPathSegment,
};
use std::collections::HashMap;

// ============================================================================
// Message Format Presets
// ============================================================================

/// Pre-configured message format builders for major LLM providers.
///
/// These builders match the `message_format` sections of the v2 YAML configs exactly.
pub mod message_format {
    use super::*;

    /// OpenAI-compatible message format builder.
    ///
    /// Matches `config/service-v2/openai.v2.yaml` message_format section:
    /// - Roles: user, assistant, system, tool
    /// - System handling: inline (system messages are regular messages)
    /// - Content array: true
    /// - String shorthand: true
    /// - Supports: text, image (data URI), audio, tool_use, tool_result
    pub fn openai() -> MessageFormatBuilder {
        MessageFormatBuilder::new()
            .name("openai")
            // Text-only messages use string shorthand
            .text_message_template(r#"{"role": {{json role}}, "content": {{json content}}}"#)
            // Multimodal messages use content array
            .multimodal_message_template(r#"{"role": {{json role}}, "content": {{json content_blocks}}}"#)
            // Text block
            .text_block(r#"{"type": "text", "text": {{json text}}}"#)
            // Image blocks (data URI format)
            .binary_block_with_priority(
                vec!["image/jpeg".to_string()],
                r#"{"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,{{base64_data}}"}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/png".to_string()],
                r#"{"type": "image_url", "image_url": {"url": "data:image/png;base64,{{base64_data}}"}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/gif".to_string()],
                r#"{"type": "image_url", "image_url": {"url": "data:image/gif;base64,{{base64_data}}"}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/webp".to_string()],
                r#"{"type": "image_url", "image_url": {"url": "data:image/webp;base64,{{base64_data}}"}}"#,
                10,
            )
            // Image URL block
            .url_block_with_priority(
                vec!["image/*".to_string()],
                r#"{"type": "image_url", "image_url": {"url": {{json url}}, "detail": "auto"}}"#,
                10,
            )
            // Audio blocks (for GPT-4o audio)
            .binary_block_with_priority(
                vec!["audio/wav".to_string()],
                r#"{"type": "input_audio", "input_audio": {"data": {{json base64_data}}, "format": "wav"}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["audio/mp3".to_string()],
                r#"{"type": "input_audio", "input_audio": {"data": {{json base64_data}}, "format": "mp3"}}"#,
                10,
            )
            // Catch-all for other binary types
            .binary_block(
                vec![],
                r#"{"type": "file", "mime_type": {{json mime_type}}, "data": {{json base64_data}}}"#,
            )
            // Catch-all for URLs
            .url_block(vec![], r#"{"type": "image_url", "image_url": {"url": {{json url}}}}"#)
    }

    /// Anthropic Claude message format builder.
    ///
    /// Matches `config/service-v2/anthropic.v2.yaml` message_format section:
    /// - Roles: user, assistant (system handled separately)
    /// - System handling: separate (goes to top-level "system" parameter)
    /// - Content array: true (always)
    /// - String shorthand: false
    /// - Supports: text, image, document (PDF), tool_use, tool_result
    pub fn anthropic() -> MessageFormatBuilder {
        MessageFormatBuilder::new()
            .name("anthropic")
            // Anthropic always uses content array format
            .text_message_template(r#"{"role": {{json role}}, "content": [{"type": "text", "text": {{json content}}}]}"#)
            .multimodal_message_template(r#"{"role": {{json role}}, "content": {{json content_blocks}}}"#)
            // Text block
            .text_block(r#"{"type": "text", "text": {{json text}}}"#)
            // Image blocks by MIME type
            .binary_block_with_priority(
                vec!["image/jpeg".to_string()],
                r#"{"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": {{json base64_data}}}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/png".to_string()],
                r#"{"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": {{json base64_data}}}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/gif".to_string()],
                r#"{"type": "image", "source": {"type": "base64", "media_type": "image/gif", "data": {{json base64_data}}}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/webp".to_string()],
                r#"{"type": "image", "source": {"type": "base64", "media_type": "image/webp", "data": {{json base64_data}}}}"#,
                10,
            )
            // Image URL block
            .url_block_with_priority(
                vec!["image/*".to_string()],
                r#"{"type": "image", "source": {"type": "url", "url": {{json url}}}}"#,
                10,
            )
            // Document blocks (PDFs)
            .binary_block_with_priority(
                vec!["application/pdf".to_string()],
                r#"{"type": "document", "source": {"type": "base64", "media_type": "application/pdf", "data": {{json base64_data}}}}"#,
                10,
            )
            // Catch-all for other binary types (images)
            .binary_block(
                vec!["image/*".to_string()],
                r#"{"type": "image", "source": {"type": "base64", "media_type": {{json mime_type}}, "data": {{json base64_data}}}}"#,
            )
    }

    /// Google Gemini message format builder.
    ///
    /// Matches `config/service-v2/google.v2.yaml` message_format section:
    /// - Roles: user, model (not assistant)
    /// - System handling: separate (goes to "systemInstruction")
    /// - Content array: true (uses "parts" array)
    /// - String shorthand: false
    /// - Supports: text, image, document, video, audio, tool_use (functionCall), tool_result (functionResponse)
    pub fn google() -> MessageFormatBuilder {
        MessageFormatBuilder::new()
            .name("google")
            // Google uses "parts" array structure
            .text_message_template(r#"{"role": {{json role}}, "parts": [{"text": {{json content}}}]}"#)
            .multimodal_message_template(r#"{"role": {{json role}}, "parts": {{json content_blocks}}}"#)
            // Text part
            .text_block(r#"{"text": {{json text}}}"#)
            // Image parts - Google uses "inlineData"
            .binary_block_with_priority(
                vec!["image/jpeg".to_string()],
                r#"{"inlineData": {"mimeType": "image/jpeg", "data": {{json base64_data}}}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/png".to_string()],
                r#"{"inlineData": {"mimeType": "image/png", "data": {{json base64_data}}}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/gif".to_string()],
                r#"{"inlineData": {"mimeType": "image/gif", "data": {{json base64_data}}}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["image/webp".to_string()],
                r#"{"inlineData": {"mimeType": "image/webp", "data": {{json base64_data}}}}"#,
                10,
            )
            // Document parts (PDFs)
            .binary_block_with_priority(
                vec!["application/pdf".to_string()],
                r#"{"inlineData": {"mimeType": "application/pdf", "data": {{json base64_data}}}}"#,
                10,
            )
            // Video parts
            .binary_block_with_priority(
                vec!["video/mp4".to_string()],
                r#"{"inlineData": {"mimeType": "video/mp4", "data": {{json base64_data}}}}"#,
                10,
            )
            // Audio parts
            .binary_block_with_priority(
                vec!["audio/wav".to_string()],
                r#"{"inlineData": {"mimeType": "audio/wav", "data": {{json base64_data}}}}"#,
                10,
            )
            .binary_block_with_priority(
                vec!["audio/mp3".to_string()],
                r#"{"inlineData": {"mimeType": "audio/mpeg", "data": {{json base64_data}}}}"#,
                10,
            )
            // Catch-all for other binary types
            .binary_block(
                vec![],
                r#"{"inlineData": {"mimeType": {{json mime_type}}, "data": {{json base64_data}}}}"#,
            )
    }
}

// ============================================================================
// SSE Parser Presets
// ============================================================================

/// Pre-configured SSE parser configurations for major LLM providers.
///
/// These configurations match the `streaming` sections of the v2 YAML configs exactly.
pub mod sse_parser {
    use super::*;

    /// SSE parser configuration builder for programmatic construction.
    #[derive(Debug, Clone)]
    pub struct SseParserBuilder {
        /// Line prefix for SSE data (e.g., "data: ")
        pub line_prefix: Option<&'static str>,
        /// Event type prefix for typed events (e.g., "event: ")
        pub event_type_prefix: Option<&'static str>,
        /// Primary token extraction path
        pub token_path: JsonPath,
        /// End conditions for stream termination
        pub end_conditions: Vec<EndCondition>,
        /// Raw payload that signals end of stream
        pub end_payload: Option<&'static str>,
        /// Usage extraction paths
        pub usage_paths: Option<UsagePaths>,
    }

    /// Paths for extracting usage/token information
    #[derive(Debug, Clone)]
    pub struct UsagePaths {
        pub input_tokens: JsonPath,
        pub output_tokens: JsonPath,
        pub total_tokens: Option<JsonPath>,
        pub cache_read_tokens: Option<JsonPath>,
        pub cache_write_tokens: Option<JsonPath>,
    }

    impl SseParserBuilder {
        /// Create a new empty SSE parser builder
        pub fn new() -> Self {
            Self {
                line_prefix: None,
                event_type_prefix: None,
                token_path: JsonPath(vec![]),
                end_conditions: vec![],
                end_payload: None,
                usage_paths: None,
            }
        }

        /// Set the line prefix (e.g., "data: ")
        pub fn line_prefix(mut self, prefix: &'static str) -> Self {
            self.line_prefix = Some(prefix);
            self
        }

        /// Set the event type prefix (e.g., "event: ")
        pub fn event_type_prefix(mut self, prefix: &'static str) -> Self {
            self.event_type_prefix = Some(prefix);
            self
        }

        /// Set the token extraction path
        pub fn token_path(mut self, path: JsonPath) -> Self {
            self.token_path = path;
            self
        }

        /// Add an end condition
        pub fn end_condition(mut self, condition: EndCondition) -> Self {
            self.end_conditions.push(condition);
            self
        }

        /// Set the raw end payload marker
        pub fn end_payload(mut self, payload: &'static str) -> Self {
            self.end_payload = Some(payload);
            self
        }

        /// Set usage extraction paths
        pub fn usage_paths(mut self, paths: UsagePaths) -> Self {
            self.usage_paths = Some(paths);
            self
        }

        /// Build into an ExtractorConfig for use with JsonPathExtractor
        pub fn build_extractor_config(&self) -> ExtractorConfig {
            // Use the first JSON-based end condition, or None
            let json_end_condition = self
                .end_conditions
                .iter()
                .find(|c| !matches!(c, EndCondition::PayloadEquals(_)))
                .cloned()
                .unwrap_or(EndCondition::None);

            ExtractorConfig {
                token_path: self.token_path.clone(),
                json_end_condition,
                end_payload: self.end_payload,
            }
        }

        /// Build into a JsonPathExtractor
        pub fn build_extractor(&self) -> JsonPathExtractor {
            JsonPathExtractor::new(self.build_extractor_config())
        }
    }

    impl Default for SseParserBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    /// OpenAI SSE parser configuration.
    ///
    /// Matches `config/service-v2/openai.v2.yaml` streaming section:
    /// - Format: text/event-stream
    /// - Line prefix: "data: "
    /// - Token path: choices[0].delta.content
    /// - End conditions: [DONE] or finish_reason not null
    pub fn openai() -> SseParserBuilder {
        use JsonPathSegment::*;

        SseParserBuilder::new()
            .line_prefix("data: ")
            // Token extraction: choices[0].delta.content
            .token_path(JsonPath(vec![
                Key("choices"),
                Index(0),
                Key("delta"),
                Key("content"),
            ]))
            // End when raw line is "[DONE]"
            .end_payload("[DONE]")
            // End when finish_reason is not null
            .end_condition(EndCondition::JsonPathExists(JsonPath(vec![
                Key("choices"),
                Index(0),
                Key("finish_reason"),
            ])))
            // Usage paths (with stream_options.include_usage=true)
            .usage_paths(UsagePaths {
                input_tokens: JsonPath(vec![Key("usage"), Key("prompt_tokens")]),
                output_tokens: JsonPath(vec![Key("usage"), Key("completion_tokens")]),
                total_tokens: Some(JsonPath(vec![Key("usage"), Key("total_tokens")])),
                cache_read_tokens: None,
                cache_write_tokens: None,
            })
    }

    /// Anthropic SSE parser configuration.
    ///
    /// Matches `config/service-v2/anthropic.v2.yaml` streaming section:
    /// - Format: text/event-stream
    /// - Event type prefix: "event: "
    /// - Token path: delta.text
    /// - End conditions: type == "message_stop" or error exists
    pub fn anthropic() -> SseParserBuilder {
        use JsonPathSegment::*;

        SseParserBuilder::new()
            .event_type_prefix("event: ")
            // Token extraction: delta.text
            .token_path(JsonPath(vec![Key("delta"), Key("text")]))
            // End when type == "message_stop"
            .end_condition(EndCondition::JsonPathEquals {
                path: JsonPath(vec![Key("type")]),
                value: "message_stop",
            })
            // End when error exists
            .end_condition(EndCondition::JsonPathExists(JsonPath(vec![Key("error")])))
            // Usage paths for Anthropic
            .usage_paths(UsagePaths {
                input_tokens: JsonPath(vec![Key("message"), Key("usage"), Key("input_tokens")]),
                output_tokens: JsonPath(vec![Key("usage"), Key("output_tokens")]),
                total_tokens: None,
                cache_read_tokens: Some(JsonPath(vec![
                    Key("message"),
                    Key("usage"),
                    Key("cache_read_input_tokens"),
                ])),
                cache_write_tokens: Some(JsonPath(vec![
                    Key("message"),
                    Key("usage"),
                    Key("cache_creation_input_tokens"),
                ])),
            })
    }

    /// Google Gemini SSE parser configuration.
    ///
    /// Matches `config/service-v2/google.v2.yaml` streaming section:
    /// - Format: text/event-stream
    /// - Line prefix: "data: "
    /// - Token path: candidates[0].content.parts[0].text
    /// - End conditions: finishReason not null or error exists
    pub fn google() -> SseParserBuilder {
        use JsonPathSegment::*;

        SseParserBuilder::new()
            .line_prefix("data: ")
            // Token extraction: candidates[0].content.parts[0].text
            .token_path(JsonPath(vec![
                Key("candidates"),
                Index(0),
                Key("content"),
                Key("parts"),
                Index(0),
                Key("text"),
            ]))
            // End when finishReason is not null
            .end_condition(EndCondition::JsonPathExists(JsonPath(vec![
                Key("candidates"),
                Index(0),
                Key("finishReason"),
            ])))
            // End when error exists
            .end_condition(EndCondition::JsonPathExists(JsonPath(vec![Key("error")])))
            // Usage paths for Google
            .usage_paths(UsagePaths {
                input_tokens: JsonPath(vec![Key("usageMetadata"), Key("promptTokenCount")]),
                output_tokens: JsonPath(vec![Key("usageMetadata"), Key("candidatesTokenCount")]),
                total_tokens: Some(JsonPath(vec![Key("usageMetadata"), Key("totalTokenCount")])),
                cache_read_tokens: None,
                cache_write_tokens: None,
            })
    }
}

// ============================================================================
// Response Extractor Presets
// ============================================================================

/// Pre-configured response extractor configurations for major LLM providers.
///
/// These configurations match the `response` sections of the v2 YAML configs exactly.
pub mod response_extractor {
    use super::*;

    /// Response extractor configuration builder.
    #[derive(Debug, Clone)]
    pub struct ResponseExtractorBuilder {
        /// HTTP success codes
        pub success_codes: Vec<u16>,
        /// Path to extract content
        pub content_path: String,
        /// Path to extract role (optional)
        pub role_path: Option<String>,
        /// Path to extract finish reason (optional)
        pub finish_reason_path: Option<String>,
        /// Path to extract message ID (optional)
        pub message_id_path: Option<String>,
        /// Path to extract model name (optional)
        pub model_path: Option<String>,
        /// Usage extraction paths
        pub usage_paths: Option<UsageExtractPaths>,
        /// Tool calls extraction path (optional)
        pub tool_calls_path: Option<String>,
        /// Error extraction paths
        pub error_paths: HashMap<String, String>,
        /// Finish reason value mappings
        pub finish_reason_map: HashMap<String, String>,
        /// Role value mappings
        pub role_map: HashMap<String, String>,
    }

    /// Paths for extracting usage information from responses
    #[derive(Debug, Clone)]
    pub struct UsageExtractPaths {
        pub input: String,
        pub output: String,
        pub total: Option<String>,
        pub cache_read: Option<String>,
        pub cache_creation: Option<String>,
    }

    impl ResponseExtractorBuilder {
        /// Create a new empty response extractor builder
        pub fn new() -> Self {
            Self {
                success_codes: vec![200],
                content_path: String::new(),
                role_path: None,
                finish_reason_path: None,
                message_id_path: None,
                model_path: None,
                usage_paths: None,
                tool_calls_path: None,
                error_paths: HashMap::new(),
                finish_reason_map: HashMap::new(),
                role_map: HashMap::new(),
            }
        }

        /// Set success codes
        pub fn success_codes(mut self, codes: Vec<u16>) -> Self {
            self.success_codes = codes;
            self
        }

        /// Set content extraction path
        pub fn content_path(mut self, path: impl Into<String>) -> Self {
            self.content_path = path.into();
            self
        }

        /// Set role extraction path
        pub fn role_path(mut self, path: impl Into<String>) -> Self {
            self.role_path = Some(path.into());
            self
        }

        /// Set finish reason extraction path
        pub fn finish_reason_path(mut self, path: impl Into<String>) -> Self {
            self.finish_reason_path = Some(path.into());
            self
        }

        /// Set message ID extraction path
        pub fn message_id_path(mut self, path: impl Into<String>) -> Self {
            self.message_id_path = Some(path.into());
            self
        }

        /// Set model extraction path
        pub fn model_path(mut self, path: impl Into<String>) -> Self {
            self.model_path = Some(path.into());
            self
        }

        /// Set usage extraction paths
        pub fn usage_paths(mut self, paths: UsageExtractPaths) -> Self {
            self.usage_paths = Some(paths);
            self
        }

        /// Set tool calls extraction path
        pub fn tool_calls_path(mut self, path: impl Into<String>) -> Self {
            self.tool_calls_path = Some(path.into());
            self
        }

        /// Add an error extraction path
        pub fn error_path(mut self, key: impl Into<String>, path: impl Into<String>) -> Self {
            self.error_paths.insert(key.into(), path.into());
            self
        }

        /// Add a finish reason mapping
        pub fn finish_reason_mapping(
            mut self,
            from: impl Into<String>,
            to: impl Into<String>,
        ) -> Self {
            self.finish_reason_map.insert(from.into(), to.into());
            self
        }

        /// Add a role mapping
        pub fn role_mapping(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
            self.role_map.insert(from.into(), to.into());
            self
        }

        /// Build into a ResponseConfig
        pub fn build(&self) -> ResponseConfig {
            let usage = self.usage_paths.as_ref().map(|u| UsageExtract {
                input: u.input.clone(),
                output: u.output.clone(),
                total: u.total.clone(),
            });

            ResponseConfig {
                success_codes: self.success_codes.clone(),
                extract: ResponseExtract {
                    content: self.content_path.clone(),
                    role: self.role_path.clone(),
                    finish_reason: self.finish_reason_path.clone(),
                    usage,
                },
                error: self.error_paths.clone(),
            }
        }
    }

    impl Default for ResponseExtractorBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    /// OpenAI response extractor configuration.
    ///
    /// Matches `config/service-v2/openai.v2.yaml` response section:
    /// - Content: choices[0].message.content
    /// - Role: choices[0].message.role
    /// - Finish reason: choices[0].finish_reason
    /// - Usage: usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
    pub fn openai() -> ResponseExtractorBuilder {
        ResponseExtractorBuilder::new()
            .success_codes(vec![200])
            .content_path("choices[0].message.content")
            .role_path("choices[0].message.role")
            .finish_reason_path("choices[0].finish_reason")
            .message_id_path("id")
            .model_path("model")
            .usage_paths(UsageExtractPaths {
                input: "usage.prompt_tokens".to_string(),
                output: "usage.completion_tokens".to_string(),
                total: Some("usage.total_tokens".to_string()),
                cache_read: None,
                cache_creation: None,
            })
            .tool_calls_path("choices[0].message.tool_calls")
            .error_path("message", "error.message")
            .error_path("type", "error.type")
            .error_path("code", "error.code")
            .error_path("param", "error.param")
            // Finish reason mappings (OpenAI uses standard names)
            .finish_reason_mapping("stop", "stop")
            .finish_reason_mapping("length", "length")
            .finish_reason_mapping("tool_calls", "tool_calls")
            .finish_reason_mapping("content_filter", "content_filter")
    }

    /// Anthropic response extractor configuration.
    ///
    /// Matches `config/service-v2/anthropic.v2.yaml` response section:
    /// - Content: content[0].text
    /// - Role: role
    /// - Finish reason: stop_reason
    /// - Usage: usage.input_tokens, usage.output_tokens
    pub fn anthropic() -> ResponseExtractorBuilder {
        ResponseExtractorBuilder::new()
            .success_codes(vec![200])
            .content_path("content[0].text")
            .role_path("role")
            .finish_reason_path("stop_reason")
            .message_id_path("id")
            .model_path("model")
            .usage_paths(UsageExtractPaths {
                input: "usage.input_tokens".to_string(),
                output: "usage.output_tokens".to_string(),
                total: None,
                cache_read: Some("usage.cache_read_input_tokens".to_string()),
                cache_creation: Some("usage.cache_creation_input_tokens".to_string()),
            })
            .tool_calls_path("content[?(@.type==\"tool_use\")]")
            .error_path("message", "error.message")
            .error_path("type", "error.type")
            .error_path("code", "error.code")
            // Finish reason mappings (Anthropic uses different names)
            .finish_reason_mapping("end_turn", "stop")
            .finish_reason_mapping("max_tokens", "length")
            .finish_reason_mapping("stop_sequence", "stop")
            .finish_reason_mapping("tool_use", "tool_calls")
    }

    /// Google Gemini response extractor configuration.
    ///
    /// Matches `config/service-v2/google.v2.yaml` response section:
    /// - Content: candidates[0].content.parts[0].text
    /// - Role: candidates[0].content.role (mapped from "model" to "assistant")
    /// - Finish reason: candidates[0].finishReason
    /// - Usage: usageMetadata.promptTokenCount, usageMetadata.candidatesTokenCount
    pub fn google() -> ResponseExtractorBuilder {
        ResponseExtractorBuilder::new()
            .success_codes(vec![200])
            .content_path("candidates[0].content.parts[0].text")
            .role_path("candidates[0].content.role")
            .finish_reason_path("candidates[0].finishReason")
            .usage_paths(UsageExtractPaths {
                input: "usageMetadata.promptTokenCount".to_string(),
                output: "usageMetadata.candidatesTokenCount".to_string(),
                total: Some("usageMetadata.totalTokenCount".to_string()),
                cache_read: None,
                cache_creation: None,
            })
            .tool_calls_path("candidates[0].content.parts[?(@.functionCall)]")
            .error_path("message", "error.message")
            .error_path("type", "error.code")
            .error_path("code", "error.status")
            // Finish reason mappings (Google uses uppercase names)
            .finish_reason_mapping("STOP", "stop")
            .finish_reason_mapping("MAX_TOKENS", "length")
            .finish_reason_mapping("SAFETY", "content_filter")
            .finish_reason_mapping("RECITATION", "content_filter")
            .finish_reason_mapping("OTHER", "other")
            // Role mapping (Google uses "model" instead of "assistant")
            .role_mapping("model", "assistant")
    }
}

// ============================================================================
// Convenience Re-exports
// ============================================================================

/// Create an OpenAI message format builder
pub use message_format::openai as openai_message_format;

/// Create an Anthropic message format builder
pub use message_format::anthropic as anthropic_message_format;

/// Create a Google message format builder
pub use message_format::google as google_message_format;

/// Create an OpenAI SSE parser builder
pub use sse_parser::openai as openai_sse_parser;

/// Create an Anthropic SSE parser builder
pub use sse_parser::anthropic as anthropic_sse_parser;

/// Create a Google SSE parser builder
pub use sse_parser::google as google_sse_parser;

/// Create an OpenAI response extractor builder
pub use response_extractor::openai as openai_response_extractor;

/// Create an Anthropic response extractor builder
pub use response_extractor::anthropic as anthropic_response_extractor;

/// Create a Google response extractor builder
pub use response_extractor::google as google_response_extractor;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_message_format_builds() {
        let builder = message_format::openai();
        let config = builder.build().expect("OpenAI message format should build");
        assert_eq!(config.name, "openai");
    }

    #[test]
    fn test_anthropic_message_format_builds() {
        let builder = message_format::anthropic();
        let config = builder.build().expect("Anthropic message format should build");
        assert_eq!(config.name, "anthropic");
    }

    #[test]
    fn test_google_message_format_builds() {
        let builder = message_format::google();
        let config = builder.build().expect("Google message format should build");
        assert_eq!(config.name, "google");
    }

    #[test]
    fn test_openai_sse_parser_builds() {
        let builder = sse_parser::openai();
        let extractor = builder.build_extractor();

        // Verify it can extract from OpenAI format
        let json: serde_json::Value = serde_json::json!({
            "choices": [{
                "delta": {
                    "content": "Hello"
                }
            }]
        });

        use crate::streaming::sse::TokenExtractor;
        match extractor.extract_token(&json) {
            crate::streaming::sse::TokenExtractionResult::Token(s) => {
                assert_eq!(s, "Hello");
            }
            _ => panic!("Expected token extraction"),
        }
    }

    #[test]
    fn test_anthropic_sse_parser_builds() {
        let builder = sse_parser::anthropic();
        let extractor = builder.build_extractor();

        // Verify it can extract from Anthropic format
        let json: serde_json::Value = serde_json::json!({
            "delta": {
                "text": "Hello"
            },
            "type": "content_block_delta"
        });

        use crate::streaming::sse::TokenExtractor;
        match extractor.extract_token(&json) {
            crate::streaming::sse::TokenExtractionResult::Token(s) => {
                assert_eq!(s, "Hello");
            }
            _ => panic!("Expected token extraction"),
        }
    }

    #[test]
    fn test_google_sse_parser_builds() {
        let builder = sse_parser::google();
        let extractor = builder.build_extractor();

        // Verify it can extract from Google format
        let json: serde_json::Value = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "Hello"
                    }]
                }
            }]
        });

        use crate::streaming::sse::TokenExtractor;
        match extractor.extract_token(&json) {
            crate::streaming::sse::TokenExtractionResult::Token(s) => {
                assert_eq!(s, "Hello");
            }
            _ => panic!("Expected token extraction"),
        }
    }

    #[test]
    fn test_openai_response_extractor_builds() {
        let builder = response_extractor::openai();
        let config = builder.build();

        assert_eq!(config.success_codes, vec![200]);
        assert_eq!(config.extract.content, "choices[0].message.content");
    }

    #[test]
    fn test_anthropic_response_extractor_builds() {
        let builder = response_extractor::anthropic();
        let config = builder.build();

        assert_eq!(config.success_codes, vec![200]);
        assert_eq!(config.extract.content, "content[0].text");
    }

    #[test]
    fn test_google_response_extractor_builds() {
        let builder = response_extractor::google();
        let config = builder.build();

        assert_eq!(config.success_codes, vec![200]);
        assert_eq!(config.extract.content, "candidates[0].content.parts[0].text");
    }

    #[test]
    fn test_openai_sse_end_detection() {
        let builder = sse_parser::openai();
        let extractor = builder.build_extractor();

        use crate::streaming::sse::TokenExtractor;

        // Should detect [DONE] as end
        assert!(extractor.is_end_payload("[DONE]"));
        assert!(!extractor.is_end_payload("data: {}"));

        // Should detect finish_reason in JSON
        let end_json: serde_json::Value = serde_json::json!({
            "choices": [{
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        match extractor.extract_token(&end_json) {
            crate::streaming::sse::TokenExtractionResult::EndStream => {}
            _ => panic!("Expected end stream detection"),
        }
    }

    #[test]
    fn test_anthropic_sse_end_detection() {
        let builder = sse_parser::anthropic();
        let extractor = builder.build_extractor();

        use crate::streaming::sse::TokenExtractor;

        // Should detect message_stop type
        let end_json: serde_json::Value = serde_json::json!({
            "type": "message_stop"
        });

        match extractor.extract_token(&end_json) {
            crate::streaming::sse::TokenExtractionResult::EndStream => {}
            _ => panic!("Expected end stream detection"),
        }
    }

    #[test]
    fn test_google_sse_end_detection() {
        let builder = sse_parser::google();
        let extractor = builder.build_extractor();

        use crate::streaming::sse::TokenExtractor;

        // Should detect finishReason
        let end_json: serde_json::Value = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": ""
                    }]
                },
                "finishReason": "STOP"
            }]
        });

        match extractor.extract_token(&end_json) {
            crate::streaming::sse::TokenExtractionResult::EndStream => {}
            _ => panic!("Expected end stream detection"),
        }
    }
}
