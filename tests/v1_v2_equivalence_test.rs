//! Comprehensive equivalence tests between v1 and v2 message formatting
//!
//! These tests ensure that migrating from MessageBuilder (v1) to
//! MessageFormatter (v2) produces identical output for all message types.

use cllient::types::{MessageContent, ContentBlock};
use cllient::message_format::{MessageFormatter, anthropic_format, openai_format};
use cllient::template::MessageBuilder;
use serde_json::Value;

/// Helper to compare two JSON values with helpful diff output
fn assert_json_equivalent(v1: &Value, v2: &Value, context: &str) {
    if v1 != v2 {
        println!("\n=== JSON Equivalence Failure: {} ===", context);
        println!("V1 Output:\n{}", serde_json::to_string_pretty(v1).unwrap());
        println!("V2 Output:\n{}", serde_json::to_string_pretty(v2).unwrap());
        panic!("V1 and V2 outputs differ for {}", context);
    }
}

#[test]
fn test_anthropic_text_message_equivalence() {
    let messages = vec![
        MessageContent::Text {
            role: "user".to_string(),
            content: "Hello, world!".to_string(),
        }
    ];

    let v1 = MessageBuilder::build_messages("anthropic", &messages).unwrap();

    let formatter = MessageFormatter::new(anthropic_format().unwrap()).unwrap();
    let v2 = formatter.format_messages(&messages).unwrap();

    assert_json_equivalent(&v1, &v2, "Anthropic text message");
}

#[test]
fn test_anthropic_multimodal_equivalence() {
    let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header

    let messages = vec![
        MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![
                ContentBlock::Text("What's in this image?".to_string()),
                ContentBlock::Binary {
                    data: image_data.clone(),
                    mime_type: "image/jpeg".to_string(),
                    filename: Some("test.jpg".to_string()),
                },
            ],
        }
    ];

    let v1 = MessageBuilder::build_messages("anthropic", &messages).unwrap();

    let formatter = MessageFormatter::new(anthropic_format().unwrap()).unwrap();
    let v2 = formatter.format_messages(&messages).unwrap();

    assert_json_equivalent(&v1, &v2, "Anthropic multimodal with image");
}

#[test]
fn test_anthropic_url_image_equivalence() {
    let messages = vec![
        MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![
                ContentBlock::Url {
                    url: "https://example.com/image.jpg".to_string(),
                    mime_type: Some("image/jpeg".to_string()),
                },
            ],
        }
    ];

    let v1 = MessageBuilder::build_messages("anthropic", &messages).unwrap();

    let formatter = MessageFormatter::new(anthropic_format().unwrap()).unwrap();
    let v2 = formatter.format_messages(&messages).unwrap();

    assert_json_equivalent(&v1, &v2, "Anthropic URL image");
}

#[test]
fn test_openai_text_message_equivalence() {
    let messages = vec![
        MessageContent::Text {
            role: "assistant".to_string(),
            content: "I can help with that.".to_string(),
        }
    ];

    let v1 = MessageBuilder::build_messages("openai", &messages).unwrap();

    let formatter = MessageFormatter::new(openai_format().unwrap()).unwrap();
    let v2 = formatter.format_messages(&messages).unwrap();

    assert_json_equivalent(&v1, &v2, "OpenAI text message");
}

#[test]
fn test_openai_image_url_equivalence() {
    let image_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header

    let messages = vec![
        MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![
                ContentBlock::Text("Analyze this".to_string()),
                ContentBlock::Binary {
                    data: image_data.clone(),
                    mime_type: "image/png".to_string(),
                    filename: None,
                },
            ],
        }
    ];

    let v1 = MessageBuilder::build_messages("openai", &messages).unwrap();

    let formatter = MessageFormatter::new(openai_format().unwrap()).unwrap();
    let v2 = formatter.format_messages(&messages).unwrap();

    assert_json_equivalent(&v1, &v2, "OpenAI multimodal with image");
}

