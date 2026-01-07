use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{ClientError, ConfigError, Result};
use crate::message_format::MessageFormatConfig;

/// Macro to implement Display, Serialize, and Deserialize for enums with a Custom(String) fallback.
/// Each variant maps to a string representation; unknown strings become Custom(s).
macro_rules! impl_string_enum {
    // Case-insensitive variant
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $str:literal ),* $(,)?
        }
        default: $default_variant:ident
        case_insensitive: true
    ) => {
        impl_string_enum!(@impl
            $(#[$meta])*
            $vis enum $name {
                $( $(#[$variant_meta])* $variant => $str ),*
            }
            default: $default_variant
            match_fn: |s: &str| s.to_lowercase()
        );
    };

    // Case-sensitive variant (default)
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $str:literal ),* $(,)?
        }
        default: $default_variant:ident
    ) => {
        impl_string_enum!(@impl
            $(#[$meta])*
            $vis enum $name {
                $( $(#[$variant_meta])* $variant => $str ),*
            }
            default: $default_variant
            match_fn: |s: &str| s.to_string()
        );
    };

    // Internal implementation
    (@impl
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $str:literal ),* $(,)?
        }
        default: $default_variant:ident
        match_fn: $match_fn:expr
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $( $(#[$variant_meta])* $variant, )*
            /// Custom variant for extensibility
            Custom(String),
        }

        impl Default for $name {
            fn default() -> Self {
                $name::$default_variant
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $( $name::$variant => write!(f, $str), )*
                    $name::Custom(s) => write!(f, "{}", s),
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                let match_fn: fn(&str) -> String = $match_fn;
                Ok(match match_fn(&s).as_str() {
                    $( $str => $name::$variant, )*
                    _ => $name::Custom(s),
                })
            }
        }
    };
}

impl_string_enum! {
    /// Message builder format - determines how messages are structured for the API
    #[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
    pub enum MessageFormat {
        /// OpenAI-compatible message format (used by OpenAI, DeepSeek, Azure, most providers)
        OpenAI => "openai",
        /// Anthropic Claude message format
        Anthropic => "anthropic",
        /// Google Gemini message format
        Google => "google",
    }
    default: OpenAI
    case_insensitive: true
}

impl_string_enum! {
    /// SSE parser type - determines how to parse streaming responses
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SseParser {
        /// OpenAI-compatible SSE format
        OpenAiSse => "openai_sse",
        /// Anthropic Claude SSE format
        AnthropicSse => "anthropic_sse",
        /// Google Gemini SSE format
        GoogleSse => "google_sse",
    }
    default: OpenAiSse
}

impl_string_enum! {
    /// Streaming format type
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum StreamingFormat {
        /// Server-Sent Events (text/event-stream)
        TextEventStream => "text/event-stream",
        /// Newline-delimited JSON
        Ndjson => "application/x-ndjson",
    }
    default: TextEventStream
}

/// Currency for pricing
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default, JsonSchema)]
pub enum Currency {
    #[default]
    USD,
    EUR,
    GBP,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub service: ServiceInfo,
    pub http: HttpConfig,
    #[serde(default)]
    pub optional: HashMap<String, String>,
    pub streaming: StreamingConfigYaml,
    pub response: ResponseConfig,
    #[serde(default)]
    pub message_builder: Option<MessageFormat>,
    #[serde(default)]
    pub message_format: Option<MessageFormatConfig>,
    #[serde(default)]
    pub rate_limits: RateLimits,
}

impl ServiceConfig {
    pub fn validate_message_format(&self) -> Result<()> {
        if self.message_builder.is_none() && self.message_format.is_none() {
            return Err(ClientError::Config(ConfigError::MissingField(
                "Either 'message_builder' or 'message_format' must be specified".to_string()
            )));
        }
        Ok(())
    }

