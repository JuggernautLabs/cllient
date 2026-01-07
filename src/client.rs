use async_trait::async_trait;
use reqwest::{Client, Method};
use serde_json::Value;
use std::collections::HashMap;

use crate::config::{ConfigLoader, ModelConfig, ServiceConfig};
use crate::embedded_config::EmbeddedConfigLoader;
use crate::error::{ClientError, ConfigError, Result};
use crate::message_format::MessageFormatter;
use crate::streaming::StreamProcessor;
use crate::template::{HttpRequest, TemplateProcessor};
use crate::types::{CompletionRequest, CompletionResponse, Usage};

/// Trait for configuration providers (file-based or embedded)
pub trait ConfigProvider {
    fn get_service(&self, name: &str) -> Result<&ServiceConfig>;
    fn get_model(&self, id: &str) -> Result<&ModelConfig>;
    fn list_services(&self) -> Vec<&str>;
    fn list_models(&self) -> Vec<&str>;
    fn get_model_with_service(&self, model_id: &str) -> Result<(&ModelConfig, &ServiceConfig)>;
}

/// Implements ConfigProvider by delegating to inherent methods with matching signatures.
macro_rules! impl_config_provider {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ConfigProvider for $ty {
                fn get_service(&self, name: &str) -> Result<&ServiceConfig> { self.get_service(name) }
                fn get_model(&self, id: &str) -> Result<&ModelConfig> { self.get_model(id) }
                fn list_services(&self) -> Vec<&str> { self.list_services() }
                fn list_models(&self) -> Vec<&str> { self.list_models() }
                fn get_model_with_service(&self, model_id: &str) -> Result<(&ModelConfig, &ServiceConfig)> {
                    self.get_model_with_service(model_id)
                }
            }
        )+
    };
}

impl_config_provider!(ConfigLoader, EmbeddedConfigLoader);

/// Core trait for low-level LLM client operations
#[async_trait]
pub trait LowLevelClient: Send + Sync {
    /// Send a raw completion request and get a complete response
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse>;
    
    /// Send a raw completion request and get a streaming response
    async fn complete_stream(&self, request: &CompletionRequest) -> Result<crate::streaming::Stream>;
}

/// HTTP-based client that implements LowLevelClient
pub struct HttpClient {
    model_config: ModelConfig,
    service_config: ServiceConfig,
    http_client: Client,
    template_processor: TemplateProcessor,
    stream_processor: StreamProcessor,
    message_formatter: MessageFormatter,
}

impl HttpClient {
    pub fn new(
        model_config: ModelConfig,
        service_config: ServiceConfig,
    ) -> Result<Self> {
        let http_client = Client::new();
        let template_processor = TemplateProcessor::new();
        let stream_processor = StreamProcessor::new(&service_config.streaming)?;

        // NEW: Initialize message formatter from config
        let message_formatter = Self::create_message_formatter(&service_config)?;

        Ok(Self {
            model_config,
            service_config,
            http_client,
            template_processor,
            stream_processor,
            message_formatter,
        })
    }

    /// Create a message formatter from service config (v1 or v2)
    fn create_message_formatter(service_config: &ServiceConfig) -> Result<MessageFormatter> {
        // Priority: v2 config first, then v1 auto-upgrade
        if let Some(ref message_format_config) = service_config.message_format {
            tracing::debug!(
                "Using v2 message format config: {}",
                message_format_config.name
            );
            MessageFormatter::new(message_format_config.clone())
        } else if let Some(ref message_builder) = service_config.message_builder {
            let format_name = message_builder.to_string();
            tracing::warn!(
                "Using deprecated v1 message_builder '{}', auto-upgrading to v2. \
                 Consider migrating to message_format config.",
                format_name
            );
            let upgraded_config = crate::message_format::upgrade_v1_to_v2(&format_name)?;
            MessageFormatter::new(upgraded_config)
        } else {
            Err(ClientError::Config(ConfigError::MissingField(
                "Either message_builder or message_format must be specified".to_string()
            )))
        }
    }

    pub fn from_model_id<T: ConfigProvider + ?Sized>(config_provider: &T, model_id: &str) -> Result<Self> {
        let (model_config, service_config) = config_provider.get_model_with_service(model_id)?;
        Self::new(model_config.clone(), service_config.clone())
    }

