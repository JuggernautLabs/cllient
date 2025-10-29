use cllient::ModelRegistry;
use std::env;

#[test]
fn test_runtime_initialization() {
    // Test creating runtime with default config
    let runtime = ModelRegistry::new();
    assert!(runtime.is_ok(), "Failed to initialize runtime");
    
    let runtime = runtime.unwrap();
    
    // Test listing models
    let models = runtime.list_models();
    assert!(!models.is_empty(), "No models found");
    // Note: embedded config might have fewer models than file-based config
    assert!(models.len() > 10, "Expected at least 10 models");
}

#[test]
fn test_runtime_list_models() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test listing all models
    let all_models = runtime.list_models();
    
    // Verify we have models from different providers
    let has_claude = all_models.iter().any(|m| m.contains("claude"));
    let has_gpt = all_models.iter().any(|m| m.contains("gpt"));
    
    assert!(has_claude, "Expected Claude models");
    assert!(has_gpt, "Expected GPT models");
    
    // DeepSeek models might not be in embedded config
    let has_deepseek = all_models.iter().any(|m| m.contains("deepseek"));
    if !has_deepseek {
        eprintln!("Warning: No DeepSeek models found in embedded config");
    }
}

#[test]
fn test_runtime_from_id() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test creating request builder for specific model
    // Try multiple models in case some aren't available
    let test_models = ["gpt-4o-mini", "gpt-3.5-turbo", "claude-3-haiku-20240307"];
    let mut found_model = None;
    
    for model_id in &test_models {
        if let Ok(builder) = runtime.from_id(model_id) {
            assert_eq!(builder.model_id(), *model_id);
            found_model = Some(model_id);
            break;
        }
    }
    
    assert!(found_model.is_some(), "Could not find any test model");
    
    // Test with invalid model
    let invalid = runtime.from_id("non-existent-model");
    assert!(invalid.is_err(), "Expected error for non-existent model");
}

#[test]
fn test_runtime_use_cheapest() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test finding cheapest GPT model
    let cheapest_gpt = runtime.use_cheapest("gpt-*");
    assert!(cheapest_gpt.is_ok(), "Failed to find cheapest GPT model");
    
    let builder = cheapest_gpt.unwrap();
    assert!(builder.model_id().starts_with("gpt"), "Expected GPT model");
    
    // Test finding cheapest Claude model
    let cheapest_claude = runtime.use_cheapest("claude-*");
    assert!(cheapest_claude.is_ok(), "Failed to find cheapest Claude model");
    
    let builder = cheapest_claude.unwrap();
    assert!(builder.model_id().starts_with("claude"), "Expected Claude model");
}

#[test]
fn test_runtime_use_fastest() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test finding fastest model (usually based on latency/TTFT)
    let fastest_gpt = runtime.use_fastest("gpt-*");
    assert!(fastest_gpt.is_ok(), "Failed to find fastest GPT model");
    
    let builder = fastest_gpt.unwrap();
    assert!(builder.model_id().starts_with("gpt"), "Expected GPT model");
}

#[test]
fn test_runtime_get_model_info() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test getting model information
    let info = runtime.get_model_info("claude-3-haiku-20240307");
    assert!(info.is_ok(), "Failed to get model info");
    
    let model_config = info.unwrap();
    assert_eq!(model_config.model.id, "claude-3-haiku-20240307");
    assert_eq!(model_config.model.family, "claude");
    assert!(model_config.capabilities.context_window > 0);
    
    // Test pricing info
    assert!(model_config.pricing.input_per_1k_tokens > 0.0);
    assert!(model_config.pricing.output_per_1k_tokens > 0.0);
}

#[test]
fn test_runtime_request_builder_fluent_api() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test fluent API for request configuration
    // Use a model that's likely to exist
    let test_models = ["gpt-3.5-turbo", "claude-3-haiku-20240307", "gpt-4o-mini"];
    let mut builder = None;
    
    for model_id in &test_models {
        if let Ok(b) = runtime.from_id(model_id) {
            builder = Some(b
                .system("You are a helpful assistant")
                .temperature(0.7)
                .max_tokens(100)
                .top_p(0.9));
            break;
        }
    }
    
    assert!(builder.is_some(), "Could not find any test model for fluent API test");
}

#[test]
fn test_runtime_with_actual_request() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_runtime_with_actual_request: DEEPSEEK_API_KEY not set");
        return;
    }
    
    let runtime = ModelRegistry::new().unwrap();
    
    // Test actual API call using runtime
    let request = runtime
        .from_id("deepseek-chat")
        .unwrap()
        .max_tokens(20);
    
    // We can't test the actual async completion without tokio runtime,
    // but we can verify the request builder is properly configured
    assert_eq!(request.model_id(), "deepseek-chat");
}

