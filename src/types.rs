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

impl MessageContent {
    /// Get the role of this message
    pub fn role(&self) -> &str {
        match self {
            MessageContent::Text { role, .. } => role,
            MessageContent::Multimodal { role, .. } => role,
        }
    }
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

/// A list structure for managing messages with modification capabilities
#[derive(Debug, Clone, Default)]
pub struct MessageList {
    messages: Vec<Message>,
}

impl MessageList {
    /// Create a new empty message list
    ///
    /// # Examples
    /// ```
    /// use cllient::MessageList;
    /// let msgs = MessageList::new();
    /// ```
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Create a new message list with pre-allocated capacity
    ///
    /// # Examples
    /// ```
    /// use cllient::MessageList;
    /// let msgs = MessageList::with_capacity(10);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            messages: Vec::with_capacity(capacity),
        }
    }

    /// Create a message list from an existing Vec<Message>
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let msgs = MessageList::from_vec(vec![
    ///     Message::user("Hello"),
    ///     Message::assistant("Hi!"),
    /// ]);
    /// ```
    pub fn from_vec(messages: Vec<Message>) -> Self {
        Self { messages }
    }

    /// Add a message to the end of the list
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let mut msgs = MessageList::new();
    /// msgs.push(Message::user("Hello"));
    /// msgs.push(Message::assistant("Hi!"));
    /// ```
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Get a reference to a message at the specified index
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let mut msgs = MessageList::new();
    /// msgs.push(Message::user("Hello"));
    /// assert_eq!(msgs.get(0).unwrap().role(), "user");
    /// ```
    pub fn get(&self, index: usize) -> Option<&Message> {
        self.messages.get(index)
    }

    /// Get a mutable reference to a message at the specified index
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let mut msgs = MessageList::new();
    /// msgs.push(Message::user("Hello"));
    /// if let Some(msg) = msgs.get_mut(0) {
    ///     // Can modify the message
    /// }
    /// ```
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Message> {
        self.messages.get_mut(index)
    }

    /// Replace a message at the specified index, returning the old message
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let mut msgs = MessageList::new();
    /// msgs.push(Message::user("Hello"));
    /// let old = msgs.set(0, Message::user("Hi"));
    /// assert!(old.is_some());
    /// ```
    pub fn set(&mut self, index: usize, message: Message) -> Option<Message> {
        if index < self.messages.len() {
            Some(std::mem::replace(&mut self.messages[index], message))
        } else {
            None
        }
    }

    /// Remove a message at the specified index, returning it
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let mut msgs = MessageList::new();
    /// msgs.push(Message::user("Hello"));
    /// msgs.push(Message::assistant("Hi!"));
    /// let removed = msgs.remove(0);
    /// assert!(removed.is_some());
    /// assert_eq!(msgs.len(), 1);
    /// ```
    pub fn remove(&mut self, index: usize) -> Option<Message> {
        if index < self.messages.len() {
            Some(self.messages.remove(index))
        } else {
            None
        }
    }

    /// Insert a message at the specified index
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let mut msgs = MessageList::new();
    /// msgs.push(Message::user("Hello"));
    /// msgs.push(Message::assistant("Hi!"));
    /// msgs.insert(1, Message::system("Be helpful"));
    /// assert_eq!(msgs.len(), 3);
    /// ```
    pub fn insert(&mut self, index: usize, message: Message) {
        self.messages.insert(index, message);
    }

    /// Clear all messages and return them as a Vec
    ///
    /// # Examples
    /// ```
    /// use cllient::{MessageList, Message};
    /// let mut msgs = MessageList::new();
    /// msgs.push(Message::user("Hello"));
    /// msgs.push(Message::assistant("Hi!"));
    /// let all_messages = msgs.clear();
    /// assert_eq!(all_messages.len(), 2);
    /// assert_eq!(msgs.len(), 0);
    /// ```
    pub fn clear(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }

    /// Get the number of messages in the list
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the message list is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get an iterator over the messages
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter()
    }

    /// Get an mutable iterator over the messages
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Message> {
        self.messages.iter_mut()
    }

    /// Get a slice of all messages
    pub fn as_slice(&self) -> &[Message] {
        &self.messages
    }

    /// Convert the message list into a Vec<Message>, consuming self
    pub fn into_vec(self) -> Vec<Message> {
        self.messages
    }

    /// Clone the messages into a new Vec
    pub fn to_vec(&self) -> Vec<Message> {
        self.messages.clone()
    }

    /// Extend the message list with messages from an iterator
    pub fn extend<I: IntoIterator<Item = Message>>(&mut self, iter: I) {
        self.messages.extend(iter);
    }

    /// Retain only the messages that satisfy the predicate
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Message) -> bool,
    {
        self.messages.retain(f);
    }
}

