//! Configuration validation framework.
//!
//! Provides multi-level validation for service and model configurations:
//! - Schema: YAML structure and required fields
//! - CrossRef: Service ↔ model reference integrity
//! - Semantic: Logical constraints (pricing, token limits)
//! - Live: API connectivity testing

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::ConfigProvider;
use crate::config::{ModelConfig, ServiceConfig};
use crate::registry_index::RegistryIndex;

/// Validation level - determines what checks are performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLevel {
    /// YAML structure validation, required fields present
    Schema,
    /// Cross-reference validation (services ↔ models)
    CrossRef,
    /// Semantic validation (logical constraints)
    Semantic,
    /// Live API connectivity test
    Live,
}

impl std::fmt::Display for ValidationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationLevel::Schema => write!(f, "schema"),
            ValidationLevel::CrossRef => write!(f, "crossref"),
            ValidationLevel::Semantic => write!(f, "semantic"),
            ValidationLevel::Live => write!(f, "live"),
        }
    }
}

impl std::str::FromStr for ValidationLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "schema" => Ok(ValidationLevel::Schema),
            "crossref" | "cross-ref" | "cross_ref" => Ok(ValidationLevel::CrossRef),
            "semantic" => Ok(ValidationLevel::Semantic),
            "live" => Ok(ValidationLevel::Live),
            _ => Err(format!("Unknown validation level: {}", s)),
        }
    }
}

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Critical error - config is broken
    Error,
    /// Warning - config works but has issues
    Warning,
    /// Info - suggestion for improvement
    Info,
}

/// A single validation issue.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationIssue {
    /// Severity of the issue
    pub severity: Severity,
    /// Validation level that caught this
    pub level: ValidationLevel,
    /// Path to the config file (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// The model or service ID this relates to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    /// Specific field with the issue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Human-readable message describing the issue
    pub message: String,
    /// Suggested fix (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        };

        write!(f, "[{}] ", severity)?;

        if let Some(id) = &self.target_id {
            write!(f, "{}: ", id)?;
        }

        write!(f, "{}", self.message)?;

        if let Some(field) = &self.field {
            write!(f, " (field: {})", field)?;
        }

        if let Some(suggestion) = &self.suggestion {
            write!(f, " -> {}", suggestion)?;
        }

        Ok(())
    }
}

/// Result of validating the registry or a single config.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationReport {
    /// Whether all checks passed (no errors)
    pub passed: bool,
    /// Total number of errors
    pub error_count: usize,
    /// Total number of warnings
    pub warning_count: usize,
    /// All issues found
    pub issues: Vec<ValidationIssue>,
    /// Which validation levels were run
    pub levels_checked: Vec<ValidationLevel>,
}

impl ValidationReport {
    /// Create an empty (passing) report.
    pub fn new(levels: Vec<ValidationLevel>) -> Self {
        Self {
            passed: true,
            error_count: 0,
            warning_count: 0,
            issues: Vec::new(),
            levels_checked: levels,
        }
    }

    /// Add an issue to the report.
    pub fn add_issue(&mut self, issue: ValidationIssue) {
        match issue.severity {
            Severity::Error => {
                self.error_count += 1;
                self.passed = false;
            }
            Severity::Warning => {
                self.warning_count += 1;
            }
            Severity::Info => {}
        }
        self.issues.push(issue);
    }

    /// Get only errors.
    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }

    /// Get only warnings.
    pub fn warnings(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.severity == Severity::Warning)
    }

    /// Merge another report into this one.
    pub fn merge(&mut self, other: ValidationReport) {
        self.error_count += other.error_count;
        self.warning_count += other.warning_count;
        self.passed = self.passed && other.passed;
        self.issues.extend(other.issues);
        for level in other.levels_checked {
            if !self.levels_checked.contains(&level) {
                self.levels_checked.push(level);
            }
        }
    }
}

impl std::fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.passed {
            writeln!(f, "Validation PASSED")?;
        } else {
            writeln!(f, "Validation FAILED")?;
        }

        writeln!(f, "  Errors: {}, Warnings: {}", self.error_count, self.warning_count)?;

        if !self.issues.is_empty() {
            writeln!(f, "\nIssues:")?;
            for issue in &self.issues {
                writeln!(f, "  {}", issue)?;
            }
        }

        Ok(())
    }
}

/// Validator for registry configurations.
pub struct ConfigValidator<'a, P: ConfigProvider + ?Sized> {
    provider: &'a P,
    index: &'a RegistryIndex,
}

