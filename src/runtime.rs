use std::sync::Arc;
use std::collections::{HashSet, HashMap};
use regex::Regex;
use crate::client::ConfigProvider;
use crate::embedded_config::EmbeddedConfigLoader;
use crate::config::{ModelConfig, ServiceConfig, VerificationStatus};
use crate::error::{ClientError, Result};
use crate::types::RequestBuilder;
use crate::export::{
    RegistryExport, ServiceExport, ModelExport, RegistryStats, RateLimitsExport,
};
use crate::registry_index::{RegistryIndex, IndexStats, BrokenReference};
use crate::query::ModelQuery;
use crate::validation::{ConfigValidator, ValidationLevel, ValidationReport};

/// Main runtime object that contains all model configurations and provides
/// a fluent API for model selection and request building
pub struct ModelRegistry {
    config_provider: Arc<dyn ConfigProvider + Send + Sync>,
    index: RegistryIndex,
}

impl ModelRegistry {
    /// Create a new runtime with embedded configurations.
    ///
    /// This builds the bidirectional index at initialization time.
    /// If there are broken references (models referencing non-existent services),
    /// an error is returned.
    pub fn new() -> Result<Self> {
        let embedded_loader = EmbeddedConfigLoader::new()?;
        let index = RegistryIndex::build(&embedded_loader);

        // Validate cross-references at startup
        index.validate_refs()?;

        Ok(Self {
            config_provider: Arc::new(embedded_loader),
            index,
        })
    }

    /// Create a new runtime without strict validation.
    ///
    /// Unlike `new()`, this allows broken references and orphan services.
    /// Use this for debugging or when you need to inspect broken configs.
    pub fn new_permissive() -> Result<Self> {
        let embedded_loader = EmbeddedConfigLoader::new()?;
        let index = RegistryIndex::build(&embedded_loader);

        Ok(Self {
            config_provider: Arc::new(embedded_loader),
            index,
        })
    }

    /// Create a runtime with a custom config provider.
    ///
    /// This builds the bidirectional index from the provider.
    /// If there are broken references, an error is returned.
    pub fn with_provider<T: ConfigProvider + Send + Sync + 'static>(provider: T) -> Result<Self> {
        let index = RegistryIndex::build(&provider);

        // Validate cross-references
        index.validate_refs()?;

