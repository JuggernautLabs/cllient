//! OpenAI-compatible token extractor implementation

use super::super::core::types::{TokenExtractor, TokenExtractionResult};

/// OpenAI-compatible token extractor
/// 
/// Handles the OpenAI SSE format: `{"choices":[{"delta":{"content":"token"}}]}`
/// Also works with DeepSeek and other OpenAI-compatible providers.
#[derive(Debug, Clone)]
pub struct OpenAITokenExtractor;

impl TokenExtractor for OpenAITokenExtractor {
    fn extract_token(&self, json_value: &serde_json::Value) -> TokenExtractionResult {
        // Check for completion first
        if json_value
            .get("choices").and_then(|c| c.get(0))
            .and_then(|c0| c0.get("finish_reason"))
            .and_then(|fr| fr.as_str())
            .is_some()
        {
            return TokenExtractionResult::EndStream;
        }
        
        // Extract token from OpenAI format
        if let Some(token) = json_value
            .get("choices").and_then(|c| c.get(0))
            .and_then(|c0| c0.get("delta"))
            .and_then(|d| d.get("content"))
            .and_then(|c| c.as_str())
        {
            TokenExtractionResult::Token(token.to_string())
        } else {
            TokenExtractionResult::NoToken
        }
    }
    
    fn is_end_payload(&self, payload: &str) -> bool {
        payload.trim() == "[DONE]"
    }
}