impl<'a, P: ConfigProvider + ?Sized> ConfigValidator<'a, P> {
    /// Create a new validator.
    pub fn new(provider: &'a P, index: &'a RegistryIndex) -> Self {
        Self { provider, index }
    }

    /// Run validation at the specified levels.
    pub fn validate(&self, levels: &[ValidationLevel]) -> ValidationReport {
        let mut report = ValidationReport::new(levels.to_vec());

        for level in levels {
            match level {
                ValidationLevel::Schema => self.validate_schema(&mut report),
                ValidationLevel::CrossRef => self.validate_crossref(&mut report),
                ValidationLevel::Semantic => self.validate_semantic(&mut report),
                ValidationLevel::Live => {
                    // Live validation requires async - skip in sync context
                    report.add_issue(ValidationIssue {
                        severity: Severity::Info,
                        level: ValidationLevel::Live,
                        path: None,
                        target_id: None,
                        field: None,
                        message: "Live validation requires async context, use validate_live_async()".to_string(),
                        suggestion: None,
                    });
                }
            }
        }

        report
    }

    /// Validate a single model.
    pub fn validate_model(&self, model_id: &str, levels: &[ValidationLevel]) -> ValidationReport {
        let mut report = ValidationReport::new(levels.to_vec());

        let model = match self.provider.get_model(model_id) {
            Ok(m) => m,
            Err(_) => {
                report.add_issue(ValidationIssue {
                    severity: Severity::Error,
                    level: ValidationLevel::Schema,
                    path: None,
                    target_id: Some(model_id.to_string()),
                    field: None,
                    message: format!("Model '{}' not found", model_id),
                    suggestion: None,
                });
                return report;
            }
        };

        for level in levels {
            match level {
                ValidationLevel::Schema => self.validate_model_schema(model, &mut report),
                ValidationLevel::CrossRef => self.validate_model_crossref(model, &mut report),
                ValidationLevel::Semantic => self.validate_model_semantic(model, &mut report),
                ValidationLevel::Live => {}
            }
        }

        report
    }

    /// Validate a single service.
    pub fn validate_service(&self, service_name: &str, levels: &[ValidationLevel]) -> ValidationReport {
        let mut report = ValidationReport::new(levels.to_vec());

        let service = match self.provider.get_service(service_name) {
            Ok(s) => s,
            Err(_) => {
                report.add_issue(ValidationIssue {
                    severity: Severity::Error,
                    level: ValidationLevel::Schema,
                    path: None,
                    target_id: Some(service_name.to_string()),
                    field: None,
                    message: format!("Service '{}' not found", service_name),
                    suggestion: None,
                });
                return report;
            }
        };

        for level in levels {
            match level {
                ValidationLevel::Schema => self.validate_service_schema(service, service_name, &mut report),
                ValidationLevel::CrossRef => self.validate_service_crossref(service_name, &mut report),
                ValidationLevel::Semantic => self.validate_service_semantic(service, service_name, &mut report),
                ValidationLevel::Live => {}
            }
        }

        report
    }

    // === Schema Validation ===

    fn validate_schema(&self, report: &mut ValidationReport) {
        // Validate all services
        for service_name in self.provider.list_services() {
            if let Ok(service) = self.provider.get_service(service_name) {
                self.validate_service_schema(service, service_name, report);
            }
        }

        // Validate all models
        for model_id in self.provider.list_models() {
            if let Ok(model) = self.provider.get_model(model_id) {
                self.validate_model_schema(model, report);
            }
        }
    }

