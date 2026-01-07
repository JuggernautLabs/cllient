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
// Role Mapping Types
// ============================================================================

/// Standard role names used internally across all providers.
///
/// These represent the canonical role types that the library uses internally,
/// which are then mapped to provider-specific role names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardRole {
    /// The user/human in the conversation
    User,
    /// The AI assistant's response
    Assistant,
    /// System instructions (may be handled separately by some providers)
    System,
    /// Tool/function call results
    Tool,
}

impl StandardRole {
    /// Returns all standard roles as an array.
    pub fn all() -> [StandardRole; 4] {
        [
            StandardRole::User,
            StandardRole::Assistant,
            StandardRole::System,
            StandardRole::Tool,
        ]
    }

    /// Returns the default provider name for this role.
    /// This is used as a fallback when no mapping is configured.
    pub fn default_name(&self) -> &'static str {
        match self {
            StandardRole::User => "user",
            StandardRole::Assistant => "assistant",
            StandardRole::System => "system",
            StandardRole::Tool => "tool",
        }
    }
}

impl std::fmt::Display for StandardRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.default_name())
    }
}

impl std::str::FromStr for StandardRole {
    type Err = ClientError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(StandardRole::User),
            "assistant" => Ok(StandardRole::Assistant),
            "system" => Ok(StandardRole::System),
            "tool" => Ok(StandardRole::Tool),
            _ => Err(ClientError::ValidationError(format!(
                "Unknown role: '{}'. Expected one of: user, assistant, system, tool",
                s
            ))),
        }
    }
}

/// Configuration for how system messages are handled by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemHandling {
    /// System messages are included inline as regular messages with a "system" role.
    /// Used by: OpenAI
    #[default]
    Inline,
    /// System messages are extracted and passed separately (e.g., top-level "system" field).
    /// Used by: Anthropic (top-level "system" parameter), Google (systemInstruction)
    Separate,
}

/// Role mapping configuration for translating between standard and provider-specific role names.
///
/// Different LLM providers use different names for message roles:
/// - OpenAI: user, assistant, system, tool
/// - Anthropic: user, assistant (system handled separately)
/// - Google: user, model (system via systemInstruction)
///
/// This struct provides bidirectional mapping between the standard role names
/// used internally and the provider-specific names required by each API.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleMappingConfig {
    /// Map from standard role to provider role name
    #[serde(default)]
    mappings: HashMap<StandardRole, String>,

    /// How system messages are handled
    #[serde(default)]
    pub system_handling: SystemHandling,

    /// Reverse map for response parsing (provider role name -> standard role)
    /// This is computed from mappings and not serialized.
    #[serde(skip)]
    reverse: HashMap<String, StandardRole>,
}

impl RoleMappingConfig {
    /// Create a new empty role mapping configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a role mapping configuration from a hashmap of mappings.
    pub fn from_mappings(mappings: HashMap<StandardRole, String>) -> Self {
        let mut config = Self {
            mappings,
            system_handling: SystemHandling::default(),
            reverse: HashMap::new(),
        };
        config.rebuild_reverse_map();
        config
    }

    /// Set the system handling mode.
    pub fn with_system_handling(mut self, handling: SystemHandling) -> Self {
        self.system_handling = handling;
        self
    }

    /// Build the reverse mapping from provider names to standard roles.
    /// This should be called after deserialization or after modifying mappings.
    pub fn rebuild_reverse_map(&mut self) {
        self.reverse.clear();
        for (standard, provider) in &self.mappings {
            self.reverse.insert(provider.clone(), *standard);
        }
    }

    /// Get the provider role name for a standard role.
    ///
    /// If no mapping exists, returns the default name for the role.
    pub fn to_provider(&self, role: StandardRole) -> &str {
        self.mappings
            .get(&role)
            .map(|s| s.as_str())
            .unwrap_or_else(|| role.default_name())
    }

    /// Get the standard role from a provider role name.
    ///
    /// Returns None if the provider role is not recognized.
    pub fn from_provider(&self, provider_role: &str) -> Option<StandardRole> {
        // First check the reverse map
        if let Some(role) = self.reverse.get(provider_role) {
            return Some(*role);
        }

        // Fall back to checking if it matches a default name
        provider_role.parse().ok()
    }

    /// Check if a mapping exists for the given standard role.
    pub fn has_mapping(&self, role: StandardRole) -> bool {
        self.mappings.contains_key(&role)
    }

    /// Get all configured mappings.
    pub fn mappings(&self) -> &HashMap<StandardRole, String> {
        &self.mappings
    }

