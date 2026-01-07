//! Config-driven message format system for transforming messages to provider-specific JSON.
//!
//! This module provides a fully config-driven approach to message formatting, eliminating
//! hardcoded provider-specific logic from the Rust code. Instead, YAML configurations define
//! how to transform content blocks (text, binary, url) to provider-specific JSON using
//! Handlebars templates and mime-type pattern matching.
//!
//! # Architecture
//!
//! The system consists of three main components:
//!
//! 1. **MessageFormatConfig** - Deserializable YAML configuration defining content transformers
//! 2. **MessageFormatter** - Applies config to transform messages to JSON
//! 3. **MessageFormatBuilder** - Fluent builder that can be recovered from config
//!
//! # Example YAML Configuration
//!
//! ```yaml
//! message_format:
//!   name: anthropic
//!   text_message:
//!     template: '{"role": "{{role}}", "content": "{{content}}"}'
//!   multimodal_message:
//!     template: '{"role": "{{role}}", "content": {{content_blocks}}}'
//!   content_blocks:
//!     - type: text
//!       template: '{"type": "text", "text": "{{text}}"}'
//!     - type: binary
//!       mime_patterns: ["image/*"]
//!       template: |
//!         {"type": "image", "source": {"type": "base64", "media_type": "{{mime_type}}", "data": "{{base64_data}}"}}
//!     - type: url
//!       mime_patterns: ["image/*"]
//!       template: '{"type": "image", "source": {"type": "url", "url": "{{url}}"}}'
//! ```

use base64::Engine;
use handlebars::{Context, Handlebars, Helper, HelperResult, Output, RenderContext};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::error::{ClientError, Result};
use crate::types::{ContentBlock, MessageContent};

// ============================================================================
// Configuration Types
// ============================================================================

/// Root configuration for message formatting
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageFormatConfig {
    /// Unique name for this format (e.g., "anthropic", "openai")
    pub name: String,

    /// Template for simple text-only messages
    pub text_message: MessageTemplate,

    /// Template for multimodal messages with content arrays
    pub multimodal_message: MessageTemplate,

    /// Content block transformers, matched by type and mime patterns
    pub content_blocks: Vec<ContentBlockConfig>,

    /// Optional preprocessing rules applied before content transformation
    #[serde(default)]
    pub preprocessing: Vec<PreprocessingRule>,

    /// Optional postprocessing rules applied after content transformation
    #[serde(default)]
    pub postprocessing: Vec<PostprocessingRule>,
}

/// Template configuration for message-level formatting
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageTemplate {
    /// Handlebars template string
    pub template: String,

    /// Optional variables to inject into the template context
    #[serde(default)]
    pub variables: HashMap<String, Value>,
}

/// Configuration for transforming a specific type of content block
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContentBlockConfig {
    /// Content block type: "text", "binary", or "url"
    #[serde(rename = "type")]
    pub block_type: ContentBlockType,

    /// Glob-style mime type patterns to match (e.g., "image/*", "audio/*", "application/pdf")
    /// If empty, matches all mime types for this block type
    #[serde(default)]
    pub mime_patterns: Vec<String>,

    /// Handlebars template for transforming this content block to JSON
    pub template: String,

    /// Optional condition expression for when to use this transformer
    #[serde(default)]
    pub condition: Option<String>,

    /// Priority for matching (higher = checked first)
    #[serde(default)]
    pub priority: i32,
}

/// Content block type discriminator
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentBlockType {
    Text,
    Binary,
    Url,
}

/// Preprocessing rules applied before content transformation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreprocessingRule {
    /// Name of this rule for debugging
    pub name: String,

    /// Condition when to apply this rule (optional)
    #[serde(default)]
    pub condition: Option<String>,

    /// Action to perform
    pub action: PreprocessingAction,
}

/// Preprocessing actions
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreprocessingAction {
    /// Set a variable in the template context
    SetVariable { name: String, value: String },
    /// Transform a mime type (e.g., "image/jpg" -> "image/jpeg")
    NormalizeMime { from: String, to: String },
    /// Skip a content block type entirely
    SkipBlockType { block_type: ContentBlockType },
}

/// Postprocessing rules applied after content transformation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostprocessingRule {
    /// Name of this rule for debugging
    pub name: String,

    /// Action to perform
    pub action: PostprocessingAction,
}

/// Postprocessing actions
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PostprocessingAction {
    /// Remove empty arrays from the result
    RemoveEmptyArrays,
    /// Remove null values from the result
    RemoveNulls,
    /// Apply a JSON path transformation
    JsonTransform { path: String, transform: String },
}

// ============================================================================
// Message Formatter
// ============================================================================

/// Applies a MessageFormatConfig to transform messages to provider-specific JSON.
///
/// The formatter is stateless and thread-safe, caching compiled templates and regex patterns.
pub struct MessageFormatter {
    config: MessageFormatConfig,
    handlebars: Handlebars<'static>,
    mime_patterns: Vec<(usize, CompiledMimePattern)>,
}

/// Pre-compiled mime pattern for efficient matching
struct CompiledMimePattern {
    pattern: String,
    regex: Regex,
}