    fn validate_service_schema(&self, service: &ServiceConfig, name: &str, report: &mut ValidationReport) {
        // Check required fields
        if service.service.name.is_empty() {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Schema,
                path: None,
                target_id: Some(name.to_string()),
                field: Some("service.name".to_string()),
                message: "Service name is empty".to_string(),
                suggestion: Some("Add a name to the service config".to_string()),
            });
        }

        if service.service.base_url.is_empty() {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Schema,
                path: None,
                target_id: Some(name.to_string()),
                field: Some("service.base_url".to_string()),
                message: "Service base_url is empty".to_string(),
                suggestion: Some("Add a base_url to the service config".to_string()),
            });
        }

        if service.http.request.is_empty() {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Schema,
                path: None,
                target_id: Some(name.to_string()),
                field: Some("http.request".to_string()),
                message: "HTTP request template is empty".to_string(),
                suggestion: Some("Add an HTTP request template".to_string()),
            });
        }
    }

    fn validate_model_schema(&self, model: &ModelConfig, report: &mut ValidationReport) {
        let model_id = &model.model.id;

        if model.model.id.is_empty() {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Schema,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("model.id".to_string()),
                message: "Model ID is empty".to_string(),
                suggestion: None,
            });
        }

        if model.model.family.is_empty() {
            report.add_issue(ValidationIssue {
                severity: Severity::Warning,
                level: ValidationLevel::Schema,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("model.family".to_string()),
                message: "Model family is empty".to_string(),
                suggestion: Some("Add a family name for grouping".to_string()),
            });
        }

        if model.model.service.is_empty() {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Schema,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("model.service".to_string()),
                message: "Model service reference is empty".to_string(),
                suggestion: Some("Specify which service this model uses".to_string()),
            });
        }
    }

    // === CrossRef Validation ===

    fn validate_crossref(&self, report: &mut ValidationReport) {
        // Check for broken model → service references
        for broken_ref in self.index.broken_refs() {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::CrossRef,
                path: broken_ref.config_path.clone(),
                target_id: Some(broken_ref.model_id.clone()),
                field: Some("model.service".to_string()),
                message: format!(
                    "Model '{}' references non-existent service '{}'",
                    broken_ref.model_id, broken_ref.referenced_service
                ),
                suggestion: Some(format!(
                    "Create service config for '{}' or change model.service",
                    broken_ref.referenced_service
                )),
            });
        }

        // Check for orphan services (warning, not error)
        for orphan in self.index.orphan_services() {
            report.add_issue(ValidationIssue {
                severity: Severity::Warning,
                level: ValidationLevel::CrossRef,
                path: None,
                target_id: Some(orphan.clone()),
                field: None,
                message: format!("Service '{}' has no models using it", orphan),
                suggestion: Some("Consider removing unused service or adding models".to_string()),
            });
        }
    }

    fn validate_model_crossref(&self, model: &ModelConfig, report: &mut ValidationReport) {
        let service_name = &model.model.service;

        if self.provider.get_service(service_name).is_err() {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::CrossRef,
                path: None,
                target_id: Some(model.model.id.clone()),
                field: Some("model.service".to_string()),
                message: format!("References non-existent service '{}'", service_name),
                suggestion: Some(format!("Create service config for '{}'", service_name)),
            });
        }
    }

    fn validate_service_crossref(&self, service_name: &str, report: &mut ValidationReport) {
        if !self.index.service_has_models(service_name) {
            report.add_issue(ValidationIssue {
                severity: Severity::Warning,
                level: ValidationLevel::CrossRef,
                path: None,
                target_id: Some(service_name.to_string()),
                field: None,
                message: "Service has no models".to_string(),
                suggestion: Some("Add models that use this service".to_string()),
            });
        }
    }

    // === Semantic Validation ===

    fn validate_semantic(&self, report: &mut ValidationReport) {
        for model_id in self.provider.list_models() {
            if let Ok(model) = self.provider.get_model(model_id) {
                self.validate_model_semantic(model, report);
            }
        }

        for service_name in self.provider.list_services() {
            if let Ok(service) = self.provider.get_service(service_name) {
                self.validate_service_semantic(service, service_name, report);
            }
        }
    }

    fn validate_model_semantic(&self, model: &ModelConfig, report: &mut ValidationReport) {
        let model_id = &model.model.id;

        // Check pricing is non-negative
        if model.pricing.input_per_1k_tokens < 0.0 {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Semantic,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("pricing.input_per_1k_tokens".to_string()),
                message: "Input pricing is negative".to_string(),
                suggestion: Some("Set to 0 or positive value".to_string()),
            });
        }

        if model.pricing.output_per_1k_tokens < 0.0 {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Semantic,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("pricing.output_per_1k_tokens".to_string()),
                message: "Output pricing is negative".to_string(),
                suggestion: Some("Set to 0 or positive value".to_string()),
            });
        }

        // Check token limits make sense
        if model.capabilities.max_output_tokens > model.capabilities.context_window {
            report.add_issue(ValidationIssue {
                severity: Severity::Warning,
                level: ValidationLevel::Semantic,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("capabilities".to_string()),
                message: format!(
                    "max_output_tokens ({}) exceeds context_window ({})",
                    model.capabilities.max_output_tokens, model.capabilities.context_window
                ),
                suggestion: Some("max_output_tokens should not exceed context_window".to_string()),
            });
        }

        // Check context window is reasonable
        if model.capabilities.context_window == 0 {
            report.add_issue(ValidationIssue {
                severity: Severity::Error,
                level: ValidationLevel::Semantic,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("capabilities.context_window".to_string()),
                message: "Context window is 0".to_string(),
                suggestion: Some("Set a valid context window size".to_string()),
            });
        }

        // Check max_output_tokens is set
        if model.capabilities.max_output_tokens == 0 {
            report.add_issue(ValidationIssue {
                severity: Severity::Warning,
                level: ValidationLevel::Semantic,
                path: None,
                target_id: Some(model_id.clone()),
                field: Some("capabilities.max_output_tokens".to_string()),
                message: "max_output_tokens is 0".to_string(),
                suggestion: Some("Set a valid max output tokens value".to_string()),
            });
        }
    }

    fn validate_service_semantic(&self, service: &ServiceConfig, name: &str, report: &mut ValidationReport) {
        // Check base_url is valid URL format
        if !service.service.base_url.starts_with("http://") && !service.service.base_url.starts_with("https://") {
            report.add_issue(ValidationIssue {
                severity: Severity::Warning,
                level: ValidationLevel::Semantic,
                path: None,
                target_id: Some(name.to_string()),
                field: Some("service.base_url".to_string()),
                message: "base_url doesn't start with http:// or https://".to_string(),
                suggestion: Some("Use a full URL starting with https://".to_string()),
            });
        }

        // Check rate limits are reasonable if set
        if let Some(rpm) = service.rate_limits.requests_per_minute {
            if rpm == 0 {
                report.add_issue(ValidationIssue {
                    severity: Severity::Warning,
                    level: ValidationLevel::Semantic,
                    path: None,
                    target_id: Some(name.to_string()),
                    field: Some("rate_limits.requests_per_minute".to_string()),
                    message: "requests_per_minute is 0".to_string(),
                    suggestion: Some("Remove rate limit or set a positive value".to_string()),
                });
            }
        }
    }
}