    /// Check if this provider handles system messages separately.
    pub fn handles_system_separately(&self) -> bool {
        self.system_handling == SystemHandling::Separate
    }
}

/// Builder for constructing RoleMappingConfig with a fluent API.
///
/// # Example
///
/// ```
/// use cllient::message_format::{RoleMappingBuilder, StandardRole};
///
/// let config = RoleMappingBuilder::new()
///     .user("user")
///     .assistant("model")  // Google uses "model" instead of "assistant"
///     .system_separate()   // Google handles system via systemInstruction
///     .build();
///
/// assert_eq!(config.to_provider(StandardRole::Assistant), "model");
/// ```
#[derive(Debug, Clone, Default)]
pub struct RoleMappingBuilder {
    mappings: HashMap<StandardRole, String>,
    system_handling: SystemHandling,
}

impl RoleMappingBuilder {
    /// Create a new builder with no mappings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the provider name for the User role.
    pub fn user(mut self, provider_name: impl Into<String>) -> Self {
        self.mappings.insert(StandardRole::User, provider_name.into());
        self
    }

    /// Set the provider name for the Assistant role.
    pub fn assistant(mut self, provider_name: impl Into<String>) -> Self {
        self.mappings.insert(StandardRole::Assistant, provider_name.into());
        self
    }

    /// Set the provider name for the System role.
    pub fn system(mut self, provider_name: impl Into<String>) -> Self {
        self.mappings.insert(StandardRole::System, provider_name.into());
        self
    }

    /// Set the provider name for the Tool role.
    pub fn tool(mut self, provider_name: impl Into<String>) -> Self {
        self.mappings.insert(StandardRole::Tool, provider_name.into());
        self
    }

    /// Set a custom mapping for any role.
    pub fn map(mut self, role: StandardRole, provider_name: impl Into<String>) -> Self {
        self.mappings.insert(role, provider_name.into());
        self
    }

    /// Set system handling to inline (system messages are regular messages).
    pub fn system_inline(mut self) -> Self {
        self.system_handling = SystemHandling::Inline;
        self
    }

    /// Set system handling to separate (system messages extracted to separate field).
    pub fn system_separate(mut self) -> Self {
        self.system_handling = SystemHandling::Separate;
        self
    }

    /// Build the RoleMappingConfig.
    pub fn build(self) -> RoleMappingConfig {
        let mut config = RoleMappingConfig {
            mappings: self.mappings,
            system_handling: self.system_handling,
            reverse: HashMap::new(),
        };
        config.rebuild_reverse_map();
        config
    }

    // ========================================================================
    // Provider Presets
    // ========================================================================

    /// Create an OpenAI-compatible role mapping.
    ///
    /// OpenAI uses standard role names:
    /// - user -> user
    /// - assistant -> assistant
    /// - system -> system (inline)
    /// - tool -> tool
    pub fn openai() -> RoleMappingConfig {
        RoleMappingBuilder::new()
            .user("user")
            .assistant("assistant")
            .system("system")
            .tool("tool")
            .system_inline()
            .build()
    }

    /// Create an Anthropic-compatible role mapping.
    ///
    /// Anthropic uses:
    /// - user -> user
    /// - assistant -> assistant
    /// - system -> handled separately (top-level "system" parameter)
    /// - tool -> not directly mapped (tool results are content blocks)
    pub fn anthropic() -> RoleMappingConfig {
        RoleMappingBuilder::new()
            .user("user")
            .assistant("assistant")
            .system_separate()
            .build()
    }

    /// Create a Google (Gemini)-compatible role mapping.
    ///
    /// Google uses:
    /// - user -> user
    /// - assistant -> model (Google uses "model" instead of "assistant")
    /// - system -> handled separately (via systemInstruction)
    /// - tool -> not directly mapped (function responses are content parts)
    pub fn google() -> RoleMappingConfig {
        RoleMappingBuilder::new()
            .user("user")
            .assistant("model")
            .system_separate()
            .build()
    }

    /// Create a DeepSeek-compatible role mapping.
    ///
    /// DeepSeek uses OpenAI-compatible role names:
    /// - user -> user
    /// - assistant -> assistant
    /// - system -> system (inline)
    /// - tool -> tool
    pub fn deepseek() -> RoleMappingConfig {
        Self::openai()
    }

    /// Create a Mistral-compatible role mapping.
    ///
    /// Mistral uses OpenAI-compatible role names:
    /// - user -> user
    /// - assistant -> assistant
    /// - system -> system (inline)
    /// - tool -> tool
    pub fn mistral() -> RoleMappingConfig {
        Self::openai()
    }

