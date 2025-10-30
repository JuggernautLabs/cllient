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
    /// let image_data = std::fs::read("photo.jpg").unwrap();
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
    /// let audio_data = std::fs::read("recording.wav").unwrap();
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
    /// let pdf_data = std::fs::read("document.pdf").unwrap();
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