impl MessageFormatter {
    /// Create a new formatter from configuration
    pub fn new(config: MessageFormatConfig) -> Result<Self> {
        let mut handlebars = Handlebars::new();

        // Register custom helpers
        handlebars.register_helper("json", Box::new(json_helper));
        handlebars.register_helper("json_escape", Box::new(json_escape_helper));
        handlebars.register_helper("base64", Box::new(base64_helper));
        handlebars.register_helper("mime_type_main", Box::new(mime_type_helper));
        handlebars.register_helper("mime_subtype", Box::new(mime_subtype_helper));
        handlebars.register_helper("make_data_uri", Box::new(data_uri_helper));

        // Disable HTML escaping since we're generating JSON
        handlebars.register_escape_fn(handlebars::no_escape);

        // Pre-compile mime patterns with their content block config indices
        let mut mime_patterns = Vec::new();
        for (idx, block_config) in config.content_blocks.iter().enumerate() {
            for pattern in &block_config.mime_patterns {
                let regex = compile_mime_pattern(pattern)?;
                mime_patterns.push((
                    idx,
                    CompiledMimePattern {
                        pattern: pattern.clone(),
                        regex,
                    },
                ));
            }
        }

        Ok(Self {
            config,
            handlebars,
            mime_patterns,
        })
    }

    /// Get the name of this message format
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Transform a slice of messages to provider-specific JSON array
    pub fn format_messages(&self, messages: &[MessageContent]) -> Result<Value> {
        let mut result = Vec::new();

        for message in messages {
            let formatted = self.format_single_message(message)?;
            result.push(formatted);
        }

        Ok(Value::Array(result))
    }

    /// Transform a single message to provider-specific JSON
    pub fn format_single_message(&self, message: &MessageContent) -> Result<Value> {
        match message {
            MessageContent::Text { role, content } => self.format_text_message(role, content),
            MessageContent::Multimodal { role, content } => {
                self.format_multimodal_message(role, content)
            }
        }
    }