impl From<Vec<Message>> for MessageList {
    fn from(messages: Vec<Message>) -> Self {
        Self { messages }
    }
}

impl From<MessageList> for Vec<Message> {
    fn from(list: MessageList) -> Self {
        list.messages
    }
}

impl IntoIterator for MessageList {
    type Item = Message;
    type IntoIter = std::vec::IntoIter<Message>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.into_iter()
    }
}

impl<'a> IntoIterator for &'a MessageList {
    type Item = &'a Message;
    type IntoIter = std::slice::Iter<'a, Message>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.iter()
    }
}

impl std::ops::Index<usize> for MessageList {
    type Output = Message;

    fn index(&self, index: usize) -> &Self::Output {
        &self.messages[index]
    }
}

impl std::ops::IndexMut<usize> for MessageList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.messages[index]
    }
}

/// Builder for constructing messages with specific roles and content
#[derive(Debug, Clone)]
pub struct Message {
    role: String,
    content: Vec<ContentBlock>,
}

impl Message {
    /// Create a message from the user role
    ///
    /// # Examples
    /// ```
    /// use cllient::Message;
    /// let msg = Message::user("Hello, how are you?");
    /// ```
    pub fn user(text: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![ContentBlock::text(text)],
        }
    }

    /// Create a message from the assistant role
    ///
    /// # Examples
    /// ```
    /// use cllient::Message;
    /// let msg = Message::assistant("I'm doing great, thanks!");
    /// ```
    pub fn assistant(text: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![ContentBlock::text(text)],
        }
    }

    /// Create a message from the system role
    ///
    /// # Examples
    /// ```
    /// use cllient::Message;
    /// let msg = Message::system("You are a helpful assistant.");
    /// ```
    pub fn system(text: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: vec![ContentBlock::text(text)],
        }
    }

    /// Create a message with a custom role
    ///
    /// # Examples
    /// ```
    /// use cllient::Message;
    /// let msg = Message::custom("elmo").add_text("Elmo loves to help!");
    /// ```
    pub fn custom(role: &str) -> Self {
        Self {
            role: role.to_string(),
            content: vec![],
        }
    }

    /// Create a user message with multimodal content
    ///
    /// # Examples
    /// ```
    /// use cllient::{Message, ContentBlock, ImageFormat};
    /// let image_data = vec![0xFF, 0xD8, 0xFF]; // JPEG data
    /// let msg = Message::user_multimodal(vec![
    ///     ContentBlock::text("What's in this image?"),
    ///     ContentBlock::image(image_data, ImageFormat::Jpeg),
    /// ]);
    /// ```
    pub fn user_multimodal(content: Vec<ContentBlock>) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }

    /// Add text content to the message
    ///
    /// # Examples
    /// ```
    /// use cllient::Message;
    /// let msg = Message::custom("narrator")
    ///     .add_text("Once upon a time...")
    ///     .add_text("in a galaxy far away...");
    /// ```
    pub fn add_text(mut self, text: &str) -> Self {
        self.content.push(ContentBlock::text(text));
        self
    }

    /// Add an image to the message
    ///
    /// # Examples
    /// ```
    /// use cllient::{Message, ImageFormat};
    /// let image_data = vec![0xFF, 0xD8, 0xFF]; // JPEG header bytes
    /// let msg = Message::user("Check this out")
    ///     .add_image(image_data, ImageFormat::Jpeg);
    /// ```
    pub fn add_image(mut self, data: Vec<u8>, format: ImageFormat) -> Self {
        self.content.push(ContentBlock::image(data, format));
        self
    }

    /// Add an image from a URL
    ///
    /// # Examples
    /// ```
    /// use cllient::Message;
    /// let msg = Message::user("What's in this image?")
    ///     .add_image_url("https://example.com/photo.jpg");
    /// ```
    pub fn add_image_url(mut self, url: &str) -> Self {
        self.content.push(ContentBlock::image_url(url));
        self
    }

    /// Add audio content to the message
    ///
    /// # Examples
    /// ```
    /// use cllient::{Message, AudioFormat};
    /// let audio_data = vec![0x52, 0x49, 0x46, 0x46]; // WAV header bytes
    /// let msg = Message::user("Transcribe this")
    ///     .add_audio(audio_data, AudioFormat::Wav, Some("recording.wav".to_string()));
    /// ```
    pub fn add_audio(mut self, data: Vec<u8>, format: AudioFormat, filename: Option<String>) -> Self {
        self.content.push(ContentBlock::audio(data, format, filename));
        self
    }

    /// Add a document to the message
    ///
    /// # Examples
    /// ```
    /// use cllient::{Message, DocumentFormat};
    /// let pdf_data = vec![0x25, 0x50, 0x44, 0x46]; // PDF header bytes
    /// let msg = Message::user("Summarize this document")
    ///     .add_document(pdf_data, DocumentFormat::Pdf, Some("document.pdf".to_string()));
    /// ```
    pub fn add_document(mut self, data: Vec<u8>, format: DocumentFormat, filename: Option<String>) -> Self {
        self.content.push(ContentBlock::document(data, format, filename));
        self
    }

    /// Add a content block to the message
    ///
    /// # Examples
    /// ```
    /// use cllient::{Message, ContentBlock};
    /// let msg = Message::user("Hello")
    ///     .add_content(ContentBlock::text("World"));
    /// ```
    pub fn add_content(mut self, block: ContentBlock) -> Self {
        self.content.push(block);
        self
    }

    /// Set the content blocks directly (replaces existing content)
    ///
    /// # Examples
    /// ```
    /// use cllient::{Message, ContentBlock};
    /// let msg = Message::custom("narrator")
    ///     .content(vec![
    ///         ContentBlock::text("Chapter 1"),
    ///         ContentBlock::text("The Beginning"),
    ///     ]);
    /// ```
    pub fn content(mut self, blocks: Vec<ContentBlock>) -> Self {
        self.content = blocks;
        self
    }

    /// Convert the message into MessageContent for internal use
    pub(crate) fn into_message_content(self) -> MessageContent {
        if self.content.len() == 1 {
            if let ContentBlock::Text(text) = &self.content[0] {
                return MessageContent::Text {
                    role: self.role,
                    content: text.clone(),
                };
            }
        }

        MessageContent::Multimodal {
            role: self.role,
            content: self.content,
        }
    }

    /// Get the role of this message
    pub fn role(&self) -> &str {
        &self.role
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

    /// Set the prompt text (convenience method for setting the user message)
    /// This allows you to chain: `registry.from_id("gpt-4o-mini")?.prompt("Hello").send().await?`
    pub fn prompt(mut self, content: &str) -> Self {
        self.request.messages = vec![MessageContent::Text {
            role: "user".to_string(),
            content: content.to_string(),
        }];
        self
    }

    /// Append a message to the conversation
    ///
    /// This allows you to build multi-turn conversations with custom roles.
    ///
    /// # Examples
    ///
    /// ## Simple conversation
    /// ```no_run
    /// # use cllient::{ModelRegistry, Message};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = ModelRegistry::new()?;
    /// let response = registry
    ///     .from_id("gpt-4o-mini")?
    ///     .append_message(Message::user("What is 2+2?"))
    ///     .append_message(Message::assistant("4"))
    ///     .append_message(Message::user("And what's 4+4?"))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Custom roles
    /// ```no_run
    /// # use cllient::{ModelRegistry, Message};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = ModelRegistry::new()?;
    /// let response = registry
    ///     .from_id("claude-3-haiku-20240307")?
    ///     .append_message(Message::custom("narrator").add_text("Once upon a time..."))
    ///     .append_message(Message::custom("elmo").add_text("Elmo loves stories!"))
    ///     .append_message(Message::user("Continue the story"))
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Multimodal content
    /// ```no_run
    /// # use cllient::{ModelRegistry, Message, ContentBlock, ImageFormat};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let image_data = std::fs::read("photo.jpg")?;
    /// let registry = ModelRegistry::new()?;
    /// let response = registry
    ///     .from_id("gpt-4o")?
    ///     .append_message(
    ///         Message::user("What's in this image?")
    ///             .add_image(image_data, ImageFormat::Jpeg)
    ///     )
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn append_message(mut self, message: Message) -> Self {
        self.request.messages.push(message.into_message_content());
        self
    }

    /// Set all messages at once from an array
    ///
    /// This replaces any previously set messages (including those from `.prompt()` or `.append_message()`).
    ///
    /// # Examples
    ///
    /// ## From a Vec
    /// ```no_run
    /// # use cllient::{ModelRegistry, Message};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = ModelRegistry::new()?;
    ///
    /// let conversation = vec![
    ///     Message::system("You are a helpful assistant"),
    ///     Message::user("What is 2+2?"),
    ///     Message::assistant("4"),
    ///     Message::user("And 4+4?"),
    /// ];
    ///
    /// let response = registry
    ///     .from_id("gpt-4o-mini")?
    ///     .messages(conversation)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Building a conversation dynamically
    /// ```no_run
    /// # use cllient::{ModelRegistry, Message};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut msgs = vec![Message::system("You are a coding assistant")];
    ///
    /// // Add conversation history
    /// for (role, text) in &[("user", "Explain Rust"), ("assistant", "Rust is...")] {
    ///     msgs.push(if *role == "user" {
    ///         Message::user(text)
    ///     } else {
    ///         Message::assistant(text)
    ///     });
    /// }
    ///
    /// msgs.push(Message::user("Tell me more"));
    ///
    /// let registry = ModelRegistry::new()?;
    /// let response = registry
    ///     .from_id("deepseek-chat")?
    ///     .messages(msgs)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## With custom roles
    /// ```no_run
    /// # use cllient::{ModelRegistry, Message};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let registry = ModelRegistry::new()?;
    ///
    /// let response = registry
    ///     .from_id("claude-3-haiku-20240307")?
    ///     .messages(vec![
    ///         Message::custom("narrator").add_text("The scene opens..."),
    ///         Message::custom("hero").add_text("I will save the day!"),
    ///         Message::custom("villain").add_text("Not so fast!"),
    ///         Message::user("Continue the story"),
    ///     ])
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn messages<T: Into<Vec<Message>>>(mut self, messages: T) -> Self {
        self.request.messages = messages
            .into()
            .into_iter()
            .map(|m| m.into_message_content())
            .collect();
        self
    }

    /// Send the request with the configured messages
    /// Use `.prompt("text")` before calling this to set the message, or use `.send_text("text")` as a shortcut
    pub async fn send(self) -> crate::error::Result<CompletionResponse> {
        if self.request.messages.is_empty() {
            return Err(crate::error::Error::ValidationError(
                "No messages set. Use .prompt(text) or .send_text(text) instead.".to_string()
            ));
        }

        // Create client and send request
        let client = crate::client::HttpClient::from_model_id(&*self.config_provider, &self.model_id)?;
        client.complete(&self.request).await
    }

    /// Convenience method to set a text message and send in one call
    pub async fn send_text(mut self, content: &str) -> crate::error::Result<CompletionResponse> {
        let message = MessageContent::Text {
            role: "user".to_string(),
            content: content.to_string(),
        };
        self.request.messages = vec![message];

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
    
    /// Start a streaming request with the configured messages
    /// Use `.prompt("text")` before calling this to set the message, or use `.stream_text("text")` as a shortcut
    pub async fn stream(mut self) -> crate::error::Result<crate::streaming::Stream> {
        if self.request.messages.is_empty() {
            return Err(crate::error::Error::ValidationError(
                "No messages set. Use .prompt(text) or .stream_text(text) instead.".to_string()
            ));
        }

        self.request.stream = true;

        // Create client and send streaming request
        let client = crate::client::HttpClient::from_model_id(&*self.config_provider, &self.model_id)?;
        client.complete_stream(&self.request).await
    }

    /// Convenience method to set a text message and stream in one call
    pub async fn stream_text(mut self, content: &str) -> crate::error::Result<crate::streaming::Stream> {
        let message = MessageContent::Text {
            role: "user".to_string(),
            content: content.to_string(),
        };
        self.request.messages = vec![message];
        self.request.stream = true;

        let client = crate::client::HttpClient::from_model_id(&*self.config_provider, &self.model_id)?;
        client.complete_stream(&self.request).await
    }
    
    /// Create a chat builder for conversation management
    pub fn chat(self, chatter_id: crate::chat::ChatterId) -> crate::chat::ChatBuilder {
        crate::chat::ChatBuilder::new(self, chatter_id)
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: Option<u32>,
    /// Cache read tokens (Anthropic)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,
    /// Cache creation tokens (Anthropic)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u32>,
}

impl Usage {
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: Some(input_tokens + output_tokens),
            cache_read_tokens: None,
            cache_creation_tokens: None,
        }
    }

    /// Create a Usage with all fields specified
    pub fn with_cache(
        input_tokens: u32,
        output_tokens: u32,
        cache_read_tokens: Option<u32>,
        cache_creation_tokens: Option<u32>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: Some(input_tokens + output_tokens),
            cache_read_tokens,
            cache_creation_tokens,
        }
    }

    /// Calculate cost based on pricing
    pub fn calculate_cost(&self, input_cost_per_1k: f64, output_cost_per_1k: f64) -> f64 {
        let input_cost = (self.input_tokens as f64 / 1000.0) * input_cost_per_1k;
        let output_cost = (self.output_tokens as f64 / 1000.0) * output_cost_per_1k;
        input_cost + output_cost
    }

    /// Calculate cost including cache pricing (for Anthropic)
    ///
    /// # Arguments
    /// * `input_cost_per_1k` - Cost per 1K input tokens
    /// * `output_cost_per_1k` - Cost per 1K output tokens
    /// * `cache_read_cost_per_1k` - Cost per 1K cache read tokens (typically discounted)
    /// * `cache_creation_cost_per_1k` - Cost per 1K cache creation tokens (typically premium)
    pub fn calculate_cost_with_cache(
        &self,
        input_cost_per_1k: f64,
        output_cost_per_1k: f64,
        cache_read_cost_per_1k: f64,
        cache_creation_cost_per_1k: f64,
    ) -> f64 {
        let input_cost = (self.input_tokens as f64 / 1000.0) * input_cost_per_1k;
        let output_cost = (self.output_tokens as f64 / 1000.0) * output_cost_per_1k;
        let cache_read_cost = self.cache_read_tokens
            .map(|t| (t as f64 / 1000.0) * cache_read_cost_per_1k)
            .unwrap_or(0.0);
        let cache_creation_cost = self.cache_creation_tokens
            .map(|t| (t as f64 / 1000.0) * cache_creation_cost_per_1k)
            .unwrap_or(0.0);
        input_cost + output_cost + cache_read_cost + cache_creation_cost
    }
}