    pub fn message_format_name(&self) -> String {
        if let Some(ref config) = self.message_format {
            config.name.clone()
        } else if let Some(ref builder) = self.message_builder {
            builder.to_string()
        } else {
            "unknown".to_string()
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceInfo {
    pub name: String,
    pub base_url: String,
    /// Verification status for this service
    #[serde(default)]
    pub status: VerificationStatus,
    /// Optional description of the service
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpConfig {
    pub request: String, // The HTTP template
}

/// Streaming configuration from YAML files
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamingConfigYaml {
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub format: Option<StreamingFormat>,
    #[serde(default)]
    pub parser: Option<SseParser>,
    #[serde(default)]
    pub line_prefix: Option<String>,
    #[serde(default)]
    pub done_marker: Option<String>,
    #[serde(default)]
    pub events: Vec<StreamEventConfig>,
    #[serde(default)]
    pub extract: HashMap<String, String>,
}

impl StreamingConfigYaml {
    /// Resolve template reference and merge with inline config
    pub fn resolve_template(&mut self, templates: &StreamingTemplateRegistry) -> Result<()> {
        if let Some(ref template_name) = self.template {
            let template = templates.get(template_name)?;

            // Start with the template as base
            let mut resolved = template.clone();

            // Override with any inline config values
            if let Some(format) = &self.format {
                resolved.format = Some(format.clone());
            }
            if let Some(parser) = &self.parser {
                resolved.parser = Some(parser.clone());
            }
            if let Some(prefix) = &self.line_prefix {
                resolved.line_prefix = Some(prefix.clone());
            }
            if let Some(marker) = &self.done_marker {
                resolved.done_marker = Some(marker.clone());
            }
            if !self.events.is_empty() {
                resolved.events = self.events.clone();
            }
            if !self.extract.is_empty() {
                resolved.extract = self.extract.clone();
            }

            *self = resolved;
        }

        Ok(())
    }

    /// Ensure required fields are present (either from template or inline)
    pub fn validate(&self) -> Result<()> {
        if self.format.is_none() {
            return Err(ClientError::Config(ConfigError::MissingField(
                "Streaming config missing 'format' field".to_string()
            )));
        }
        if self.parser.is_none() {
            return Err(ClientError::Config(ConfigError::MissingField(
                "Streaming config missing 'parser' field".to_string()
            )));
        }
        Ok(())
    }
}

/// Backwards-compatible alias
pub type StreamingConfig = StreamingConfigYaml;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamEventConfig {
    #[serde(rename = "type")]
    pub event_type: String,
    pub extract: String,
    pub action: String,
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseConfig {
    pub success_codes: Vec<u16>,
    pub extract: ResponseExtract,
    #[serde(default)]
    pub error: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseExtract {
    pub content: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<UsageExtract>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UsageExtract {
    pub input: String,
    pub output: String,
    #[serde(default)]
    pub total: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RateLimits {
    #[serde(default)]
    pub requests_per_minute: Option<u32>,
    #[serde(default)]
    pub tokens_per_minute: Option<u32>,
    #[serde(default)]
    pub concurrent_requests: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
    pub model: ModelInfo,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
    #[serde(default)]
    pub constraints: Constraints,
    #[serde(default)]
    pub defaults: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub quality: HashMap<String, u8>,
    #[serde(default)]
    pub behaviors: HashMap<String, bool>,
    #[serde(default)]
    pub use_cases: Vec<String>,
}

/// Verification status for a model configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    /// Model has been manually tested and verified to work
    Verified,
    /// Model config was auto-generated and has not been tested
    #[default]
    Unverified,
    /// Model was tested but found to have issues
    Broken,
    /// Model is deprecated and may stop working
    Deprecated,
    /// Model endpoint returned an error (e.g., 404, auth issues)
    Error,
    /// Model was not found at the provider
    NotFound,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub family: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
    pub service: String,
    /// Verification status - whether this model config has been tested
    #[serde(default)]
    pub status: VerificationStatus,
    /// Optional lab/organization that created the model
    #[serde(default)]
    pub lab: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct Capabilities {
    pub context_window: u32,
    pub max_output_tokens: u32,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub functions: bool,
    #[serde(default)]
    pub streaming: bool,
    #[serde(default)]
    pub json_mode: bool,
    #[serde(default)]
    pub system_prompt: bool,
    #[serde(default)]
    pub multimodal: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct Pricing {
    #[serde(default)]
    pub currency: Currency,
    pub input_per_1k_tokens: f64,
    pub output_per_1k_tokens: f64,
    #[serde(default)]
    pub cached_input_per_1k_tokens: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema)]
pub struct Constraints {
    #[serde(default)]
    pub max_images_per_message: Option<u32>,
    #[serde(default)]
    pub max_image_size_mb: Option<u32>,
    #[serde(default)]
    pub supported_image_formats: Vec<String>,
    #[serde(default)]
    pub max_function_calls_per_message: Option<u32>,
}

/// Registry for streaming configuration templates
pub struct StreamingTemplateRegistry {
    templates: HashMap<String, StreamingConfigYaml>,
}

impl StreamingTemplateRegistry {
    /// Load all streaming templates from a directory
    pub fn load<P: AsRef<Path>>(template_dir: P) -> Result<Self> {
        let mut templates = HashMap::new();
        let dir = template_dir.as_ref();

        if !dir.exists() {
            // No templates directory is okay - just return empty registry
            return Ok(Self { templates });
        }

        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "yaml" || ext == "yml"))
        {
            let path = entry.path();
            let content = fs::read_to_string(path)?;
            let mut template: StreamingConfigYaml = serde_yaml::from_str(&content)
                .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                    format!("Failed to parse streaming template '{}': {}", path.display(), e)
                )))?;

            // Validate the template has required fields
            template.validate()?;

            let template_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| ClientError::Config(ConfigError::InvalidPath(
                    format!("Invalid template file name: {}", path.display())
                )))?
                .to_string();

            templates.insert(template_name, template);
        }

        Ok(Self { templates })
    }

    /// Get a template by name
    pub fn get(&self, name: &str) -> Result<&StreamingConfigYaml> {
        self.templates
            .get(name)
            .ok_or_else(|| ClientError::Config(ConfigError::InvalidValue(
                format!("Streaming template '{}' not found", name)
            )))
    }

    /// Check if a template exists
    pub fn has(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }
}

pub struct ConfigLoader {
    config_dir: PathBuf,
    services: HashMap<String, ServiceConfig>,
    models: HashMap<String, ModelConfig>,
    streaming_templates: StreamingTemplateRegistry,
}

impl ConfigLoader {
    pub fn new<P: AsRef<Path>>(config_dir: P) -> Result<Self> {
        // Try to load .env file from the config directory or current directory
        let config_path = config_dir.as_ref();
        if config_path.join(".env").exists() {
            dotenv::from_path(config_path.join(".env")).ok();
        } else {
            dotenv::dotenv().ok();
        }
        
        let config_dir = config_dir.as_ref().to_path_buf();
        
        if !config_dir.exists() {
            return Err(ClientError::Config(ConfigError::InvalidPath(
                format!("Config directory does not exist: {}", config_dir.display())
            )));
        }

        let mut loader = Self {
            config_dir: config_dir.clone(),
            services: HashMap::new(),
            models: HashMap::new(),
            streaming_templates: StreamingTemplateRegistry::load(&config_dir.join("service-v2/streaming"))?,
        };

        loader.load_all()?;
        Ok(loader)
    }

    fn load_all(&mut self) -> Result<()> {
        self.load_services()?;
        self.load_models()?;
        Ok(())
    }

    fn load_services(&mut self) -> Result<()> {
        let services_dir = self.config_dir.join("service");

        if !services_dir.exists() {
            return Ok(()); // No services directory is ok
        }

        let templates = &self.streaming_templates;

        for entry in WalkDir::new(&services_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "yaml" || ext == "yml"))
        {
            let path = entry.path();
            let content = fs::read_to_string(path)?;
            let mut service_config: ServiceConfig = serde_yaml::from_str(&content)
                .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                    format!("Failed to parse service config '{}': {}", path.display(), e)
                )))?;

