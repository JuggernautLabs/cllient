//! Test template rendering edge cases and errors

use cllient::template::TemplateProcessor;
use cllient::{ClientFactory, ConfigLoader, LowLevelClient};
use std::collections::HashMap;
use serde_json::json;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_undefined_template_variables() {
    let processor = TemplateProcessor::new();
    
    // Template with undefined variable that should be handled gracefully
    let template = r#"
POST /api/test HTTP/1.1
Host: api.test.com
Content-Type: application/json

{
  "model": "${model_id}",
  "messages": ${messages},
  "stream": ${stream}${undefined_optional_field}
}
"#;
    
    let mut variables = HashMap::new();
    variables.insert("model_id".to_string(), json!("test-model"));
    variables.insert("messages".to_string(), json!([{"role": "user", "content": "test"}]));
    variables.insert("stream".to_string(), json!(false));
    // Note: undefined_optional_field is intentionally missing
    
    let result = processor.process_http_template(template, &variables);
    
    // This should either succeed with the undefined variable left as-is,
    // or fail with a descriptive error
    match result {
        Ok(http_request) => {
            // If it succeeds, the body should contain the literal ${undefined_optional_field}
            println!("Generated body: {}", http_request.body);
            assert!(http_request.body.contains("${undefined_optional_field}"));
        },
        Err(e) => {
            // If it fails, it should be a clear error about the undefined variable
            println!("Expected error for undefined variable: {:?}", e);
        }
    }
}

#[test]
fn test_malformed_json_in_template() {
    let processor = TemplateProcessor::new();

    // Template that will produce malformed JSON due to undefined variables
    // Note: Handlebars uses {{...}} syntax, not ${...}
    let template = r#"
POST /api/test HTTP/1.1
Host: api.test.com
Content-Type: application/json

{
  "model": "{{model_id}}",
  "stream": {{stream}}{{undefined_suffix}}
}
"#;

    let mut variables = HashMap::new();
    variables.insert("model_id".to_string(), json!("test-model"));
    variables.insert("stream".to_string(), json!(false));
    // undefined_suffix is missing, which will create: "stream": false (with empty string for undefined)

    let result = processor.process_http_template(template, &variables);

    // This should succeed in creating the HTTP request
    assert!(result.is_ok());
    let http_request = result.unwrap();

    // The body should contain the rendered template
    println!("Rendered JSON body: {}", http_request.body);
    // Handlebars renders undefined variables as empty string, so we get "false" followed by nothing
    assert!(http_request.body.contains("\"stream\": false"));
    assert!(http_request.body.contains("\"model\": \"test-model\""));
}

#[test]
fn test_debug_template_variables() {
    // Simple test to debug what variables are being used
    let processor = TemplateProcessor::new();
    
    let template = r#"
POST /api/test HTTP/1.1
Host: api.test.com
Content-Type: application/json

{
  "model": "${model_id}",
  "messages": ${messages},
  "temperature": ${temperature},
  "max_tokens": ${max_tokens},
  "stream": ${stream}
}
"#;
    
    let mut variables = HashMap::new();
    variables.insert("model_id".to_string(), json!("test-model"));
    variables.insert("messages".to_string(), json!([{"role": "user", "content": "test"}]));
    variables.insert("temperature".to_string(), json!(1.0));
    variables.insert("max_tokens".to_string(), json!(4096));
    variables.insert("stream".to_string(), json!(false));
    
    let result = processor.process_http_template(template, &variables);
    match result {
        Ok(http_request) => {
            println!("Successfully processed template:");
            println!("Body: {}", http_request.body);
            // Try to parse as JSON
            match serde_json::from_str::<serde_json::Value>(&http_request.body) {
                Ok(_) => println!("JSON is valid"),
                Err(e) => println!("JSON parse error: {}", e),
            }
        },
        Err(e) => {
            println!("Template processing error: {:?}", e);
        }
    }
}

#[test]
fn test_deepseek_config_template_issue() {
    // Create a temporary config setup that reproduces the DeepSeek issue
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path();
    
    fs::create_dir(config_dir.join("service")).unwrap();
    fs::create_dir_all(config_dir.join("family/deepseek")).unwrap();
    
    // Create a DeepSeek-like service config with the problematic template
    let service_config = r#"
service:
  name: TestDeepSeek
  base_url: https://api.deepseek.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Host: api.deepseek.com
    Content-Type: application/json
    Authorization: Bearer ${DEEPSEEK_API_KEY}

    {
      "model": "${model_id}",
      "messages": ${messages},
      "temperature": ${temperature},
      "max_tokens": ${max_tokens},
      "stream": ${stream}${optional_fields}
    }

streaming:
  format: text/event-stream
  parser: openai_sse
  line_prefix: "data: "
  done_marker: "[DONE]"
  events: []
  extract:
    content: choices[0].delta.content

response:
  success_codes: [200]
  extract:
    content: choices[0].message.content

message_builder: openai
"#;
    
    fs::write(config_dir.join("service/testdeepseek.yaml"), service_config).unwrap();
    
    // Create a model config
    let model_config = r#"
model:
  id: test-deepseek-v3
  family: deepseek
  name: Test DeepSeek V3
  version: "3"
  variant: base
  service: testdeepseek

capabilities:
  context_window: 64000
  max_output_tokens: 8192
  streaming: true

pricing:
  currency: USD
  input_per_1k_tokens: 0.00027
  output_per_1k_tokens: 0.0011

defaults:
  temperature: 1.0
  max_tokens: 4096
"#;
    
    fs::write(config_dir.join("family/deepseek/test-deepseek-v3.yaml"), model_config).unwrap();
    
    // Set environment variable
    std::env::set_var("DEEPSEEK_API_KEY", "test-key");
    
    // Try to create a client and build a request
    let config_loader = ConfigLoader::new(config_dir).unwrap();
    let factory = ClientFactory::new(config_loader);
    let client = factory.create_client("test-deepseek-v3").unwrap();
    
    // Try to make a completion request (this should fail due to the template issue)
    let request = cllient::CompletionRequest::text("user", "test prompt");
    
    // This will likely fail when the template is processed
    let result = tokio_test::block_on(client.complete(&request));
    
    match result {
        Ok(_) => panic!("Expected this to fail due to template issue"),
        Err(e) => {
            println!("Captured expected error: {:?}", e);
            // The error should be related to JSON parsing or template rendering
            let error_string = format!("{:?}", e);
            assert!(
                error_string.contains("JSON") || 
                error_string.contains("parse") || 
                error_string.contains("Render"),
                "Error should be related to JSON/template issue: {}",
                error_string
            );
        }
    }
    
    // Clean up
    std::env::remove_var("DEEPSEEK_API_KEY");
}