/// Configuration for extracting usage/token count data from JSON responses
///
/// This struct defines JSON paths for extracting token usage information from
/// different LLM provider responses. Each field is an optional JSON path string
/// using dot notation with array index support (e.g., "usage.prompt_tokens" or
/// "choices[0].usage.input_tokens").
///
/// # Example
///
/// ```
/// use cllient::UsagePathConfig;
/// use serde_json::json;
///
/// // OpenAI-style response
/// let config = UsagePathConfig::builder()
///     .input_tokens("usage.prompt_tokens")
///     .output_tokens("usage.completion_tokens")
///     .total_tokens("usage.total_tokens")
///     .build();
///
/// let response = json!({
///     "usage": {
///         "prompt_tokens": 100,
///         "completion_tokens": 50,
///         "total_tokens": 150
///     }
/// });
///
/// let usage = config.extract(&response);
/// assert_eq!(usage.input_tokens, 100);
/// assert_eq!(usage.output_tokens, 50);
/// assert_eq!(usage.total_tokens, Some(150));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsagePathConfig {
    /// Path to input/prompt token count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<String>,
    /// Path to output/completion token count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<String>,
    /// Path to total token count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<String>,
    /// Path to cache read tokens (Anthropic)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<String>,
    /// Path to cache creation tokens (Anthropic)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<String>,
}

