use cllient::{FileBasedClientFactory, ConfigLoader, LowLevelClient, CompletionRequest, MessageContent, ContentBlock};
use cllient::streaming::StreamEvent;
use futures::StreamExt;
use std::env;

#[test]
fn test_client_factory_creation() {
    // Test creating client factory
    let config_loader = ConfigLoader::new("config").unwrap();
    let factory = FileBasedClientFactory::new(config_loader);
    
    // Test listing available models
    let models = factory.list_available_models();
    assert!(!models.is_empty(), "No models available");
    assert!(models.len() > 100, "Expected at least 100 models");
}

#[test]
fn test_create_client_for_model() {
    let config_loader = ConfigLoader::new("config").unwrap();
    let factory = FileBasedClientFactory::new(config_loader);
    
    // Test creating clients for different providers
    let openai_client = factory.create_client("gpt-3.5-turbo");
    assert!(openai_client.is_ok(), "Failed to create OpenAI client");
    
    let anthropic_client = factory.create_client("claude-3-haiku-20240307");
    assert!(anthropic_client.is_ok(), "Failed to create Anthropic client");
    
    let deepseek_client = factory.create_client("deepseek-chat");
    assert!(deepseek_client.is_ok(), "Failed to create DeepSeek client");
    
    // Test invalid model
    let invalid_client = factory.create_client("non-existent-model");
    assert!(invalid_client.is_err(), "Expected error for non-existent model");
}

#[test]
fn test_completion_request_builder() {
    // Test basic text request
    let request = CompletionRequest::text("user", "Hello, world!");
    assert_eq!(request.messages.len(), 1);
    
    // Check that message is text type with user role
    match &request.messages[0] {
        MessageContent::Text { role, content } => {
            assert_eq!(role, "user");
            assert_eq!(content, "Hello, world!");
        },
        _ => panic!("Expected text message"),
    }
    
    // Test with system prompt
    let request = CompletionRequest::text("user", "Hello")
        .with_system_prompt("You are a helpful assistant".to_string());
    assert!(request.system_prompt.is_some());
    assert_eq!(request.system_prompt.as_ref().unwrap(), "You are a helpful assistant");
    assert_eq!(request.messages.len(), 1); // System prompt is separate, not a message
    
    // Test with parameters
    let request = CompletionRequest::text("user", "Test")
        .with_parameter("temperature", 0.7)
        .with_parameter("max_tokens", 100)
        .with_streaming(true);
    assert!(request.stream);
    assert_eq!(request.parameters.get("temperature"), Some(&serde_json::json!(0.7)));
    assert_eq!(request.parameters.get("max_tokens"), Some(&serde_json::json!(100)));
}

#[test]
fn test_message_content_types() {
    // Test text content block
    let text_block = ContentBlock::text("Hello");
    match text_block {
        ContentBlock::Text(text) => assert_eq!(text, "Hello"),
        _ => panic!("Expected text block"),
    }
    
    // Test image content block
    let image_data = vec![0xFF, 0xD8];
    let image_block = ContentBlock::image(image_data.clone(), cllient::ImageFormat::Jpeg);
    match image_block {
        ContentBlock::Binary { data, mime_type, .. } => {
            assert_eq!(data, image_data);
            assert_eq!(mime_type, "image/jpeg");
        },
        _ => panic!("Expected binary block"),
    }
    
    // Test creating multimodal message
    let message = MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![
            ContentBlock::text("What's in this image?"),
            ContentBlock::image_url("https://example.com/image.jpg"),
        ],
    };
    
    match message {
        MessageContent::Multimodal { role, content } => {
            assert_eq!(role, "user");
            assert_eq!(content.len(), 2);
        },
        _ => panic!("Expected multimodal message"),
    }
}

