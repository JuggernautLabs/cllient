//! Claude/Anthropic SSE provider implementation
//! 
//! Handles the Claude SSE format with delta text and message_stop events.
//! Format: `data: {"delta":{"text":"token"}, "type":"content_block_delta"}`

use serde::de::DeserializeOwned;
use schemars::JsonSchema;
use super::super::super::{StreamItem, SSEProvider};
use super::super::core::processor::SSEStreamProcessor;
use super::super::extractors::claude::ClaudeTokenExtractor;
use futures_core::stream::Stream;
use bytes::Bytes;
use std::pin::Pin;

/// Claude-specific SSE provider.
/// 
/// Handles the Claude SSE format with delta text and message_stop events.
/// Format: `data: {"delta":{"text":"token"}, "type":"content_block_delta"}`
/// 
/// Uses the common SSEStreamProcessor with Claude-specific token extraction.
#[derive(Debug, Clone)]
pub struct ClaudeSSEProvider;

impl<T> SSEProvider<T> for ClaudeSSEProvider
where 
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    fn parse_sse_stream(
        &self,
        byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, super::super::super::error::AIError>> + Send>>
    ) -> Pin<Box<dyn Stream<Item = Result<StreamItem<T>, super::super::super::error::QueryResolverError>> + Send>> {
        // Use the base processor with Claude-specific token extraction
        SSEStreamProcessor::process_sse_stream(byte_stream, ClaudeTokenExtractor)
    }
}