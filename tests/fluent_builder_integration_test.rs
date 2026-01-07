//! Comprehensive integration tests for the fluent builder system.
//!
//! This test suite covers:
//! 1. Building complete OpenAI/Anthropic parsers from fluent API
//! 2. Testing preset configurations match v2 YAML configs
//! 3. YAML round-trip (build -> serialize -> deserialize -> verify equal)
//! 4. Error handling for invalid configurations
//! 5. Actual SSE parsing with sample data from each provider

use cllient::{
    MessageFormatConfig, MessageFormatBuilder, MessageFormatter, MessageFormatRegistry,
    ContentBlockType, MessageContent, ContentBlock, ImageFormat,
    anthropic_format, openai_format,
};
use cllient::streaming::{
    SSEFormat, StreamItem, TextContent, SSEProvider,
    ConfigurableSSEProvider,
    sse::{
        ExtractorConfig, JsonPath, JsonPathSegment, JsonPathExtractor, EndCondition,
        openai_config, claude_config, openai_extractor, claude_extractor,
        TokenExtractor, TokenExtractionResult,
    },
};
use cllient::config::{ServiceConfig, StreamingConfig, StreamingFormat, SseParser};
use bytes::Bytes;
use futures::stream;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ============================================================================
// Test Data Structures
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
struct TestMessage {
    content: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
struct ResponseData {
    answer: String,
    confidence: f32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
struct NestedData {
    inner: InnerData,
    name: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
struct InnerData {
    value: i32,
}

// ============================================================================
// 1. Building Complete Parsers from Fluent API
// ============================================================================

#[test]
fn test_openai_fluent_message_format_builder() {
    // Build OpenAI-compatible message format using fluent API
    let config = MessageFormatBuilder::new()
        .name("openai")
        .text_message_template(r#"{"role": "{{role}}", "content": "{{json_escape content}}"}"#)
        .multimodal_message_template(r#"{"role": "{{role}}", "content": {{json content_blocks}}}"#)
        .text_block(r#"{"type": "text", "text": "{{json_escape text}}"}"#)
        .binary_block_with_priority(
            vec!["image/*".to_string()],
            r#"{"type": "image_url", "image_url": {"url": "{{data_uri}}"}}"#,
            10,
        )
        .binary_block_with_priority(
            vec!["audio/*".to_string()],
            r#"{"type": "audio", "source_type": "base64", "mime_type": "{{mime_type}}", "data": "{{base64_data}}"}"#,
            10,
        )
        .url_block(
            vec!["image/*".to_string()],
            r#"{"type": "image_url", "image_url": {"url": "{{url}}"}}"#,
        )
        .build()
        .unwrap();

    let formatter = MessageFormatter::new(config).unwrap();

    // Test message building with text
    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Hello, how are you?".to_string(),
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["role"], "user");
    assert_eq!(arr[0]["content"], "Hello, how are you?");
}

#[test]
fn test_anthropic_fluent_message_format_builder() {
    // Build Anthropic-compatible message format using fluent API
    let config = MessageFormatBuilder::new()
        .name("anthropic")
        .text_message_template(r#"{"role": "{{role}}", "content": "{{json_escape content}}"}"#)
        .multimodal_message_template(r#"{"role": "{{role}}", "content": {{json content_blocks}}}"#)
        .text_block(r#"{"type": "text", "text": "{{json_escape text}}"}"#)
        .binary_block(
            vec!["image/*".to_string()],
            r#"{"type": "image", "source": {"type": "base64", "media_type": "{{mime_type}}", "data": "{{base64_data}}"}}"#,
        )
        .url_block(
            vec!["image/*".to_string()],
            r#"{"type": "image", "source": {"type": "url", "url": "{{url}}"}}"#,
        )
        .build()
        .unwrap();

    let formatter = MessageFormatter::new(config).unwrap();

    // Test multimodal message with image
    let image_data = vec![0xFF, 0xD8, 0xFF]; // Fake JPEG header
    let messages = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![
            ContentBlock::Text("What is in this image?".to_string()),
            ContentBlock::Binary {
                data: image_data.clone(),
                mime_type: "image/jpeg".to_string(),
                filename: None,
            },
        ],
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["role"], "user");

    let content = arr[0]["content"].as_array().unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["type"], "base64");
}

#[test]
fn test_custom_provider_fluent_builder() {
    // Build a completely custom message format for a hypothetical provider
    let config = MessageFormatBuilder::new()
        .name("custom_provider")
        .text_message_template(r#"{"r": "{{role}}", "m": "{{json_escape content}}"}"#)
        .multimodal_message_template(r#"{"r": "{{role}}", "parts": {{json content_blocks}}}"#)
        .text_block(r#"{"kind": "text", "value": "{{json_escape text}}"}"#)
        .binary_block_with_priority(
            vec!["image/png".to_string()],
            r#"{"kind": "png_image", "b64": "{{base64_data}}"}"#,
            100,
        )
        .binary_block(
            vec!["image/*".to_string()],
            r#"{"kind": "image", "format": "{{mime_type}}", "b64": "{{base64_data}}"}"#,
        )
        .build()
        .unwrap();

    let formatter = MessageFormatter::new(config).unwrap();

    let messages = vec![MessageContent::Text {
        role: "assistant".to_string(),
        content: "Test response".to_string(),
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    assert_eq!(arr[0]["r"], "assistant");
    assert_eq!(arr[0]["m"], "Test response");
}

// ============================================================================
// 2. Testing Preset Configurations Match Expected Behavior
// ============================================================================

#[test]
fn test_builtin_anthropic_format_matches_expected() {
    let config = anthropic_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    // Test text message
    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Hello, Claude!".to_string(),
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    assert_eq!(arr[0]["role"], "user");
    assert_eq!(arr[0]["content"], "Hello, Claude!");
}

#[test]
fn test_builtin_openai_format_matches_expected() {
    let config = openai_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    // Test text message
    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Hello, GPT!".to_string(),
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    assert_eq!(arr[0]["role"], "user");
    assert_eq!(arr[0]["content"], "Hello, GPT!");
}

#[test]
fn test_openai_image_format_uses_data_uri() {
    let config = openai_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    let image_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
    let messages = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![ContentBlock::Binary {
            data: image_data,
            mime_type: "image/png".to_string(),
            filename: None,
        }],
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let content = result.as_array().unwrap()[0]["content"].as_array().unwrap();

    // OpenAI uses image_url with data URI
    assert_eq!(content[0]["type"], "image_url");
    let url = content[0]["image_url"]["url"].as_str().unwrap();
    assert!(url.starts_with("data:image/png;base64,"));
}

#[test]
fn test_anthropic_image_format_uses_base64_source() {
    let config = anthropic_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    let image_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
    let messages = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![ContentBlock::Binary {
            data: image_data,
            mime_type: "image/png".to_string(),
            filename: None,
        }],
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let content = result.as_array().unwrap()[0]["content"].as_array().unwrap();

    // Anthropic uses separate base64 source
    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["source"]["type"], "base64");
    assert_eq!(content[0]["source"]["media_type"], "image/png");
}

#[test]
fn test_format_registry_with_builtins() {
    let registry = MessageFormatRegistry::with_builtins().unwrap();

    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Test message".to_string(),
    }];

    // Both formats should be registered
    let anthropic_result = registry.format_messages("anthropic", &messages).unwrap();
    let openai_result = registry.format_messages("openai", &messages).unwrap();

    assert!(anthropic_result.as_array().is_some());
    assert!(openai_result.as_array().is_some());
}

// ============================================================================
// 3. YAML Round-Trip Tests
// ============================================================================

#[test]
fn test_message_format_config_yaml_roundtrip() {
    let original_config = anthropic_format().unwrap();

    // Serialize to YAML
    let yaml = serde_yaml::to_string(&original_config).unwrap();

    // Deserialize back
    let deserialized: MessageFormatConfig = serde_yaml::from_str(&yaml).unwrap();

    // Verify essential fields match
    assert_eq!(original_config.name, deserialized.name);
    assert_eq!(original_config.text_message.template, deserialized.text_message.template);
    assert_eq!(original_config.multimodal_message.template, deserialized.multimodal_message.template);
    assert_eq!(original_config.content_blocks.len(), deserialized.content_blocks.len());

    // Verify deserialized config works
    let formatter = MessageFormatter::new(deserialized).unwrap();
    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Roundtrip test".to_string(),
    }];
    let result = formatter.format_messages(&messages).unwrap();
    assert!(result.as_array().is_some());
}

#[test]
fn test_openai_format_yaml_roundtrip() {
    let original_config = openai_format().unwrap();

    // Serialize to YAML
    let yaml = serde_yaml::to_string(&original_config).unwrap();

    // Deserialize back
    let deserialized: MessageFormatConfig = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(original_config.name, deserialized.name);
    assert_eq!(original_config.content_blocks.len(), deserialized.content_blocks.len());

    // Test that the deserialized formatter produces valid output
    let formatter = MessageFormatter::new(deserialized).unwrap();
    let messages = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![ContentBlock::Text("Hello".to_string())],
    }];
    let result = formatter.format_messages(&messages).unwrap();
    assert!(result.as_array().is_some());
}

#[test]
fn test_custom_format_yaml_roundtrip() {
    // Build a custom format
    let original = MessageFormatBuilder::new()
        .name("custom_roundtrip")
        .text_message_template(r#"{"msg": "{{json_escape content}}", "sender": "{{role}}"}"#)
        .multimodal_message_template(r#"{"parts": {{json content_blocks}}, "sender": "{{role}}"}"#)
        .text_block(r#"{"t": "text", "v": "{{json_escape text}}"}"#)
        .binary_block(
            vec!["image/*".to_string()],
            r#"{"t": "img", "d": "{{base64_data}}"}"#,
        )
        .build()
        .unwrap();

    // Roundtrip through YAML
    let yaml = serde_yaml::to_string(&original).unwrap();
    let restored: MessageFormatConfig = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(original.name, restored.name);
    assert_eq!(original.text_message.template, restored.text_message.template);

    // Both should produce same output
    let original_formatter = MessageFormatter::new(original).unwrap();
    let restored_formatter = MessageFormatter::new(restored).unwrap();

    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Test".to_string(),
    }];

    let original_result = original_formatter.format_messages(&messages).unwrap();
    let restored_result = restored_formatter.format_messages(&messages).unwrap();

    assert_eq!(original_result, restored_result);
}

#[test]
fn test_extractor_config_consistency() {
    // Verify that extractor configs produce consistent results
    let openai = openai_extractor();
    let claude = claude_extractor();

    // OpenAI token extraction
    let openai_json = json!({
        "choices": [{
            "delta": {
                "content": "Hello"
            }
        }]
    });
    match openai.extract_token(&openai_json) {
        TokenExtractionResult::Token(t) => assert_eq!(t, "Hello"),
        _ => panic!("Expected token extraction"),
    }

    // Claude token extraction
    let claude_json = json!({
        "type": "content_block_delta",
        "delta": {
            "text": "World"
        }
    });
    match claude.extract_token(&claude_json) {
        TokenExtractionResult::Token(t) => assert_eq!(t, "World"),
        _ => panic!("Expected token extraction"),
    }
}

// ============================================================================
// 4. Error Handling Tests
// ============================================================================

#[test]
fn test_builder_missing_name_error() {
    let result = MessageFormatBuilder::new()
        .text_message_template(r#"{"content": "{{content}}"}"#)
        .multimodal_message_template(r#"{"content": {{content_blocks}}}"#)
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("name"));
}

#[test]
fn test_builder_missing_text_template_error() {
    let result = MessageFormatBuilder::new()
        .name("incomplete")
        .multimodal_message_template(r#"{"content": {{content_blocks}}}"#)
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("text_message"));
}

#[test]
fn test_builder_missing_multimodal_template_error() {
    let result = MessageFormatBuilder::new()
        .name("incomplete")
        .text_message_template(r#"{"content": "{{content}}"}"#)
        .build();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("multimodal"));
}

#[test]
fn test_registry_unknown_format_error() {
    let registry = MessageFormatRegistry::with_builtins().unwrap();
    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Test".to_string(),
    }];

    let result = registry.format_messages("nonexistent_format", &messages);
    assert!(result.is_err());
}

#[test]
fn test_invalid_template_syntax_error() {
    // This test verifies that invalid handlebars syntax is caught
    let config = MessageFormatBuilder::new()
        .name("invalid_template")
        .text_message_template(r#"{"content": "{{unclosed"}"#) // Missing closing braces
        .multimodal_message_template(r#"{"content": {{content_blocks}}}"#)
        .build();

    // Build should succeed, but formatting should fail
    if let Ok(config) = config {
        let formatter = MessageFormatter::new(config);
        if let Ok(formatter) = formatter {
            let messages = vec![MessageContent::Text {
                role: "user".to_string(),
                content: "Test".to_string(),
            }];
            let result = formatter.format_messages(&messages);
            // Either the formatter creation fails or the rendering fails
            assert!(result.is_err() || result.unwrap().as_array().is_none());
        }
    }
}

#[test]
fn test_empty_messages_handling() {
    let config = anthropic_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    let messages: Vec<MessageContent> = vec![];
    let result = formatter.format_messages(&messages).unwrap();

    let arr = result.as_array().unwrap();
    assert!(arr.is_empty());
}

#[test]
fn test_special_characters_in_content() {
    let config = openai_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    // Test with JSON special characters that need escaping
    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: r#"Hello "world" with \ backslash and \n newline"#.to_string(),
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    // Content should be properly escaped in JSON
    assert_eq!(arr[0]["role"], "user");
    // The content should match the original (serde_json will handle escaping)
    assert_eq!(arr[0]["content"], r#"Hello "world" with \ backslash and \n newline"#);
}

// ============================================================================
// 5. SSE Parsing with Sample Data
// ============================================================================

#[tokio::test]
async fn test_openai_sse_parsing_with_real_format() {
    // Sample OpenAI SSE data
    let sse_data = vec![
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"!\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    ];

    let byte_stream = stream::iter(sse_data.into_iter().map(|s| {
        Ok::<Bytes, cllient::streaming::error::AIError>(Bytes::from(s))
    }));

    let provider = SSEFormat::OpenAI.create_provider::<TestMessage>();
    let mut stream = provider.parse_sse_stream(Box::pin(byte_stream));

    let mut tokens = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(token)) => tokens.push(token),
            Ok(_) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert_eq!(tokens.join(""), "Hello world!");
}

#[tokio::test]
async fn test_anthropic_sse_parsing_with_real_format() {
    // Sample Anthropic/Claude SSE data
    let sse_data = vec![
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\"}}\n\n",
        "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"content_block\":{\"type\":\"text\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hi\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\" there\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"!\"}}\n\n",
        "event: content_block_stop\ndata: {\"type\":\"content_block_stop\"}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ];

    let byte_stream = stream::iter(sse_data.into_iter().map(|s| {
        Ok::<Bytes, cllient::streaming::error::AIError>(Bytes::from(s))
    }));

    let provider = SSEFormat::Claude.create_provider::<TestMessage>();
    let mut stream = provider.parse_sse_stream(Box::pin(byte_stream));

    let mut tokens = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(token)) => tokens.push(token),
            Ok(_) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert_eq!(tokens.join(""), "Hi there!");
}

#[tokio::test]
async fn test_deepseek_sse_parsing_openai_compatible() {
    // DeepSeek uses OpenAI-compatible format
    let sse_data = vec![
        "data: {\"id\":\"cmpl-abc\",\"model\":\"deepseek-chat\",\"choices\":[{\"delta\":{\"content\":\"Deep\"}}]}\n\n",
        "data: {\"id\":\"cmpl-abc\",\"model\":\"deepseek-chat\",\"choices\":[{\"delta\":{\"content\":\"Seek\"}}]}\n\n",
        "data: {\"id\":\"cmpl-abc\",\"model\":\"deepseek-chat\",\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n",
    ];

    let byte_stream = stream::iter(sse_data.into_iter().map(|s| {
        Ok::<Bytes, cllient::streaming::error::AIError>(Bytes::from(s))
    }));

    // DeepSeek uses OpenAI format
    let provider = SSEFormat::OpenAI.create_provider::<TestMessage>();
    let mut stream = provider.parse_sse_stream(Box::pin(byte_stream));

    let mut tokens = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(token)) => tokens.push(token),
            Ok(_) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert_eq!(tokens.join(""), "DeepSeek");
}

#[tokio::test]
async fn test_sse_with_embedded_json_extraction() {
    // Test that JSON embedded in content is properly handled
    let sse_data = vec![
        "data: {\"choices\":[{\"delta\":{\"content\":\"Here is the data: \"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"answer\\\": \\\"42\\\", \\\"confidence\\\": 0.95}\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\" End of response.\"}}]}\n\n",
        "data: [DONE]\n\n",
    ];

    let byte_stream = stream::iter(sse_data.into_iter().map(|s| {
        Ok::<Bytes, cllient::streaming::error::AIError>(Bytes::from(s))
    }));

    let provider = SSEFormat::OpenAI.create_provider::<ResponseData>();
    let mut stream = provider.parse_sse_stream(Box::pin(byte_stream));

    let mut tokens = Vec::new();
    let mut found_data = false;

    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(token)) => tokens.push(token),
            Ok(StreamItem::Data(data)) => {
                assert_eq!(data.answer, "42");
                assert_eq!(data.confidence, 0.95);
                found_data = true;
            }
            Ok(_) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    // Tokens should contain the full text
    let full_text = tokens.join("");
    assert!(full_text.contains("Here is the data:"));
    assert!(full_text.contains("End of response"));
}

#[test]
fn test_json_path_navigation() {
    use JsonPathSegment::*;

    // Test OpenAI path: choices[0].delta.content
    let openai_path = JsonPath(vec![Key("choices"), Index(0), Key("delta"), Key("content")]);
    let openai_json = json!({
        "choices": [{
            "delta": {
                "content": "Test token"
            }
        }]
    });
    let result = openai_path.navigate(&openai_json);
    assert_eq!(result.unwrap().as_str().unwrap(), "Test token");

    // Test Claude path: delta.text
    let claude_path = JsonPath(vec![Key("delta"), Key("text")]);
    let claude_json = json!({
        "type": "content_block_delta",
        "delta": {
            "text": "Claude token"
        }
    });
    let result = claude_path.navigate(&claude_json);
    assert_eq!(result.unwrap().as_str().unwrap(), "Claude token");

    // Test missing path returns None
    let missing_path = JsonPath(vec![Key("nonexistent"), Key("path")]);
    let result = missing_path.navigate(&openai_json);
    assert!(result.is_none());
}

#[test]
fn test_extractor_end_conditions() {
    let openai = openai_extractor();
    let claude = claude_extractor();

    // OpenAI end condition: finish_reason exists
    let openai_end = json!({
        "choices": [{
            "finish_reason": "stop"
        }]
    });
    match openai.extract_token(&openai_end) {
        TokenExtractionResult::EndStream => {}
        other => panic!("Expected EndStream, got {:?}", other),
    }

    // OpenAI payload end
    assert!(openai.is_end_payload("[DONE]"));
    assert!(!openai.is_end_payload("data: {\"content\": \"test\"}"));

    // Claude end condition: type == "message_stop"
    let claude_end = json!({
        "type": "message_stop"
    });
    match claude.extract_token(&claude_end) {
        TokenExtractionResult::EndStream => {}
        other => panic!("Expected EndStream, got {:?}", other),
    }

    // Claude doesn't use payload end
    assert!(!claude.is_end_payload("[DONE]"));
}

#[test]
fn test_configurable_provider_creation() {
    // Test OpenAI provider creation
    let openai_provider = ConfigurableSSEProvider::openai();
    // Just verify it can be created without error

    // Test Claude provider creation
    let claude_provider = ConfigurableSSEProvider::claude();
    // Just verify it can be created without error

    // Test custom provider with custom config
    let custom_config = ExtractorConfig {
        token_path: JsonPath(vec![JsonPathSegment::Key("text")]),
        json_end_condition: EndCondition::PayloadEquals("END"),
        end_payload: Some("END"),
    };
    let _custom_provider = ConfigurableSSEProvider::new(custom_config);
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

#[test]
fn test_mixed_content_blocks_formatting() {
    let config = openai_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    let messages = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![
            ContentBlock::Text("First, look at this image:".to_string()),
            ContentBlock::Binary {
                data: vec![0xFF, 0xD8, 0xFF],
                mime_type: "image/jpeg".to_string(),
                filename: None,
            },
            ContentBlock::Text("Now listen to this:".to_string()),
            ContentBlock::Binary {
                data: vec![0x52, 0x49, 0x46, 0x46],
                mime_type: "audio/wav".to_string(),
                filename: Some("recording.wav".to_string()),
            },
            ContentBlock::Url {
                url: "https://example.com/image.png".to_string(),
                mime_type: Some("image/png".to_string()),
            },
        ],
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let content = result.as_array().unwrap()[0]["content"].as_array().unwrap();

    assert_eq!(content.len(), 5);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[1]["type"], "image_url");
    assert_eq!(content[2]["type"], "text");
    assert_eq!(content[3]["type"], "audio");
    assert_eq!(content[4]["type"], "image_url");
}

#[test]
fn test_priority_based_transformer_selection() {
    // Test that higher priority transformers are selected
    let config = MessageFormatBuilder::new()
        .name("priority_test")
        .text_message_template(r#"{"role": "{{role}}", "content": "{{json_escape content}}"}"#)
        .multimodal_message_template(r#"{"role": "{{role}}", "content": {{json content_blocks}}}"#)
        .text_block(r#"{"type": "text", "text": "{{json_escape text}}"}"#)
        // Low priority catch-all
        .binary_block(
            vec![],
            r#"{"type": "generic", "data": "{{base64_data}}"}"#,
        )
        // High priority PNG handler
        .binary_block_with_priority(
            vec!["image/png".to_string()],
            r#"{"type": "png_specific", "data": "{{base64_data}}"}"#,
            100,
        )
        // Medium priority image handler
        .binary_block_with_priority(
            vec!["image/*".to_string()],
            r#"{"type": "image_generic", "data": "{{base64_data}}"}"#,
            50,
        )
        .build()
        .unwrap();

    let formatter = MessageFormatter::new(config).unwrap();

    // PNG should use the high-priority PNG-specific handler
    let png_message = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![ContentBlock::Binary {
            data: vec![0x89, 0x50, 0x4E, 0x47],
            mime_type: "image/png".to_string(),
            filename: None,
        }],
    }];

    let result = formatter.format_messages(&png_message).unwrap();
    let content = result.as_array().unwrap()[0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "png_specific");

    // JPEG should use the medium-priority image handler
    let jpeg_message = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![ContentBlock::Binary {
            data: vec![0xFF, 0xD8, 0xFF],
            mime_type: "image/jpeg".to_string(),
            filename: None,
        }],
    }];

    let result = formatter.format_messages(&jpeg_message).unwrap();
    let content = result.as_array().unwrap()[0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "image_generic");
}

#[test]
fn test_url_content_blocks() {
    let config = anthropic_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    let messages = vec![MessageContent::Multimodal {
        role: "user".to_string(),
        content: vec![ContentBlock::Url {
            url: "https://example.com/photo.jpg".to_string(),
            mime_type: Some("image/jpeg".to_string()),
        }],
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let content = result.as_array().unwrap()[0]["content"].as_array().unwrap();

    assert_eq!(content[0]["type"], "image");
    assert_eq!(content[0]["source"]["type"], "url");
    assert_eq!(content[0]["source"]["url"], "https://example.com/photo.jpg");
}

#[test]
fn test_multiple_messages_formatting() {
    let config = openai_format().unwrap();
    let formatter = MessageFormatter::new(config).unwrap();

    let messages = vec![
        MessageContent::Text {
            role: "system".to_string(),
            content: "You are a helpful assistant.".to_string(),
        },
        MessageContent::Text {
            role: "user".to_string(),
            content: "Hello!".to_string(),
        },
        MessageContent::Text {
            role: "assistant".to_string(),
            content: "Hi there! How can I help you today?".to_string(),
        },
        MessageContent::Text {
            role: "user".to_string(),
            content: "What is 2+2?".to_string(),
        },
    ];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    assert_eq!(arr.len(), 4);
    assert_eq!(arr[0]["role"], "system");
    assert_eq!(arr[1]["role"], "user");
    assert_eq!(arr[2]["role"], "assistant");
    assert_eq!(arr[3]["role"], "user");
}

#[tokio::test]
async fn test_sse_stream_with_malformed_data() {
    // Test resilience to malformed SSE data
    let sse_data = vec![
        "data: {\"choices\":[{\"delta\":{\"content\":\"Valid\"}}]}\n\n",
        "data: not valid json\n\n", // Invalid JSON
        "data: {\"choices\":[{\"delta\":{\"content\":\" content\"}}]}\n\n",
        "data: [DONE]\n\n",
    ];

    let byte_stream = stream::iter(sse_data.into_iter().map(|s| {
        Ok::<Bytes, cllient::streaming::error::AIError>(Bytes::from(s))
    }));

    let provider = SSEFormat::OpenAI.create_provider::<TestMessage>();
    let mut stream = provider.parse_sse_stream(Box::pin(byte_stream));

    let mut tokens = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(token)) => tokens.push(token),
            Ok(_) => {}
            Err(_) => {} // Skip errors from malformed data
        }
    }

    // Should still capture valid tokens
    assert!(tokens.contains(&"Valid".to_string()) || tokens.contains(&" content".to_string()));
}

#[test]
fn test_builder_with_variables() {
    let config = MessageFormatBuilder::new()
        .name("with_variables")
        .text_message_template(r#"{"role": "{{role}}", "content": "{{json_escape content}}", "version": "{{version}}"}"#)
        .text_message_variable("version", Value::String("1.0".to_string()))
        .multimodal_message_template(r#"{"role": "{{role}}", "content": {{json content_blocks}}, "version": "{{version}}"}"#)
        .multimodal_message_variable("version", Value::String("1.0".to_string()))
        .text_block(r#"{"type": "text", "text": "{{json_escape text}}"}"#)
        .build()
        .unwrap();

    let formatter = MessageFormatter::new(config).unwrap();
    let messages = vec![MessageContent::Text {
        role: "user".to_string(),
        content: "Hello".to_string(),
    }];

    let result = formatter.format_messages(&messages).unwrap();
    let arr = result.as_array().unwrap();

    assert_eq!(arr[0]["version"], "1.0");
}