#[tokio::test]
async fn test_low_level_client_complete() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_low_level_client_complete: DEEPSEEK_API_KEY not set");
        return;
    }
    
    let config_loader = ConfigLoader::new("config").unwrap();
    let factory = FileBasedClientFactory::new(config_loader);
    let client = factory.create_client("deepseek-chat").unwrap();
    
    // Test basic completion
    let request = CompletionRequest::text("user", "What is 3+3? Reply with just the number.");
    let response = client.complete(&request).await;
    
    assert!(response.is_ok(), "Completion failed: {:?}", response.err());
    
    let response = response.unwrap();
    assert!(response.text().contains("6"), "Expected '6' in response");
    assert!(response.usage.is_some(), "Expected usage data");
    
    if let Some(usage) = response.usage {
        assert!(usage.input_tokens > 0, "Expected input tokens");
        assert!(usage.output_tokens > 0, "Expected output tokens");
        assert_eq!(usage.total_tokens, Some(usage.input_tokens + usage.output_tokens));
    }
}

#[tokio::test]
async fn test_low_level_client_streaming() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_low_level_client_streaming: DEEPSEEK_API_KEY not set");
        return;
    }
    
    let config_loader = ConfigLoader::new("config").unwrap();
    let factory = FileBasedClientFactory::new(config_loader);
    let client = factory.create_client("deepseek-chat").unwrap();
    
    // Test streaming completion
    let request = CompletionRequest::text("user", "Count from 1 to 3")
        .with_streaming(true);
    
    let stream = client.complete_stream(&request).await;
    assert!(stream.is_ok(), "Failed to create stream");
    
    let mut stream = stream.unwrap();
    let mut content = String::new();
    let mut saw_start = false;
    let mut saw_finish = false;
    
    while let Some(event) = stream.next().await {
        match event {
            Ok(StreamEvent::Start) => saw_start = true,
            Ok(StreamEvent::Content(chunk)) => content.push_str(&chunk),
            Ok(StreamEvent::Finish { .. }) => saw_finish = true,
            Ok(_) => {}, // Other events like Role, Usage
            Err(e) => panic!("Stream error: {}", e),
        }
    }
    
    assert!(saw_start, "Expected Start event");
    assert!(saw_finish, "Expected Finish event");
    assert!(!content.is_empty(), "Expected some content");
    assert!(content.contains("1"), "Expected '1' in stream");
    assert!(content.contains("2"), "Expected '2' in stream");
    assert!(content.contains("3"), "Expected '3' in stream");
}

#[test]
fn test_get_model_config() {
    let config_loader = ConfigLoader::new("config").unwrap();
    
    // Test getting model configuration directly from config loader
    let model_config = config_loader.get_model("gpt-3.5-turbo");
    assert!(model_config.is_ok(), "Failed to get model config");
    
    let config = model_config.unwrap();
    assert_eq!(config.model.id, "gpt-3.5-turbo");
    assert_eq!(config.model.family, "gpt");
    assert!(config.capabilities.context_window > 0);
    assert!(config.pricing.input_per_1k_tokens > 0.0);
}

#[test]
fn test_get_service_config() {
    let config_loader = ConfigLoader::new("config").unwrap();
    
    // Test getting service configuration directly from config loader
    let service_config = config_loader.get_service("openai");
    assert!(service_config.is_ok(), "Failed to get service config");
    
    let config = service_config.unwrap();
    assert_eq!(config.service.name, "OpenAI");
    assert!(!config.service.base_url.is_empty());
}

#[test]
fn test_error_handling() {
    let config_loader = ConfigLoader::new("config").unwrap();
    let factory = FileBasedClientFactory::new(config_loader);
    
    // Test various error scenarios
    
    // Non-existent model
    let result = factory.create_client("non-existent-model-xyz");
    assert!(result.is_err(), "Expected error for non-existent model");
    
    // Empty model ID
    let result = factory.create_client("");
    assert!(result.is_err(), "Expected error for empty model ID");
}

#[test]
fn test_multimodal_request() {
    // Test creating multimodal completion request
    let content_blocks = vec![
        ContentBlock::text("What's in this image?"),
        ContentBlock::image(vec![1, 2, 3], cllient::ImageFormat::Png),
    ];
    
    let request = CompletionRequest::multimodal("user", content_blocks);
    assert_eq!(request.messages.len(), 1);
    
    match &request.messages[0] {
        MessageContent::Multimodal { role, content } => {
            assert_eq!(role, "user");
            assert_eq!(content.len(), 2);
        },
        _ => panic!("Expected multimodal message"),
    }
}