#[test]
fn test_special_characters_equivalence() {
    let messages = vec![
        MessageContent::Text {
            role: "user".to_string(),
            content: "Test \"quotes\", \nnewlines, and \ttabs".to_string(),
        }
    ];

    for format in &["anthropic", "openai"] {
        let v1 = MessageBuilder::build_messages(format, &messages).unwrap();

        let config = match *format {
            "anthropic" => anthropic_format().unwrap(),
            "openai" => openai_format().unwrap(),
            _ => unreachable!(),
        };
        let formatter = MessageFormatter::new(config).unwrap();
        let v2 = formatter.format_messages(&messages).unwrap();

        assert_json_equivalent(&v1, &v2, &format!("{} special characters", format));
    }
}

#[test]
fn test_multiple_messages_equivalence() {
    let messages = vec![
        MessageContent::Text {
            role: "user".to_string(),
            content: "First message".to_string(),
        },
        MessageContent::Text {
            role: "assistant".to_string(),
            content: "Second message".to_string(),
        },
        MessageContent::Text {
            role: "user".to_string(),
            content: "Third message".to_string(),
        },
    ];

    for format in &["anthropic", "openai"] {
        let v1 = MessageBuilder::build_messages(format, &messages).unwrap();

        let config = match *format {
            "anthropic" => anthropic_format().unwrap(),
            "openai" => openai_format().unwrap(),
            _ => unreachable!(),
        };
        let formatter = MessageFormatter::new(config).unwrap();
        let v2 = formatter.format_messages(&messages).unwrap();

        assert_json_equivalent(&v1, &v2, &format!("{} multiple messages", format));
    }
}

#[test]
fn test_empty_messages_equivalence() {
    let messages: Vec<MessageContent> = vec![];

    for format in &["anthropic", "openai"] {
        let v1 = MessageBuilder::build_messages(format, &messages).unwrap();

        let config = match *format {
            "anthropic" => anthropic_format().unwrap(),
            "openai" => openai_format().unwrap(),
            _ => unreachable!(),
        };
        let formatter = MessageFormatter::new(config).unwrap();
        let v2 = formatter.format_messages(&messages).unwrap();

        assert_json_equivalent(&v1, &v2, &format!("{} empty messages", format));
    }
}

#[test]
fn test_mixed_content_blocks_equivalence() {
    let messages = vec![
        MessageContent::Multimodal {
            role: "user".to_string(),
            content: vec![
                ContentBlock::Text("First text".to_string()),
                ContentBlock::Binary {
                    data: vec![0xFF, 0xD8, 0xFF],
                    mime_type: "image/jpeg".to_string(),
                    filename: None,
                },
                ContentBlock::Text("Second text".to_string()),
                ContentBlock::Url {
                    url: "https://example.com/image.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                },
            ],
        }
    ];

    for format in &["anthropic", "openai"] {
        let v1 = MessageBuilder::build_messages(format, &messages).unwrap();

        let config = match *format {
            "anthropic" => anthropic_format().unwrap(),
            "openai" => openai_format().unwrap(),
            _ => unreachable!(),
        };
        let formatter = MessageFormatter::new(config).unwrap();
        let v2 = formatter.format_messages(&messages).unwrap();

        assert_json_equivalent(&v1, &v2, &format!("{} mixed content blocks", format));
    }
}

#[test]
fn test_unicode_content_equivalence() {
    let messages = vec![
        MessageContent::Text {
            role: "user".to_string(),
            content: "Hello 世界! Привет мир! مرحبا بالعالم!".to_string(),
        }
    ];

    for format in &["anthropic", "openai"] {
        let v1 = MessageBuilder::build_messages(format, &messages).unwrap();

        let config = match *format {
            "anthropic" => anthropic_format().unwrap(),
            "openai" => openai_format().unwrap(),
            _ => unreachable!(),
        };
        let formatter = MessageFormatter::new(config).unwrap();
        let v2 = formatter.format_messages(&messages).unwrap();

        assert_json_equivalent(&v1, &v2, &format!("{} unicode content", format));
    }
}
