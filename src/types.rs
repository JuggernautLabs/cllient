use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use crate::client::LowLevelClient;

/// Low-level completion request
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<MessageContent>,
    pub system_prompt: Option<String>,
    pub parameters: HashMap<String, Value>,
    pub stream: bool,
}

impl CompletionRequest {
    pub fn new(messages: Vec<MessageContent>) -> Self {
        Self {
            messages,
            system_prompt: None,
            parameters: HashMap::new(),
            stream: false,
        }
    }
    
    /// Create a simple text-only request
    pub fn text(role: &str, content: &str) -> Self {
        Self::new(vec![MessageContent::Text {
            role: role.to_string(),
            content: content.to_string(),
        }])
    }
    
    /// Create a multimodal request
    pub fn multimodal(role: &str, content_blocks: Vec<ContentBlock>) -> Self {
        Self::new(vec![MessageContent::Multimodal {
            role: role.to_string(),
            content: content_blocks,
        }])
    }

    pub fn with_system_prompt(mut self, system_prompt: String) -> Self {
        self.system_prompt = Some(system_prompt);
        self
    }

    pub fn with_parameter<T: Into<Value>>(mut self, key: &str, value: T) -> Self {
        self.parameters.insert(key.to_string(), value.into());
        self
    }

    pub fn with_streaming(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

/// Content that can be sent in a message
#[derive(Debug, Clone)]
pub enum MessageContent {
    /// Plain text content
    Text { 
        role: String, 
        content: String 
    },
    /// Multimodal message with mixed content
    Multimodal {
        role: String,
        content: Vec<ContentBlock>
    },
}

/// Individual content blocks within a message
#[derive(Debug, Clone)]
pub enum ContentBlock {
    /// Text content
    Text(String),
    /// Binary content (images, audio, video, documents)
    Binary {
        data: Vec<u8>,
        mime_type: String,
        filename: Option<String>,
    },
    /// URL reference (for images, documents, etc.)
    Url {
        url: String,
        mime_type: Option<String>,
    },
}

impl ContentBlock {
    /// Create a text content block
    pub fn text(content: &str) -> Self {
        Self::Text(content.to_string())
    }
    
    /// Create an image content block from bytes
    pub fn image(data: Vec<u8>, format: ImageFormat) -> Self {
        let mime_type = match format {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png", 
            ImageFormat::Gif => "image/gif",
            ImageFormat::Webp => "image/webp",
        };
        
        Self::Binary {
            data,
            mime_type: mime_type.to_string(),
            filename: None,
        }
    }
    
    /// Create an audio content block from bytes
    pub fn audio(data: Vec<u8>, format: AudioFormat, filename: Option<String>) -> Self {
        let mime_type = match format {
            AudioFormat::Wav => "audio/wav",
            AudioFormat::Mp3 => "audio/mp3",
            AudioFormat::M4a => "audio/m4a",
            AudioFormat::Flac => "audio/flac",
            AudioFormat::Ogg => "audio/ogg",
        };
        
        Self::Binary {
            data,
            mime_type: mime_type.to_string(),
            filename,
        }
    }
    
    /// Create a document content block from bytes
    pub fn document(data: Vec<u8>, format: DocumentFormat, filename: Option<String>) -> Self {
        let mime_type = match format {
            DocumentFormat::Pdf => "application/pdf",
            DocumentFormat::Txt => "text/plain",
            DocumentFormat::Csv => "text/csv",
            DocumentFormat::Doc => "application/msword",
            DocumentFormat::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        };
        
        Self::Binary {
            data,
            mime_type: mime_type.to_string(),
            filename,
        }
    }
    
    /// Create an image URL reference
    pub fn image_url(url: &str) -> Self {
        Self::Url {
            url: url.to_string(),
            mime_type: Some("image/*".to_string()),
        }
    }
    
    /// Create a generic binary content block with custom MIME type
    pub fn binary(data: Vec<u8>, mime_type: &str, filename: Option<String>) -> Self {
        Self::Binary {
            data,
            mime_type: mime_type.to_string(),
            filename,
        }
    }
    
    /// Create a generic URL reference with custom MIME type
    pub fn url(url: &str, mime_type: Option<&str>) -> Self {
        Self::Url {
            url: url.to_string(),
            mime_type: mime_type.map(|s| s.to_string()),
        }
    }
}

/// Supported image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Webp,
}

impl ImageFormat {
    /// Get the MIME type for this image format
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Webp => "image/webp",
        }
    }
    
    /// Try to detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "png" => Some(ImageFormat::Png),
            "gif" => Some(ImageFormat::Gif),
            "webp" => Some(ImageFormat::Webp),
            _ => None,
        }
    }
}

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Mp3,
    M4a,
    Flac,
    Ogg,
}

