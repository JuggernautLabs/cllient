//! Bidirectional index for service ↔ model relationships.
//!
//! This module provides eager indexing of the registry, built at initialization time.
//! It enables fast lookups in both directions:
//! - model → service (which service does this model use?)
//! - service → models (which models use this service?)

use std::collections::HashMap;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::ConfigProvider;
use crate::config::VerificationStatus;
use crate::error::{ClientError, ConfigError, Result};

/// A broken reference where a model references a non-existent service.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrokenReference {
    /// The model ID that has the broken reference
    pub model_id: String,
    /// The service name that was referenced but doesn't exist
    pub referenced_service: String,
    /// Optional path to the config file (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<PathBuf>,
}

impl std::fmt::Display for BrokenReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Model '{}' references non-existent service '{}'",
            self.model_id, self.referenced_service
        )?;
        if let Some(path) = &self.config_path {
            write!(f, " (in {})", path.display())?;
        }
        Ok(())
    }
}

/// Bidirectional index for the model registry.
///
/// This structure is built once at registry initialization and provides
/// O(1) lookups for common queries.
#[derive(Debug, Clone, Default)]
pub struct RegistryIndex {
    /// Forward mapping: model_id → service_name
    model_to_service: HashMap<String, String>,

    /// Reverse mapping: service_name → list of model_ids
    service_to_models: HashMap<String, Vec<String>>,

    /// Family grouping: family_name → list of model_ids
    family_to_models: HashMap<String, Vec<String>>,

    /// Model status grouping: status → list of model_ids
    status_to_models: HashMap<VerificationStatus, Vec<String>>,

    /// Service status grouping: status → list of service_names
    service_status_to_services: HashMap<VerificationStatus, Vec<String>>,

    /// Service name → verification status
    service_statuses: HashMap<String, VerificationStatus>,

    /// Services that have no models using them
    orphan_services: Vec<String>,

    /// Models that reference non-existent services
    broken_refs: Vec<BrokenReference>,

    /// All known service names
    all_services: Vec<String>,

    /// All known model IDs
    all_models: Vec<String>,
}

impl RegistryIndex {
    /// Build the index from a config provider.
    ///
    /// This iterates through all services and models once to build
    /// all the lookup tables.
    pub fn build<P: ConfigProvider>(provider: &P) -> Self {
        let mut index = Self::default();

        // Collect all services and their statuses
        let services: Vec<String> = provider.list_services().iter().map(|s| s.to_string()).collect();
        index.all_services = services.clone();

        // Initialize service_to_models with empty vecs and track service statuses
        for service_name in &services {
            index.service_to_models.insert(service_name.clone(), Vec::new());

            // Get service status
            if let Ok(service_config) = provider.get_service(service_name) {
                let status = service_config.service.status.clone();
                index.service_statuses.insert(service_name.clone(), status.clone());
                index
                    .service_status_to_services
                    .entry(status)
                    .or_default()
                    .push(service_name.clone());
            }
        }

        // Collect all models and build mappings
        for model_id in provider.list_models() {
            index.all_models.push(model_id.to_string());

            if let Ok(model_config) = provider.get_model(model_id) {
                let service_name = &model_config.model.service;
                let family = &model_config.model.family;
                let status = &model_config.model.status;

                // Forward mapping: model → service
                index.model_to_service.insert(model_id.to_string(), service_name.clone());

                // Reverse mapping: service → models
                if index.service_to_models.contains_key(service_name) {
                    index
                        .service_to_models
                        .get_mut(service_name)
                        .unwrap()
                        .push(model_id.to_string());
                } else {
                    // Service doesn't exist - this is a broken reference
                    index.broken_refs.push(BrokenReference {
                        model_id: model_id.to_string(),
                        referenced_service: service_name.clone(),
                        config_path: None,
                    });
                }

                // Family grouping
                index
                    .family_to_models
                    .entry(family.clone())
                    .or_default()
                    .push(model_id.to_string());

                // Status grouping
                index
                    .status_to_models
                    .entry(status.clone())
                    .or_default()
                    .push(model_id.to_string());
            }
        }

        // Find orphan services (services with no models)
        for (service, models) in &index.service_to_models {
            if models.is_empty() {
                index.orphan_services.push(service.clone());
            }
        }

        // Sort for consistent ordering
        index.orphan_services.sort();
        index.all_models.sort();
        for models in index.service_to_models.values_mut() {
            models.sort();
        }
        for models in index.family_to_models.values_mut() {
            models.sort();
        }
        for models in index.status_to_models.values_mut() {
            models.sort();
        }

        index
    }

    /// Check if the index has any broken references.
    pub fn has_broken_refs(&self) -> bool {
        !self.broken_refs.is_empty()
    }

    /// Get all broken references.
    pub fn broken_refs(&self) -> &[BrokenReference] {
        &self.broken_refs
    }

    /// Get all orphan services (services with no models).
    pub fn orphan_services(&self) -> &[String] {
        &self.orphan_services
    }