impl UsagePathConfig {
    /// Create a new empty UsagePathConfig
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder for constructing UsagePathConfig
    pub fn builder() -> UsagePathBuilder {
        UsagePathBuilder::new()
    }

    /// Create an OpenAI-compatible usage path configuration
    pub fn openai() -> Self {
        Self::builder()
            .input_tokens("usage.prompt_tokens")
            .output_tokens("usage.completion_tokens")
            .total_tokens("usage.total_tokens")
            .build()
    }

    /// Create an Anthropic-compatible usage path configuration
    pub fn anthropic() -> Self {
        Self::builder()
            .input_tokens("usage.input_tokens")
            .output_tokens("usage.output_tokens")
            .cache_read_tokens("usage.cache_read_input_tokens")
            .cache_creation_tokens("usage.cache_creation_input_tokens")
            .build()
    }

    /// Create a Google Gemini-compatible usage path configuration
    pub fn google() -> Self {
        Self::builder()
            .input_tokens("usageMetadata.promptTokenCount")
            .output_tokens("usageMetadata.candidatesTokenCount")
            .total_tokens("usageMetadata.totalTokenCount")
            .build()
    }

    /// Extract usage information from a JSON value using the configured paths
    ///
    /// Returns a Usage struct with extracted values. Fields that cannot be
    /// extracted will default to 0 (for required fields) or None (for optional fields).
    pub fn extract(&self, value: &Value) -> Usage {
        let input_tokens = self.input_tokens.as_ref()
            .and_then(|path| extract_json_path(path, value))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);

