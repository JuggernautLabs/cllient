//! Server-Sent Events (SSE) processing for different AI providers
//!
//! This module provides a modular architecture for parsing SSE streams from
//! different AI providers (OpenAI, Claude, etc.) into our standard StreamItem format.
//!
//! ## Structure
//! - `core/` - Core processing logic and types
//! - `extractors/` - Provider-specific token extraction strategies  
//! - `providers/` - Complete provider implementations
//! - `utils/` - Utility functions and backwards compatibility

pub mod core;
pub mod extractors;
pub mod providers;
pub mod utils;

// Re-export the main types and functions for convenience
pub use core::{SSEStreamProcessor, TokenExtractor, TokenExtractionResult};
pub use extractors::{OpenAITokenExtractor, ClaudeTokenExtractor};
pub use providers::{OpenAISSEProvider, ClaudeSSEProvider};
pub use utils::stream_from_sse_bytes;