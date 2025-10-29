//! Utility functions and backwards compatibility helpers
//!
//! This module provides utility functions for SSE processing and maintains
//! backwards compatibility with older API usage patterns.

pub mod compat;

// Re-export for backwards compatibility
pub use compat::stream_from_sse_bytes;