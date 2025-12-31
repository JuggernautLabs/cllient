//! Per-method event types for streaming plugin responses
//!
//! Each plugin method has its own event enum to provide type-safe streaming.
//! These are only available with the `plugin` feature.

#![cfg(feature = "plugin")]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================================
// Completion Events
// ============================================================================

/// Events emitted during LLM completion streaming
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompletionEvent {
    /// Stream started
    Start,
    /// Content chunk received
    Content { text: String },
    /// Token usage information
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_tokens: Option<u32>,
    },
    /// Stream completed successfully
    Done {
        #[serde(skip_serializing_if = "Option::is_none")]
        finish_reason: Option<String>,
    },
    /// Error occurred
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
}

// ============================================================================
// Model Listing Events
// ============================================================================

/// Events emitted when listing models
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelEvent {
    /// Single model info
    Model(ModelInfo),
    /// Listing complete
    Done { count: usize },
}

/// Summary information about a model
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub family: String,
    pub service: String,
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub capabilities: CapabilitySummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PricingSummary>,
}

/// Summary of model capabilities
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CapabilitySummary {
    pub streaming: bool,
    pub vision: bool,
    pub functions: bool,
    pub json_mode: bool,
    pub multimodal: bool,
}

/// Summary of model pricing
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PricingSummary {
    pub input_per_1m_tokens: f64,
    pub output_per_1m_tokens: f64,
    pub currency: String,
}

// ============================================================================
// Service Listing Events
// ============================================================================

/// Events emitted when listing services
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServiceEvent {
    /// Single service info
    Service(ServiceInfo),
    /// Listing complete
    Done { count: usize },
}

/// Summary information about a service
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceInfo {
    pub name: String,
    pub base_url: String,
    pub model_count: usize,
    pub message_format: String,
}

// ============================================================================
// Verification Events
// ============================================================================

/// Events emitted during model verification
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerifyEvent {
    /// Starting verification for a model
    Starting { model_id: String },
    /// Model verified successfully
    Success { model_id: String, latency_ms: u64 },
    /// Model verification failed
    Failed { model_id: String, error: String },
    /// Verification skipped (e.g., no API key)
    Skipped { model_id: String, reason: String },
    /// All verifications complete
    Done {
        total: usize,
        passed: usize,
        failed: usize,
        skipped: usize,
    },
}

// ============================================================================
// Query Events
// ============================================================================

/// Events emitted when querying models with filters
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueryEvent {
    /// Single matching model
    Match(ModelInfo),
    /// Query complete
    Done { count: usize },
}

// ============================================================================
// Conversions from internal types
// ============================================================================

impl From<&crate::config::ModelConfig> for ModelInfo {
    fn from(config: &crate::config::ModelConfig) -> Self {
        Self {
            id: config.model.id.clone(),
            name: config.model.name.clone(),
            family: config.model.family.clone(),
            service: config.model.service.clone(),
            verified: config.model.status == crate::config::VerificationStatus::Verified,
            version: config.model.version.clone(),
            capabilities: CapabilitySummary {
                streaming: config.capabilities.streaming,
                vision: config.capabilities.vision,
                functions: config.capabilities.functions,
                json_mode: config.capabilities.json_mode,
                multimodal: config.capabilities.multimodal,
            },
            pricing: Some(PricingSummary {
                input_per_1m_tokens: config.pricing.input_per_1k_tokens * 1000.0,
                output_per_1m_tokens: config.pricing.output_per_1k_tokens * 1000.0,
                currency: format!("{:?}", config.pricing.currency),
            }),
        }
    }
}

impl From<&crate::config::ServiceConfig> for ServiceInfo {
    fn from(config: &crate::config::ServiceConfig) -> Self {
        Self {
            name: config.service.name.clone(),
            base_url: config.service.base_url.clone(),
            model_count: 0, // Filled in by caller
            message_format: format!("{:?}", config.message_builder),
        }
    }
}
