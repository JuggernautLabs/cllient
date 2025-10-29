//! SSE provider implementations for different AI providers
//!
//! This module contains the concrete implementations of the SSEProvider trait
//! for various AI providers, using the core processor with provider-specific
//! token extractors.

pub mod openai;
pub mod claude;

// Re-export providers for convenience
pub use openai::OpenAISSEProvider;
pub use claude::ClaudeSSEProvider;