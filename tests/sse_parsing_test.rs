//! Integration tests for SSE parsing

use cllient::streaming::{SSEFormat, StreamItem, TextContent};
use bytes::Bytes;
use futures::stream;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
struct TestMessage {
    content: String,
}

#[tokio::test]
async fn test_openai_sse_parsing() {
    // Create a mock OpenAI SSE stream
    let sse_data = vec![
        "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n",
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
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    
    assert_eq!(tokens.join(""), "Hello world");
}

#[tokio::test]
async fn test_claude_sse_parsing() {
    // Create a mock Claude SSE stream
    let sse_data = vec![
        "event: message_start\ndata: {\"type\":\"message_start\"}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hi\"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\" there\"}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ];
    
    let byte_stream = stream::iter(sse_data.into_iter().map(|s| {
        Ok::<Bytes, cllient::streaming::error::AIError>(Bytes::from(s))
    }));
    
    let provider = SSEFormat::Claude.create_provider::<TestMessage>();
    let mut stream = provider.parse_sse_stream(Box::pin(byte_stream));
    
    let mut tokens = Vec::new();
    let mut has_start = false;
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(token)) => tokens.push(token),
            Ok(StreamItem::Text(_)) => has_start = true,
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    
    assert_eq!(tokens.join(""), "Hi there");
}

#[tokio::test]
async fn test_sse_with_json_extraction() {
    // Test that JSON objects in the stream are properly extracted
    #[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
    struct ResponseData {
        answer: String,
        confidence: f32,
    }
    
    let sse_data = vec![
        "data: {\"choices\":[{\"delta\":{\"content\":\"The answer is \"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"answer\\\": \\\"42\\\", \\\"confidence\\\": 0.95}\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\". That's all.\"}}]}\n\n",
        "data: [DONE]\n\n",
    ];
    
    let byte_stream = stream::iter(sse_data.into_iter().map(|s| {
        Ok::<Bytes, cllient::streaming::error::AIError>(Bytes::from(s))
    }));
    
    let provider = SSEFormat::OpenAI.create_provider::<ResponseData>();
    let mut stream = provider.parse_sse_stream(Box::pin(byte_stream));
    
    let mut found_data = false;
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Data(data)) => {
                assert_eq!(data.answer, "42");
                assert_eq!(data.confidence, 0.95);
                found_data = true;
            },
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
    
    assert!(found_data, "Expected to find extracted JSON data");
}