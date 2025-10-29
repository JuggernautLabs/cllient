use serde::de::DeserializeOwned;
use schemars::JsonSchema;
use super::json_utils::{find_json_structures, deserialize_stream_map, ParsedOrUnknown, JsonStreamParser};
use super::{StreamItem, TextContent};
use tracing::debug;

/// Handles incremental JSON structure detection and processing within text streams.
/// 
/// This processor eliminates code duplication by providing a single implementation
/// for the common pattern of:
/// 1. Detecting complete JSON structures in streaming text
/// 2. Extracting structured data matching type T
/// 3. Preserving non-matching JSON and text as TextContent
/// 4. Managing text buffer offsets and consumption
#[derive(Debug)]
pub struct JsonStreamProcessor<T> {
    parser: JsonStreamParser,
    text_buf: String,
    last_offset: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> JsonStreamProcessor<T> 
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    /// Create a new JSON stream processor
    pub fn new() -> Self {
        Self {
            parser: JsonStreamParser::new(),
            text_buf: String::new(),
            last_offset: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Process a new chunk of text, returning any complete StreamItems found.
    /// 
    /// This handles the common streaming pattern:
    /// - Accumulate text in buffer
    /// - Detect complete JSON structures  
    /// - Extract matching data as StreamItem::Data(T)
    /// - Preserve non-matching content as StreamItem::Text
    /// - Update buffer offsets appropriately
    pub fn process_chunk(&mut self, chunk: &str) -> Vec<StreamItem<T>> {
        let mut items = Vec::new();
        
        // Add chunk to accumulated text
        self.text_buf.push_str(chunk);
        
        // Process any complete JSON structures found
        for node in self.parser.feed(chunk) {
            // Emit text before this node
            if node.start > self.last_offset && node.start <= self.text_buf.len() {
                let text_slice = &self.text_buf[self.last_offset..node.start];
                if !text_slice.trim().is_empty() {
                    items.push(StreamItem::Text(TextContent { text: text_slice.to_string() }));
                }
            }

            // Process the JSON structure
            let end = node.end + 1;
            if end <= self.text_buf.len() {
                let json_slice = &self.text_buf[node.start..end];
                let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
                
                if mapped.is_empty() {
                    // No valid structures, preserve as text
                    items.push(StreamItem::Text(TextContent { text: json_slice.to_string() }));
                } else {
                    let mut any_parsed = false;
                    for item in mapped {
                        match item {
                            ParsedOrUnknown::Parsed(v) => {
                                any_parsed = true;
                                items.push(StreamItem::Data(v));
                            }
                            ParsedOrUnknown::Unknown(u) => {
                                // Preserve unknown JSON chunks as text
                                let u_end = u.end + 1;
                                if u_end <= json_slice.len() && u.start < u_end {
                                    let sub = &json_slice[u.start..u_end];
                                    items.push(StreamItem::Text(TextContent { text: sub.to_string() }));
                                } else {
                                    debug!(target = "semantic_query::json_stream", "Skipping invalid unknown coordinates");
                                }
                            }
                        }
                    }
                    if !any_parsed {
                        // Fallback: preserve full slice to avoid losing info
                        items.push(StreamItem::Text(TextContent { text: json_slice.to_string() }));
                    }
                }
                
                self.last_offset = end;
            }
        }
        
        items
    }


    /// Finalize processing and return any remaining text in the buffer.
    /// 
    /// Call this when the stream is complete to ensure no trailing text is lost.
    pub fn finalize(self) -> Vec<StreamItem<T>> {
        let mut items = Vec::new();
        
        // Emit any remaining text
        if self.last_offset < self.text_buf.len() {
            let text_slice = &self.text_buf[self.last_offset..];
            if !text_slice.trim().is_empty() {
                items.push(StreamItem::Text(TextContent { text: text_slice.to_string() }));
            }
        }
        
        items
    }

    /// Access the current accumulated text buffer (for debugging/inspection)
    pub fn text_buffer(&self) -> &str {
        &self.text_buf
    }
}

impl<T> Default for JsonStreamProcessor<T> 
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Process complete text for JSON structures (non-streaming version).
/// 
/// This is used for processing complete text where we want to find all JSON structures
/// at once rather than incrementally. This is the standalone version that doesn't
/// require the full JsonStreamProcessor trait bounds.
pub fn process_complete_text<T>(text: &str) -> Vec<StreamItem<T>>
where
    T: DeserializeOwned + JsonSchema,
{
    let mut items = Vec::new();
    let roots = find_json_structures(text);
    let mut cursor = 0usize;

    for node in roots {
        // Emit text before this node
        if node.start > cursor {
            let text_slice = &text[cursor..node.start];
            let trimmed = text_slice.trim();
            if !trimmed.is_empty() {
                items.push(StreamItem::Text(TextContent { text: text_slice.to_string() }));
            }
        }

        // Process the JSON structure
        let end = node.end + 1; // inclusive -> exclusive
        let json_slice = &text[node.start..end];
        let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
        
        if mapped.is_empty() {
            // No structures detected, preserve as text
            items.push(StreamItem::Text(TextContent { text: json_slice.to_string() }));
        } else {
            let mut any_parsed = false;
            for item in mapped {
                match item {
                    ParsedOrUnknown::Parsed(v) => {
                        any_parsed = true;
                        items.push(StreamItem::Data(v));
                    }
                    ParsedOrUnknown::Unknown(u) => {
                        let u_end = u.end + 1;
                        if u_end <= json_slice.len() && u.start < u_end {
                            let sub = &json_slice[u.start..u_end];
                            items.push(StreamItem::Text(TextContent { text: sub.to_string() }));
                        } else {
                            debug!(target = "semantic_query::json_stream", "Skipping invalid unknown coordinates");
                        }
                    }
                }
            }
            if !any_parsed {
                items.push(StreamItem::Text(TextContent { text: json_slice.to_string() }));
            }
        }

        cursor = end;
    }

    // Emit trailing text
    if cursor < text.len() {
        let text_slice = &text[cursor..];
        let trimmed = text_slice.trim();
        if !trimmed.is_empty() {
            items.push(StreamItem::Text(TextContent { text: text_slice.to_string() }));
        }
    }

    items
}