//! Streaming response processing for AI providers
//!
//! This module provides comprehensive streaming support for processing AI provider responses,
//! including real-time JSON extraction, provider-specific SSE parsing, and text content handling.
//!
//! ## Core Components
//!
//! - **Stream Items**: `StreamItem<T>` represents individual elements in the stream
//! - **JSON Processing**: `JsonStreamProcessor` handles incremental JSON parsing  
//! - **SSE Providers**: Provider-specific Server-Sent Events parsing
//! - **Stream Adapters**: Convert between different stream types
//!
//! ## Usage
//!
//! ```rust
//! use cllient::streaming::{StreamItem, build_parsed_stream};
//! use serde::{Deserialize, Serialize};
//! use schemars::JsonSchema;
//! 
//! #[derive(Deserialize, JsonSchema)]
//! struct MyData { value: i32 }
//! 
//! let raw_text = "Here's the data: {\"value\": 42}";
//! let items = build_parsed_stream::<MyData>(raw_text);
//! ```

// Core streaming types and functions
pub mod parsers;
pub mod json_utils;
pub mod error;
pub mod compat;
pub mod end_condition;

// Re-exports for convenience
pub use parsers::{JsonStreamProcessor, process_complete_text};

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tracing::instrument;
use tokio::io::{AsyncRead, AsyncReadExt};
use async_stream::stream;
use futures_core::stream::Stream as FuturesStream;
use futures_util::StreamExt;
use bytes::Bytes;
use std::pin::Pin;

/// Supported SSE format types for different providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SSEFormat {
    /// OpenAI-compatible format (used by OpenAI, DeepSeek, etc.)
    /// Format: `data: {"choices":[{"delta":{"content":"token"}}]}`
    OpenAI,
    
    /// Claude/Anthropic format
    /// Format: `data: {"delta":{"text":"token"}, "type":"content_block_delta"}`
    Claude,
}

impl SSEFormat {
    /// Create an SSE provider instance for this format
    pub fn create_provider<T>(&self) -> Box<dyn SSEProvider<T>>
    where
        T: DeserializeOwned + JsonSchema + Send + 'static,
    {
        match self {
            SSEFormat::OpenAI => Box::new(sse::openai_provider()),
            SSEFormat::Claude => Box::new(sse::claude_provider()),
        }
    }
}

/// Trait for handling provider-specific SSE (Server-Sent Events) parsing.
/// 
/// Each provider implements this trait to parse their specific SSE format
/// and guarantee consistent StreamItem<T> output.
pub trait SSEProvider<T>: Send + Sync 
where 
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    /// Parse raw SSE bytes stream into our standard StreamItem format.
    /// 
    /// Takes a stream of raw bytes from the provider's SSE endpoint
    /// and returns a stream of parsed StreamItem<T> events.
    fn parse_sse_stream(
        &self,
        byte_stream: Pin<Box<dyn FuturesStream<Item = Result<Bytes, error::AIError>> + Send>>
    ) -> Pin<Box<dyn FuturesStream<Item = Result<StreamItem<T>, error::QueryResolverError>> + Send>>;
}

/// Represents a piece of unstructured text content returned by the model.
///
/// Usage:
/// - `StreamItem::Text(TextContent { text })` preserves non-JSON content in the
///   order it appears, so you never lose commentary or context.
#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
pub struct TextContent {
    /// Plain text content. Downstream systems can render or log this.
    pub text: String,
}

/// An item in the model's response stream: token, text, or typed data `T`.
///
/// Usage:
/// - One-shot: `Vec<StreamItem<T>>` via `QueryResolver::query_stream`.
/// - Streaming: `Stream<Item=StreamItem<T>>` via `stream_from_async_read`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "content")]
pub enum StreamItem<T>
where
    T: JsonSchema,
{
    /// Individual token for real-time display
    #[serde(skip)]
    Token(String),
    /// Free-form text emitted by the model.
    Text(TextContent),
    /// Structured data conforming to the user-provided schema.
    Data(T),
}

/// Convenience alias describing the full response as an ordered stream.
pub type ParsedStream<T> = Vec<StreamItem<T>>;

/// Build a parsed stream (ordered list of Text/Data(T)) from raw text.
/// 
/// Now implemented using JsonStreamProcessor to eliminate code duplication.
#[instrument(skip(raw))]
pub fn build_parsed_stream<T>(raw: &str) -> ParsedStream<T>
where
    T: DeserializeOwned + JsonSchema,
{
    process_complete_text(raw)
}

/// Stream `StreamItem<T>` from an `AsyncRead` by incrementally parsing JSON
/// structures and interleaving free-form text between them.
///
/// Use this for realtime toolcalls or progressive UIs.
/// Now uses JsonStreamProcessor to eliminate code duplication.
pub fn stream_from_async_read<R, T>(mut reader: R, buf_size: usize) -> impl FuturesStream<Item = StreamItem<T>>
where
    R: AsyncRead + Unpin + Send + 'static,
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    stream! {
        let mut processor = JsonStreamProcessor::<T>::new();
        let mut buf = vec![0u8; buf_size.max(1024)];
        
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        // Process this chunk and yield any complete items found
                        let items = processor.process_chunk(s);
                        for item in items {
                            yield item;
                        }
                    }
                }
                Err(_) => break,
            }
        }
        
        // Finalize and yield any remaining items
        let final_items = processor.finalize();
        for item in final_items {
            yield item;
        }
    }
}

/// Stream `StreamItem<T>` from a bytes stream (such as from an HTTP response).
///
/// This is the high-level streaming adapter that converts raw bytes into stream items
/// with proper error handling. It automatically handles UTF-8 conversion and incremental
/// JSON parsing without exposing low-level buffer management.
/// Now uses JsonStreamProcessor to eliminate code duplication.
pub fn stream_from_bytes<T>(
    byte_stream: Pin<Box<dyn FuturesStream<Item = Result<Bytes, error::AIError>> + Send>>
) -> impl FuturesStream<Item = Result<StreamItem<T>, error::QueryResolverError>>
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    stream! {
        let mut processor = JsonStreamProcessor::<T>::new();
        
        let mut byte_stream = byte_stream;
        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    // Convert bytes to string
                    match std::str::from_utf8(&bytes) {
                        Ok(s) => {
                            // Process this chunk and yield any complete items found
                            let items = processor.process_chunk(s);
                            for item in items {
                                yield Ok(item);
                            }
                        }
                        Err(utf8_err) => {
                            yield Err(error::QueryResolverError::Ai(
                                error::AIError::Mock(format!("UTF-8 decode error: {}", utf8_err))
                            ));
                            break;
                        }
                    }
                }
                Err(ai_error) => {
                    yield Err(error::QueryResolverError::Ai(ai_error));
                    break;
                }
            }
        }
        
        // Finalize and yield any remaining items
        let final_items = processor.finalize();
        for item in final_items {
            yield Ok(item);
        }
    }
}

// SSE processing submodule
pub mod sse;

// Re-export SSE providers for convenience
pub use sse::{
    ConfigurableSSEProvider, OpenAISSEProvider, ClaudeSSEProvider,
    openai_provider, claude_provider, stream_from_sse_bytes
};

// Re-export compatibility layer
pub use compat::{StreamEvent, StreamProcessor, StreamEventExt};

// Type alias for streaming results
pub type Stream = Pin<Box<dyn FuturesStream<Item = crate::error::Result<StreamEvent>> + Send>>;