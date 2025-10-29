//! Integration test for environment variable handling

use cllient::{ClientFactory, ConfigLoader};
use std::env;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_api_key_substitution_in_templates() {
    // Create a temporary config directory
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path();
    
    // Create service and family directories
    fs::create_dir(config_dir.join("service")).unwrap();
    fs::create_dir_all(config_dir.join("family/test")).unwrap();
    
    // Create a test service config that uses an environment variable
    let service_config = r#"
service:
  name: TestService
  base_url: https://api.test.com

http:
  request: |
    POST /v1/test HTTP/1.1
    Host: api.test.com
    Authorization: Bearer ${TEST_SERVICE_API_KEY}
    Content-Type: application/json

    {
      "model": "${model_id}",
      "messages": ${messages}
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
    content: result.text

message_builder: openai
"#;
    
    fs::write(config_dir.join("service/test.yaml"), service_config).unwrap();
    
    // Create a test model config
    let model_config = r#"
model:
  id: test-model-1
  family: test
  name: Test Model
  version: "1"
  variant: base
  service: test

capabilities:
  context_window: 1000
  max_output_tokens: 100

pricing:
  currency: USD
  input_per_1k_tokens: 0.001
  output_per_1k_tokens: 0.002

defaults:
  temperature: 0.7
  max_tokens: 100
"#;
    
    fs::write(config_dir.join("family/test/test-model-1.yaml"), model_config).unwrap();
    
    // Create a .env file
    let env_content = "TEST_SERVICE_API_KEY=super-secret-key-123\n";
    fs::write(config_dir.join(".env"), env_content).unwrap();
    
    // Clear the env var to ensure we're loading from .env
    env::remove_var("TEST_SERVICE_API_KEY");
    
    // Load the config (which should load the .env file)
    let config_loader = ConfigLoader::new(config_dir).unwrap();
    
    // Verify the environment variable was loaded
    assert_eq!(
        env::var("TEST_SERVICE_API_KEY").unwrap(),
        "super-secret-key-123"
    );
    
    // Create a client factory
    let factory = ClientFactory::new(config_loader);
    
    // Verify we can create a client for our test model
    let client = factory.create_client("test-model-1");
    assert!(client.is_ok(), "Failed to create client: {:?}", client.err());
    
    // Clean up
    env::remove_var("TEST_SERVICE_API_KEY");
}

#[test]
fn test_fallback_to_root_dotenv() {
    use std::fs::File;
    use std::io::Write;
    
    // Create a .env file in the current directory
    let mut env_file = File::create(".env.test").unwrap();
    writeln!(env_file, "FALLBACK_TEST_KEY=fallback-value-456").unwrap();
    
    // Save current dir and change to use our test .env
    let original_env = env::current_dir().unwrap();
    
    // Temporarily rename our test file
    fs::rename(".env.test", ".env").ok();
    
    // Create a temp directory without a .env file
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path();
    
    // Create necessary directories
    fs::create_dir(config_dir.join("service")).unwrap();
    fs::create_dir(config_dir.join("family")).unwrap();
    
    // Clear the env var
    env::remove_var("FALLBACK_TEST_KEY");
    
    // Load config - should fall back to root .env
    let _config_loader = ConfigLoader::new(config_dir).unwrap();
    
    // Verify the variable was loaded from root .env
    assert_eq!(
        env::var("FALLBACK_TEST_KEY").unwrap_or_default(),
        "fallback-value-456"
    );
    
    // Clean up
    env::set_current_dir(original_env).unwrap();
    fs::remove_file(".env").ok();
    env::remove_var("FALLBACK_TEST_KEY");
}