//! Core SSE processing functionality
//!
//! This module provides the fundamental types and processing logic
//! for Server-Sent Events streams.

pub mod types;
pub mod processor;

// Re-export core types for convenience
pub use types::{TokenExtractor, TokenExtractionResult};
pub use processor::SSEStreamProcessor;