    fn build_template_variables(
        &self,
        request: &CompletionRequest,
    ) -> Result<HashMap<String, Value>> {
        let mut variables = HashMap::new();

        // Model ID
        variables.insert("model_id".to_string(), Value::String(self.model_config.model.id.clone()));

        // Build messages using MessageFormatter (v2)
        let messages = self.message_formatter.format_messages(&request.messages)?;
        variables.insert("messages".to_string(), messages);

        // System prompt (for services that use it separately like Anthropic)
        // Always set system variable to ensure template renders correctly
        let system_value = match &request.system_prompt {
            Some(system) => Value::String(system.clone()),
            None => Value::Null,  // Use null instead of omitting
        };
        variables.insert("system".to_string(), system_value);

        // Apply default parameters from model config  
        let defaults = &self.model_config.defaults;
        for (key, value) in defaults {
            // Convert serde_yaml::Value to serde_json::Value
            let json_value = serde_json::to_value(value)?;
            variables.insert(key.clone(), json_value);
        }

        // Override with provided parameters
        for (key, value) in &request.parameters {
            variables.insert(key.clone(), value.clone());
        }

        // Set streaming flag
        variables.insert("stream".to_string(), Value::Bool(request.stream));

        Ok(variables)
    }

    async fn send_http_request(&self, http_request: &HttpRequest) -> Result<reqwest::Response> {
        let method = Method::from_bytes(http_request.method.as_bytes())
            .map_err(|e| ClientError::Render(handlebars::RenderError::new(format!("Invalid HTTP method: {}", e))))?;

        let mut request_builder = self.http_client.request(method, &http_request.url);

        // Add headers
        for (key, value) in &http_request.headers {
            request_builder = request_builder.header(key, value);
        }

        // Add body if present
        if !http_request.body.is_empty() {
            request_builder = request_builder.body(http_request.body.clone());
        }

        let response = request_builder.send().await?;

        // Check if response is successful
        if !self.service_config.response.success_codes.contains(&response.status().as_u16()) {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ClientError::Render(handlebars::RenderError::new(format!("HTTP error: {}", error_text))));
        }

