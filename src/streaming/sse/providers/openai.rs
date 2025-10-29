//! OpenAI-compatible SSE provider implementation
//! 
//! Handles the standard OpenAI SSE format used by OpenAI, DeepSeek, and other providers.
//! Format: `data: {"choices":[{"delta":{"content":"token"}}]}`

use serde::de::DeserializeOwned;
use schemars::JsonSchema;
use super::super::super::{StreamItem, SSEProvider};
use super::super::core::processor::SSEStreamProcessor;
use super::super::extractors::openai::OpenAITokenExtractor;
use futures_core::stream::Stream;
use bytes::Bytes;
use std::pin::Pin;

/// Default OpenAI-compatible SSE provider.
/// 
/// Handles the standard OpenAI SSE format used by OpenAI, DeepSeek, and other providers.
/// Format: `data: {"choices":[{"delta":{"content":"token"}}]}`
/// 
/// Uses the common SSEStreamProcessor with OpenAI-specific token extraction.
#[derive(Debug, Clone)]
pub struct OpenAISSEProvider;

impl<T> SSEProvider<T> for OpenAISSEProvider
where 
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    fn parse_sse_stream(
        &self,
        byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, super::super::super::error::AIError>> + Send>>
    ) -> Pin<Box<dyn Stream<Item = Result<StreamItem<T>, super::super::super::error::QueryResolverError>> + Send>> {
        // Use the base processor with OpenAI-specific token extraction
        SSEStreamProcessor::process_sse_stream(byte_stream, OpenAITokenExtractor)
    }
}