    /// Format a simple text message
    fn format_text_message(&self, role: &str, content: &str) -> Result<Value> {
        let mut context = HashMap::new();
        context.insert("role".to_string(), Value::String(role.to_string()));
        context.insert("content".to_string(), Value::String(content.to_string()));

        // Add any configured variables
        for (key, value) in &self.config.text_message.variables {
            context.insert(key.clone(), value.clone());
        }

        let rendered = self
            .handlebars
            .render_template(&self.config.text_message.template, &context)?;

        serde_json::from_str(&rendered).map_err(|e| {
            ClientError::JsonSerialization(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Failed to parse rendered template as JSON: {}. Rendered: {}",
                    e, rendered
                ),
            )))
        })
    }

    /// Format a multimodal message with content blocks
    fn format_multimodal_message(&self, role: &str, content: &[ContentBlock]) -> Result<Value> {
        // First, format all content blocks
        let content_blocks = self.format_content_blocks(content)?;

        // Then render the multimodal message template
        let mut context = HashMap::new();
        context.insert("role".to_string(), Value::String(role.to_string()));
        context.insert("content_blocks".to_string(), content_blocks.clone());

        // Add any configured variables
        for (key, value) in &self.config.multimodal_message.variables {
            context.insert(key.clone(), value.clone());
        }

        let rendered = self
            .handlebars
            .render_template(&self.config.multimodal_message.template, &context)?;

        serde_json::from_str(&rendered).map_err(|e| {
            ClientError::JsonSerialization(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Failed to parse rendered multimodal template as JSON: {}. Rendered: {}",
                    e, rendered
                ),
            )))
        })
    }

    /// Format an array of content blocks to JSON array
    fn format_content_blocks(&self, blocks: &[ContentBlock]) -> Result<Value> {
        let mut result = Vec::new();

        for block in blocks {
            if let Some(formatted) = self.format_content_block(block)? {
                result.push(formatted);
            }
        }

        Ok(Value::Array(result))
    }

    /// Format a single content block, returning None if it should be skipped
    fn format_content_block(&self, block: &ContentBlock) -> Result<Option<Value>> {
        // Find the appropriate transformer for this block
        let (block_type, mime_type) = match block {
            ContentBlock::Text(_) => (ContentBlockType::Text, None),
            ContentBlock::Binary { mime_type, .. } => {
                (ContentBlockType::Binary, Some(mime_type.as_str()))
            }
            ContentBlock::Url { mime_type, .. } => {
                (ContentBlockType::Url, mime_type.as_ref().map(|s| s.as_str()))
            }
        };

        // Find matching transformer (respecting priority)
        let transformer = self.find_transformer(&block_type, mime_type)?;

        // Build context for the template
        let context = self.build_block_context(block)?;

        // Render the template
        let rendered = self.handlebars.render_template(&transformer.template, &context)?;

        let value: Value = serde_json::from_str(&rendered).map_err(|e| {
            ClientError::JsonSerialization(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Failed to parse rendered block template as JSON: {}. Rendered: {}",
                    e, rendered
                ),
            )))
        })?;

        Ok(Some(value))
    }

    /// Find the appropriate transformer for a content block
    fn find_transformer(
        &self,
        block_type: &ContentBlockType,
        mime_type: Option<&str>,
    ) -> Result<&ContentBlockConfig> {
        // Collect matching transformers with their priorities
        let mut candidates: Vec<(i32, &ContentBlockConfig)> = Vec::new();

        for config in &self.config.content_blocks {
            if &config.block_type != block_type {
                continue;
            }

            // If no mime patterns specified, it's a catch-all for this type
            if config.mime_patterns.is_empty() {
                candidates.push((config.priority, config));
                continue;
            }

            // Check mime patterns if we have a mime type
            if let Some(mime) = mime_type {
                for pattern in &config.mime_patterns {
                    if self.matches_mime_pattern(pattern, mime)? {
                        candidates.push((config.priority, config));
                        break;
                    }
                }
            }
        }

        // Sort by priority (descending) and take the first
        candidates.sort_by(|a, b| b.0.cmp(&a.0));

        candidates
            .first()
            .map(|(_, config)| *config)
            .ok_or_else(|| {
                ClientError::Render(handlebars::RenderError::new(format!(
                    "No transformer found for block type {:?} with mime type {:?}",
                    block_type, mime_type
                )))
            })
    }

    /// Check if a mime type matches a pattern (e.g., "image/*" matches "image/jpeg")
    fn matches_mime_pattern(&self, pattern: &str, mime_type: &str) -> Result<bool> {
        // First check pre-compiled patterns
        for (_, compiled) in &self.mime_patterns {
            if compiled.pattern == pattern {
                return Ok(compiled.regex.is_match(mime_type));
            }
        }

        // Fall back to compiling on the fly (shouldn't happen normally)
        let regex = compile_mime_pattern(pattern)?;
        Ok(regex.is_match(mime_type))
    }

    /// Build the template context for a content block
    fn build_block_context(&self, block: &ContentBlock) -> Result<HashMap<String, Value>> {
        let mut context = HashMap::new();

        match block {
            ContentBlock::Text(text) => {
                context.insert("text".to_string(), Value::String(text.clone()));
            }
            ContentBlock::Binary {
                data,
                mime_type,
                filename,
            } => {
                let base64_data = base64::prelude::BASE64_STANDARD.encode(data);
                context.insert("base64_data".to_string(), Value::String(base64_data.clone()));
                context.insert("mime_type".to_string(), Value::String(mime_type.clone()));
                context.insert(
                    "data_uri".to_string(),
                    Value::String(format!("data:{};base64,{}", mime_type, base64_data)),
                );

                if let Some(name) = filename {
                    context.insert("filename".to_string(), Value::String(name.clone()));
                } else {
                    context.insert("filename".to_string(), Value::Null);
                }

                // Parse mime type components
                if let Some((type_part, subtype)) = mime_type.split_once('/') {
                    context.insert("mime_type_main".to_string(), Value::String(type_part.to_string()));
                    context.insert("mime_subtype".to_string(), Value::String(subtype.to_string()));
                }
            }
            ContentBlock::Url { url, mime_type } => {
                context.insert("url".to_string(), Value::String(url.clone()));

                if let Some(mime) = mime_type {
                    context.insert("mime_type".to_string(), Value::String(mime.clone()));

                    if let Some((type_part, subtype)) = mime.split_once('/') {
                        context.insert("mime_type_main".to_string(), Value::String(type_part.to_string()));
                        context.insert("mime_subtype".to_string(), Value::String(subtype.to_string()));
                    }
                } else {
                    context.insert("mime_type".to_string(), Value::Null);
                }
            }
        }

        Ok(context)
    }
}

/// Compile a glob-style mime pattern to a regex
fn compile_mime_pattern(pattern: &str) -> Result<Regex> {
    static PATTERN_CACHE: OnceLock<std::sync::Mutex<HashMap<String, Regex>>> = OnceLock::new();

    let cache = PATTERN_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = cache.lock().unwrap();

    if let Some(regex) = guard.get(pattern) {
        return Ok(regex.clone());
    }

    // Convert glob pattern to regex
    let regex_pattern = pattern
        .replace('.', r"\.")
        .replace('*', r"[^/]*")
        .replace('?', r"[^/]");

    let regex = Regex::new(&format!("^{}$", regex_pattern)).map_err(|e| {
        ClientError::Render(handlebars::RenderError::new(format!(
            "Invalid mime pattern '{}': {}",
            pattern, e
        )))
    })?;

    guard.insert(pattern.to_string(), regex.clone());
    Ok(regex)
}

// ============================================================================
// Handlebars Helpers
// ============================================================================

/// JSON serialization helper
fn json_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(param) = h.param(0) {
        let value = param.value();
        let json_str = serde_json::to_string(value)?;
        out.write(&json_str)?;
    }
    Ok(())
}