        let output_tokens = self.output_tokens.as_ref()
            .and_then(|path| extract_json_path(path, value))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(0);

        let total_tokens = self.total_tokens.as_ref()
            .and_then(|path| extract_json_path(path, value))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let cache_read_tokens = self.cache_read_tokens.as_ref()
            .and_then(|path| extract_json_path(path, value))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let cache_creation_tokens = self.cache_creation_tokens.as_ref()
            .and_then(|path| extract_json_path(path, value))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        Usage {
            input_tokens,
            output_tokens,
            total_tokens,
            cache_read_tokens,
            cache_creation_tokens,
        }
    }

    /// Check if all paths are None (empty configuration)
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
            && self.cache_read_tokens.is_none()
            && self.cache_creation_tokens.is_none()
    }
}

/// Builder for constructing UsagePathConfig with a fluent API
///
/// # Example
///
/// ```
/// use cllient::UsagePathBuilder;
///
/// let config = UsagePathBuilder::new()
///     .input_tokens("usage.prompt_tokens")
///     .output_tokens("usage.completion_tokens")
///     .total_tokens("usage.total_tokens")
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct UsagePathBuilder {
    input_tokens: Option<String>,
    output_tokens: Option<String>,
    total_tokens: Option<String>,
    cache_read_tokens: Option<String>,
    cache_creation_tokens: Option<String>,
}

