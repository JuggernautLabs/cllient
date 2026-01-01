//! Compatibility layer for the old streaming interface
//!
//! This module provides a backwards-compatible streaming interface using
//! shared SSE parsing infrastructure from the `sse` module. Provider-specific
//! parsing is delegated to extractor implementations to minimize duplication.

use futures::Stream;
use reqwest::Response;
use serde_json::Value;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::config::StreamingConfig;
use crate::error::{ClientError, Result};

/// Events that can be emitted from a streaming response
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Text content chunk
    Content(String),
    /// Role information (usually at start)
    Role(String),
    /// Start of response
    Start,
    /// End of response with finish reason
    Finish(Option<String>),
    /// Usage/token information
    Usage {
        input_tokens: Option<u32>,
        output_tokens: Option<u32>,
        total_tokens: Option<u32>,
    },
    /// Error occurred
    Error(String),
    /// Raw event data (for debugging)
    Raw(String),
}

// =============================================================================
// Shared SSE Parsing Helpers
// =============================================================================

/// Extract a u32 value from a JSON path
fn extract_u32(value: &Value, path: &[&str]) -> Option<u32> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_u64().map(|t| t as u32)
}

/// Extract a string value from a JSON path
fn extract_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

/// Parse the data portion of an SSE line (after "data: " prefix)
fn parse_sse_data(data: &str) -> Option<Value> {
    serde_json::from_str(data).ok()
}

/// Extract usage information from common paths
fn extract_usage(value: &Value, input_path: &[&str], output_path: &[&str], total_path: Option<&[&str]>) -> StreamEvent {
    StreamEvent::Usage {
        input_tokens: extract_u32(value, input_path),
        output_tokens: extract_u32(value, output_path),
        total_tokens: total_path.and_then(|p| extract_u32(value, p)),
    }
}

// =============================================================================
// Provider-Specific Event Handlers
// =============================================================================

/// Anthropic event type handler - maps event types to StreamEvents
fn handle_anthropic_event(event_type: &str, data: &Value) -> Option<StreamEvent> {
    match event_type {
        "message_start" => Some(StreamEvent::Start),
        "content_block_delta" => {
            extract_str(data, &["delta", "text"])
                .map(|text| StreamEvent::Content(text.to_string()))
        }
        "message_delta" => {
            // Check for usage in delta
            if data.get("delta").and_then(|d| d.get("usage")).is_some() {
                Some(extract_usage(
                    data,
                    &["delta", "usage", "input_tokens"],
                    &["delta", "usage", "output_tokens"],
                    None,
                ))
            } else {
                None
            }
        }
        "message_stop" => Some(StreamEvent::Finish(None)),
        "error" => {
            let error_msg = extract_str(data, &["error", "message"])
                .unwrap_or("Unknown error");
            Some(StreamEvent::Error(error_msg.to_string()))
        }
        _ => Some(StreamEvent::Raw(serde_json::to_string(data).unwrap_or_default())),
    }
}

/// OpenAI event handler - extracts content, role, finish reason, or usage
fn handle_openai_event(data: &Value) -> Option<StreamEvent> {
    // Extract content from choices[0].delta.content
    if let Some(content) = extract_str(data, &["choices", "0", "delta", "content"]) {
        return Some(StreamEvent::Content(content.to_string()));
    }

    // Fallback: try numeric index access for arrays
    if let Some(content) = data.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(|c| c.as_str())
    {
        return Some(StreamEvent::Content(content.to_string()));
    }

    // Check for role
    if let Some(role) = data.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("role"))
        .and_then(|r| r.as_str())
    {
        return Some(StreamEvent::Role(role.to_string()));
    }

    // Check for finish reason
    if let Some(finish_reason) = data.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(|r| r.as_str())
    {
        return Some(StreamEvent::Finish(Some(finish_reason.to_string())));
    }

    // Check for usage (usually at the end)
    if data.get("usage").is_some() {
        return Some(extract_usage(
            data,
            &["usage", "prompt_tokens"],
            &["usage", "completion_tokens"],
            Some(&["usage", "total_tokens"]),
        ));
    }

    None
}

// =============================================================================
// SSE Line Parsers
// =============================================================================

/// Parse an Anthropic SSE line into a StreamEvent
fn parse_anthropic_sse(line: &str) -> Option<StreamEvent> {
    // Skip event type lines
    if line.starts_with("event: ") {
        return None;
    }

    let data = line.strip_prefix("data: ")?;
    let json = parse_sse_data(data)?;
    let event_type = extract_str(&json, &["type"])?;

    handle_anthropic_event(event_type, &json)
}

/// Parse an OpenAI SSE line into a StreamEvent
fn parse_openai_sse(config: &StreamingConfig, line: &str) -> Option<StreamEvent> {
    // Check line prefix if configured
    if let Some(prefix) = &config.line_prefix {
        if !line.starts_with(prefix) {
            return None;
        }
    }

    let data = config.line_prefix.as_ref()
        .map(|p| &line[p.len()..])
        .unwrap_or(line);

    // Check for done marker
    if let Some(done_marker) = &config.done_marker {
        if data.trim() == done_marker {
            return Some(StreamEvent::Finish(None));
        }
    }

    let json = parse_sse_data(data)?;
    handle_openai_event(&json)
}

