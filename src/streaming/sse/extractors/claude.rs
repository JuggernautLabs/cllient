//! Claude/Anthropic token extractor implementation

use super::super::core::types::{TokenExtractor, TokenExtractionResult};

/// Claude/Anthropic token extractor
/// 
/// Handles the Claude SSE format: `{"delta":{"text":"token"}, "type":"content_block_delta"}`
#[derive(Debug, Clone)]
pub struct ClaudeTokenExtractor;

impl TokenExtractor for ClaudeTokenExtractor {
    fn extract_token(&self, json_value: &serde_json::Value) -> TokenExtractionResult {
        // Check for message_stop event
        if json_value.get("type").and_then(|t| t.as_str()) == Some("message_stop") {
            return TokenExtractionResult::EndStream;
        }
        
        // Extract token from Claude format
        if let Some(token) = json_value
            .get("delta")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
        {
            TokenExtractionResult::Token(token.to_string())
        } else {
            TokenExtractionResult::NoToken
        }
    }
    
    fn is_end_payload(&self, _payload: &str) -> bool {
        // Claude doesn't use a simple payload check - it uses JSON type field
        false
    }
}