/// JSON string escaping helper (escapes a string for embedding in JSON)
fn json_escape_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(param) = h.param(0) {
        let value = param.value();
        if let Some(s) = value.as_str() {
            // Use serde_json to properly escape the string, then strip the surrounding quotes
            let escaped = serde_json::to_string(s)?;
            // Remove the surrounding quotes that to_string adds
            let inner = &escaped[1..escaped.len() - 1];
            out.write(inner)?;
        }
    }
    Ok(())
}

/// Base64 encoding helper
fn base64_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(param) = h.param(0) {
        let value = param.value();
        if let Some(s) = value.as_str() {
            let encoded = base64::prelude::BASE64_STANDARD.encode(s.as_bytes());
            out.write(&encoded)?;
        } else if let Some(arr) = value.as_array() {
            // Handle byte array as JSON array of numbers
            let bytes: Vec<u8> = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            let encoded = base64::prelude::BASE64_STANDARD.encode(&bytes);
            out.write(&encoded)?;
        }
    }
    Ok(())
}

/// Extract main type from mime type (e.g., "image" from "image/jpeg")
fn mime_type_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(param) = h.param(0) {
        let value = param.value();
        if let Some(mime) = value.as_str() {
            if let Some((main_type, _)) = mime.split_once('/') {
                out.write(main_type)?;
            }
        }
    }
    Ok(())
}

/// Extract subtype from mime type (e.g., "jpeg" from "image/jpeg")
fn mime_subtype_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    if let Some(param) = h.param(0) {
        let value = param.value();
        if let Some(mime) = value.as_str() {
            if let Some((_, subtype)) = mime.split_once('/') {
                out.write(subtype)?;
            }
        }
    }
    Ok(())
}

/// Create a data URI from mime type and base64 data
fn data_uri_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let mime = h.param(0).and_then(|p| p.value().as_str()).unwrap_or("");
    let data = h.param(1).and_then(|p| p.value().as_str()).unwrap_or("");
    out.write(&format!("data:{};base64,{}", mime, data))?;
    Ok(())
}

// ============================================================================
// Message Format Builder
// ============================================================================

/// Fluent builder for constructing MessageFormatConfig programmatically.
///
/// This builder can be used to create configs in code or to reconstruct
/// a config from YAML.
#[derive(Debug, Clone, Default)]
pub struct MessageFormatBuilder {
    name: Option<String>,
    text_message_template: Option<String>,
    text_message_variables: HashMap<String, Value>,
    multimodal_message_template: Option<String>,
    multimodal_message_variables: HashMap<String, Value>,
    content_blocks: Vec<ContentBlockConfig>,
    preprocessing: Vec<PreprocessingRule>,
    postprocessing: Vec<PostprocessingRule>,
}

impl MessageFormatBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the format name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the text message template
    pub fn text_message_template(mut self, template: impl Into<String>) -> Self {
        self.text_message_template = Some(template.into());
        self
    }

    /// Add a variable for text messages
    pub fn text_message_variable(mut self, key: impl Into<String>, value: Value) -> Self {
        self.text_message_variables.insert(key.into(), value);
        self
    }

    /// Set the multimodal message template
    pub fn multimodal_message_template(mut self, template: impl Into<String>) -> Self {
        self.multimodal_message_template = Some(template.into());
        self
    }

    /// Add a variable for multimodal messages
    pub fn multimodal_message_variable(mut self, key: impl Into<String>, value: Value) -> Self {
        self.multimodal_message_variables.insert(key.into(), value);
        self
    }

    /// Add a text content block transformer
    pub fn text_block(mut self, template: impl Into<String>) -> Self {
        self.content_blocks.push(ContentBlockConfig {
            block_type: ContentBlockType::Text,
            mime_patterns: vec![],
            template: template.into(),
            condition: None,
            priority: 0,
        });
        self
    }

    /// Add a binary content block transformer
    pub fn binary_block(
        mut self,
        mime_patterns: Vec<String>,
        template: impl Into<String>,
    ) -> Self {
        self.content_blocks.push(ContentBlockConfig {
            block_type: ContentBlockType::Binary,
            mime_patterns,
            template: template.into(),
            condition: None,
            priority: 0,
        });
        self
    }

    /// Add a binary content block transformer with priority
    pub fn binary_block_with_priority(
        mut self,
        mime_patterns: Vec<String>,
        template: impl Into<String>,
        priority: i32,
    ) -> Self {
        self.content_blocks.push(ContentBlockConfig {
            block_type: ContentBlockType::Binary,
            mime_patterns,
            template: template.into(),
            condition: None,
            priority,
        });
        self
    }

    /// Add a URL content block transformer
    pub fn url_block(mut self, mime_patterns: Vec<String>, template: impl Into<String>) -> Self {
        self.content_blocks.push(ContentBlockConfig {
            block_type: ContentBlockType::Url,
            mime_patterns,
            template: template.into(),
            condition: None,
            priority: 0,
        });
        self
    }

    /// Add a URL content block transformer with priority
    pub fn url_block_with_priority(
        mut self,
        mime_patterns: Vec<String>,
        template: impl Into<String>,
        priority: i32,
    ) -> Self {
        self.content_blocks.push(ContentBlockConfig {
            block_type: ContentBlockType::Url,
            mime_patterns,
            template: template.into(),
            condition: None,
            priority,
        });
        self
    }

    /// Add a custom content block configuration
    pub fn content_block(mut self, config: ContentBlockConfig) -> Self {
        self.content_blocks.push(config);
        self
    }

    /// Add a preprocessing rule
    pub fn preprocessing_rule(mut self, rule: PreprocessingRule) -> Self {
        self.preprocessing.push(rule);
        self
    }

    /// Add a postprocessing rule
    pub fn postprocessing_rule(mut self, rule: PostprocessingRule) -> Self {
        self.postprocessing.push(rule);
        self
    }

    /// Build the MessageFormatConfig
    pub fn build(self) -> Result<MessageFormatConfig> {
        let name = self
            .name
            .ok_or_else(|| ClientError::ValidationError("name is required".to_string()))?;

        let text_message_template = self.text_message_template.ok_or_else(|| {
            ClientError::ValidationError("text_message_template is required".to_string())
        })?;

        let multimodal_message_template = self.multimodal_message_template.ok_or_else(|| {
            ClientError::ValidationError("multimodal_message_template is required".to_string())
        })?;

        Ok(MessageFormatConfig {
            name,
            text_message: MessageTemplate {
                template: text_message_template,
                variables: self.text_message_variables,
            },
            multimodal_message: MessageTemplate {
                template: multimodal_message_template,
                variables: self.multimodal_message_variables,
            },
            content_blocks: self.content_blocks,
            preprocessing: self.preprocessing,
            postprocessing: self.postprocessing,
        })
    }

    /// Build directly into a MessageFormatter
    pub fn build_formatter(self) -> Result<MessageFormatter> {
        let config = self.build()?;
        MessageFormatter::new(config)
    }
}

