//! SSE provider implementations for different AI providers
//!
//! This module provides a generic `ConfigurableSSEProvider` that eliminates the need
//! for separate provider implementations. Provider-specific behavior is configured
//! through extractor configurations.

use serde::de::DeserializeOwned;
use schemars::JsonSchema;
use super::super::{StreamItem, SSEProvider};
use super::core::processor::SSEStreamProcessor;
use super::extractors::{JsonPathExtractor, ExtractorConfig, openai_config, claude_config};
use futures_core::stream::Stream;
use bytes::Bytes;
use std::pin::Pin;

/// Generic SSE provider that works with any extractor configuration.
///
/// This eliminates the nearly-identical wrapper code that was previously
/// duplicated between OpenAI and Claude providers.
#[derive(Debug, Clone)]
pub struct ConfigurableSSEProvider {
    config: ExtractorConfig,
}

impl ConfigurableSSEProvider {
    /// Create a new provider with the given extractor configuration
    pub fn new(config: ExtractorConfig) -> Self {
        Self { config }
    }

    /// Create an OpenAI-compatible provider
    pub fn openai() -> Self {
        Self::new(openai_config())
    }

    /// Create a Claude/Anthropic provider
    pub fn claude() -> Self {
        Self::new(claude_config())
    }
}

impl<T> SSEProvider<T> for ConfigurableSSEProvider
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    fn parse_sse_stream(
        &self,
        byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, super::super::error::AIError>> + Send>>
    ) -> Pin<Box<dyn Stream<Item = Result<StreamItem<T>, super::super::error::QueryResolverError>> + Send>> {
        let extractor = JsonPathExtractor::new(self.config.clone());
        SSEStreamProcessor::process_sse_stream(byte_stream, extractor)
    }
}

// ============================================================================
// Type aliases for backwards compatibility
// ============================================================================

/// OpenAI-compatible SSE provider (backwards compatibility alias)
pub type OpenAISSEProvider = ConfigurableSSEProvider;

/// Claude/Anthropic SSE provider (backwards compatibility alias)
pub type ClaudeSSEProvider = ConfigurableSSEProvider;

/// Wrapper to provide Default-like construction for OpenAI provider
#[derive(Debug, Clone, Default)]
pub struct OpenAISSEProviderDefault;

impl OpenAISSEProviderDefault {
    pub fn provider() -> ConfigurableSSEProvider {
        ConfigurableSSEProvider::openai()
    }
}

/// Wrapper to provide Default-like construction for Claude provider
#[derive(Debug, Clone, Default)]
pub struct ClaudeSSEProviderDefault;

impl ClaudeSSEProviderDefault {
    pub fn provider() -> ConfigurableSSEProvider {
        ConfigurableSSEProvider::claude()
    }
}

// ============================================================================
// Factory functions for convenience
// ============================================================================

/// Create an OpenAI-compatible SSE provider
pub fn openai_provider() -> ConfigurableSSEProvider {
    ConfigurableSSEProvider::openai()
}

/// Create a Claude/Anthropic SSE provider
pub fn claude_provider() -> ConfigurableSSEProvider {
    ConfigurableSSEProvider::claude()
}