impl UsagePathBuilder {
    /// Create a new builder with no paths configured
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the path to input/prompt token count
    pub fn input_tokens(mut self, path: impl Into<String>) -> Self {
        self.input_tokens = Some(path.into());
        self
    }

    /// Set the path to output/completion token count
    pub fn output_tokens(mut self, path: impl Into<String>) -> Self {
        self.output_tokens = Some(path.into());
        self
    }

    /// Set the path to total token count
    pub fn total_tokens(mut self, path: impl Into<String>) -> Self {
        self.total_tokens = Some(path.into());
        self
    }

    /// Set the path to cache read tokens (Anthropic)
    pub fn cache_read_tokens(mut self, path: impl Into<String>) -> Self {
        self.cache_read_tokens = Some(path.into());
        self
    }

    /// Set the path to cache creation tokens (Anthropic)
    pub fn cache_creation_tokens(mut self, path: impl Into<String>) -> Self {
        self.cache_creation_tokens = Some(path.into());
        self
    }

    /// Build the UsagePathConfig
    pub fn build(self) -> UsagePathConfig {
        UsagePathConfig {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            total_tokens: self.total_tokens,
            cache_read_tokens: self.cache_read_tokens,
            cache_creation_tokens: self.cache_creation_tokens,
        }
    }
}