        Ok(response)
    }

    fn extract_response_data(&self, response_json: &Value) -> Result<CompletionResponse> {
        let extract_config = &self.service_config.response.extract;

        // Debug: Log the raw response structure
        tracing::debug!("Raw API response: {}", serde_json::to_string_pretty(response_json).unwrap_or_else(|_| "<unparseable>".to_string()));
        tracing::debug!("Attempting to extract content using path: '{}'", extract_config.content);
        
        // Log the top-level keys to help diagnose structure mismatches
        if let Some(obj) = response_json.as_object() {
            let keys: Vec<&String> = obj.keys().collect();
            tracing::debug!("Top-level response keys: {:?}", keys);
        }

        // Extract content using JSONPath
        let content = match self.extract_json_path(&extract_config.content, response_json) {
            Ok(value) => {
                tracing::debug!("Successfully extracted content value: {:?}", value);
                value.as_str().unwrap_or("").to_string()
            },
            Err(e) => {
                tracing::error!("Failed to extract content from response.");
                tracing::error!("Expected path: '{}'", extract_config.content);
                tracing::error!("Actual response structure: {}", serde_json::to_string_pretty(response_json).unwrap_or_else(|_| "<unparseable>".to_string()));
                return Err(e);
            }
        };

        // Extract optional fields
        let role = extract_config.role.as_ref()
            .and_then(|path| self.extract_json_path(path, response_json).ok())
            .and_then(|v| v.as_str().map(|s| s.to_string()));

        let finish_reason = extract_config.finish_reason.as_ref()
            .and_then(|path| self.extract_json_path(path, response_json).ok())
            .and_then(|v| v.as_str().map(|s| s.to_string()));

        // Extract usage information
        let usage = if let Some(usage_config) = &extract_config.usage {
            let input_tokens = self.extract_json_path(&usage_config.input, response_json)?
                .as_u64()
                .unwrap_or(0) as u32;

            let output_tokens = self.extract_json_path(&usage_config.output, response_json)?
                .as_u64()
                .unwrap_or(0) as u32;

            let total_tokens = usage_config.total.as_ref()
                .and_then(|path| self.extract_json_path(path, response_json).ok())
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);

            Some(Usage {
                input_tokens,
                output_tokens,
                total_tokens,
            })
        } else {
            None
        };

        Ok(CompletionResponse {
            content,
            role,
            finish_reason,
            usage,
            raw_response: response_json.clone(),
        })
    }

    fn extract_json_path(&self, path: &str, json: &Value) -> Result<Value> {
        // Enhanced JSONPath implementation with better debugging
        tracing::debug!("Extracting JSONPath: '{}'", path);
        
        // Handle the special case of "choices[0]" style paths
        if path.contains('[') && !path.starts_with('[') {
            return self.extract_complex_json_path(path, json);
        }
        
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = json;
        tracing::debug!("Path parts: {:?}", parts);

        for (i, part) in parts.iter().enumerate() {
            tracing::debug!("Processing part {}: '{}'", i, part);
            
            if part.starts_with('[') && part.ends_with(']') {
                // Array index
                let index_str = &part[1..part.len()-1];
                let index: usize = index_str.parse()
                    .map_err(|_| {
                        tracing::error!("Invalid array index '{}' in path '{}'", index_str, path);
                        ClientError::JsonPath(format!("Invalid array index: {}", index_str))
                    })?;
                
                tracing::debug!("Accessing array index: {}", index);
                if let Some(array) = current.as_array() {
                    tracing::debug!("Array length: {}", array.len());
                }
                
                current = current.get(index)
                    .ok_or_else(|| {
                        tracing::error!("Array index {} not found in path '{}'. Available indices: 0..{}", 
                                      index, path, current.as_array().map(|a| a.len()).unwrap_or(0));
                        tracing::error!("Current value type: {}", match current {
                            serde_json::Value::Array(_) => "Array",
                            serde_json::Value::Object(_) => "Object", 
                            serde_json::Value::String(_) => "String",
                            serde_json::Value::Number(_) => "Number",
                            serde_json::Value::Bool(_) => "Bool",
                            serde_json::Value::Null => "Null",
                        });
                        ClientError::JsonPath(format!("Array index {} not found", index))
                    })?;
            } else {
                // Object key
                tracing::debug!("Accessing object key: '{}'", part);
                if let Some(obj) = current.as_object() {
                    let available_keys: Vec<&String> = obj.keys().collect();
                    tracing::debug!("Available keys: {:?}", available_keys);
                }
                
                current = current.get(part)
                    .ok_or_else(|| {
                        let available_keys = if let Some(obj) = current.as_object() {
                            obj.keys().cloned().collect::<Vec<_>>()
                        } else {
                            vec![]
                        };
                        tracing::error!("Key '{}' not found in path '{}'. Available keys: {:?}", part, path, available_keys);
                        tracing::error!("Current value type: {}", match current {
                            serde_json::Value::Array(_) => "Array",
                            serde_json::Value::Object(_) => "Object", 
                            serde_json::Value::String(_) => "String",
                            serde_json::Value::Number(_) => "Number",
                            serde_json::Value::Bool(_) => "Bool",
                            serde_json::Value::Null => "Null",
                        });
                        ClientError::JsonPath(format!("Key '{}' not found", part))
                    })?;
            }
            
            tracing::debug!("After processing part '{}', current value type: {}", part, match current {
                serde_json::Value::Array(_) => "Array",
                serde_json::Value::Object(_) => "Object", 
                serde_json::Value::String(_) => "String",
                serde_json::Value::Number(_) => "Number",
                serde_json::Value::Bool(_) => "Bool",
                serde_json::Value::Null => "Null",
            });
        }

        tracing::debug!("Successfully extracted value: {:?}", current);
        Ok(current.clone())
    }
    
    fn extract_complex_json_path(&self, path: &str, json: &Value) -> Result<Value> {
        // Handle paths like "choices[0].message.content"
        tracing::debug!("Extracting complex JSONPath: '{}'", path);
        
        let mut current = json;
        let mut i = 0;
        let chars: Vec<char> = path.chars().collect();
        
        while i < chars.len() {
            // Find the next part (either before '.' or before '[')
            let mut part_end = i;
            while part_end < chars.len() && chars[part_end] != '.' && chars[part_end] != '[' {
                part_end += 1;
            }
            
            if part_end > i {
                // Extract object key
                let key: String = chars[i..part_end].iter().collect();
                tracing::debug!("Extracting object key: '{}'", key);
                
                current = current.get(&key)
                    .ok_or_else(|| {
                        let available_keys = if let Some(obj) = current.as_object() {
                            obj.keys().cloned().collect::<Vec<_>>()
                        } else {
                            vec![]
                        };
                        tracing::error!("Key '{}' not found in complex path '{}'. Available keys: {:?}", key, path, available_keys);
                        ClientError::JsonPath(format!("Key '{}' not found", key))
                    })?;
                
                i = part_end;
            }
            
            // Handle array access
            if i < chars.len() && chars[i] == '[' {
                i += 1; // Skip '['
                let mut index_end = i;
                while index_end < chars.len() && chars[index_end] != ']' {
                    index_end += 1;
                }
                
                if index_end >= chars.len() {
                    return Err(ClientError::JsonPath(format!("Unclosed array bracket in path '{}'", path)));
                }
                
                let index_str: String = chars[i..index_end].iter().collect();
                let index: usize = index_str.parse()
                    .map_err(|_| ClientError::JsonPath(format!("Invalid array index '{}' in path '{}'", index_str, path)))?;
                
                tracing::debug!("Extracting array index: {}", index);
                current = current.get(index)
                    .ok_or_else(|| {
                        tracing::error!("Array index {} not found in complex path '{}'", index, path);
                        ClientError::JsonPath(format!("Array index {} not found", index))
                    })?;
                
                i = index_end + 1; // Skip ']'
            }
            
            // Skip '.'
            if i < chars.len() && chars[i] == '.' {
                i += 1;
            }
        }
        
        tracing::debug!("Successfully extracted complex value: {:?}", current);
        Ok(current.clone())
    }
}