    /// Check if a service has any models.
    pub fn service_has_models(&self, service: &str) -> bool {
        self.service_to_models
            .get(service)
            .map(|models| !models.is_empty())
            .unwrap_or(false)
    }

    /// Get the service name for a model.
    pub fn service_for_model(&self, model_id: &str) -> Option<&str> {
        self.model_to_service.get(model_id).map(String::as_str)
    }

    /// Get all models for a service.
    pub fn models_for_service(&self, service: &str) -> &[String] {
        self.service_to_models
            .get(service)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get all models in a family.
    pub fn models_in_family(&self, family: &str) -> &[String] {
        self.family_to_models
            .get(family)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get all models with a specific status.
    pub fn models_with_status(&self, status: &VerificationStatus) -> &[String] {
        self.status_to_models
            .get(status)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get the verification status of a service.
    pub fn service_status(&self, service: &str) -> Option<&VerificationStatus> {
        self.service_statuses.get(service)
    }

    /// Get all services with a specific verification status.
    pub fn services_with_status(&self, status: &VerificationStatus) -> &[String] {
        self.service_status_to_services
            .get(status)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get verified services.
    pub fn verified_services(&self) -> &[String] {
        self.services_with_status(&VerificationStatus::Verified)
    }

    /// Get unverified services.
    pub fn unverified_services(&self) -> &[String] {
        self.services_with_status(&VerificationStatus::Unverified)
    }

    /// Get all known services.
    pub fn all_services(&self) -> &[String] {
        &self.all_services
    }

    /// Get all known models.
    pub fn all_models(&self) -> &[String] {
        &self.all_models
    }

    /// Get all families.
    pub fn all_families(&self) -> Vec<&str> {
        let mut families: Vec<&str> = self.family_to_models.keys().map(String::as_str).collect();
        families.sort();
        families
    }

    /// Get count of models per service.
    pub fn model_counts_by_service(&self) -> HashMap<&str, usize> {
        self.service_to_models
            .iter()
            .map(|(k, v)| (k.as_str(), v.len()))
            .collect()
    }

    /// Get count of models per status.
    pub fn model_counts_by_status(&self) -> HashMap<VerificationStatus, usize> {
        self.status_to_models
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect()
    }

    /// Validate that all cross-references are valid.
    /// Returns Ok(()) if valid, or an error with all broken references.
    pub fn validate_refs(&self) -> Result<()> {
        if self.broken_refs.is_empty() {
            Ok(())
        } else {
            let messages: Vec<String> = self.broken_refs.iter().map(|r| r.to_string()).collect();
            Err(ClientError::Config(ConfigError::BrokenReferences(messages)))
        }
    }

    /// Get count of services per status.
    pub fn service_counts_by_status(&self) -> HashMap<VerificationStatus, usize> {
        self.service_status_to_services
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect()
    }

    /// Get summary statistics.
    pub fn stats(&self) -> IndexStats {
        let model_status_counts = self.model_counts_by_status();
        let service_status_counts = self.service_counts_by_status();
        IndexStats {
            total_services: self.all_services.len(),
            verified_services: service_status_counts.get(&VerificationStatus::Verified).copied().unwrap_or(0),
            unverified_services: service_status_counts.get(&VerificationStatus::Unverified).copied().unwrap_or(0),
            total_models: self.all_models.len(),
            verified_models: model_status_counts.get(&VerificationStatus::Verified).copied().unwrap_or(0),
            unverified_models: model_status_counts.get(&VerificationStatus::Unverified).copied().unwrap_or(0),
            total_families: self.family_to_models.len(),
            orphan_services: self.orphan_services.len(),
            broken_refs: self.broken_refs.len(),
        }
    }
}

/// Summary statistics for the registry index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexStats {
    pub total_services: usize,
    pub verified_services: usize,
    pub unverified_services: usize,
    pub total_models: usize,
    pub verified_models: usize,
    pub unverified_models: usize,
    pub total_families: usize,
    pub orphan_services: usize,
    pub broken_refs: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests will use the embedded config loader
    #[test]
    fn test_index_build() {
        use crate::embedded_config::EmbeddedConfigLoader;

        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);

        // Should have services and models
        assert!(!index.all_services().is_empty());
        assert!(!index.all_models().is_empty());

        // Check that anthropic service has claude models
        let anthropic_models = index.models_for_service("anthropic");
        assert!(!anthropic_models.is_empty());
        assert!(anthropic_models.iter().any(|m| m.contains("claude")));
    }

    #[test]
    fn test_bidirectional_lookup() {
        use crate::embedded_config::EmbeddedConfigLoader;

        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);

        // For any model, we should be able to look up its service
        // and then find the model in that service's model list
        for model_id in index.all_models().iter().take(10) {
            if let Some(service) = index.service_for_model(model_id) {
                let models_for_service = index.models_for_service(service);
                assert!(
                    models_for_service.contains(model_id),
                    "Model {} should be in service {} model list",
                    model_id,
                    service
                );
            }
        }
    }
}
