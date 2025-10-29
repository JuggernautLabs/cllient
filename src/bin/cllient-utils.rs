use std::env;
use std::sync::Arc;
use std::fs;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use cllient::runtime::ModelRegistry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv::dotenv().ok();
    
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: cllient-utils <command> [args]");
        eprintln!("Commands:");
        eprintln!("  test-family <family> [--update-status]  - Test all models in a family");
        eprintln!("  debug-families                          - Show available families");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --update-status  - Automatically update model config files with status based on test results");
        std::process::exit(1);
    }
    
    let command = &args[1];
    let family = if args.len() > 2 && !args[2].starts_with("--") { &args[2] } else { "" };
    let update_status = args.contains(&"--update-status".to_string());
    
    match command.as_str() {
        "test-family" => test_family(family, update_status).await?,
        "debug-families" => debug_families().await?,
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Available commands: test-family, debug-families");
            std::process::exit(1);
        }
    }
    
    Ok(())
}

async fn test_family(family: &str, update_status: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Create runtime and get models from family
    let runtime = ModelRegistry::new()?;
    let models: Vec<String> = runtime.list_models_in_family(family)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    
    if models.is_empty() {
        eprintln!("No models found for family: {}", family);
        return Ok(());
    }
    
    println!("Testing {} models in family '{}':", models.len(), family);
    for model in &models {
        println!("  - {}", model);
    }
    println!();
    
    // Run tests in parallel with semaphore to limit concurrency
    let semaphore = Arc::new(Semaphore::new(5)); // Limit to 5 concurrent requests
    let mut join_set = JoinSet::new();
    
    for model in models {
        let permit = semaphore.clone().acquire_owned().await?;
        join_set.spawn(async move {
            let _permit = permit; // Keep permit alive
            test_model_stream(&model).await
        });
    }
    
    // Collect results
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(test_result) => results.push(test_result),
            Err(e) => eprintln!("Task join error: {}", e),
        }
    }
    
    // Sort results by model name for consistent output
    results.sort_by(|a, b| a.model.cmp(&b.model));
    
    // Output all results and optionally update statuses
    for result in &results {
        println!("{}", "=".repeat(80));
        println!("Model: {}", result.model);
        println!("Status: {}", if result.success { "✅ SUCCESS" } else { "❌ FAILED" });
        println!("Output:");
        println!("{}", result.output);
        if !result.error.is_empty() {
            println!("Error:");
            println!("{}", result.error);
        }
        println!();
    }
    
    // Update status in config files if requested
    if update_status {
        println!("{}", "=".repeat(80));
        println!("Updating model statuses in config files...");
        for result in &results {
            if let Err(e) = update_model_status(&result.model, result).await {
                eprintln!("Failed to update status for {}: {}", result.model, e);
            } else {
                println!("Updated status for {}: {}", result.model, get_status_from_result(result));
            }
        }
    }
    
    Ok(())
}

#[derive(Debug)]
struct TestResult {
    model: String,
    success: bool,
    output: String,
    error: String,
}

async fn test_model_stream(model: &str) -> TestResult {
    match test_model_with_runtime(model).await {
        Ok(response) => TestResult {
            model: model.to_string(),
            success: true,
            output: response,
            error: String::new(),
        },
        Err(e) => TestResult {
            model: model.to_string(),
            success: false,
            output: String::new(),
            error: format!("Error: {}", e),
        }
    }
}

async fn test_model_with_runtime(model: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use futures::StreamExt;
    
    let runtime = ModelRegistry::new()?;
    let mut stream = runtime.from_id(model)?
        .stream("respond with just hi")
        .await?;
    
    let mut response_parts = Vec::new();
    
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(stream_event) => {
                // Extract content from StreamEvent
                response_parts.push(format!("{:?}", stream_event));
            },
            Err(e) => return Err(format!("Stream error: {}", e).into()),
        }
    }
    
    Ok(response_parts.join(""))
}

fn get_status_from_result(result: &TestResult) -> &'static str {
    if result.success {
        "verified"
    } else if result.error.contains("authentication_error") || result.error.contains("invalid x-api-key") {
        "auth_error"
    } else if result.error.contains("not_found_error") || result.error.contains("does not exist") || result.error.contains("deprecated") {
        "not_found"
    } else if result.error.contains("organization must be verified") || result.error.contains("unsupported_value") {
        "access_restricted"
    } else if result.error.contains("requires audio") || result.error.contains("invalid_value") {
        "special_requirements"
    } else if result.error.contains("unsupported parameter") || result.error.contains("not supported") {
        "endpoint_error"
    } else {
        "error"
    }
}

async fn update_model_status(model_id: &str, result: &TestResult) -> Result<(), Box<dyn std::error::Error>> {
    let status = get_status_from_result(result);
    
    // Find the config file for this model by searching through all config directories
    let config_dirs = [
        "config/family/openai",
        "config/family/gpt", 
        "config/family/claude",
        "config/family/anthropic",
    ];
    
    for config_dir in &config_dirs {
        if let Ok(entries) = fs::read_dir(config_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "yaml") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        // Check if this config file contains our model ID
                        if content.contains(&format!("id: {}", model_id)) {
                            if let Some(path_str) = path.to_str() {
                                update_config_file_status(path_str, status).await?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }
    
    Err(format!("Config file not found for model: {}", model_id).into())
}

async fn update_config_file_status(config_path: &str, status: &str) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(config_path)?;
    
    // Parse as YAML to check if status field exists
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    
    // Find where to insert the status field (after model: section)
    let mut insert_index = None;
    let mut in_model_section = false;
    let mut found_status = false;
    
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("model:") {
            in_model_section = true;
        } else if in_model_section && line.starts_with("  status:") {
            // Update existing status
            lines[i] = format!("  status: {}", status);
            found_status = true;
            break;
        } else if in_model_section && !line.starts_with("  ") && !line.trim().is_empty() {
            // We've left the model section
            insert_index = Some(i);
            break;
        } else if in_model_section && line.trim().is_empty() && insert_index.is_none() {
            insert_index = Some(i);
        }
    }
    
    // If status wasn't found, insert it
    if !found_status {
        if let Some(index) = insert_index {
            lines.insert(index, format!("  status: {}", status));
        } else {
            // Insert after the last model field
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("model:") {
                    // Find the end of the model section
                    for j in (i + 1)..lines.len() {
                        if !lines[j].starts_with("  ") && !lines[j].trim().is_empty() {
                            lines.insert(j, format!("  status: {}", status));
                            break;
                        }
                    }
                    break;
                }
            }
        }
    }
    
    // Write back to file
    let updated_content = lines.join("\n");
    fs::write(config_path, updated_content)?;
    
    Ok(())
}

async fn debug_families() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = ModelRegistry::new()?;
    
    println!("Available families:");
    let families = runtime.list_families();
    for family in &families {
        println!("  - {}", family);
        let models = runtime.list_models_in_family(family);
        println!("    Models ({}): {:?}", models.len(), models);
    }
    
    println!("\nLooking for claude models:");
    let claude_models = runtime.list_models_in_family("claude");
    println!("Claude family models: {:?}", claude_models);
    
    println!("\nAll models (first 10):");
    let all_models = runtime.list_models();
    for (i, model) in all_models.iter().take(10).enumerate() {
        if let Ok(info) = runtime.get_model_info(model) {
            println!("  {}: {} (family: {})", i+1, model, info.model.family);
        }
    }
    
    Ok(())
}