/// Parse a generic SSE line (fallback for unsupported formats)
///
/// Note on Google SSE: Google's Gemini streaming API uses a similar SSE format
/// to OpenAI but with different JSON structure. Currently falls back to raw
/// event passthrough. Full Google SSE support would require:
/// 1. Parsing `candidates[0].content.parts[0].text` for content
/// 2. Handling `candidates[0].finishReason` for completion
/// 3. Extracting `usageMetadata` for token counts
///
/// For production use, consider using the `sse` module's `TokenExtractor` trait
/// to implement a `GoogleTokenExtractor` following the pattern in
/// `src/streaming/sse/extractors/`.
fn parse_generic_sse(line: &str) -> Option<StreamEvent> {
    if !line.is_empty() {
        Some(StreamEvent::Raw(line.to_string()))
    } else {
        None
    }
}

// =============================================================================
// Stream Processor
// =============================================================================

/// Processor for streaming responses based on service configuration
pub struct StreamProcessor {
    config: StreamingConfig,
}

impl StreamProcessor {
    pub fn new(config: &StreamingConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    pub async fn process_response(&self, response: Response) -> Result<super::Stream> {
        use crate::config::StreamingFormat;
        match &self.config.format {
            StreamingFormat::TextEventStream => self.process_sse_response(response).await,
            StreamingFormat::Ndjson => Err(ClientError::Stream(
                "NDJSON streaming not yet implemented".to_string()
            )),
            StreamingFormat::Custom(format) => Err(ClientError::Stream(format!(
                "Unsupported streaming format: {}",
                format
            ))),
        }
    }

    async fn process_sse_response(&self, response: Response) -> Result<super::Stream> {
        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();

        tokio::spawn(async move {
            use futures::StreamExt as FuturesStreamExt;

            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = FuturesStreamExt::next(&mut stream).await {
                match chunk_result {
                    Ok(chunk) => {
                        buffer.push_str(&String::from_utf8_lossy(&chunk));

                        // Process complete lines
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].trim().to_string();
                            buffer = buffer[newline_pos + 1..].to_string();

                            if let Some(event) = Self::parse_sse_line(&config, &line) {
                                if tx.send(Ok(event)).await.is_err() {
                                    return; // Receiver dropped
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(ClientError::Http(e))).await;
                        return;
                    }
                }
            }

            // Process any remaining buffer content
            if !buffer.trim().is_empty() {
                if let Some(event) = Self::parse_sse_line(&config, buffer.trim()) {
                    let _ = tx.send(Ok(event)).await;
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn parse_sse_line(config: &StreamingConfig, line: &str) -> Option<StreamEvent> {
        use crate::config::SseParser;
        match &config.parser {
            SseParser::AnthropicSse => parse_anthropic_sse(line),
            SseParser::OpenAiSse => parse_openai_sse(config, line),
            SseParser::GoogleSse => parse_generic_sse(line),
            SseParser::Custom(_) => parse_generic_sse(line),
        }
    }
}

// =============================================================================
// Stream Extension Trait
// =============================================================================

/// Helper trait for working with streaming responses.
///
/// Provides convenience methods for collecting content, filtering, and parsing JSON.
pub trait StreamEventExt {
    /// Collect only content events into a single string
    fn collect_content(self) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Filter stream to only content events
    fn content_only(self) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

    /// Parse streaming content into typed `StreamItem<T>` values.
    fn parse_json<T>(self) -> Pin<Box<dyn Stream<Item = super::StreamItem<T>> + Send>>
    where
        T: serde::de::DeserializeOwned + schemars::JsonSchema + Send + 'static;
}

impl<S> StreamEventExt for S
where
    S: Stream<Item = Result<StreamEvent>> + Send + 'static,
{
    async fn collect_content(self) -> Result<String> {
        use futures::StreamExt as FuturesStreamExt;

        let mut content = String::new();
        let mut stream = Box::pin(self);

        while let Some(event_result) = FuturesStreamExt::next(&mut stream).await {
            match event_result? {
                StreamEvent::Content(text) => content.push_str(&text),
                StreamEvent::Error(error) => return Err(ClientError::Stream(error)),
                _ => {}
            }
        }

        Ok(content)
    }

    fn content_only(self) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>> {
        use futures::StreamExt as FuturesStreamExt;

        let stream = FuturesStreamExt::filter_map(self, |event_result| async move {
            match event_result {
                Ok(StreamEvent::Content(text)) => Some(Ok(text)),
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            }
        });

        Box::pin(stream)
    }

    fn parse_json<T>(self) -> Pin<Box<dyn Stream<Item = super::StreamItem<T>> + Send>>
    where
        T: serde::de::DeserializeOwned + schemars::JsonSchema + Send + 'static,
    {
        use super::parsers::JsonStreamProcessor;

        Box::pin(async_stream::stream! {
            let mut processor = JsonStreamProcessor::<T>::new();
            let mut stream = Box::pin(self);

            while let Some(event_result) = futures::StreamExt::next(&mut stream).await {
                match event_result {
                    Ok(StreamEvent::Content(chunk)) => {
                        for item in processor.process_chunk(&chunk) {
                            yield item;
                        }
                    }
                    Ok(StreamEvent::Error(e)) => {
                        yield super::StreamItem::Text(super::TextContent {
                            text: format!("[Error: {}]", e),
                        });
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }

            for item in processor.finalize() {
                yield item;
            }
        })
    }
}