impl AudioFormat {
    /// Get the MIME type for this audio format
    pub fn mime_type(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "audio/wav",
            AudioFormat::Mp3 => "audio/mp3",
            AudioFormat::M4a => "audio/m4a",
            AudioFormat::Flac => "audio/flac",
            AudioFormat::Ogg => "audio/ogg",
        }
    }
    
    /// Try to detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "wav" => Some(AudioFormat::Wav),
            "mp3" => Some(AudioFormat::Mp3),
            "m4a" => Some(AudioFormat::M4a),
            "flac" => Some(AudioFormat::Flac),
            "ogg" => Some(AudioFormat::Ogg),
            _ => None,
        }
    }
}

/// Supported document formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFormat {
    Pdf,
    Txt,
    Csv,
    Doc,
    Docx,
}

impl DocumentFormat {
    /// Get the MIME type for this document format
    pub fn mime_type(&self) -> &'static str {
        match self {
            DocumentFormat::Pdf => "application/pdf",
            DocumentFormat::Txt => "text/plain",
            DocumentFormat::Csv => "text/csv",
            DocumentFormat::Doc => "application/msword",
            DocumentFormat::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        }
    }
    
    /// Try to detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "pdf" => Some(DocumentFormat::Pdf),
            "txt" => Some(DocumentFormat::Txt),
            "csv" => Some(DocumentFormat::Csv),
            "doc" => Some(DocumentFormat::Doc),
            "docx" => Some(DocumentFormat::Docx),
            _ => None,
        }
    }
}

/// Response from a completion request
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub content: String,
    pub role: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
    pub raw_response: Value,
}

impl CompletionResponse {
    /// Get the text content of the response
    pub fn text(&self) -> &str {
        &self.content
    }
    
    /// Calculate the cost of this response if usage information is available
    pub fn cost(&self) -> Option<f64> {
        self.usage.as_ref().and({
            // We need pricing info from somewhere - this would need to be enhanced
            // For now, return None since we don't have pricing context here
            None
        })
    }
}

/// Fluent request builder for creating and sending completion requests
pub struct RequestBuilder {
    pub(crate) model_id: String,
    pub(crate) request: CompletionRequest,
    pub(crate) config_provider: Arc<dyn crate::client::ConfigProvider + Send + Sync>,
}

impl RequestBuilder {
    /// Create a new request builder
    pub(crate) fn new(
        model_id: String, 
        config_provider: Arc<dyn crate::client::ConfigProvider + Send + Sync>
    ) -> crate::error::Result<Self> {
        Ok(Self {
            model_id,
            request: CompletionRequest::new(vec![]),
            config_provider,
        })
    }
    
    /// Get the model ID this request builder is configured for
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
    
    /// Set the system prompt
    pub fn system(mut self, prompt: &str) -> Self {
        self.request.system_prompt = Some(prompt.to_string());
        self
    }
    
    /// Set the temperature parameter
    pub fn temperature(mut self, temp: f64) -> Self {
        self.request.parameters.insert("temperature".to_string(), Value::Number(
            serde_json::Number::from_f64(temp).unwrap_or_else(|| serde_json::Number::from(0))
        ));
        self
    }
    
    /// Set the max_tokens parameter
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.request.parameters.insert("max_tokens".to_string(), Value::Number(
            serde_json::Number::from(tokens)
        ));
        self
    }
    
    /// Set the top_p parameter
    pub fn top_p(mut self, top_p: f64) -> Self {
        self.request.parameters.insert("top_p".to_string(), Value::Number(
            serde_json::Number::from_f64(top_p).unwrap_or_else(|| serde_json::Number::from(0))
        ));
        self
    }
    
    /// Set a custom parameter
    pub fn parameter<T: Into<Value>>(mut self, key: &str, value: T) -> Self {
        self.request.parameters.insert(key.to_string(), value.into());
        self
    }
    
    /// Send a simple text message
    pub async fn send(mut self, content: &str) -> crate::error::Result<CompletionResponse> {
        // Create the message
        let message = MessageContent::Text {
            role: "user".to_string(),
            content: content.to_string(),
        };
        self.request.messages = vec![message];
        
        // Create client and send request
        let client = crate::client::HttpClient::from_model_id(&*self.config_provider, &self.model_id)?;
        client.complete(&self.request).await
    }
    
    /// Send a multimodal message
    pub async fn send_multimodal(mut self, content_blocks: Vec<ContentBlock>) -> crate::error::Result<CompletionResponse> {
        // Create the multimodal message
        let message = MessageContent::Multimodal {
            role: "user".to_string(),
            content: content_blocks,
        };
        self.request.messages = vec![message];
        
        // Create client and send request
        let client = crate::client::HttpClient::from_model_id(&*self.config_provider, &self.model_id)?;
        client.complete(&self.request).await
    }
    
    /// Start a streaming request
    pub async fn stream(mut self, content: &str) -> crate::error::Result<crate::streaming::Stream> {
        // Create the message
        let message = MessageContent::Text {
            role: "user".to_string(),
            content: content.to_string(),
        };
        self.request.messages = vec![message];
        self.request.stream = true;
        
        // Create client and send streaming request
        let client = crate::client::HttpClient::from_model_id(&*self.config_provider, &self.model_id)?;
        client.complete_stream(&self.request).await
    }
    
    /// Create a chat builder for conversation management
    pub fn chat(self, chatter_id: crate::chat::ChatterId) -> crate::chat::ChatBuilder {
        crate::chat::ChatBuilder::new(self, chatter_id)
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: Option<u32>,
}

