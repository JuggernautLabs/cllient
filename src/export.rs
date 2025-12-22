//! Registry export types for RPC/serialization boundaries
//!
//! This module provides serializable types for exposing the full registry
//! state through RPC interfaces without requiring N+1 calls.

use serde::{Deserialize, Serialize};
use crate::config::{
    Capabilities, Pricing, Constraints, RateLimits, VerificationStatus,
    MessageFormat,
};

/// Complete registry export - all services, families, and models in one structure
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl RegistryExport {
    /// Filter to only verified models
    pub fn verified_only(&self) -> Self {
        let models: Vec<ModelExport> = self.models
            .iter()
            .filter(|m| m.status == VerificationStatus::Verified)
            .cloned()
            .collect();

        let verified_count = models.len();

        Self {
            services: self.services.clone(),
            families: self.families.clone(),
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

    /// Filter models by family
    pub fn filter_by_family(&self, family: &str) -> Self {
        let models: Vec<ModelExport> = self.models
            .iter()
            .filter(|m| m.family == family)
            .cloned()
            .collect();

        let verified_count = models.iter()
            .filter(|m| m.status == VerificationStatus::Verified)
            .count();

        Self {
            services: self.services.clone(),
            families: vec![family.to_string()],
            models: models.clone(),
            stats: RegistryStats {
                service_count: self.stats.service_count,
                family_count: 1,
                model_count: models.len(),
                verified_count,
                unverified_count: models.len() - verified_count,
            },
        }
    }

    /// Filter models by service
    pub fn filter_by_service(&self, service: &str) -> Self {
        let models: Vec<ModelExport> = self.models
            .iter()
            .filter(|m| m.service == service)
            .cloned()
            .collect();

        let families: Vec<String> = models
            .iter()
            .map(|m| m.family.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let verified_count = models.iter()
            .filter(|m| m.status == VerificationStatus::Verified)
            .count();

        Self {
            services: self.services
                .iter()
                .filter(|s| s.name == service)
                .cloned()
                .collect(),
            families,
            models: models.clone(),
            stats: RegistryStats {
                service_count: 1,
                family_count: models.len(),
                model_count: models.len(),
                verified_count,
                unverified_count: models.len() - verified_count,
            },
        }
    }
}