/// Extract a value from a JSON object using a dot-notation path with array index support
///
/// Supports paths like:
/// - "usage.prompt_tokens"
/// - "choices[0].message.content"
/// - "usageMetadata.promptTokenCount"
/// - "message.usage.input_tokens"
fn extract_json_path(path: &str, json: &Value) -> Option<Value> {
    let mut current = json;
    let mut i = 0;
    let chars: Vec<char> = path.chars().collect();

    while i < chars.len() {
        // Find the next part (either before '.' or before '[')
        let mut part_end = i;
        while part_end < chars.len() && chars[part_end] != '.' && chars[part_end] != '[' {
            part_end += 1;
        }

        if part_end > i {
            // Extract object key
            let key: String = chars[i..part_end].iter().collect();
            current = current.get(&key)?;
            i = part_end;
        }

        // Handle array access
        if i < chars.len() && chars[i] == '[' {
            i += 1; // Skip '['
            let mut index_end = i;
            while index_end < chars.len() && chars[index_end] != ']' {
                index_end += 1;
            }

            if index_end >= chars.len() {
                return None; // Unclosed bracket
            }

            let index_str: String = chars[i..index_end].iter().collect();
            let index: usize = index_str.parse().ok()?;
            current = current.get(index)?;
            i = index_end + 1; // Skip ']'
        }

        // Skip '.'
        if i < chars.len() && chars[i] == '.' {
            i += 1;
        }
    }

    Some(current.clone())
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
    use serde_json::json;

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

    // =========================================================================
    // UsagePathConfig Tests
    // =========================================================================

    #[test]
    fn test_usage_path_builder_basic() {
        let config = UsagePathBuilder::new()
            .input_tokens("usage.prompt_tokens")
            .output_tokens("usage.completion_tokens")
            .build();

        assert_eq!(config.input_tokens, Some("usage.prompt_tokens".to_string()));
        assert_eq!(config.output_tokens, Some("usage.completion_tokens".to_string()));
        assert_eq!(config.total_tokens, None);
        assert_eq!(config.cache_read_tokens, None);
        assert_eq!(config.cache_creation_tokens, None);
    }

    #[test]
    fn test_usage_path_builder_full() {
        let config = UsagePathBuilder::new()
            .input_tokens("usage.input_tokens")
            .output_tokens("usage.output_tokens")
            .total_tokens("usage.total_tokens")
            .cache_read_tokens("usage.cache_read")
            .cache_creation_tokens("usage.cache_creation")
            .build();

        assert_eq!(config.input_tokens, Some("usage.input_tokens".to_string()));
        assert_eq!(config.output_tokens, Some("usage.output_tokens".to_string()));
        assert_eq!(config.total_tokens, Some("usage.total_tokens".to_string()));
        assert_eq!(config.cache_read_tokens, Some("usage.cache_read".to_string()));
        assert_eq!(config.cache_creation_tokens, Some("usage.cache_creation".to_string()));
    }

    #[test]
    fn test_usage_path_config_openai_preset() {
        let config = UsagePathConfig::openai();

        assert_eq!(config.input_tokens, Some("usage.prompt_tokens".to_string()));
        assert_eq!(config.output_tokens, Some("usage.completion_tokens".to_string()));
        assert_eq!(config.total_tokens, Some("usage.total_tokens".to_string()));
        assert_eq!(config.cache_read_tokens, None);
    }

    #[test]
    fn test_usage_path_config_anthropic_preset() {
        let config = UsagePathConfig::anthropic();

        assert_eq!(config.input_tokens, Some("usage.input_tokens".to_string()));
        assert_eq!(config.output_tokens, Some("usage.output_tokens".to_string()));
        assert_eq!(config.total_tokens, None);
        assert_eq!(config.cache_read_tokens, Some("usage.cache_read_input_tokens".to_string()));
        assert_eq!(config.cache_creation_tokens, Some("usage.cache_creation_input_tokens".to_string()));
    }

    #[test]
    fn test_usage_path_config_google_preset() {
        let config = UsagePathConfig::google();

        assert_eq!(config.input_tokens, Some("usageMetadata.promptTokenCount".to_string()));
        assert_eq!(config.output_tokens, Some("usageMetadata.candidatesTokenCount".to_string()));
        assert_eq!(config.total_tokens, Some("usageMetadata.totalTokenCount".to_string()));
    }

    #[test]
    fn test_usage_extract_openai_response() {
        let config = UsagePathConfig::openai();

        let response = json!({
            "id": "chatcmpl-123",
            "choices": [{
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, Some(150));
        assert_eq!(usage.cache_read_tokens, None);
        assert_eq!(usage.cache_creation_tokens, None);
    }

    #[test]
    fn test_usage_extract_anthropic_response() {
        let config = UsagePathConfig::anthropic();

        let response = json!({
            "id": "msg_123",
            "role": "assistant",
            "content": [{"type": "text", "text": "Hello!"}],
            "usage": {
                "input_tokens": 200,
                "output_tokens": 75,
                "cache_read_input_tokens": 50,
                "cache_creation_input_tokens": 25
            }
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 200);
        assert_eq!(usage.output_tokens, 75);
        assert_eq!(usage.total_tokens, None);
        assert_eq!(usage.cache_read_tokens, Some(50));
        assert_eq!(usage.cache_creation_tokens, Some(25));
    }

    #[test]
    fn test_usage_extract_google_response() {
        let config = UsagePathConfig::google();

        let response = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Hello!"}], "role": "model"}
            }],
            "usageMetadata": {
                "promptTokenCount": 150,
                "candidatesTokenCount": 80,
                "totalTokenCount": 230
            }
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 150);
        assert_eq!(usage.output_tokens, 80);
        assert_eq!(usage.total_tokens, Some(230));
    }

    #[test]
    fn test_usage_extract_with_array_index() {
        let config = UsagePathBuilder::new()
            .input_tokens("responses[0].usage.input")
            .output_tokens("responses[0].usage.output")
            .build();

        let response = json!({
            "responses": [{
                "usage": {
                    "input": 300,
                    "output": 125
                }
            }]
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 125);
    }

    #[test]
    fn test_usage_extract_nested_array() {
        let config = UsagePathBuilder::new()
            .input_tokens("data[0].results[1].tokens.input")
            .output_tokens("data[0].results[1].tokens.output")
            .build();

        let response = json!({
            "data": [{
                "results": [
                    {"tokens": {"input": 10, "output": 5}},
                    {"tokens": {"input": 400, "output": 200}}
                ]
            }]
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 400);
        assert_eq!(usage.output_tokens, 200);
    }

    #[test]
    fn test_usage_extract_missing_path_returns_zero() {
        let config = UsagePathConfig::openai();

        let response = json!({
            "id": "chatcmpl-123",
            "choices": [{"message": {"content": "Hello!"}}]
            // No usage field
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, None);
    }

    #[test]
    fn test_usage_extract_partial_response() {
        let config = UsagePathConfig::openai();

        let response = json!({
            "usage": {
                "prompt_tokens": 100
                // Missing completion_tokens and total_tokens
            }
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, None);
    }

    #[test]
    fn test_usage_extract_empty_config() {
        let config = UsagePathConfig::new();

        let response = json!({
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50
            }
        });

        let usage = config.extract(&response);
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, None);
    }

    #[test]
    fn test_usage_path_config_is_empty() {
        let empty = UsagePathConfig::new();
        assert!(empty.is_empty());

        let with_input = UsagePathBuilder::new()
            .input_tokens("usage.input")
            .build();
        assert!(!with_input.is_empty());

        let full = UsagePathConfig::openai();
        assert!(!full.is_empty());
    }

    #[test]
    fn test_usage_path_config_serialization() {
        let config = UsagePathConfig::openai();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: UsagePathConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_usage_path_config_yaml_deserialization() {
        let yaml = r#"
input_tokens: usage.prompt_tokens
output_tokens: usage.completion_tokens
total_tokens: usage.total_tokens
"#;
        let config: UsagePathConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.input_tokens, Some("usage.prompt_tokens".to_string()));
        assert_eq!(config.output_tokens, Some("usage.completion_tokens".to_string()));
        assert_eq!(config.total_tokens, Some("usage.total_tokens".to_string()));
    }

    #[test]
    fn test_extract_json_path_simple() {
        let json = json!({"usage": {"tokens": 100}});
        let result = extract_json_path("usage.tokens", &json);
        assert_eq!(result, Some(json!(100)));
    }

    #[test]
    fn test_extract_json_path_array() {
        let json = json!({"items": [{"value": 1}, {"value": 2}]});
        let result = extract_json_path("items[1].value", &json);
        assert_eq!(result, Some(json!(2)));
    }

    #[test]
    fn test_extract_json_path_deeply_nested() {
        let json = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": "deep"
                    }
                }
            }
        });
        let result = extract_json_path("level1.level2.level3.value", &json);
        assert_eq!(result, Some(json!("deep")));
    }

    #[test]
    fn test_extract_json_path_invalid() {
        let json = json!({"usage": {"tokens": 100}});

        // Invalid key
        let result = extract_json_path("nonexistent.path", &json);
        assert_eq!(result, None);

        // Invalid array index
        let result = extract_json_path("usage[0]", &json);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_json_path_unclosed_bracket() {
        let json = json!({"items": [{"value": 1}]});
        let result = extract_json_path("items[0", &json);
        assert_eq!(result, None);
    }

    #[test]
    fn test_usage_with_cache_constructor() {
        let usage = Usage::with_cache(1000, 500, Some(200), Some(100));

        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 500);
        assert_eq!(usage.total_tokens, Some(1500));
        assert_eq!(usage.cache_read_tokens, Some(200));
        assert_eq!(usage.cache_creation_tokens, Some(100));
    }

    #[test]
    fn test_usage_calculate_cost_with_cache() {
        let usage = Usage::with_cache(1000, 500, Some(1000), Some(500));

        // $0.01 input, $0.03 output, $0.005 cache read, $0.02 cache creation per 1k
        let cost = usage.calculate_cost_with_cache(0.01, 0.03, 0.005, 0.02);

        // (1000/1000 * 0.01) + (500/1000 * 0.03) + (1000/1000 * 0.005) + (500/1000 * 0.02)
        // = 0.01 + 0.015 + 0.005 + 0.01 = 0.04
        assert!((cost - 0.04).abs() < 0.0001);
    }

    #[test]
    fn test_usage_calculate_cost_with_cache_no_cache_used() {
        let usage = Usage::new(1000, 500);

        // Without cache tokens, should still work
        let cost = usage.calculate_cost_with_cache(0.01, 0.03, 0.005, 0.02);

        // Just input + output: 0.01 + 0.015 = 0.025
        assert!((cost - 0.025).abs() < 0.0001);
    }

    #[test]
    fn test_streaming_usage_path_extraction() {
        // Simulates usage extraction from streaming response (OpenAI with include_usage=true)
        let config = UsagePathBuilder::new()
            .input_tokens("usage.prompt_tokens")
            .output_tokens("usage.completion_tokens")
            .total_tokens("usage.total_tokens")
            .build();

        let streaming_chunk = json!({
            "id": "chatcmpl-123",
            "choices": [],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 25,
                "total_tokens": 75
            }
        });

        let usage = config.extract(&streaming_chunk);
        assert_eq!(usage.input_tokens, 50);
        assert_eq!(usage.output_tokens, 25);
        assert_eq!(usage.total_tokens, Some(75));
    }

    #[test]
    fn test_anthropic_streaming_usage_paths() {
        // Anthropic streaming has different structure for message_start vs message_delta
        let message_start_config = UsagePathBuilder::new()
            .input_tokens("message.usage.input_tokens")
            .cache_read_tokens("message.usage.cache_read_input_tokens")
            .build();

        let message_start = json!({
            "type": "message_start",
            "message": {
                "usage": {
                    "input_tokens": 100,
                    "cache_read_input_tokens": 50
                }
            }
        });

        let usage = message_start_config.extract(&message_start);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.cache_read_tokens, Some(50));

        // For message_delta, output tokens come differently
        let message_delta_config = UsagePathBuilder::new()
            .output_tokens("usage.output_tokens")
            .build();

        let message_delta = json!({
            "type": "message_delta",
            "usage": {
                "output_tokens": 75
            }
        });

        let delta_usage = message_delta_config.extract(&message_delta);
        assert_eq!(delta_usage.output_tokens, 75);
    }

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, None);
        assert_eq!(usage.cache_read_tokens, None);
        assert_eq!(usage.cache_creation_tokens, None);
    }

    #[test]
    fn test_usage_path_from_builder_via_config() {
        // Test that UsagePathConfig::builder() returns a working builder
        let config = UsagePathConfig::builder()
            .input_tokens("a.b.c")
            .output_tokens("x.y.z")
            .build();

        assert_eq!(config.input_tokens, Some("a.b.c".to_string()));
        assert_eq!(config.output_tokens, Some("x.y.z".to_string()));
    }
}