impl Usage {
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: Some(input_tokens + output_tokens),
        }
    }
    
    /// Calculate cost based on pricing
    pub fn calculate_cost(&self, input_cost_per_1k: f64, output_cost_per_1k: f64) -> f64 {
        let input_cost = (self.input_tokens as f64 / 1000.0) * input_cost_per_1k;
        let output_cost = (self.output_tokens as f64 / 1000.0) * output_cost_per_1k;
        input_cost + output_cost
    }
}

/// Helper trait for creating content from files
pub trait FromFile {
    fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> 
    where 
        Self: Sized;
}

impl FromFile for ContentBlock {
    fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
        let path = path.as_ref();
        let data = std::fs::read(path)?;
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        
        // Try to detect the format from extension
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(image_format) = ImageFormat::from_extension(ext) {
                return Ok(Self::image(data, image_format));
            }
            
            if let Some(audio_format) = AudioFormat::from_extension(ext) {
                return Ok(Self::audio(data, audio_format, filename));
            }
            
            if let Some(doc_format) = DocumentFormat::from_extension(ext) {
                return Ok(Self::document(data, doc_format, filename));
            }
        }
        
        // Default to binary with generic MIME type
        Ok(Self::binary(data, "application/octet-stream", filename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_detection() {
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_content_block_builders() {
        let text_block = ContentBlock::text("Hello world");
        match text_block {
            ContentBlock::Text(content) => assert_eq!(content, "Hello world"),
            _ => panic!("Expected text block"),
        }

        let image_data = vec![0xFF, 0xD8, 0xFF]; // JPEG magic bytes
        let image_block = ContentBlock::image(image_data.clone(), ImageFormat::Jpeg);
        match image_block {
            ContentBlock::Binary { data, mime_type, .. } => {
                assert_eq!(data, image_data);
                assert_eq!(mime_type, "image/jpeg");
            },
            _ => panic!("Expected binary block"),
        }

        let url_block = ContentBlock::image_url("https://example.com/image.jpg");
        match url_block {
            ContentBlock::Url { url, mime_type } => {
                assert_eq!(url, "https://example.com/image.jpg");
                assert_eq!(mime_type, Some("image/*".to_string()));
            },
            _ => panic!("Expected URL block"),
        }
    }

    #[test]
    fn test_completion_request_builders() {
        let request = CompletionRequest::text("user", "Hello")
            .with_system_prompt("You are helpful".to_string())
            .with_parameter("temperature", 0.7)
            .with_streaming(true);

        assert_eq!(request.system_prompt, Some("You are helpful".to_string()));
        assert!(request.stream);

        let multimodal_request = CompletionRequest::multimodal("user", vec![
            ContentBlock::text("What's this?"),
            ContentBlock::image(vec![1, 2, 3], ImageFormat::Png),
        ]);

        match &multimodal_request.messages[0] {
            MessageContent::Multimodal { role, content } => {
                assert_eq!(role, "user");
                assert_eq!(content.len(), 2);
            },
            _ => panic!("Expected multimodal message"),
        }
    }

    #[test]
    fn test_usage_calculations() {
        let usage = Usage::new(1000, 500);
        assert_eq!(usage.total_tokens, Some(1500));

        let cost = usage.calculate_cost(0.01, 0.03); // $0.01 input, $0.03 output per 1k
        assert_eq!(cost, 0.025); // (1000/1000 * 0.01) + (500/1000 * 0.03)
    }
}