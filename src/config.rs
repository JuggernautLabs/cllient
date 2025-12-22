use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::fmt;
use walkdir::WalkDir;
use crate::error::{ClientError, ConfigError, Result};

/// Message builder format - determines how messages are structured for the API
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MessageFormat {
    /// OpenAI-compatible message format (used by OpenAI, DeepSeek, Azure, most providers)
    #[default]
    OpenAI,
    /// Anthropic Claude message format
    Anthropic,
    /// Google Gemini message format
    Google,
    /// Custom message builder (for extensibility)
    Custom(String),
}

impl fmt::Display for MessageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageFormat::OpenAI => write!(f, "openai"),
            MessageFormat::Anthropic => write!(f, "anthropic"),
            MessageFormat::Google => write!(f, "google"),
            MessageFormat::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl Serialize for MessageFormat {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for MessageFormat {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "openai" => MessageFormat::OpenAI,
            "anthropic" => MessageFormat::Anthropic,
            "google" => MessageFormat::Google,
            _ => MessageFormat::Custom(s),
        })
    }
}

/// SSE parser type - determines how to parse streaming responses
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SseParser {
    /// OpenAI-compatible SSE format
    #[default]
    OpenAiSse,
    /// Anthropic Claude SSE format
    AnthropicSse,
    /// Google Gemini SSE format
    GoogleSse,
    /// Custom SSE parser (for extensibility)
    Custom(String),
}

impl fmt::Display for SseParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SseParser::OpenAiSse => write!(f, "openai_sse"),
            SseParser::AnthropicSse => write!(f, "anthropic_sse"),
            SseParser::GoogleSse => write!(f, "google_sse"),
            SseParser::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl Serialize for SseParser {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SseParser {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "openai_sse" => SseParser::OpenAiSse,
            "anthropic_sse" => SseParser::AnthropicSse,
            "google_sse" => SseParser::GoogleSse,
            _ => SseParser::Custom(s),
        })
    }
}

/// Streaming format type
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StreamingFormat {
    /// Server-Sent Events (text/event-stream)
    #[default]
    TextEventStream,
    /// Newline-delimited JSON
    Ndjson,
    /// Custom streaming format
    Custom(String),
}

impl fmt::Display for StreamingFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamingFormat::TextEventStream => write!(f, "text/event-stream"),
            StreamingFormat::Ndjson => write!(f, "application/x-ndjson"),
            StreamingFormat::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl Serialize for StreamingFormat {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for StreamingFormat {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "text/event-stream" => StreamingFormat::TextEventStream,
            "application/x-ndjson" => StreamingFormat::Ndjson,
            _ => StreamingFormat::Custom(s),
        })
    }
}

/// Currency for pricing
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
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
    pub message_builder: MessageFormat,
    #[serde(default)]
    pub rate_limits: RateLimits,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceInfo {
    pub name: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpConfig {
    pub request: String, // The HTTP template
}

/// Streaming configuration from YAML files
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamingConfigYaml {
    pub format: StreamingFormat,
    pub parser: SseParser,
    #[serde(default)]
    pub line_prefix: Option<String>,
    #[serde(default)]
    pub done_marker: Option<String>,
    #[serde(default)]
    pub events: Vec<StreamEventConfig>,
    #[serde(default)]
    pub extract: HashMap<String, String>,
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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pricing {
    #[serde(default)]
    pub currency: Currency,
    pub input_per_1k_tokens: f64,
    pub output_per_1k_tokens: f64,
    #[serde(default)]
    pub cached_input_per_1k_tokens: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
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

pub struct ConfigLoader {
    config_dir: PathBuf,
    services: HashMap<String, ServiceConfig>,
    models: HashMap<String, ModelConfig>,
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
            config_dir,
            services: HashMap::new(),
            models: HashMap::new(),
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

        for entry in WalkDir::new(&services_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "yaml" || ext == "yml"))
        {
            let path = entry.path();
            let content = fs::read_to_string(path)?;
            let service_config: ServiceConfig = serde_yaml::from_str(&content)
                .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                    format!("Failed to parse service config '{}': {}", path.display(), e)
                )))?;
            
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
}