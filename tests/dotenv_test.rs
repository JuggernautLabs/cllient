//! Test that dotenv loading works correctly

use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_dotenv_loading_from_config_dir() {
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let config_dir = temp_dir.path();
    
    // Create a .env file in the config directory
    let env_content = "TEST_API_KEY_FROM_DOTENV=test-value-12345\n";
    fs::write(config_dir.join(".env"), env_content).unwrap();
    
    // Create service and model directories
    fs::create_dir(config_dir.join("service")).unwrap();
    fs::create_dir(config_dir.join("family")).unwrap();
    
    // Clear the env var first to ensure we're testing the loading
    env::remove_var("TEST_API_KEY_FROM_DOTENV");
    
    // Load the config which should load the .env file
    let config_loader = cllient::ConfigLoader::new(config_dir).unwrap();
    
    // Verify the environment variable was loaded
    assert_eq!(
        env::var("TEST_API_KEY_FROM_DOTENV").unwrap(),
        "test-value-12345"
    );
    
    // Clean up
    drop(config_loader);
    env::remove_var("TEST_API_KEY_FROM_DOTENV");
}

#[test]
fn test_dotenv_template_substitution() {
    use cllient::template::TemplateProcessor;
    use std::collections::HashMap;
    
    // Set a test environment variable
    env::set_var("TEST_TEMPLATE_API_KEY", "secret-key-999");
    
    let processor = TemplateProcessor::new();
    let template = r#"
    POST /api/endpoint HTTP/1.1
    Host: api.example.com
    Authorization: Bearer ${TEST_TEMPLATE_API_KEY}
    "#;
    
    let variables = HashMap::new();
    let result = processor.process_http_template(template, &variables).unwrap();
    
    // Verify the environment variable was substituted
    assert_eq!(
        result.headers.get("Authorization").unwrap(),
        "Bearer secret-key-999"
    );
    
    // Clean up
    env::remove_var("TEST_TEMPLATE_API_KEY");
}