#[async_trait]
impl LowLevelClient for HttpClient {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let variables = self.build_template_variables(request)?;
        let http_request = self.template_processor.process_http_template(&self.service_config.http.request, &variables)?;
        
        let response = self.send_http_request(&http_request).await?;
        let response_json: Value = response.json().await?;
        
        self.extract_response_data(&response_json)
    }

    async fn complete_stream(&self, request: &CompletionRequest) -> Result<crate::streaming::Stream> {
        let mut stream_request = request.clone();
        stream_request.stream = true;
        
        let variables = self.build_template_variables(&stream_request)?;
        let http_request = self.template_processor.process_http_template(&self.service_config.http.request, &variables)?;
        
        let response = self.send_http_request(&http_request).await?;
        
        // Process the streaming response
        self.stream_processor.process_response(response).await
    }
}

/// Factory for creating clients
pub struct ClientFactory<T: ConfigProvider> {
    config_provider: T,
}

impl<T: ConfigProvider> ClientFactory<T> {
    pub fn new(config_provider: T) -> Self {
        Self { config_provider }
    }

    pub fn create_client(&self, model_id: &str) -> Result<HttpClient> {
        HttpClient::from_model_id(&self.config_provider, model_id)
    }

    pub fn list_available_models(&self) -> Vec<&str> {
        self.config_provider.list_models()
    }

    pub fn list_available_services(&self) -> Vec<&str> {
        self.config_provider.list_services()
    }

    pub fn list_families(&self) -> Vec<String> {
        use std::collections::HashSet;
        
        let mut families = HashSet::new();
        
        for model_id in self.config_provider.list_models() {
            if let Ok(model_config) = self.config_provider.get_model(model_id) {
                families.insert(model_config.model.family.clone());
            }
        }
        
        let mut family_list: Vec<String> = families.into_iter().collect();
        family_list.sort();
        family_list
    }

    pub fn list_models_in_family(&self, family: &str) -> Vec<&str> {
        self.config_provider.list_models()
            .into_iter()
            .filter(|model_id| {
                if let Ok(model_config) = self.config_provider.get_model(model_id) {
                    model_config.model.family == family
                } else {
                    false
                }
            })
            .collect()
    }
}

// Type aliases for convenience
pub type FileBasedClientFactory = ClientFactory<ConfigLoader>;
pub type EmbeddedClientFactory = ClientFactory<EmbeddedConfigLoader>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentBlock, ImageFormat, MessageContent};
    use serde_json::json;
    
    #[test]
    fn test_completion_request_builder() {
        let request = CompletionRequest::text("user", "Hello")
            .with_system_prompt("You are helpful".to_string())
            .with_parameter("temperature", 0.7)
            .with_parameter("max_tokens", 100)
            .with_streaming(true);
        
        assert_eq!(request.system_prompt, Some("You are helpful".to_string()));
        assert_eq!(request.parameters.get("temperature").unwrap(), &json!(0.7));
        assert_eq!(request.parameters.get("max_tokens").unwrap(), &json!(100));
        assert!(request.stream);
    }
    
    #[test]
    fn test_multimodal_request() {
        let image_data = vec![0xFF, 0xD8, 0xFF]; // Fake JPEG header
        
        let request = CompletionRequest::multimodal("user", vec![
            ContentBlock::text("What's in this image?"),
            ContentBlock::image(image_data, ImageFormat::Jpeg),
        ]);
        
        match &request.messages[0] {
            MessageContent::Multimodal { role, content } => {
                assert_eq!(role, "user");
                assert_eq!(content.len(), 2);
                
                match &content[0] {
                    ContentBlock::Text(text) => assert_eq!(text, "What's in this image?"),
                    _ => panic!("Expected text block"),
                }
                
                match &content[1] {
                    ContentBlock::Binary { mime_type, .. } => assert_eq!(mime_type, "image/jpeg"),
                    _ => panic!("Expected binary block"),
                }
            },
            _ => panic!("Expected multimodal message"),
        }
    }
}