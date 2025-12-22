//! Compatibility layer for the old streaming interface

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

    pub async fn process_response(
        &self,
        response: Response,
    ) -> Result<super::Stream> {
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

    async fn process_sse_response(
        &self,
        response: Response,
    ) -> Result<super::Stream> {
        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();

        tokio::spawn(async move {
            use futures::StreamExt as FuturesStreamExt;
            
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = FuturesStreamExt::next(&mut stream).await {
                match chunk_result {
                    Ok(chunk) => {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        buffer.push_str(&chunk_str);

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
        // Handle different SSE parsing strategies based on the parser type
        match &config.parser {
            SseParser::AnthropicSse => Self::parse_anthropic_sse(config, line),
            SseParser::OpenAiSse => Self::parse_openai_sse(config, line),
            SseParser::GoogleSse => Self::parse_generic_sse(config, line), // TODO: implement Google SSE
            SseParser::Custom(_) => Self::parse_generic_sse(config, line),
        }
    }

    fn parse_anthropic_sse(_config: &StreamingConfig, line: &str) -> Option<StreamEvent> {
        if line.starts_with("event: ") {
            return None;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            // Remove "data: " prefix
            
            if let Ok(json) = serde_json::from_str::<Value>(data) {
                if let Some(event_type) = json.get("type").and_then(|t| t.as_str()) {
                    return Self::handle_anthropic_event(event_type, &json);
                }
            }
        }

        None
    }

    fn handle_anthropic_event(event_type: &str, data: &Value) -> Option<StreamEvent> {
        match event_type {
            "message_start" => Some(StreamEvent::Start),
            "content_block_delta" => {
                data.get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str()).map(|text| StreamEvent::Content(text.to_string()))
            }
            "message_delta" => {
                // Handle usage updates
                if let Some(usage) = data.get("delta").and_then(|d| d.get("usage")) {
                    let input_tokens = usage.get("input_tokens").and_then(|t| t.as_u64()).map(|t| t as u32);
                    let output_tokens = usage.get("output_tokens").and_then(|t| t.as_u64()).map(|t| t as u32);
                    
                    Some(StreamEvent::Usage {
                        input_tokens,
                        output_tokens,
                        total_tokens: None,
                    })
                } else {
                    None
                }
            }
            "message_stop" => Some(StreamEvent::Finish(None)),
            "error" => {
                let error_msg = data.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                Some(StreamEvent::Error(error_msg.to_string()))
            }
            _ => Some(StreamEvent::Raw(serde_json::to_string(data).unwrap_or_default())),
        }
    }

    fn parse_openai_sse(config: &StreamingConfig, line: &str) -> Option<StreamEvent> {
        if let Some(prefix) = &config.line_prefix {
            if !line.starts_with(prefix) {
                return None;
            }
        }

        let data = if let Some(prefix) = &config.line_prefix {
            &line[prefix.len()..]
        } else {
            line
        };

        // Check for done marker
        if let Some(done_marker) = &config.done_marker {
            if data.trim() == done_marker {
                return Some(StreamEvent::Finish(None));
            }
        }

        // Parse JSON data
        if let Ok(json) = serde_json::from_str::<Value>(data) {
            return Self::handle_openai_event(&json);
        }

        None
    }

    fn handle_openai_event(data: &Value) -> Option<StreamEvent> {
        // Debug: Log the event structure for troubleshooting
        tracing::debug!("Processing OpenAI SSE event: {}", serde_json::to_string(data).unwrap_or_else(|_| "<unparseable>".to_string()));
        
        // Log available top-level keys
        if let Some(obj) = data.as_object() {
            let keys: Vec<&String> = obj.keys().collect();
            tracing::debug!("Available top-level keys in SSE event: {:?}", keys);
        }
        
        // Extract content from choices[0].delta.content
        if let Some(content) = data.get("choices")
            .and_then(|c| {
                tracing::debug!("Found 'choices' array with length: {}", c.as_array().map(|a| a.len()).unwrap_or(0));
                c.get(0)
            })
            .and_then(|choice| {
                if let Some(choice_obj) = choice.as_object() {
                    let choice_keys: Vec<&String> = choice_obj.keys().collect();
                    tracing::debug!("Available keys in choice[0]: {:?}", choice_keys);
                }
                choice.get("delta")
            })
            .and_then(|delta| {
                if let Some(delta_obj) = delta.as_object() {
                    let delta_keys: Vec<&String> = delta_obj.keys().collect();
                    tracing::debug!("Available keys in delta: {:?}", delta_keys);
                }
                delta.get("content")
            })
            .and_then(|content| content.as_str())
        {
            tracing::debug!("Successfully extracted content: {:?}", content);
            return Some(StreamEvent::Content(content.to_string()));
        } else {
            tracing::debug!("Could not extract content from choices[0].delta.content - checking alternative paths");
        }

        // Check for role
        if let Some(role) = data.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|choice| choice.get("delta"))
            .and_then(|delta| delta.get("role"))
            .and_then(|role| role.as_str())
        {
            return Some(StreamEvent::Role(role.to_string()));
        }

        // Check for finish reason
        if let Some(finish_reason) = data.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(|reason| reason.as_str())
        {
            return Some(StreamEvent::Finish(Some(finish_reason.to_string())));
        }

        // Check for usage (usually at the end)
        if let Some(usage) = data.get("usage") {
            let input_tokens = usage.get("prompt_tokens").and_then(|t| t.as_u64()).map(|t| t as u32);
            let output_tokens = usage.get("completion_tokens").and_then(|t| t.as_u64()).map(|t| t as u32);
            let total_tokens = usage.get("total_tokens").and_then(|t| t.as_u64()).map(|t| t as u32);

            return Some(StreamEvent::Usage {
                input_tokens,
                output_tokens,
                total_tokens,
            });
        }

        None
    }

    fn parse_generic_sse(_config: &StreamingConfig, line: &str) -> Option<StreamEvent> {
        // Generic SSE parser - just return raw events
        if !line.is_empty() {
            Some(StreamEvent::Raw(line.to_string()))
        } else {
            None
        }
    }
}

/// Helper trait for working with streaming responses.
///
/// Provides convenience methods for collecting content, filtering, and parsing JSON.
pub trait StreamEventExt {
    /// Collect only content events into a single string
    fn collect_content(self) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Filter stream to only content events
    fn content_only(self) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

    /// Parse streaming content into typed `StreamItem<T>` values.
    ///
    /// This adapter extracts `Content` chunks from the stream, feeds them through
    /// a `JsonStreamProcessor`, and yields `StreamItem<T>` for each detected structure.
    ///
    /// - `StreamItem::Data(T)` - Successfully parsed JSON matching type T
    /// - `StreamItem::Text(TextContent)` - Non-JSON text or JSON that doesn't match T
    /// - `StreamItem::Token(String)` - Individual tokens (if enabled)
    ///
    /// # Example
    /// ```ignore
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct ToolCall { name: String, args: serde_json::Value }
    ///
    /// let mut stream = client.complete_stream(&request).await?;
    /// let mut typed_stream = stream.parse_json::<ToolCall>();
    ///
    /// while let Some(item) = typed_stream.next().await {
    ///     match item {
    ///         StreamItem::Data(tool) => println!("Tool call: {}", tool.name),
    ///         StreamItem::Text(t) => print!("{}", t.text),
    ///         _ => {}
    ///     }
    /// }
    /// ```
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
                _ => {} // Ignore other events
            }
        }

        Ok(content)
    }

    fn content_only(self) -> Pin<Box<dyn Stream<Item = Result<String>> + Send>> {
        use futures::StreamExt as FuturesStreamExt;

        let stream = FuturesStreamExt::filter_map(self, |event_result| async move {
            match event_result {
                Ok(StreamEvent::Content(text)) => Some(Ok(text)),
                Ok(_) => None, // Ignore non-content events
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
                        // Feed content through the JSON processor
                        for item in processor.process_chunk(&chunk) {
                            yield item;
                        }
                    }
                    Ok(StreamEvent::Error(e)) => {
                        // Emit error as text so caller knows something went wrong
                        yield super::StreamItem::Text(super::TextContent {
                            text: format!("[Error: {}]", e),
                        });
                    }
                    Ok(_) => {
                        // Ignore Start, Finish, Usage, Role, Raw events
                    }
                    Err(_e) => {
                        // Stream error - stop processing
                        break;
                    }
                }
            }

            // Finalize and yield any remaining buffered content
            for item in processor.finalize() {
                yield item;
            }
        })
    }
}