            // Validate message format configuration
            service_config.validate_message_format()?;

            // Resolve streaming templates if present
            service_config.streaming.resolve_template(templates)?;
            service_config.streaming.validate()?;

            let service_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| ClientError::Config(ConfigError::InvalidPath(
                    format!("Invalid service file name: {}", path.display())
                )))?;

            self.services.insert(service_name.to_string(), service_config);
        }

        Ok(())
    }

    fn load_models(&mut self) -> Result<()> {
        let family_dir = self.config_dir.join("family");
        
        if !family_dir.exists() {
            return Ok(()); // No family directory is ok
        }

        for entry in WalkDir::new(&family_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "yaml" || ext == "yml"))
        {
            let path = entry.path();
            let content = fs::read_to_string(path)?;
            let model_config: ModelConfig = serde_yaml::from_str(&content)
                .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                    format!("Failed to parse model config '{}': {}", path.display(), e)
                )))?;
            
            // Use the model ID from the config, not the filename
            let model_id = model_config.model.id.clone();
            self.models.insert(model_id, model_config);
        }

        Ok(())
    }

    pub fn get_service(&self, name: &str) -> Result<&ServiceConfig> {
        self.services
            .get(name)
            .ok_or_else(|| ClientError::Config(ConfigError::ServiceNotFound(name.to_string())))
    }

    pub fn get_model(&self, id: &str) -> Result<&ModelConfig> {
        self.models
            .get(id)
            .ok_or_else(|| ClientError::Config(ConfigError::ModelNotFound(id.to_string())))
    }

    pub fn list_services(&self) -> Vec<&str> {
        self.services.keys().map(String::as_str).collect()
    }

    pub fn list_models(&self) -> Vec<&str> {
        self.models.keys().map(String::as_str).collect()
    }

    pub fn get_model_with_service(&self, model_id: &str) -> Result<(&ModelConfig, &ServiceConfig)> {
        let model = self.get_model(model_id)?;
        let service = self.get_service(&model.model.service)?;
        Ok((model, service))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        // Create service directory and file
        let service_dir = config_dir.join("service");
        fs::create_dir_all(&service_dir).unwrap();

        let service_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1
    Host: api.test.com
    Content-Type: application/json
    Authorization: Bearer ${API_KEY}

    {
      "model": "${model_id}",
      "messages": ${messages}
    }

streaming:
  format: text/event-stream
  parser: test_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: test
"#;

        fs::write(service_dir.join("test.yaml"), service_config).unwrap();

        let loader = ConfigLoader::new(config_dir).unwrap();
        assert!(loader.get_service("test").is_ok());
        assert_eq!(loader.list_services(), vec!["test"]);
    }

    #[test]
    fn test_streaming_template_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        // Create streaming template directory
        let streaming_dir = config_dir.join("service-v2/streaming");
        fs::create_dir_all(&streaming_dir).unwrap();

        // Create a streaming template
        let template = r#"
