//! Backwards compatibility utilities for SSE processing

use serde::de::DeserializeOwned;
use schemars::JsonSchema;
use super::super::super::{StreamItem, SSEProvider};
use super::super::providers::openai::OpenAISSEProvider;
use futures_core::stream::Stream;
use bytes::Bytes;
use std::pin::Pin;

/// Stream `StreamItem<T>` from an SSE bytes stream with proper token aggregation.
///
/// This processes Server-Sent Events format and aggregates tokens from the content field
/// into stream items. It handles both OpenAI-style and Claude-style SSE formats.
/// 
/// **Deprecated**: Use the SSEProvider trait implementations instead.
/// This function now delegates to OpenAISSEProvider for backwards compatibility.
pub fn stream_from_sse_bytes<T>(
    byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, super::super::super::error::AIError>> + Send>>
) -> impl Stream<Item = Result<StreamItem<T>, super::super::super::error::QueryResolverError>>
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    // Use the default OpenAI provider for backwards compatibility
    let provider = OpenAISSEProvider;
    provider.parse_sse_stream(byte_stream)
}