        Ok(Self {
            config_provider: Arc::new(provider),
            index,
        })
    }

    /// Create a runtime with a custom config provider without strict validation.
    pub fn with_provider_permissive<T: ConfigProvider + Send + Sync + 'static>(provider: T) -> Self {
        let index = RegistryIndex::build(&provider);
        Self {
            config_provider: Arc::new(provider),
            index,
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

    // === Service Access Methods (delegated from ConfigProvider) ===

    /// List all available services
    pub fn list_services(&self) -> Vec<&str> {
        self.config_provider.list_services()
    }

    /// Get service configuration by name
    pub fn get_service(&self, name: &str) -> Result<&ServiceConfig> {
        self.config_provider.get_service(name)
    }

    /// Get both model and service configuration in one call
    pub fn get_model_with_service(&self, model_id: &str) -> Result<(&ModelConfig, &ServiceConfig)> {
        self.config_provider.get_model_with_service(model_id)
    }

    // === Verification Status Methods ===

    /// List only verified models
    pub fn list_verified_models(&self) -> Vec<&str> {
        self.config_provider.list_models()
            .into_iter()
            .filter(|model_id| {
                if let Ok(cfg) = self.config_provider.get_model(model_id) {
                    cfg.model.status == VerificationStatus::Verified
                } else {
                    false
                }
            })
            .collect()
    }

    /// List models by verification status
    pub fn list_models_by_status(&self, status: VerificationStatus) -> Vec<&str> {
        self.config_provider.list_models()
            .into_iter()
            .filter(|model_id| {
                if let Ok(cfg) = self.config_provider.get_model(model_id) {
                    cfg.model.status == status
                } else {
                    false
                }
            })
            .collect()
    }

    /// Check if a model is verified
    pub fn is_verified(&self, model_id: &str) -> bool {
        self.config_provider.get_model(model_id)
            .map(|cfg| cfg.model.status == VerificationStatus::Verified)
            .unwrap_or(false)
    }

    // === Registry Export ===

    /// Export the full registry as a serializable structure.
    ///
    /// This is useful for RPC boundaries where you need to expose
    /// the complete registry state in a single call.
    pub fn export(&self) -> RegistryExport {
        // Count models per service
        let mut service_model_counts: HashMap<String, usize> = HashMap::new();
        let mut verified_count = 0;
        let mut unverified_count = 0;

        // Build model exports
        let models: Vec<ModelExport> = self.config_provider.list_models()
            .into_iter()
            .filter_map(|model_id| {
                let cfg = self.config_provider.get_model(model_id).ok()?;

                // Track service usage
                *service_model_counts.entry(cfg.model.service.clone()).or_insert(0) += 1;

                // Track verification stats
                if cfg.model.status == VerificationStatus::Verified {
                    verified_count += 1;
                } else {
                    unverified_count += 1;
                }

                Some(ModelExport {
                    id: cfg.model.id.clone(),
                    family: cfg.model.family.clone(),
                    name: cfg.model.name.clone(),
                    service: cfg.model.service.clone(),
                    version: cfg.model.version.clone(),
                    variant: cfg.model.variant.clone(),
                    lab: cfg.model.lab.clone(),
                    status: cfg.model.status.clone(),
                    capabilities: cfg.capabilities.clone(),
                    pricing: cfg.pricing.clone(),
                    constraints: cfg.constraints.clone(),
                    use_cases: cfg.use_cases.clone(),
                })
            })
            .collect();

        // Build service exports
        let services: Vec<ServiceExport> = self.config_provider.list_services()
            .into_iter()
            .filter_map(|service_name| {
                let cfg = self.config_provider.get_service(service_name).ok()?;

                let rate_limits = if cfg.rate_limits.requests_per_minute.is_some()
                    || cfg.rate_limits.tokens_per_minute.is_some()
                    || cfg.rate_limits.concurrent_requests.is_some()
                {
                    Some(RateLimitsExport::from(&cfg.rate_limits))
                } else {
                    None
                };

                Some(ServiceExport {
                    name: service_name.to_string(),
                    base_url: cfg.service.base_url.clone(),
                    message_format: cfg.message_builder.clone(),
                    rate_limits,
                    model_count: *service_model_counts.get(service_name).unwrap_or(&0),
                })
            })
            .collect();

        let families = self.list_families();

        RegistryExport {
            stats: RegistryStats {
                service_count: services.len(),
                family_count: families.len(),
                model_count: models.len(),
                verified_count,
                unverified_count,
            },
            services,
            families,
            models,
        }
    }

    /// Export only verified models and their services
    pub fn export_verified(&self) -> RegistryExport {
        self.export().verified_only()
    }

    /// Export models for a specific service
    pub fn export_by_service(&self, service: &str) -> RegistryExport {
        self.export().filter_by_service(service)
    }

    /// Export models for a specific family
    pub fn export_by_family(&self, family: &str) -> RegistryExport {
        self.export().filter_by_family(family)
    }

    // === Query API ===

    /// Create a fluent query builder for filtering models.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let models = registry.query()
    ///     .service("openai")
    ///     .verified()
    ///     .with_vision()
    ///     .fuzzy("gpt turbo")
    ///     .list();
    /// ```
    pub fn query(&self) -> ModelQuery<'_, dyn ConfigProvider + Send + Sync> {
        ModelQuery::new(self.config_provider.as_ref(), &self.index)
    }

    // === Index Access ===

    /// Get the bidirectional registry index.
    pub fn index(&self) -> &RegistryIndex {
        &self.index
    }

    /// Get index statistics.
    pub fn index_stats(&self) -> IndexStats {
        self.index.stats()
    }

    /// Get all models for a specific service (fast indexed lookup).
    pub fn models_for_service(&self, service: &str) -> &[String] {
        self.index.models_for_service(service)
    }

    /// Get the service for a specific model (fast indexed lookup).
    pub fn service_for_model(&self, model_id: &str) -> Option<&str> {
        self.index.service_for_model(model_id)
    }

    /// Get all broken references (models referencing non-existent services).
    pub fn broken_refs(&self) -> &[BrokenReference] {
        self.index.broken_refs()
    }

    /// Get all orphan services (services with no models).
    pub fn orphan_services(&self) -> &[String] {
        self.index.orphan_services()
    }

    /// Check if the registry has any broken references.
    pub fn has_broken_refs(&self) -> bool {
        self.index.has_broken_refs()
    }

    // === Validation API ===

    /// Run validation at the specified levels.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let report = registry.validate(&[
    ///     ValidationLevel::Schema,
    ///     ValidationLevel::CrossRef,
    ///     ValidationLevel::Semantic,
    /// ]);
    /// if !report.passed {
    ///     for error in report.errors() {
    ///         eprintln!("{}", error);
    ///     }
    /// }
    /// ```
    pub fn validate(&self, levels: &[ValidationLevel]) -> ValidationReport {
        let validator = ConfigValidator::new(self.config_provider.as_ref(), &self.index);
        validator.validate(levels)
    }

    /// Validate a single model.
    pub fn validate_model(&self, model_id: &str, levels: &[ValidationLevel]) -> ValidationReport {
        let validator = ConfigValidator::new(self.config_provider.as_ref(), &self.index);
        validator.validate_model(model_id, levels)
    }

    /// Validate a single service.
    pub fn validate_service(&self, service_name: &str, levels: &[ValidationLevel]) -> ValidationReport {
        let validator = ConfigValidator::new(self.config_provider.as_ref(), &self.index);
        validator.validate_service(service_name, levels)
    }

    /// Quick check if the registry is valid (no errors at any level).
    pub fn is_valid(&self) -> bool {
        let report = self.validate(&[
            ValidationLevel::Schema,
            ValidationLevel::CrossRef,
            ValidationLevel::Semantic,
        ]);
        report.passed
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