// ============================================================================
// Built-in Format Configurations (for backwards compatibility during migration)
// ============================================================================

/// Create an Anthropic-compatible message format configuration
pub fn anthropic_format() -> Result<MessageFormatConfig> {
    MessageFormatBuilder::new()
        .name("anthropic")
        .text_message_template(r#"{"role": "{{role}}", "content": "{{json_escape content}}"}"#)
        .multimodal_message_template(r#"{"role": "{{role}}", "content": {{json content_blocks}}}"#)
        .text_block(r#"{"type": "text", "text": "{{json_escape text}}"}"#)
        .binary_block(
            vec!["image/*".to_string()],
            r#"{"type": "image", "source": {"type": "base64", "media_type": "{{mime_type}}", "data": "{{base64_data}}"}}"#,
        )
        .url_block(
            vec!["image/*".to_string()],
            r#"{"type": "image", "source": {"type": "url", "url": "{{url}}"}}"#,
        )
        .build()
}

/// Create an OpenAI-compatible message format configuration
pub fn openai_format() -> Result<MessageFormatConfig> {
    MessageFormatBuilder::new()
        .name("openai")
        .text_message_template(r#"{"role": "{{role}}", "content": "{{json_escape content}}"}"#)
        .multimodal_message_template(r#"{"role": "{{role}}", "content": {{json content_blocks}}}"#)
        .text_block(r#"{"type": "text", "text": "{{json_escape text}}"}"#)
        // Images use data URI format for OpenAI
        .binary_block_with_priority(
            vec!["image/*".to_string()],
            r#"{"type": "image_url", "image_url": {"url": "{{data_uri}}"}}"#,
            10,
        )
        // Audio uses a different structure
        .binary_block_with_priority(
            vec!["audio/*".to_string()],
            r#"{"type": "audio", "source_type": "base64", "mime_type": "{{mime_type}}", "data": "{{base64_data}}"}"#,
            10,
        )
        // Catch-all for other binary types (skipped by default)
        .binary_block(
            vec![],
            r#"{"type": "file", "mime_type": "{{mime_type}}", "data": "{{base64_data}}"}"#,
        )
        // URL images
        .url_block(
            vec!["image/*".to_string()],
            r#"{"type": "image_url", "image_url": {"url": "{{url}}"}}"#,
        )
        // Generic URL (catch-all)
        .url_block(vec![], r#"{"type": "image_url", "image_url": {"url": "{{url}}"}}"#)
        .build()
}

// ============================================================================
// Config-driven MessageBuilder replacement
// ============================================================================

/// Registry of message formatters, keyed by name
pub struct MessageFormatRegistry {
    formatters: HashMap<String, MessageFormatter>,
}

impl MessageFormatRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            formatters: HashMap::new(),
        }
    }

    /// Register a formatter
    pub fn register(&mut self, formatter: MessageFormatter) {
        let name = formatter.name().to_string();
        self.formatters.insert(name, formatter);
    }

    /// Register a formatter from config
    pub fn register_config(&mut self, config: MessageFormatConfig) -> Result<()> {
        let formatter = MessageFormatter::new(config)?;
        self.register(formatter);
        Ok(())
    }

    /// Get a formatter by name
    pub fn get(&self, name: &str) -> Option<&MessageFormatter> {
        self.formatters.get(name)
    }

    /// Format messages using a named formatter
    pub fn format_messages(&self, format_name: &str, messages: &[MessageContent]) -> Result<Value> {
        let formatter = self.get(format_name).ok_or_else(|| {
            ClientError::Render(handlebars::RenderError::new(format!(
                "Unknown message format: {}",
                format_name
            )))
        })?;

        formatter.format_messages(messages)
    }

    /// Get a formatter by name, auto-upgrading from v1 if needed
    ///
    /// This method will check if the formatter is already registered. If not,
    /// it will attempt to auto-upgrade from a v1 format string using the
    /// built-in presets.
    ///
    /// # Arguments
    /// * `format_name` - The format name to get/upgrade
    ///
    /// # Returns
    /// * `Ok(&MessageFormatter)` - Reference to the formatter (either cached or newly upgraded)
    /// * `Err` - If the format is unknown or upgrade fails
    pub fn get_or_upgrade(&mut self, format_name: &str) -> Result<&MessageFormatter> {
        // Check if already registered
        if self.formatters.contains_key(format_name) {
            return Ok(self.formatters.get(format_name).unwrap());
        }

        // Try to auto-upgrade from v1
        tracing::info!("Auto-upgrading v1 format '{}' to v2", format_name);
        let config = upgrade_v1_to_v2(format_name)?;
        let formatter = MessageFormatter::new(config)?;
        self.register(formatter);

        Ok(self.formatters.get(format_name).unwrap())
    }

    /// Create a registry with built-in formats
    pub fn with_builtins() -> Result<Self> {
        let mut registry = Self::new();

        // Register built-in formats
        registry.register(MessageFormatter::new(anthropic_format()?)?);
        registry.register(MessageFormatter::new(openai_format()?)?);

        Ok(registry)
    }
}

impl Default for MessageFormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// V1-to-V2 Auto-Upgrade System
// ============================================================================

/// Auto-upgrade a v1 message_builder string to v2 MessageFormatConfig
///
/// This function enables backwards compatibility by automatically converting
/// v1 string-based message_builder values to v2 MessageFormatConfig objects.
///
/// # Arguments
/// * `format_name` - The v1 format name (e.g., "anthropic", "openai", "google")
///
/// # Returns
/// * `Ok(MessageFormatConfig)` - The upgraded v2 config
/// * `Err` - If the format is unknown or not supported
///
/// # Examples
/// ```
/// use cllient::message_format::upgrade_v1_to_v2;
///
/// let config = upgrade_v1_to_v2("anthropic").unwrap();
/// assert_eq!(config.name, "anthropic");
/// ```
pub fn upgrade_v1_to_v2(format_name: &str) -> Result<MessageFormatConfig> {
    match format_name.to_lowercase().as_str() {
        "anthropic" => anthropic_format(),
        "openai" => openai_format(),
        "google" => {
            // Note: google_format() may not be implemented yet
            // If not, return an error suggesting manual v2 config
            Err(ClientError::Render(handlebars::RenderError::new(
                "Google format not yet implemented in v2. Please create a message_format config manually.".to_string()
            )))
        }
        _ => Err(ClientError::Render(handlebars::RenderError::new(
            format!(
                "Unknown v1 message format '{}'. Please migrate to v2 message_format config. \
                 See docs/message_format_migration.md for details.",
                format_name
            )
        )))
    }
}

/// Check if a v1 format name has a v2 upgrade available
///
/// # Arguments
/// * `format_name` - The v1 format name to check
///
/// # Returns
/// * `true` if an auto-upgrade is available
/// * `false` if the format is unknown or requires manual v2 config
pub fn has_v2_upgrade(format_name: &str) -> bool {
    matches!(
        format_name.to_lowercase().as_str(),
        "anthropic" | "openai" | "google"
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_message_formatting() {
        let config = anthropic_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let messages = vec![MessageContent::Text {
            role: "user".to_string(),
            content: "Hello, world!".to_string(),
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["role"], "user");
        assert_eq!(arr[0]["content"], "Hello, world!");
    }

    #[test]
    fn test_text_message_with_special_characters() {
        let config = anthropic_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let messages = vec![MessageContent::Text {
            role: "user".to_string(),
            content: "Hello \"world\" with\nnewline".to_string(),
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["content"], "Hello \"world\" with\nnewline");
    }

    #[test]
    fn test_multimodal_message_with_text() {
        let config = anthropic_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let messages = vec![MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![ContentBlock::Text("What is in this image?".to_string())],
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["role"], "user");

        let content = arr[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "What is in this image?");
    }

    #[test]
    fn test_multimodal_message_with_image() {
        let config = anthropic_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let image_data = vec![0xFF, 0xD8, 0xFF]; // Fake JPEG data

        let messages = vec![MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![
                ContentBlock::Text("What is in this image?".to_string()),
                ContentBlock::Binary {
                    data: image_data.clone(),
                    mime_type: "image/jpeg".to_string(),
                    filename: None,
                },
            ],
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr.len(), 1);

        let content = arr[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);

        // Text block
        assert_eq!(content[0]["type"], "text");

        // Image block (Anthropic format)
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["type"], "base64");
        assert_eq!(content[1]["source"]["media_type"], "image/jpeg");

        // Verify base64 encoding
        let expected_base64 = base64::prelude::BASE64_STANDARD.encode(&image_data);
        assert_eq!(content[1]["source"]["data"], expected_base64);
    }

    #[test]
    fn test_openai_image_format() {
        let config = openai_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let image_data = vec![0xFF, 0xD8, 0xFF];

        let messages = vec![MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![ContentBlock::Binary {
                data: image_data.clone(),
                mime_type: "image/jpeg".to_string(),
                filename: None,
            }],
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();
        let content = arr[0]["content"].as_array().unwrap();

        // OpenAI format uses image_url with data URI
        assert_eq!(content[0]["type"], "image_url");

        let expected_base64 = base64::prelude::BASE64_STANDARD.encode(&image_data);
        let expected_data_uri = format!("data:image/jpeg;base64,{}", expected_base64);
        assert_eq!(content[0]["image_url"]["url"], expected_data_uri);
    }

    #[test]
    fn test_openai_audio_format() {
        let config = openai_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let audio_data = vec![0x52, 0x49, 0x46, 0x46]; // RIFF header

        let messages = vec![MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![ContentBlock::Binary {
                data: audio_data.clone(),
                mime_type: "audio/wav".to_string(),
                filename: Some("recording.wav".to_string()),
            }],
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();
        let content = arr[0]["content"].as_array().unwrap();

        // OpenAI audio format
        assert_eq!(content[0]["type"], "audio");
        assert_eq!(content[0]["mime_type"], "audio/wav");
    }

    #[test]
    fn test_url_content_block() {
        let config = anthropic_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let messages = vec![MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![ContentBlock::Url {
                url: "https://example.com/image.jpg".to_string(),
                mime_type: Some("image/jpeg".to_string()),
            }],
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();
        let content = arr[0]["content"].as_array().unwrap();

        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["source"]["type"], "url");
        assert_eq!(content[0]["source"]["url"], "https://example.com/image.jpg");
    }

    #[test]
    fn test_mime_pattern_matching() {
        // Test glob-style patterns
        let image_regex = compile_mime_pattern("image/*").unwrap();
        assert!(image_regex.is_match("image/jpeg"));
        assert!(image_regex.is_match("image/png"));
        assert!(image_regex.is_match("image/gif"));
        assert!(!image_regex.is_match("audio/mp3"));

        let audio_regex = compile_mime_pattern("audio/*").unwrap();
        assert!(audio_regex.is_match("audio/wav"));
        assert!(audio_regex.is_match("audio/mp3"));
        assert!(!audio_regex.is_match("image/jpeg"));

        // Test exact match
        let exact_regex = compile_mime_pattern("application/pdf").unwrap();
        assert!(exact_regex.is_match("application/pdf"));
        assert!(!exact_regex.is_match("application/json"));
    }

    #[test]
    fn test_builder_api() {
        let config = MessageFormatBuilder::new()
            .name("custom")
            .text_message_template(r#"{"r": "{{role}}", "c": "{{json_escape content}}"}"#)
            .multimodal_message_template(r#"{"r": "{{role}}", "c": {{json content_blocks}}}"#)
            .text_block(r#"{"t": "text", "v": "{{json_escape text}}"}"#)
            .build()
            .unwrap();

        assert_eq!(config.name, "custom");

        let formatter = MessageFormatter::new(config).unwrap();

        let messages = vec![MessageContent::Text {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();

        assert_eq!(arr[0]["r"], "user");
        assert_eq!(arr[0]["c"], "Hello");
    }

    #[test]
    fn test_format_registry() {
        let registry = MessageFormatRegistry::with_builtins().unwrap();

        let messages = vec![MessageContent::Text {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];

        // Test Anthropic format
        let anthropic_result = registry.format_messages("anthropic", &messages).unwrap();
        assert!(anthropic_result.as_array().is_some());

        // Test OpenAI format
        let openai_result = registry.format_messages("openai", &messages).unwrap();
        assert!(openai_result.as_array().is_some());

        // Test unknown format
        let unknown_result = registry.format_messages("unknown", &messages);
        assert!(unknown_result.is_err());
    }

    #[test]
    fn test_priority_matching() {
        // Build a config with multiple matchers for the same type
        let config = MessageFormatBuilder::new()
            .name("priority_test")
            .text_message_template(r#"{"role": "{{role}}", "content": "{{json_escape content}}"}"#)
            .multimodal_message_template(r#"{"role": "{{role}}", "content": {{json content_blocks}}}"#)
            .text_block(r#"{"type": "text", "text": "{{json_escape text}}"}"#)
            // Low priority catch-all
            .binary_block(vec![], r#"{"type": "generic", "data": "{{base64_data}}"}"#)
            // High priority image handler
            .binary_block_with_priority(
                vec!["image/*".to_string()],
                r#"{"type": "image", "data": "{{base64_data}}"}"#,
                100,
            )
            .build()
            .unwrap();

        let formatter = MessageFormatter::new(config).unwrap();

        let messages = vec![MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![ContentBlock::Binary {
                data: vec![1, 2, 3],
                mime_type: "image/png".to_string(),
                filename: None,
            }],
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let content = result.as_array().unwrap()[0]["content"].as_array().unwrap();

        // Should use the high-priority image handler
        assert_eq!(content[0]["type"], "image");
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = anthropic_format().unwrap();

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Deserialize back
        let deserialized: MessageFormatConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config.name, deserialized.name);
        assert_eq!(
            config.content_blocks.len(),
            deserialized.content_blocks.len()
        );

        // Verify the deserialized config works
        let formatter = MessageFormatter::new(deserialized).unwrap();

        let messages = vec![MessageContent::Text {
            role: "user".to_string(),
            content: "Test".to_string(),
        }];

        let result = formatter.format_messages(&messages).unwrap();
        assert!(result.as_array().is_some());
    }

    #[test]
    fn test_empty_messages() {
        let config = anthropic_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let messages: Vec<MessageContent> = vec![];
        let result = formatter.format_messages(&messages).unwrap();

        let arr = result.as_array().unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn test_mixed_content_blocks() {
        let config = openai_format().unwrap();
        let formatter = MessageFormatter::new(config).unwrap();

        let messages = vec![MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![
                ContentBlock::Text("First, look at this image:".to_string()),
                ContentBlock::Binary {
                    data: vec![0xFF, 0xD8, 0xFF],
                    mime_type: "image/jpeg".to_string(),
                    filename: None,
                },
                ContentBlock::Text("Then, listen to this:".to_string()),
                ContentBlock::Binary {
                    data: vec![0x52, 0x49, 0x46, 0x46],
                    mime_type: "audio/wav".to_string(),
                    filename: None,
                },
                ContentBlock::Url {
                    url: "https://example.com/another.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                },
            ],
        }];

        let result = formatter.format_messages(&messages).unwrap();
        let arr = result.as_array().unwrap();
        let content = arr[0]["content"].as_array().unwrap();

        assert_eq!(content.len(), 5);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(content[2]["type"], "text");
        assert_eq!(content[3]["type"], "audio");
        assert_eq!(content[4]["type"], "image_url");
    }

    #[test]
    fn test_v1_to_v2_upgrade() {
        // Test anthropic upgrade
        let config = upgrade_v1_to_v2("anthropic").unwrap();
        assert_eq!(config.name, "anthropic");
        assert!(!config.text_message.template.is_empty());
        assert!(!config.content_blocks.is_empty());

        // Test openai upgrade
        let config = upgrade_v1_to_v2("openai").unwrap();
        assert_eq!(config.name, "openai");
        assert!(!config.text_message.template.is_empty());

        // Test case insensitivity
        let config = upgrade_v1_to_v2("ANTHROPIC").unwrap();
        assert_eq!(config.name, "anthropic");

        let config = upgrade_v1_to_v2("OpenAI").unwrap();
        assert_eq!(config.name, "openai");

        // Test unknown format returns error
        let result = upgrade_v1_to_v2("unknown-provider");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unknown v1 message format"));
        assert!(err_msg.contains("migrate to v2"));
    }

    #[test]
    fn test_registry_auto_upgrade() {
        let mut registry = MessageFormatRegistry::new();

        // First call should auto-upgrade
        let formatter = registry.get_or_upgrade("anthropic").unwrap();
        assert_eq!(formatter.name(), "anthropic");

        // Second call should use cached version (not re-upgrade)
        let formatter2 = registry.get_or_upgrade("anthropic").unwrap();
        assert_eq!(formatter2.name(), "anthropic");

        // Registry should now contain the formatter
        assert!(registry.get("anthropic").is_some());

        // Try with openai
        let formatter = registry.get_or_upgrade("openai").unwrap();
        assert_eq!(formatter.name(), "openai");
    }

    #[test]
    fn test_has_v2_upgrade() {
        assert!(has_v2_upgrade("anthropic"));
        assert!(has_v2_upgrade("ANTHROPIC"));
        assert!(has_v2_upgrade("openai"));
        assert!(has_v2_upgrade("OpenAI"));
        assert!(has_v2_upgrade("google"));
        assert!(!has_v2_upgrade("unknown"));
        assert!(!has_v2_upgrade("custom-provider"));
    }

    #[test]
    fn test_upgrade_produces_valid_formatter() {
        // Verify that upgraded configs can create valid formatters
        for format_name in &["anthropic", "openai"] {
            let config = upgrade_v1_to_v2(format_name).unwrap();
            let formatter = MessageFormatter::new(config).unwrap();

            // Test that formatter can format a simple message
            let messages = vec![MessageContent::Text {
                role: "user".to_string(),
                content: "Hello, world!".to_string(),
            }];

            let result = formatter.format_messages(&messages);
            assert!(result.is_ok(), "Formatter for {} failed", format_name);

            let json = result.unwrap();
            assert!(json.is_array());
            assert!(!json.as_array().unwrap().is_empty());
        }
    }
}
