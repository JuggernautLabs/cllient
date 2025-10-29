use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use walkdir::WalkDir;
use crate::error::{ClientError, ConfigError, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub service: ServiceInfo,
    pub http: HttpConfig,
    #[serde(default)]
    pub optional: HashMap<String, String>,
    pub streaming: StreamingConfig,
    pub response: ResponseConfig,
    pub message_builder: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamingConfig {
    pub format: String,
    pub parser: String,
    #[serde(default)]
    pub line_prefix: Option<String>,
    #[serde(default)]
    pub done_marker: Option<String>,
    #[serde(default)]
    pub events: Vec<StreamEvent>,
    #[serde(default)]
    pub extract: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamEvent {
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
    pub currency: String,
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