/// Quick validation check - returns true if no errors.
pub fn is_valid<P: ConfigProvider + ?Sized>(provider: &P, index: &RegistryIndex) -> bool {
    let validator = ConfigValidator::new(provider, index);
    let report = validator.validate(&[
        ValidationLevel::Schema,
        ValidationLevel::CrossRef,
        ValidationLevel::Semantic,
    ]);
    report.passed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedded_config::EmbeddedConfigLoader;

    #[test]
    fn test_validation_levels_parse() {
        assert_eq!("schema".parse::<ValidationLevel>().unwrap(), ValidationLevel::Schema);
        assert_eq!("crossref".parse::<ValidationLevel>().unwrap(), ValidationLevel::CrossRef);
        assert_eq!("cross-ref".parse::<ValidationLevel>().unwrap(), ValidationLevel::CrossRef);
        assert_eq!("semantic".parse::<ValidationLevel>().unwrap(), ValidationLevel::Semantic);
        assert_eq!("live".parse::<ValidationLevel>().unwrap(), ValidationLevel::Live);
    }

    #[test]
    fn test_validation_report() {
        let mut report = ValidationReport::new(vec![ValidationLevel::Schema]);
        assert!(report.passed);
        assert_eq!(report.error_count, 0);

        report.add_issue(ValidationIssue {
            severity: Severity::Warning,
            level: ValidationLevel::Schema,
            path: None,
            target_id: None,
            field: None,
            message: "Test warning".to_string(),
            suggestion: None,
        });
        assert!(report.passed); // Warnings don't fail

        report.add_issue(ValidationIssue {
            severity: Severity::Error,
            level: ValidationLevel::Schema,
            path: None,
            target_id: None,
            field: None,
            message: "Test error".to_string(),
            suggestion: None,
        });
        assert!(!report.passed); // Errors fail
        assert_eq!(report.error_count, 1);
        assert_eq!(report.warning_count, 1);
    }

    #[test]
    fn test_embedded_config_validation() {
        let loader = EmbeddedConfigLoader::new().expect("Failed to load configs");
        let index = RegistryIndex::build(&loader);
        let validator = ConfigValidator::new(&loader, &index);

        let report = validator.validate(&[ValidationLevel::Schema, ValidationLevel::CrossRef]);

        // Should have some results
        println!("{}", report);

        // The embedded configs should be mostly valid
        // (there may be warnings for orphan services, etc.)
    }
}
