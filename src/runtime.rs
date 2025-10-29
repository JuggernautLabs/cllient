use std::sync::Arc;
use std::collections::HashSet;
use regex::Regex;
use crate::client::ConfigProvider;
use crate::embedded_config::EmbeddedConfigLoader;
use crate::config::ModelConfig;
use crate::error::{ClientError, Result};
use crate::types::RequestBuilder;

/// Main runtime object that contains all model configurations and provides
/// a fluent API for model selection and request building
pub struct ModelRegistry {
    config_provider: Arc<dyn ConfigProvider + Send + Sync>,
}

impl ModelRegistry {
    /// Create a new runtime with embedded configurations
    pub fn new() -> Result<Self> {
        let embedded_loader = EmbeddedConfigLoader::new()?;
        Ok(Self {
            config_provider: Arc::new(embedded_loader),
        })
    }
    
    /// Create a runtime with a custom config provider
    pub fn with_provider<T: ConfigProvider + Send + Sync + 'static>(provider: T) -> Self {
        Self {
            config_provider: Arc::new(provider),
        }
    }
    
    /// Create a request builder for a specific model ID
    pub fn from_id(&self, model_id: &str) -> Result<RequestBuilder> {
        // Validate that the model exists
        let _ = self.config_provider.get_model(model_id)?;
        
        RequestBuilder::new(model_id.to_string(), Arc::clone(&self.config_provider))
    }
    
    /// Find the cheapest model matching a pattern and create a request builder
    pub fn use_cheapest(&self, pattern: &str) -> Result<RequestBuilder> {
        let model_id = self.find_cheapest_model(pattern)?;
        self.from_id(&model_id)
    }
    
    /// Find the fastest model (lowest latency/highest throughput) matching a pattern
    pub fn use_fastest(&self, pattern: &str) -> Result<RequestBuilder> {
        let model_id = self.find_fastest_model(pattern)?;
        self.from_id(&model_id)
    }
    
    /// Find the best quality model matching a pattern
    pub fn use_best_quality(&self, pattern: &str) -> Result<RequestBuilder> {
        let model_id = self.find_best_quality_model(pattern)?;
        self.from_id(&model_id)
    }
    
    /// List all available models
    pub fn list_models(&self) -> Vec<&str> {
        self.config_provider.list_models()
    }
    
    /// List models matching a pattern
    pub fn list_models_matching(&self, pattern: &str) -> Result<Vec<&str>> {
        let regex = self.pattern_to_regex(pattern)?;
        let models = self.config_provider.list_models();
        
        Ok(models.into_iter()
            .filter(|model_id| regex.is_match(model_id))
            .collect())
    }
    
    /// Get model information
    pub fn get_model_info(&self, model_id: &str) -> Result<&ModelConfig> {
        self.config_provider.get_model(model_id)
    }
    
    /// List all available model families
    pub fn list_families(&self) -> Vec<String> {
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
    
    /// List models in a specific family
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
    
    // Private helper methods
    
    fn find_cheapest_model(&self, pattern: &str) -> Result<String> {
        let matching_models = self.list_models_matching(pattern)?;
        
        if matching_models.is_empty() {
            return Err(ClientError::Config(crate::error::ConfigError::ModelNotFound(
                format!("No models found matching pattern: {}", pattern)
            )));
        }
        
        let mut cheapest_model = None;
        let mut lowest_cost = f64::MAX;
        
        for model_id in matching_models {
            if let Ok(model_config) = self.config_provider.get_model(model_id) {
                // Calculate cost as average of input + output (weighted for typical usage)
                // Assume 70% input, 30% output for typical usage
                let avg_cost = (model_config.pricing.input_per_1k_tokens * 0.7) + 
                              (model_config.pricing.output_per_1k_tokens * 0.3);
                
                if avg_cost < lowest_cost {
                    lowest_cost = avg_cost;
                    cheapest_model = Some(model_id.to_string());
                }
            }
        }
        
        cheapest_model.ok_or_else(|| ClientError::Config(crate::error::ConfigError::ModelNotFound(
            format!("No valid models found matching pattern: {}", pattern)
        )))
    }
    
    fn find_fastest_model(&self, pattern: &str) -> Result<String> {
        let matching_models = self.list_models_matching(pattern)?;
        
        if matching_models.is_empty() {
            return Err(ClientError::Config(crate::error::ConfigError::ModelNotFound(
                format!("No models found matching pattern: {}", pattern)
            )));
        }
        
        // For now, prioritize by model variant (haiku > sonnet > opus for Claude)
        // In the future, this could use actual latency measurements
        let speed_order = ["haiku", "sonnet", "opus", "mini", "turbo"];
        
        for variant in &speed_order {
            for model_id in &matching_models {
                if model_id.contains(variant) {
                    return Ok(model_id.to_string());
                }
            }
        }
        
        // If no speed indicators found, return the first match
        Ok(matching_models[0].to_string())
    }
    
    fn find_best_quality_model(&self, pattern: &str) -> Result<String> {
        let matching_models = self.list_models_matching(pattern)?;
        
        if matching_models.is_empty() {
            return Err(ClientError::Config(crate::error::ConfigError::ModelNotFound(
                format!("No models found matching pattern: {}", pattern)
            )));
        }
        
        // Prioritize by model variant (opus > sonnet > haiku for Claude)
        let quality_order = ["opus", "sonnet", "haiku", "turbo", "mini"];
        
        for variant in &quality_order {
            for model_id in &matching_models {
                if model_id.contains(variant) {
                    return Ok(model_id.to_string());
                }
            }
        }
        
        // If no quality indicators found, return the first match
        Ok(matching_models[0].to_string())
    }
    
    fn pattern_to_regex(&self, pattern: &str) -> Result<Regex> {
        // Convert glob-style pattern to regex
        let escaped = regex::escape(pattern);
        let regex_pattern = escaped.replace(r"\*", ".*").replace(r"\?", ".");
        let regex_pattern = format!("^{}$", regex_pattern);
        
        Regex::new(&regex_pattern).map_err(|e| ClientError::Config(
            crate::error::ConfigError::InvalidPath(format!("Invalid pattern: {}", e))
        ))
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new().expect("Failed to create default runtime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_runtime_creation() {
        let runtime = ModelRegistry::new().unwrap();
        let models = runtime.list_models();
        assert!(!models.is_empty());
    }
    
    #[test]
    fn test_model_pattern_matching() {
        let runtime = ModelRegistry::new().unwrap();
        
        // Test exact match
        let exact_matches = runtime.list_models_matching("claude-3-haiku-20240307").unwrap();
        assert!(exact_matches.contains(&"claude-3-haiku-20240307"));
        
        // Test wildcard pattern
        let haiku_matches = runtime.list_models_matching("claude-3-haiku-*").unwrap();
        assert!(!haiku_matches.is_empty());
        assert!(haiku_matches.iter().all(|m| m.contains("claude-3-haiku")));
    }
    
    #[test]
    fn test_cheapest_model_selection() {
        let runtime = ModelRegistry::new().unwrap();
        
        // This should find a claude-3-haiku variant as it's typically cheapest
        let cheapest = runtime.use_cheapest("claude-3-haiku-*").unwrap();
        // Just verify we can create the request builder without error
        assert!(cheapest.model_id.contains("claude-3-haiku"));
    }
    
    #[test]
    fn test_model_info_retrieval() {
        let runtime = ModelRegistry::new().unwrap();
        let models = runtime.list_models();
        
        if let Some(model_id) = models.first() {
            let model_info = runtime.get_model_info(model_id).unwrap();
            assert_eq!(&model_info.model.id, model_id);
        }
    }
}