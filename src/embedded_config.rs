use rust_embed::RustEmbed;
use std::collections::HashMap;
use crate::config::{ServiceConfig, ModelConfig};
use crate::error::{ClientError, ConfigError, Result};

#[derive(RustEmbed)]
#[folder = "config/"]
struct ConfigFiles;

pub struct EmbeddedConfigLoader {
    services: HashMap<String, ServiceConfig>,
    models: HashMap<String, ModelConfig>,
}

impl EmbeddedConfigLoader {
    pub fn new() -> Result<Self> {
        let mut loader = Self {
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
        // Load all files in the service/ directory
        for file_path in ConfigFiles::iter() {
            if file_path.starts_with("service/") && file_path.ends_with(".yaml") {
                let content = ConfigFiles::get(&file_path)
                    .ok_or_else(|| ClientError::Config(ConfigError::InvalidPath(
                        format!("Could not read embedded service file: {}", file_path)
                    )))?;
                
                let content_str = std::str::from_utf8(&content.data)
                    .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                        format!("Invalid UTF-8 in service file '{}': {}", file_path, e)
                    )))?;

                let service_config: ServiceConfig = serde_yaml::from_str(content_str)
                    .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                        format!("Failed to parse service config '{}': {}", file_path, e)
                    )))?;

                // Validate message format configuration
                service_config.validate_message_format()?;

                // Extract service name from path (e.g., "service/openai.yaml" -> "openai")
                let service_name = file_path
                    .strip_prefix("service/")
                    .and_then(|s| s.strip_suffix(".yaml"))
                    .ok_or_else(|| ClientError::Config(ConfigError::InvalidPath(
                        format!("Invalid service file path: {}", file_path)
                    )))?;

                self.services.insert(service_name.to_string(), service_config);
            }
        }

        Ok(())
    }

    fn load_models(&mut self) -> Result<()> {
        // Load all files in the family/ directory
        for file_path in ConfigFiles::iter() {
            if file_path.starts_with("family/") && file_path.ends_with(".yaml") {
                let content = ConfigFiles::get(&file_path)
                    .ok_or_else(|| ClientError::Config(ConfigError::InvalidPath(
                        format!("Could not read embedded model file: {}", file_path)
                    )))?;
                
                let content_str = std::str::from_utf8(&content.data)
                    .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                        format!("Invalid UTF-8 in model file '{}': {}", file_path, e)
                    )))?;

                let model_config: ModelConfig = serde_yaml::from_str(content_str)
                    .map_err(|e| ClientError::Config(ConfigError::InvalidYaml(
                        format!("Failed to parse model config '{}': {}", file_path, e)
                    )))?;
                
                // Use the model ID from the config, not the filename
                let model_id = model_config.model.id.clone();
                self.models.insert(model_id, model_config);
            }
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

impl Default for EmbeddedConfigLoader {
    fn default() -> Self {
        Self::new().expect("Failed to load embedded configs")
    }
}