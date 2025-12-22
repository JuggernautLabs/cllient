//! Integration tests for the streaming module

use cllient::streaming::{StreamEvent, StreamProcessor, StreamItem, TextContent};
use cllient::config::StreamingConfig;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct TestData {
    value: i32,
}

#[tokio::test]
async fn test_streaming_event_compatibility() {
    // Test that StreamEvent types work correctly
    let events = vec![
        StreamEvent::Start,
        StreamEvent::Content("Hello".to_string()),
        StreamEvent::Content(" world".to_string()),
        StreamEvent::Finish(Some("completed".to_string())),
    ];
    
    assert!(matches!(events[0], StreamEvent::Start));
    assert!(matches!(events[1], StreamEvent::Content(_)));
}

#[tokio::test]
async fn test_stream_item_types() {
    // Test the new StreamItem types
    let items: Vec<StreamItem<TestData>> = vec![
        StreamItem::Token("token1".to_string()),
        StreamItem::Text(TextContent { text: "Some text".to_string() }),
        StreamItem::Data(TestData { value: 42 }),
    ];
    
    assert!(matches!(items[0], StreamItem::Token(_)));
    assert!(matches!(items[1], StreamItem::Text(_)));
    assert!(matches!(items[2], StreamItem::Data(_)));
}

#[test]
fn test_json_structure_detection() {
    use cllient::streaming::build_parsed_stream;
    
    let raw_text = r#"Here's some text followed by JSON: {"value": 42} and more text"#;
    let items: Vec<StreamItem<TestData>> = build_parsed_stream(raw_text);
    
    // Should have text before JSON, the JSON data, and text after
    assert!(items.len() >= 2);
    
    // Find the Data item
    let has_data = items.iter().any(|item| matches!(item, StreamItem::Data(_)));
    assert!(has_data);
    
    // Check that we got the correct data
    for item in items {
        if let StreamItem::Data(data) = item {
            assert_eq!(data.value, 42);
        }
    }
}

#[test]
fn test_nested_json_extraction() {
    use cllient::streaming::build_parsed_stream;
    
    #[derive(Debug, Clone, Deserialize, JsonSchema)]
    struct NestedData {
        inner: TestData,
        name: String,
    }
    
    let raw_text = r#"Complex JSON: {"inner": {"value": 100}, "name": "test"}"#;
    let items: Vec<StreamItem<NestedData>> = build_parsed_stream(raw_text);
    
    // Find the Data item
    for item in items {
        if let StreamItem::Data(data) = item {
            assert_eq!(data.inner.value, 100);
            assert_eq!(data.name, "test");
        }
    }
}

#[tokio::test]
async fn test_stream_processor_creation() {
    use cllient::{StreamingFormat, SseParser};
    let config = StreamingConfig {
        format: StreamingFormat::TextEventStream,
        parser: SseParser::OpenAiSse,
        line_prefix: Some("data: ".to_string()),
        done_marker: Some("[DONE]".to_string()),
        events: vec![],
        extract: Default::default(),
    };

    let processor = StreamProcessor::new(&config).unwrap();
    // Just ensure it creates without error
    drop(processor);
}