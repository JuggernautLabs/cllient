use cllient::{ModelRegistry, ChatterId, ContentBlock, ImageFormat};

#[tokio::test]
async fn test_runtime_creation_and_model_listing() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test basic functionality
    let models = runtime.list_models();
    assert!(!models.is_empty(), "Should have embedded models");
    
    // Test pattern matching
    let haiku_models = runtime.list_models_matching("claude-3-haiku-*").unwrap();
    assert!(!haiku_models.is_empty(), "Should find haiku models");
    
    // Test that all haiku models contain the pattern
    for model_id in &haiku_models {
        assert!(model_id.contains("claude-3-haiku"), "Model {} should contain claude-3-haiku", model_id);
    }
}

#[tokio::test]
async fn test_model_selection_strategies() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test cheapest model selection
    let cheapest = runtime.use_cheapest("claude-3-haiku-*").unwrap();
    assert!(cheapest.model_id().contains("claude-3-haiku"));
    
    // Test fastest model selection  
    let fastest = runtime.use_fastest("claude-3-haiku-*").unwrap();
    assert!(fastest.model_id().contains("claude-3-haiku"));
    
    // Test best quality model selection
    let best_quality = runtime.use_best_quality("claude-3-haiku-*").unwrap();
    assert!(best_quality.model_id().contains("claude-3-haiku"));
}

#[tokio::test]
async fn test_request_builder_fluent_api() {
    let runtime = ModelRegistry::new().unwrap();
    let builder = runtime.from_id("claude-3-haiku-20240307").unwrap();
    
    // Test fluent API chaining
    let configured_builder = builder
        .system("You are helpful")
        .temperature(0.7)
        .max_tokens(100)
        .top_p(0.9)
        .parameter("custom_param", "custom_value");
    
    assert_eq!(configured_builder.model_id(), "claude-3-haiku-20240307");
    
    // The fluent API should work without errors - actual parameter verification
    // would require either public getters or integration tests with real API calls
}

#[tokio::test]
async fn test_chat_builder_functionality() {
    let runtime = ModelRegistry::new().unwrap();
    let request_builder = runtime.from_id("claude-3-haiku-20240307").unwrap();
    
    // Test chat builder creation
    let chat_builder = request_builder
        .chat(ChatterId::User)
        .system("You are a helpful assistant")
        .temperature(0.5);
    
    assert_eq!(chat_builder.message_count(), 0);
    
    // Test adding messages
    let chat_with_message = chat_builder
        .add_message(ChatterId::User, "Hello")
        .add_message(ChatterId::Agent, "Hi there!");
    
    assert_eq!(chat_with_message.message_count(), 2);
    
    // Test conversation history
    let conversation = chat_with_message.conversation();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[0].chatter_id, ChatterId::User);
    assert_eq!(conversation[1].chatter_id, ChatterId::Agent);
}

#[tokio::test]
async fn test_chatter_id_functionality() {
    // Test basic chatter IDs
    assert_eq!(ChatterId::User.to_role(), "user");
    assert_eq!(ChatterId::Me.to_role(), "user");
    assert_eq!(ChatterId::System.to_role(), "system");
    assert_eq!(ChatterId::Agent.to_role(), "assistant");
    
    // Test custom chatter
    let custom = ChatterId::custom("narrator");
    assert_eq!(custom.to_role(), "narrator");
    
    // Test multiple chatters
    let multiple = ChatterId::Multiple(vec![ChatterId::User, ChatterId::Agent]);
    assert_eq!(multiple.to_role(), "user"); // Should use first one
    
    // Test equality
    assert_eq!(ChatterId::User, ChatterId::User);
    assert_ne!(ChatterId::User, ChatterId::Agent);
    assert_eq!(ChatterId::custom("alice"), ChatterId::custom("alice"));
    assert_ne!(ChatterId::custom("alice"), ChatterId::custom("bob"));
}

#[tokio::test]
async fn test_content_block_creation() {
    // Test text content
    let text_block = ContentBlock::text("Hello world");
    match text_block {
        ContentBlock::Text(content) => assert_eq!(content, "Hello world"),
        _ => panic!("Expected text block"),
    }
    
    // Test image content
    let image_data = vec![0xFF, 0xD8, 0xFF]; // JPEG magic bytes
    let image_block = ContentBlock::image(image_data.clone(), ImageFormat::Jpeg);
    match image_block {
        ContentBlock::Binary { data, mime_type, .. } => {
            assert_eq!(data, image_data);
            assert_eq!(mime_type, "image/jpeg");
        },
        _ => panic!("Expected binary block"),
    }
    
    // Test URL content
    let url_block = ContentBlock::image_url("https://example.com/image.jpg");
    match url_block {
        ContentBlock::Url { url, mime_type } => {
            assert_eq!(url, "https://example.com/image.jpg");
            assert_eq!(mime_type, Some("image/*".to_string()));
        },
        _ => panic!("Expected URL block"),
    }
}

#[tokio::test]
async fn test_model_info_retrieval() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test getting model info
    let model_id = "claude-3-haiku-20240307";
    let model_info = runtime.get_model_info(model_id).unwrap();
    
    assert_eq!(model_info.model.id, model_id);
    assert_eq!(model_info.model.family, "claude");
    assert_eq!(model_info.model.service, "anthropic");
    assert!(model_info.capabilities.context_window > 0);
    assert!(model_info.pricing.input_per_1k_tokens > 0.0);
    assert!(model_info.pricing.output_per_1k_tokens > 0.0);
}

#[tokio::test]
async fn test_error_handling() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test invalid model ID
    let result = runtime.from_id("non-existent-model");
    assert!(result.is_err());
    
    // Test invalid pattern
    let result = runtime.list_models_matching("no-models-match-this-pattern");
    assert!(result.is_ok());
    let models = result.unwrap();
    assert!(models.is_empty());
    
    // Test cheapest with no matches
    let result = runtime.use_cheapest("no-models-match-this-pattern");
    assert!(result.is_err());
}

#[tokio::test]
async fn test_integration_workflow() {
    // Test a complete workflow without actual API calls
    let runtime = ModelRegistry::new().unwrap();
    
    // 1. Find cheapest model
    let builder = runtime.use_cheapest("claude-3-haiku-*").unwrap();
    let model_id = builder.model_id().to_string();
    
    // 2. Configure request
    let configured = builder
        .system("You are helpful")
        .temperature(0.7)
        .max_tokens(100);
    
    // 3. Set up chat
    let chat = configured
        .chat(ChatterId::User)
        .add_message(ChatterId::System, "Setup message")
        .add_message(ChatterId::User, "Hello");
    
    // 4. Verify state
    assert!(model_id.contains("claude-3-haiku"));
    assert_eq!(chat.message_count(), 2);
    
    // 5. Verify we can create multimodal content
    let content_blocks = vec![
        ContentBlock::text("What's this?"),
        ContentBlock::image(vec![1, 2, 3], ImageFormat::Png),
    ];
    
    assert_eq!(content_blocks.len(), 2);
    
    println!("✅ Complete integration workflow test passed");
}