//! SSE stream processor implementation
//!
//! This module provides the core SSE processing logic that can be shared
//! between different provider implementations, reducing code duplication.

use serde::de::DeserializeOwned;
use schemars::JsonSchema;
use super::super::super::{StreamItem, TextContent};
use super::types::{TokenExtractor, TokenExtractionResult};
use super::super::super::json_utils::find_json_structures;
use tokio::io::{AsyncBufReadExt, BufReader};
use async_stream::stream;
use futures_core::stream::Stream;
use futures_util::StreamExt;
use bytes::Bytes;
use std::pin::Pin;

/// Base SSE stream processor that handles common functionality
/// 
/// This eliminates the ~95% duplicate code between OpenAI and Claude providers
/// by abstracting the common SSE processing logic and allowing providers to
/// inject only their specific token extraction and end detection logic.
pub struct SSEStreamProcessor;

impl SSEStreamProcessor {
    /// Process an SSE byte stream using provider-specific token extraction
    /// 
    /// This method handles all the common SSE processing:
    /// - Converting bytes to AsyncRead
    /// - Line-by-line SSE parsing
    /// - JSON parsing and validation
    /// - Text buffer management for JSON structure detection
    /// - Token aggregation and paragraph flushing
    /// 
    /// The provider only needs to implement token extraction and end detection.
    pub fn process_sse_stream<T, E>(
        byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, super::super::super::error::AIError>> + Send>>,
        extractor: E,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamItem<T>, super::super::super::error::QueryResolverError>> + Send>>
    where
        T: DeserializeOwned + JsonSchema + Send + 'static,
        E: TokenExtractor + 'static,
    {
        Box::pin(stream! {
            use tokio_util::io::StreamReader;
            
            // Convert bytes stream to AsyncRead
            let io_stream = byte_stream.map(|res| match res {
                Ok(bytes) => Ok::<Bytes, std::io::Error>(bytes),
                Err(e) => Err(std::io::Error::other(e.to_string())),
            });
            let reader = StreamReader::new(io_stream);
            
            // Process SSE stream
            let mut br = BufReader::new(reader).lines();
            let mut sse_event = String::new();
            let mut text_buf = String::new();
            
            while let Ok(Some(line)) = br.next_line().await {
                if line.is_empty() {
                    // Process accumulated SSE event
                    if let Some(payload) = sse_event.strip_prefix("data: ") {
                        // Check for provider-specific end condition
                        if extractor.is_end_payload(payload) {
                            let tail = text_buf.trim();
                            if !tail.is_empty() { 
                                yield Ok(StreamItem::Text(TextContent { text: tail.to_string() })); 
                            }
                            break;
                        }
                        
                        // Try to parse JSON and extract token
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                            match extractor.extract_token(&v) {
                                TokenExtractionResult::Token(token) => {
                                    // Emit raw token and accumulate for JSON parsing
                                    yield Ok(StreamItem::Token(token.clone()));
                                    text_buf.push_str(&token);

                                    // Detect completed JSON structures for type T
                                    let coords = find_json_structures(&text_buf);
                                    let mut consumed_up_to = 0usize;
                                    for node in coords {
                                        let end = node.end.saturating_add(1);
                                        let slice = &text_buf[node.start..end];
                                        if let Ok(item) = serde_json::from_str::<T>(slice) {
                                            // Emit any text before the JSON
                                            if node.start > 0 {
                                                let chunk = text_buf[..node.start].trim();
                                                if !chunk.is_empty() { 
                                                    yield Ok(StreamItem::Text(TextContent { text: chunk.to_string() })); 
                                                }
                                            }
                                            yield Ok(StreamItem::Data(item));
                                            consumed_up_to = consumed_up_to.max(end);
                                        }
                                    }
                                    if consumed_up_to > 0 { 
                                        text_buf.drain(..consumed_up_to); 
                                    }

                                    // Paragraph flush - emit text on double newlines
                                    if let Some(idx) = text_buf.find("\n\n") {
                                        let (chunk, rest) = text_buf.split_at(idx);
                                        let chunk = chunk.trim();
                                        if !chunk.is_empty() { 
                                            yield Ok(StreamItem::Text(TextContent { text: chunk.to_string() })); 
                                        }
                                        text_buf = rest[2..].to_string();
                                    }

                                    // Check for provider-specific stream completion
                                    // (OpenAI uses finish_reason, Claude uses message_stop)
                                    if let TokenExtractionResult::EndStream = extractor.extract_token(&v) {
                                        let tail = text_buf.trim();
                                        if !tail.is_empty() { 
                                            yield Ok(StreamItem::Text(TextContent { text: tail.to_string() })); 
                                        }
                                        text_buf.clear();
                                    }
                                }
                                TokenExtractionResult::EndStream => {
                                    let tail = text_buf.trim();
                                    if !tail.is_empty() { 
                                        yield Ok(StreamItem::Text(TextContent { text: tail.to_string() })); 
                                    }
                                    break;
                                }
                                TokenExtractionResult::NoToken => {
                                    // Continue processing - this payload didn't contain a token
                                }
                            }
                        }
                    }
                    sse_event.clear();
                } else {
                    // Only accumulate data lines, ignore event lines
                    if line.starts_with("data: ") {
                        sse_event = line.to_string();
                    }
                }
            }

            // Final flush of any remaining aggregated text if stream ended
            let tail = text_buf.trim();
            if !tail.is_empty() {
                yield Ok(StreamItem::Text(TextContent { text: tail.to_string() }));
            }
        })
    }
}