    /// Create a Cohere-compatible role mapping.
    ///
    /// Cohere uses different role names:
    /// - user -> USER
    /// - assistant -> CHATBOT
    /// - system -> SYSTEM (inline)
    /// - tool -> TOOL
    pub fn cohere() -> RoleMappingConfig {
        RoleMappingBuilder::new()
            .user("USER")
            .assistant("CHATBOT")
            .system("SYSTEM")
            .tool("TOOL")
            .system_inline()
            .build()
    }
}

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

    // ========================================================================
    // Role Mapping Tests
    // ========================================================================

    #[test]
    fn test_standard_role_default_names() {
        assert_eq!(StandardRole::User.default_name(), "user");
        assert_eq!(StandardRole::Assistant.default_name(), "assistant");
        assert_eq!(StandardRole::System.default_name(), "system");
        assert_eq!(StandardRole::Tool.default_name(), "tool");
    }

    #[test]
    fn test_standard_role_display() {
        assert_eq!(format!("{}", StandardRole::User), "user");
        assert_eq!(format!("{}", StandardRole::Assistant), "assistant");
        assert_eq!(format!("{}", StandardRole::System), "system");
        assert_eq!(format!("{}", StandardRole::Tool), "tool");
    }

    #[test]
    fn test_standard_role_from_str() {
        assert_eq!("user".parse::<StandardRole>().unwrap(), StandardRole::User);
        assert_eq!("assistant".parse::<StandardRole>().unwrap(), StandardRole::Assistant);
        assert_eq!("system".parse::<StandardRole>().unwrap(), StandardRole::System);
        assert_eq!("tool".parse::<StandardRole>().unwrap(), StandardRole::Tool);

        // Case insensitive
        assert_eq!("USER".parse::<StandardRole>().unwrap(), StandardRole::User);
        assert_eq!("Assistant".parse::<StandardRole>().unwrap(), StandardRole::Assistant);
        assert_eq!("SYSTEM".parse::<StandardRole>().unwrap(), StandardRole::System);

        // Invalid role
        assert!("unknown".parse::<StandardRole>().is_err());
    }

    #[test]
    fn test_standard_role_all() {
        let all = StandardRole::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&StandardRole::User));
        assert!(all.contains(&StandardRole::Assistant));
        assert!(all.contains(&StandardRole::System));
        assert!(all.contains(&StandardRole::Tool));
    }

    #[test]
    fn test_role_mapping_builder_basic() {
        let config = RoleMappingBuilder::new()
            .user("user")
            .assistant("assistant")
            .system("system")
            .tool("tool")
            .build();

        assert_eq!(config.to_provider(StandardRole::User), "user");
        assert_eq!(config.to_provider(StandardRole::Assistant), "assistant");
        assert_eq!(config.to_provider(StandardRole::System), "system");
        assert_eq!(config.to_provider(StandardRole::Tool), "tool");
    }

    #[test]
    fn test_role_mapping_builder_custom_names() {
        let config = RoleMappingBuilder::new()
            .user("human")
            .assistant("ai")
            .system("instructions")
            .tool("function")
            .build();

        assert_eq!(config.to_provider(StandardRole::User), "human");
        assert_eq!(config.to_provider(StandardRole::Assistant), "ai");
        assert_eq!(config.to_provider(StandardRole::System), "instructions");
        assert_eq!(config.to_provider(StandardRole::Tool), "function");
    }

    #[test]
    fn test_role_mapping_builder_map_method() {
        let config = RoleMappingBuilder::new()
            .map(StandardRole::User, "custom_user")
            .map(StandardRole::Assistant, "custom_assistant")
            .build();

        assert_eq!(config.to_provider(StandardRole::User), "custom_user");
        assert_eq!(config.to_provider(StandardRole::Assistant), "custom_assistant");
    }

    #[test]
    fn test_role_mapping_default_fallback() {
        // Empty config should use default names
        let config = RoleMappingBuilder::new().build();

        assert_eq!(config.to_provider(StandardRole::User), "user");
        assert_eq!(config.to_provider(StandardRole::Assistant), "assistant");
        assert_eq!(config.to_provider(StandardRole::System), "system");
        assert_eq!(config.to_provider(StandardRole::Tool), "tool");
    }

    #[test]
    fn test_role_mapping_partial_config() {
        // Only configure some roles, others should use defaults
        let config = RoleMappingBuilder::new()
            .assistant("model")
            .build();

        assert_eq!(config.to_provider(StandardRole::User), "user"); // default
        assert_eq!(config.to_provider(StandardRole::Assistant), "model"); // custom
        assert_eq!(config.to_provider(StandardRole::System), "system"); // default
    }

    #[test]
    fn test_role_mapping_reverse_lookup() {
        let config = RoleMappingBuilder::new()
            .user("user")
            .assistant("model")
            .build();

        // Lookup by provider name
        assert_eq!(config.from_provider("user"), Some(StandardRole::User));
        assert_eq!(config.from_provider("model"), Some(StandardRole::Assistant));

        // Default names still work even if not explicitly mapped
        assert_eq!(config.from_provider("system"), Some(StandardRole::System));
        assert_eq!(config.from_provider("tool"), Some(StandardRole::Tool));

        // Unknown provider role
        assert_eq!(config.from_provider("unknown"), None);
    }

    #[test]
    fn test_role_mapping_google_reverse() {
        let config = RoleMappingBuilder::google();

        // Forward lookup
        assert_eq!(config.to_provider(StandardRole::User), "user");
        assert_eq!(config.to_provider(StandardRole::Assistant), "model");

        // Reverse lookup
        assert_eq!(config.from_provider("user"), Some(StandardRole::User));
        assert_eq!(config.from_provider("model"), Some(StandardRole::Assistant));

        // "assistant" should still work via default fallback
        assert_eq!(config.from_provider("assistant"), Some(StandardRole::Assistant));
    }

    #[test]
    fn test_role_mapping_has_mapping() {
        let config = RoleMappingBuilder::new()
            .user("user")
            .assistant("model")
            .build();

        assert!(config.has_mapping(StandardRole::User));
        assert!(config.has_mapping(StandardRole::Assistant));
        assert!(!config.has_mapping(StandardRole::System));
        assert!(!config.has_mapping(StandardRole::Tool));
    }

    #[test]
    fn test_role_mapping_system_handling_inline() {
        let config = RoleMappingBuilder::new()
            .system("system")
            .system_inline()
            .build();

        assert_eq!(config.system_handling, SystemHandling::Inline);
        assert!(!config.handles_system_separately());
    }

    #[test]
    fn test_role_mapping_system_handling_separate() {
        let config = RoleMappingBuilder::new()
            .system_separate()
            .build();

        assert_eq!(config.system_handling, SystemHandling::Separate);
        assert!(config.handles_system_separately());
    }

    #[test]
    fn test_openai_preset() {
        let config = RoleMappingBuilder::openai();

        assert_eq!(config.to_provider(StandardRole::User), "user");
        assert_eq!(config.to_provider(StandardRole::Assistant), "assistant");
        assert_eq!(config.to_provider(StandardRole::System), "system");
        assert_eq!(config.to_provider(StandardRole::Tool), "tool");
        assert_eq!(config.system_handling, SystemHandling::Inline);
        assert!(!config.handles_system_separately());
    }

    #[test]
    fn test_anthropic_preset() {
        let config = RoleMappingBuilder::anthropic();

        assert_eq!(config.to_provider(StandardRole::User), "user");
        assert_eq!(config.to_provider(StandardRole::Assistant), "assistant");
        assert_eq!(config.system_handling, SystemHandling::Separate);
        assert!(config.handles_system_separately());
    }

    #[test]
    fn test_google_preset() {
        let config = RoleMappingBuilder::google();

        assert_eq!(config.to_provider(StandardRole::User), "user");
        assert_eq!(config.to_provider(StandardRole::Assistant), "model");
        assert_eq!(config.system_handling, SystemHandling::Separate);
        assert!(config.handles_system_separately());
    }

    #[test]
    fn test_cohere_preset() {
        let config = RoleMappingBuilder::cohere();

        assert_eq!(config.to_provider(StandardRole::User), "USER");
        assert_eq!(config.to_provider(StandardRole::Assistant), "CHATBOT");
        assert_eq!(config.to_provider(StandardRole::System), "SYSTEM");
        assert_eq!(config.to_provider(StandardRole::Tool), "TOOL");
        assert_eq!(config.system_handling, SystemHandling::Inline);
    }

    #[test]
    fn test_deepseek_preset() {
        // DeepSeek should be OpenAI-compatible
        let deepseek = RoleMappingBuilder::deepseek();
        let openai = RoleMappingBuilder::openai();

        assert_eq!(deepseek.to_provider(StandardRole::User), openai.to_provider(StandardRole::User));
        assert_eq!(deepseek.to_provider(StandardRole::Assistant), openai.to_provider(StandardRole::Assistant));
        assert_eq!(deepseek.to_provider(StandardRole::System), openai.to_provider(StandardRole::System));
        assert_eq!(deepseek.to_provider(StandardRole::Tool), openai.to_provider(StandardRole::Tool));
        assert_eq!(deepseek.system_handling, openai.system_handling);
    }

    #[test]
    fn test_mistral_preset() {
        // Mistral should be OpenAI-compatible
        let mistral = RoleMappingBuilder::mistral();
        let openai = RoleMappingBuilder::openai();

        assert_eq!(mistral.to_provider(StandardRole::User), openai.to_provider(StandardRole::User));
        assert_eq!(mistral.to_provider(StandardRole::Assistant), openai.to_provider(StandardRole::Assistant));
        assert_eq!(mistral.system_handling, openai.system_handling);
    }

    #[test]
    fn test_role_mapping_serialization() {
        let config = RoleMappingBuilder::google();

        // Serialize to YAML
        let yaml = serde_yaml::to_string(&config).unwrap();

        // Deserialize back
        let mut deserialized: RoleMappingConfig = serde_yaml::from_str(&yaml).unwrap();

        // Rebuild reverse map after deserialization (skipped field)
        deserialized.rebuild_reverse_map();

        assert_eq!(config.to_provider(StandardRole::User), deserialized.to_provider(StandardRole::User));
        assert_eq!(config.to_provider(StandardRole::Assistant), deserialized.to_provider(StandardRole::Assistant));
        assert_eq!(config.system_handling, deserialized.system_handling);

        // Verify reverse lookup works after rebuild
        assert_eq!(deserialized.from_provider("model"), Some(StandardRole::Assistant));
    }

    #[test]
    fn test_role_mapping_json_serialization() {
        let config = RoleMappingBuilder::new()
            .user("user")
            .assistant("model")
            .system_separate()
            .build();

        // Serialize to JSON
        let json = serde_json::to_string(&config).unwrap();

        // Deserialize back
        let mut deserialized: RoleMappingConfig = serde_json::from_str(&json).unwrap();
        deserialized.rebuild_reverse_map();

        assert_eq!(config.to_provider(StandardRole::User), deserialized.to_provider(StandardRole::User));
        assert_eq!(config.to_provider(StandardRole::Assistant), deserialized.to_provider(StandardRole::Assistant));
        assert_eq!(config.system_handling, deserialized.system_handling);
    }

    #[test]
    fn test_role_mapping_from_mappings() {
        let mut mappings = HashMap::new();
        mappings.insert(StandardRole::User, "human".to_string());
        mappings.insert(StandardRole::Assistant, "bot".to_string());

        let config = RoleMappingConfig::from_mappings(mappings);

        assert_eq!(config.to_provider(StandardRole::User), "human");
        assert_eq!(config.to_provider(StandardRole::Assistant), "bot");
        assert_eq!(config.from_provider("human"), Some(StandardRole::User));
        assert_eq!(config.from_provider("bot"), Some(StandardRole::Assistant));
    }

    #[test]
    fn test_role_mapping_with_system_handling() {
        let mut mappings = HashMap::new();
        mappings.insert(StandardRole::User, "user".to_string());

        let config = RoleMappingConfig::from_mappings(mappings)
            .with_system_handling(SystemHandling::Separate);

        assert!(config.handles_system_separately());
    }

    #[test]
    fn test_role_mapping_mappings_accessor() {
        let config = RoleMappingBuilder::new()
            .user("user")
            .assistant("model")
            .build();

        let mappings = config.mappings();
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings.get(&StandardRole::User), Some(&"user".to_string()));
        assert_eq!(mappings.get(&StandardRole::Assistant), Some(&"model".to_string()));
    }

    #[test]
    fn test_standard_role_serde() {
        // Test serde rename_all = "snake_case"
        let role = StandardRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let role = StandardRole::Assistant;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"assistant\"");

        // Deserialize
        let role: StandardRole = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(role, StandardRole::System);

        let role: StandardRole = serde_json::from_str("\"tool\"").unwrap();
        assert_eq!(role, StandardRole::Tool);
    }

    #[test]
    fn test_system_handling_serde() {
        let inline = SystemHandling::Inline;
        let json = serde_json::to_string(&inline).unwrap();
        assert_eq!(json, "\"inline\"");

        let separate = SystemHandling::Separate;
        let json = serde_json::to_string(&separate).unwrap();
        assert_eq!(json, "\"separate\"");

        // Deserialize
        let handling: SystemHandling = serde_json::from_str("\"inline\"").unwrap();
        assert_eq!(handling, SystemHandling::Inline);

        let handling: SystemHandling = serde_json::from_str("\"separate\"").unwrap();
        assert_eq!(handling, SystemHandling::Separate);
    }

    #[test]
    fn test_system_handling_default() {
        let handling = SystemHandling::default();
        assert_eq!(handling, SystemHandling::Inline);
    }
}
