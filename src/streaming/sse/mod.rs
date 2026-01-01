//! Server-Sent Events (SSE) processing for different AI providers
//!
//! This module provides a modular, configuration-driven architecture for parsing
//! SSE streams from different AI providers (OpenAI, Claude, etc.) into our standard
//! StreamItem format.
//!
//! ## Architecture
//!
//! The SSE processing uses a unified approach where provider differences are expressed
//! as configuration rather than separate implementations:
//!
//! - `core/` - Core processing logic and the `TokenExtractor` trait
//! - `extractors/` - `JsonPathExtractor` with provider-specific configurations
//! - `providers/` - `ConfigurableSSEProvider` with factory methods
//! - `utils/` - Backwards compatibility helpers
//!
//! ## Usage
//!
//! ```rust,ignore
//! use cllient::streaming::sse::{ConfigurableSSEProvider, openai_provider, claude_provider};
//!
//! // Create provider using factory function
//! let openai = openai_provider();
//! let claude = claude_provider();
//!
//! // Or use the ConfigurableSSEProvider directly
//! let openai = ConfigurableSSEProvider::openai();
//! let claude = ConfigurableSSEProvider::claude();
//! ```
//!
//! ## Adding New Providers
//!
//! To add support for a new provider, create a new configuration function in
//! `extractors/mod.rs` that returns an `ExtractorConfig` with the appropriate
//! JSON paths and end conditions.

pub mod core;
pub mod extractors;
pub mod providers;
pub mod utils;

// Re-export core types
pub use core::{SSEStreamProcessor, TokenExtractor, TokenExtractionResult};

// Re-export extractor types and configurations
pub use extractors::{
    JsonPathExtractor, JsonPath, JsonPathSegment,
    ExtractorConfig, EndCondition,
    openai_config, claude_config,
    openai_extractor, claude_extractor,
    // Backwards compatibility type aliases
    OpenAITokenExtractor, ClaudeTokenExtractor,
};

// Re-export provider types and factory functions
pub use providers::{
    ConfigurableSSEProvider,
    openai_provider, claude_provider,
    // Backwards compatibility type aliases
    OpenAISSEProvider, ClaudeSSEProvider,
};

// Re-export utility function for backwards compatibility
pub use utils::stream_from_sse_bytes;