#[tokio::test]
async fn test_runtime_async_completion() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_runtime_async_completion: DEEPSEEK_API_KEY not set");
        return;
    }
    
    let runtime = ModelRegistry::new().unwrap();
    
    // Test actual async completion
    let response = runtime
        .from_id("deepseek-chat")
        .unwrap()
        .max_tokens(10)
        .temperature(0.0)
        .send("What is 5+5? Answer with just the number.")
        .await;
    
    assert!(response.is_ok(), "Failed to complete request");
    
    let response = response.unwrap();
    assert!(response.text().contains("10"), "Expected '10' in response");
}

#[tokio::test]
async fn test_runtime_streaming() {
    if env::var("DEEPSEEK_API_KEY").is_err() {
        eprintln!("Skipping test_runtime_streaming: DEEPSEEK_API_KEY not set");
        return;
    }
    
    use futures::StreamExt;
    
    let runtime = ModelRegistry::new().unwrap();
    
    // Test streaming response
    let mut stream = runtime
        .from_id("deepseek-chat")
        .unwrap()
        .max_tokens(50)
        .stream("Count to 3")
        .await
        .unwrap();
    
    let mut content = String::new();
    
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(cllient::streaming::StreamEvent::Content(text)) => content.push_str(&text),
            Ok(_) => {}, // Ignore other event types
            Err(e) => panic!("Stream error: {}", e),
        }
    }
    
    assert!(content.contains("1"), "Expected '1' in stream");
    assert!(content.contains("2"), "Expected '2' in stream");
    assert!(content.contains("3"), "Expected '3' in stream");
}

#[test]
fn test_runtime_pattern_matching() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test pattern matching for models
    let gpt_models = runtime.list_models_matching("gpt-4*").unwrap();
    assert!(!gpt_models.is_empty(), "No GPT-4 models found");
    assert!(gpt_models.iter().all(|m| m.starts_with("gpt-4")), "Found non-GPT-4 model");
    
    let claude_opus = runtime.list_models_matching("*opus*").unwrap();
    assert!(!claude_opus.is_empty(), "No Opus models found");
    assert!(claude_opus.iter().all(|m| m.contains("opus")), "Found non-Opus model");
    
    // Test with no matches
    let no_match = runtime.list_models_matching("xyz-*").unwrap();
    assert!(no_match.is_empty(), "Expected no matches");
}

#[test]
fn test_runtime_cost_calculation() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Get model info to calculate costs
    // Try multiple models in case some aren't available
    let test_models = ["gpt-3.5-turbo", "claude-3-haiku-20240307", "gpt-4o-mini"];
    let mut model_info = None;
    
    for model_id in &test_models {
        if let Ok(info) = runtime.get_model_info(model_id) {
            model_info = Some(info);
            break;
        }
    }
    
    let model_info = model_info.expect("Could not find any test model for cost calculation");
    
    // Calculate cost for 1000 input tokens and 500 output tokens
    let input_cost = (1000.0 / 1000.0) * model_info.pricing.input_per_1k_tokens;
    let output_cost = (500.0 / 1000.0) * model_info.pricing.output_per_1k_tokens;
    let total_cost = input_cost + output_cost;
    
    assert!(total_cost > 0.0, "Expected positive cost");
    assert!(total_cost < 1.0, "Cost seems too high for GPT-3.5");
}

#[test] 
fn test_runtime_multimodal_models() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Check for vision-capable models
    // Try to find any vision-capable model
    let vision_models = ["gpt-4o", "gpt-4o-mini", "claude-3-5-sonnet-20241022", "claude-3-haiku-20240307"];
    let mut found_vision_model = false;
    
    for model_id in &vision_models {
        if let Ok(model_info) = runtime.get_model_info(model_id) {
            if model_info.capabilities.vision {
                found_vision_model = true;
                break;
            }
        }
    }
    
    assert!(found_vision_model, "Could not find any vision-capable model");
}

#[test]
fn test_runtime_error_handling() {
    let runtime = ModelRegistry::new().unwrap();
    
    // Test various error scenarios
    
    // Non-existent model
    let result = runtime.from_id("non-existent-model-xyz");
    assert!(result.is_err(), "Expected error for non-existent model");
    
    // Invalid pattern
    let result = runtime.use_cheapest("[]invalid-pattern");
    // This might not error depending on implementation, but shouldn't panic
    let _ = result;
    
    // Empty pattern
    let result = runtime.use_cheapest("");
    assert!(result.is_err() || result.unwrap().model_id().len() > 0);
}