//! Registry export types for RPC/serialization boundaries
//!
//! This module provides serializable types for exposing the full registry
//! state through RPC interfaces without requiring N+1 calls.

use crate::config::{
    Capabilities, Constraints, MessageFormat, Pricing, RateLimits, VerificationStatus,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Complete registry export - all services, families, and models in one structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegistryExport {
    /// All available services
    pub services: Vec<ServiceExport>,
    /// All model families (e.g., "claude", "gpt", "deepseek")
    pub families: Vec<String>,
    /// All available models
    pub models: Vec<ModelExport>,
    /// Summary statistics
    pub stats: RegistryStats,
}

/// Summary statistics about the registry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegistryStats {
    /// Total number of services
    pub service_count: usize,
    /// Total number of model families
    pub family_count: usize,
    /// Total number of models
    pub model_count: usize,
    /// Number of verified models
    pub verified_count: usize,
    /// Number of unverified models
    pub unverified_count: usize,
}

/// Serializable service information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceExport {
    /// Service identifier (e.g., "openai", "anthropic")
    pub name: String,
    /// Base URL for the API
    pub base_url: String,
    /// Message format type
    pub message_format: MessageFormat,
    /// Rate limits if configured
    pub rate_limits: Option<RateLimitsExport>,
    /// Number of models using this service
    pub model_count: usize,
}

/// Serializable rate limits
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitsExport {
    pub requests_per_minute: Option<u32>,
    pub tokens_per_minute: Option<u32>,
    pub concurrent_requests: Option<u32>,
}

impl From<&RateLimits> for RateLimitsExport {
    fn from(rl: &RateLimits) -> Self {
        Self {
            requests_per_minute: rl.requests_per_minute,
            tokens_per_minute: rl.tokens_per_minute,
            concurrent_requests: rl.concurrent_requests,
        }
    }
}

/// Serializable model information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelExport {
    /// Model identifier
    pub id: String,
    /// Model family (e.g., "claude", "gpt")
    pub family: String,
    /// Human-readable name
    pub name: String,
    /// Service this model uses
    pub service: String,
    /// Version string if available
    pub version: Option<String>,
    /// Variant (e.g., "haiku", "sonnet", "opus")
    pub variant: Option<String>,
    /// Lab/organization that created the model
    pub lab: Option<String>,
    /// Verification status
    pub status: VerificationStatus,
    /// Model capabilities
    pub capabilities: Capabilities,
    /// Pricing information
    pub pricing: Pricing,
    /// Model constraints
    pub constraints: Constraints,
    /// Use cases this model is suited for
    pub use_cases: Vec<String>,
}

impl From<&crate::config::ModelConfig> for ModelExport {
    fn from(cfg: &crate::config::ModelConfig) -> Self {
        Self {
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
        }
    }
}

impl RegistryExport {
    /// Filter to only verified models (borrowing version)
    pub fn verified_only(&self) -> Self {
        self.clone().into_verified_only()
    }

    /// Filter to only verified models (consuming version - more efficient)
    pub fn into_verified_only(self) -> Self {
        let models: Vec<ModelExport> = self
            .models
            .into_iter()
            .filter(|m| m.status == VerificationStatus::Verified)
            .collect();

        let verified_count = models.len();

        Self {
            services: self.services,
            families: self.families,
            models,
            stats: RegistryStats {
                service_count: self.stats.service_count,
                family_count: self.stats.family_count,
                model_count: verified_count,
                verified_count,
                unverified_count: 0,
            },
        }
    }

    /// Filter models by family (borrowing version)
    pub fn filter_by_family(&self, family: &str) -> Self {
        self.clone().into_filter_by_family(family)
    }

    /// Filter models by family (consuming version - more efficient)
    pub fn into_filter_by_family(self, family: &str) -> Self {
        let models: Vec<ModelExport> = self
            .models
            .into_iter()
            .filter(|m| m.family == family)
            .collect();

        let verified_count = models
            .iter()
            .filter(|m| m.status == VerificationStatus::Verified)
            .count();
        let model_count = models.len();

        Self {
            services: self.services,
            families: vec![family.to_string()],
            models,
            stats: RegistryStats {
                service_count: self.stats.service_count,
                family_count: 1,
                model_count,
                verified_count,
                unverified_count: model_count - verified_count,
            },
        }
    }

    /// Filter models by service (borrowing version)
    pub fn filter_by_service(&self, service: &str) -> Self {
        self.clone().into_filter_by_service(service)
    }

    /// Filter models by service (consuming version - more efficient)
    pub fn into_filter_by_service(self, service: &str) -> Self {
        let models: Vec<ModelExport> = self
            .models
            .into_iter()
            .filter(|m| m.service == service)
            .collect();

        // Deduplicate families in-place without HashSet allocation
        let mut families: Vec<&str> = models.iter().map(|m| m.family.as_str()).collect();
        families.sort_unstable();
        families.dedup();
        let families: Vec<String> = families.into_iter().map(String::from).collect();

        let verified_count = models
            .iter()
            .filter(|m| m.status == VerificationStatus::Verified)
            .count();
        let family_count = families.len();
        let model_count = models.len();

        Self {
            services: self
                .services
                .into_iter()
                .filter(|s| s.name == service)
                .collect(),
            families,
            models,
            stats: RegistryStats {
                service_count: 1,
                family_count,
                model_count,
                verified_count,
                unverified_count: model_count - verified_count,
            },
        }
    }
}
