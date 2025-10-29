//! Core types for SSE stream processing
//!
//! This module defines the fundamental types and traits used across
//! the SSE processing system.

/// Result of attempting to extract a token from SSE JSON payload
#[derive(Debug)]
pub enum TokenExtractionResult {
    /// Successfully extracted a token
    Token(String),
    /// Stream should end (provider-specific end condition met)
    EndStream,
    /// No token found in this payload, continue processing
    NoToken,
}

/// Trait for provider-specific token extraction logic
pub trait TokenExtractor: Send + Sync {
    /// Extract token from SSE payload JSON, handling provider-specific format
    fn extract_token(&self, json_value: &serde_json::Value) -> TokenExtractionResult;
    
    /// Check if payload indicates stream end, handling provider-specific end conditions
    fn is_end_payload(&self, payload: &str) -> bool;
}