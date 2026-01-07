use cllient::config::ServiceConfig;
use cllient::error::{ClientError, ConfigError};

#[test]
fn test_v1_config_parsing() {
    // Test parsing a v1-style config with message_builder as a string
    let yaml_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1
    Host: api.test.com
    Content-Type: application/json
    Authorization: Bearer ${API_KEY}

    {
      "model": "${model_id}",
      "messages": ${messages}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: openai
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse v1 config");

    // Verify that message_builder is set
    assert!(service_config.message_builder.is_some());
    assert_eq!(service_config.message_builder.as_ref().unwrap().to_string(), "openai");

    // Verify that message_format is not set
    assert!(service_config.message_format.is_none());

    // Validate should pass
    service_config.validate_message_format()
        .expect("v1 config validation should pass");

    // Verify message_format_name returns the builder string
    assert_eq!(service_config.message_format_name(), "openai");
}

#[test]
fn test_v2_config_parsing() {
    // Test parsing a v2-style config with message_format as a full object
    let yaml_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1
    Host: api.test.com
    Content-Type: application/json
    Authorization: Bearer ${API_KEY}

    {
      "model": "${model_id}",
      "messages": ${messages}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  success_codes: [200]
  extract:
    content: content

message_format:
  name: custom_openai
  text_message:
    template: '{"role": "{{role}}", "content": "{{json_escape content}}"}'
  multimodal_message:
    template: '{"role": "{{role}}", "content": {{json content_blocks}}}'
  content_blocks:
    - type: text
      template: '{"type": "text", "text": "{{json_escape text}}"}'
    - type: binary
      mime_patterns: ["image/*"]
      template: '{"type": "image_url", "image_url": {"url": "{{data_uri}}"}}'
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse v2 config");

    // Verify that message_format is set
    assert!(service_config.message_format.is_some());
    let format_config = service_config.message_format.as_ref().unwrap();
    assert_eq!(format_config.name, "custom_openai");

    // Verify that message_builder is not set
    assert!(service_config.message_builder.is_none());

    // Validate should pass
    service_config.validate_message_format()
        .expect("v2 config validation should pass");

    // Verify message_format_name returns the format name
    assert_eq!(service_config.message_format_name(), "custom_openai");
}

#[test]
fn test_both_fields_present() {
    // Test when both message_builder and message_format are present
    // This should be valid - v2 takes precedence
    let yaml_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1
    Host: api.test.com
    Content-Type: application/json

    {
      "model": "${model_id}",
      "messages": ${messages}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: openai
message_format:
  name: custom_format
  text_message:
    template: '{"role": "{{role}}", "content": "{{json_escape content}}"}'
  multimodal_message:
    template: '{"role": "{{role}}", "content": {{json content_blocks}}}'
  content_blocks:
    - type: text
      template: '{"type": "text", "text": "{{json_escape text}}"}'
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse config with both fields");

    // Both should be set
    assert!(service_config.message_builder.is_some());
    assert!(service_config.message_format.is_some());

    // Validate should pass (having both is acceptable)
    service_config.validate_message_format()
        .expect("Config with both fields should validate");

    // message_format takes precedence
    assert_eq!(service_config.message_format_name(), "custom_format");
}

#[test]
fn test_missing_message_format_validation() {
    // Test that validation fails when neither field is present
    let yaml_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1
    Host: api.test.com
    Content-Type: application/json

    {
      "model": "${model_id}",
      "messages": ${messages}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  success_codes: [200]
  extract:
    content: content
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse config");

    // Both should be None
    assert!(service_config.message_builder.is_none());
    assert!(service_config.message_format.is_none());

    // Validation should fail
    let result = service_config.validate_message_format();
    assert!(result.is_err());

    // Verify it's the right error type
    match result {
        Err(ClientError::Config(ConfigError::MissingField(msg))) => {
            assert!(msg.contains("message_builder"));
            assert!(msg.contains("message_format"));
        }
        _ => panic!("Expected MissingField error"),
    }

    // message_format_name should return "unknown"
    assert_eq!(service_config.message_format_name(), "unknown");
}

#[test]
fn test_v1_anthropic_config() {
    // Test with Anthropic format
    let yaml_config = r#"
service:
  name: AnthropicService
  base_url: https://api.anthropic.com

http:
  request: |
    POST /v1/messages HTTP/1.1
    Host: api.anthropic.com

    {
      "model": "${model_id}",
      "messages": ${messages}
    }

streaming:
  format: text/event-stream
  parser: anthropic_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: anthropic
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse Anthropic v1 config");

    assert!(service_config.message_builder.is_some());
    assert_eq!(service_config.message_builder.as_ref().unwrap().to_string(), "anthropic");

    service_config.validate_message_format()
        .expect("Anthropic v1 config should validate");

    assert_eq!(service_config.message_format_name(), "anthropic");
}

#[test]
fn test_v1_google_config() {
    // Test with Google format
    let yaml_config = r#"
service:
  name: GoogleService
  base_url: https://generativelanguage.googleapis.com

http:
  request: |
    POST /v1/models/${model_id}:generateContent HTTP/1.1
    Host: generativelanguage.googleapis.com

    {
      "contents": ${messages}
    }

streaming:
  format: text/event-stream
  parser: google_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: google
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse Google v1 config");

    assert!(service_config.message_builder.is_some());
    assert_eq!(service_config.message_builder.as_ref().unwrap().to_string(), "google");

    service_config.validate_message_format()
        .expect("Google v1 config should validate");

    assert_eq!(service_config.message_format_name(), "google");
}

#[test]
fn test_v1_custom_format() {
    // Test with a custom format (using Custom variant)
    let yaml_config = r#"
service:
  name: CustomService
  base_url: https://api.custom.com

http:
  request: |
    POST /v1/chat HTTP/1.1
    Host: api.custom.com

    {
      "model": "${model_id}",
      "messages": ${messages}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: my_custom_format
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse custom format config");

    assert!(service_config.message_builder.is_some());
    assert_eq!(service_config.message_builder.as_ref().unwrap().to_string(), "my_custom_format");

    service_config.validate_message_format()
        .expect("Custom format config should validate");

    assert_eq!(service_config.message_format_name(), "my_custom_format");
}

#[test]
fn test_case_insensitive_message_builder() {
    // MessageFormat enum should be case insensitive
    let yaml_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/chat HTTP/1.1

    {
      "model": "${model_id}"
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  success_codes: [200]
  extract:
    content: content

message_builder: OpenAI
"#;

    let service_config: ServiceConfig = serde_yaml::from_str(yaml_config)
        .expect("Failed to parse config with uppercase format");

    assert!(service_config.message_builder.is_some());
    // Should normalize to lowercase
    assert_eq!(service_config.message_builder.as_ref().unwrap().to_string(), "openai");

    service_config.validate_message_format()
        .expect("Case insensitive config should validate");
}
