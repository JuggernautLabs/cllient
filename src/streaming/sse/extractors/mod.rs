//! Token extraction implementations for different AI providers
//!
//! This module contains provider-specific implementations of the TokenExtractor
//! trait, which handle the parsing of SSE payloads to extract tokens and detect
//! stream completion conditions.

pub mod openai;
pub mod claude;

// Re-export extractors for convenience
pub use openai::OpenAITokenExtractor;
pub use claude::ClaudeTokenExtractor;