format: text/event-stream
parser: openai_sse
line_prefix: "data: "
done_marker: "[DONE]"
events:
  - type: content
    extract: choices[0].delta.content
    action: content
"#;
        fs::write(streaming_dir.join("openai_sse.yaml"), template).unwrap();

        // Create service directory and file that references the template
        let service_dir = config_dir.join("service");
        fs::create_dir_all(&service_dir).unwrap();

        let service_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1
    Host: api.test.com

streaming:
  template: openai_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: openai
"#;
        fs::write(service_dir.join("test.yaml"), service_config).unwrap();

        let loader = ConfigLoader::new(config_dir).unwrap();
        let service = loader.get_service("test").unwrap();

        // Verify template was resolved
        assert_eq!(service.streaming.format, Some(StreamingFormat::TextEventStream));
        assert_eq!(service.streaming.parser, Some(SseParser::OpenAiSse));
        assert_eq!(service.streaming.line_prefix, Some("data: ".to_string()));
        assert_eq!(service.streaming.done_marker, Some("[DONE]".to_string()));
        assert_eq!(service.streaming.events.len(), 1);
    }

    #[test]
    fn test_streaming_template_with_override() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path();

        // Create streaming template directory
        let streaming_dir = config_dir.join("service-v2/streaming");
        fs::create_dir_all(&streaming_dir).unwrap();

        // Create a streaming template
        let template = r#"
format: text/event-stream
parser: openai_sse
line_prefix: "data: "
done_marker: "[DONE]"
"#;
        fs::write(streaming_dir.join("openai_sse.yaml"), template).unwrap();

        // Create service that references template but overrides done_marker
        let service_dir = config_dir.join("service");
        fs::create_dir_all(&service_dir).unwrap();

        let service_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1

streaming:
  template: openai_sse
  done_marker: "[END]"

response:
  success_codes: [200]
  extract:
    content: content

message_builder: openai
"#;
        fs::write(service_dir.join("test.yaml"), service_config).unwrap();

        let loader = ConfigLoader::new(config_dir).unwrap();
        let service = loader.get_service("test").unwrap();

        // Template values should be present
        assert_eq!(service.streaming.format, Some(StreamingFormat::TextEventStream));
        assert_eq!(service.streaming.parser, Some(SseParser::OpenAiSse));
        assert_eq!(service.streaming.line_prefix, Some("data: ".to_string()));
        // Override should take effect
        assert_eq!(service.streaming.done_marker, Some("[END]".to_string()));
    }
}