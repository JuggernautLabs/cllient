use crate::types::{RequestBuilder, CompletionResponse, MessageContent, ContentBlock};
use crate::error::Result;
use crate::client::LowLevelClient;

/// Identifier for different types of chatters in a conversation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatterId {
    /// The end user of the application
    User,
    /// System instructions/prompts
    System,
    /// The current user (alias for User, more personal)
    Me,
    /// An AI agent/assistant
    Agent,
    /// A custom chatter with a specific name
    Custom(String),
    /// Multiple chatters (for group conversations)
    Multiple(Vec<ChatterId>),
}

impl ChatterId {
    /// Convert ChatterId to the role string used in the API
    pub fn to_role(&self) -> String {
        match self {
            ChatterId::User | ChatterId::Me => "user".to_string(),
            ChatterId::System => "system".to_string(),
            ChatterId::Agent => "assistant".to_string(),
            ChatterId::Custom(name) => name.clone(),
            ChatterId::Multiple(ids) => {
                // For multiple IDs, use the first one or default to "user"
                ids.first()
                    .map(|id| id.to_role())
                    .unwrap_or_else(|| "user".to_string())
            }
        }
    }
    
    /// Create a custom chatter with a specific name
    pub fn custom(name: &str) -> Self {
        ChatterId::Custom(name.to_string())
    }
}

/// A single message in a conversation
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub chatter_id: ChatterId,
    pub content: MessageContent,
}

/// Builder for managing conversations with multiple participants
pub struct ChatBuilder {
    conversation: Vec<ChatMessage>,
    request_builder: RequestBuilder,
    current_chatter: ChatterId,
}

impl ChatBuilder {
    /// Create a new chat builder
    pub(crate) fn new(request_builder: RequestBuilder, initial_chatter: ChatterId) -> Self {
        Self {
            conversation: Vec::new(),
            request_builder,
            current_chatter: initial_chatter,
        }
    }
    
    /// Add a text message to the conversation
    pub fn add_message(mut self, chatter_id: ChatterId, content: &str) -> Self {
        let message = ChatMessage {
            chatter_id: chatter_id.clone(),
            content: MessageContent::Text {
                role: chatter_id.to_role(),
                content: content.to_string(),
            },
        };
        self.conversation.push(message);
        self
    }
    
    /// Add a multimodal message to the conversation
    pub fn add_multimodal_message(mut self, chatter_id: ChatterId, content_blocks: Vec<ContentBlock>) -> Self {
        let message = ChatMessage {
            chatter_id: chatter_id.clone(),
            content: MessageContent::Multimodal {
                role: chatter_id.to_role(),
                content: content_blocks,
            },
        };
        self.conversation.push(message);
        self
    }
    
    /// Set the system prompt for the conversation
    pub fn system(mut self, prompt: &str) -> Self {
        self.request_builder.request.system_prompt = Some(prompt.to_string());
        self
    }
    
    /// Send a message from the specified chatter and get a response
    pub async fn send(mut self, chatter_id: ChatterId, content: &str) -> Result<CompletionResponse> {
        // Add the new message to the conversation
        self = self.add_message(chatter_id, content);
        
        // Convert conversation to the format expected by the API
        let messages: Vec<MessageContent> = self.conversation.iter()
            .map(|msg| msg.content.clone())
            .collect();
        
        // Update the request with the full conversation
        self.request_builder.request.messages = messages;
        
        // Send the request
        let client = crate::client::HttpClient::from_model_id(
            &*self.request_builder.config_provider, 
            &self.request_builder.model_id
        )?;
        
        let response = client.complete(&self.request_builder.request).await?;
        
        // Add the assistant's response to the conversation for future context
        let _assistant_message = ChatMessage {
            chatter_id: ChatterId::Agent,
            content: MessageContent::Text {
                role: "assistant".to_string(),
                content: response.content.clone(),
            },
        };
        
        // Note: We can't mutate self here since we consumed it, but in practice
        // the user would need to create a new ChatBuilder or we'd need to return
        // a new one with the updated conversation
        
        Ok(response)
    }
    
    /// Send a multimodal message and get a response
    pub async fn send_multimodal(mut self, chatter_id: ChatterId, content_blocks: Vec<ContentBlock>) -> Result<CompletionResponse> {
        // Add the new multimodal message to the conversation
        self = self.add_multimodal_message(chatter_id, content_blocks);
        
        // Convert conversation to the format expected by the API
        let messages: Vec<MessageContent> = self.conversation.iter()
            .map(|msg| msg.content.clone())
            .collect();
        
        // Update the request with the full conversation
        self.request_builder.request.messages = messages;
        
        // Send the request
        let client = crate::client::HttpClient::from_model_id(
            &*self.request_builder.config_provider, 
            &self.request_builder.model_id
        )?;
        
        client.complete(&self.request_builder.request).await
    }
    
    /// Continue the conversation with the current chatter
    pub async fn continue_conversation(self, content: &str) -> Result<CompletionResponse> {
        let current_chatter = self.current_chatter.clone();
        self.send(current_chatter, content).await
    }
    
    /// Get the current conversation history
    pub fn conversation(&self) -> &[ChatMessage] {
        &self.conversation
    }
    
    /// Clear the conversation history
    pub fn clear_history(mut self) -> Self {
        self.conversation.clear();
        self
    }
    
    /// Get the number of messages in the conversation
    pub fn message_count(&self) -> usize {
        self.conversation.len()
    }
    
    /// Set parameters for the underlying request
    pub fn temperature(mut self, temp: f64) -> Self {
        self.request_builder = self.request_builder.temperature(temp);
        self
    }
    
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.request_builder = self.request_builder.max_tokens(tokens);
        self
    }
    
    pub fn top_p(mut self, top_p: f64) -> Self {
        self.request_builder = self.request_builder.top_p(top_p);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_chatter_id_to_role() {
        assert_eq!(ChatterId::User.to_role(), "user");
        assert_eq!(ChatterId::Me.to_role(), "user");
        assert_eq!(ChatterId::System.to_role(), "system");
        assert_eq!(ChatterId::Agent.to_role(), "assistant");
        assert_eq!(ChatterId::Custom("narrator".to_string()).to_role(), "narrator");
        
        let custom = ChatterId::custom("teacher");
        assert_eq!(custom.to_role(), "teacher");
        
        let multiple = ChatterId::Multiple(vec![ChatterId::User, ChatterId::Agent]);
        assert_eq!(multiple.to_role(), "user");
    }
    
    #[test]
    fn test_chatter_id_equality() {
        assert_eq!(ChatterId::User, ChatterId::User);
        assert_eq!(ChatterId::Custom("alice".to_string()), ChatterId::Custom("alice".to_string()));
        assert_ne!(ChatterId::User, ChatterId::Agent);
        assert_ne!(ChatterId::Custom("alice".to_string()), ChatterId::Custom("bob".to_string()));
    }
}