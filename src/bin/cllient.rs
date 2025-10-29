use cllient::{EmbeddedClientFactory, FileBasedClientFactory, ConfigLoader, EmbeddedConfigLoader, LowLevelClient, CompletionRequest, ClientFactory, ConfigProvider};
use cllient::streaming::StreamEvent;
use cllient::streaming_json::StreamingJsonObject;
use futures::StreamExt as FuturesStreamExt;
use std::env;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if it exists
    dotenv::dotenv().ok();
    
    let args: Vec<String> = env::args().collect();
    
    // Check for --verbose flag
    let verbose = args.iter().any(|arg| arg == "--verbose" || arg == "-v");
    
    // Check for --pretty flag (human-readable output with decorations)
    let pretty_output = args.iter().any(|arg| arg == "--pretty");
    
    // Default to JSON output, use pretty formatting if requested
    let json_output = !pretty_output;
    
    // Initialize tracing with appropriate level
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if verbose { 
            tracing::Level::DEBUG 
        } else { 
            tracing::Level::INFO 
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Filter out flags for command parsing
    let args: Vec<String> = args.into_iter()
        .filter(|arg| arg != "--verbose" && arg != "-v" && arg != "--pretty")
        .collect();
    
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    // Use embedded configs by default, fallback to file-based if CLLIENT_CONFIG_DIR is set
    if let Ok(config_path) = env::var("CLLIENT_CONFIG_DIR") {
        // Use file-based config if explicitly requested
        let config_loader = ConfigLoader::new(&config_path)?;
        let factory = FileBasedClientFactory::new(config_loader);
        run_with_factory(args, factory, json_output).await?;
    } else {
        // Use embedded configs by default
        let embedded_loader = EmbeddedConfigLoader::new()?;
        let factory = EmbeddedClientFactory::new(embedded_loader);
        run_with_factory(args, factory, json_output).await?;
    }

    Ok(())
}

async fn run_with_factory<T: ConfigProvider>(
    args: Vec<String>,
    factory: ClientFactory<T>,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::json;

    match args[1].as_str() {
        "list" => {
            // Check if there's a filter argument
            let filter = args.get(2).map(|s| s.as_str());
            list_models(&factory, filter, json_output);
        },
        "list-services" => {
            list_services(&factory, json_output);
        },
        "ask" => {
            if args.len() < 4 {
                if json_output {
                    println!("{}", json!({"error": "Usage: cllient ask <model> <prompt>", "type": "usage_error"}));
                } else {
                    eprintln!("Usage: cllient ask <model> <prompt>");
                }
                return Ok(());
            }
            let model_id = &args[2];
            let prompt = &args[3];
            ask_model(&factory, model_id, prompt, json_output).await?;
        },
        "stream" => {
            if args.len() < 4 {
                if json_output {
                    println!("{}", json!({"error": "Usage: cllient stream <model> <prompt>", "type": "usage_error"}));
                } else {
                    eprintln!("Usage: cllient stream <model> <prompt>");
                }
                return Ok(());
            }
            let model_id = &args[2];
            let prompt = &args[3];
            stream_model(&factory, model_id, prompt, json_output).await?;
        },
        "chat" => {
            if args.len() < 3 {
                eprintln!("Usage: cllient chat <model>");
                return Ok(());
            }
            let model_id = &args[2];
            interactive_chat(&factory, model_id).await?;
        },
        "compare" => {
            if args.len() < 4 {
                eprintln!("Usage: cllient compare <model1,model2,...> <prompt>");
                return Ok(());
            }
            let models: Vec<&str> = args[2].split(',').collect();
            let prompt = &args[3];
            compare_models(&factory, &models, prompt).await?;
        },
        "debug-response" => {
            if args.len() < 4 {
                eprintln!("Usage: cllient debug-response <model> <prompt>");
                eprintln!("This command shows the raw API response structure for debugging");
                return Ok(());
            }
            let model_id = &args[2];
            let prompt = &args[3];
            debug_raw_response(&factory, model_id, prompt).await?;
        },
        _ => {
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("CLLient - Configurable LLM Client");
    println!();
    println!("Usage:");
    println!("  cllient [--verbose|-v] list [filter]                  - List available models (optionally filter by provider)");
    println!("  cllient [--verbose|-v] list-services                   - List available services");
    println!("  cllient [--verbose|-v] ask <model> <prompt>           - Single completion");
    println!("  cllient [--verbose|-v] stream <model> <prompt>        - Streaming completion");
    println!("  cllient [--verbose|-v] chat <model>                   - Interactive chat");
    println!("  cllient [--verbose|-v] compare <model1,model2> <prompt> - Compare models");
    println!("  cllient debug-response <model> <prompt>              - Debug API response issues");
    println!();
    println!("Flags:");
    println!("  --verbose, -v                          - Enable debug logging");
    println!("  --pretty                               - Human-readable output with decorations (default: JSON)");
    println!();
    println!("Environment Variables:");
    println!("  CLLIENT_CONFIG_DIR - Path to config directory (default: ./config)");
    println!("  <SERVICE>_API_KEY  - API keys for services (e.g., ANTHROPIC_API_KEY)");
    println!();
    println!("Environment files (.env):");
    println!("  Automatically loads .env file from config directory or current directory");
    println!("  See .env.example for supported variables");
    println!();
    println!("Examples:");
    println!("  cllient list                          - List all models (JSON output)");
    println!("  cllient --pretty list claude          - List Claude models with decorations");
    println!("  cllient list \"gpt,claude\"             - List GPT and Claude models (JSON)");
    println!("  cllient list \"^(gpt|claude)$\"         - List models with exact family names");
    println!("  cllient list \".*-4.*\"                 - List models with '4' in family name (regex)");
    println!("  cllient ask claude-3-opus-20240229 \"What is Rust?\"");
    println!("  cllient --pretty stream gpt-4-turbo-preview \"Tell me a story\"");
    println!("  cllient compare claude-3-opus-20240229,gpt-4-turbo-preview \"Explain quantum computing\"");
    println!("  cllient --pretty chat deepseek-v3 \"Let's discuss programming\"");
}

fn list_models<T: ConfigProvider>(factory: &ClientFactory<T>, filter: Option<&str>, json_output: bool) {
    use std::io::{self, Write};
    use regex::Regex;
    use serde_json::json;
    use std::collections::BTreeMap;
    
    // Parse filters if provided
    let filters: Option<Vec<Regex>> = filter.map(|f| {
        f.split(',')
            .filter_map(|pattern| {
                match Regex::new(pattern.trim()) {
                    Ok(regex) => Some(regex),
                    Err(e) => {
                        if !json_output {
                            eprintln!("Warning: Invalid regex pattern '{}': {}", pattern, e);
                        }
                        None
                    }
                }
            })
            .collect()
    });
    
    // Check if a family matches any filter
    let family_matches = |family: &str| -> bool {
        if let Some(ref filters) = filters {
            filters.iter().any(|regex| regex.is_match(family))
        } else {
            true // No filter means show all
        }
    };
    
    let families = factory.list_families();
    let total_models = factory.list_available_models().len();
    let total_families = families.len();
    
    if json_output {
        // Build JSON output
        let mut result = BTreeMap::new();
        let mut providers = BTreeMap::new();
        let mut shown_families = 0;
        let mut shown_models = 0;
        
        for family in &families {
            if family_matches(family) {
                let models = factory.list_models_in_family(family);
                
                if !models.is_empty() {
                    let mut sorted_models = models;
                    sorted_models.sort();
                    
                    shown_families += 1;
                    let model_count = sorted_models.len();
                    shown_models += model_count;
                    
                    providers.insert(
                        family.clone(),
                        json!({
                            "models": sorted_models.into_iter().map(|m| m.to_string()).collect::<Vec<_>>(),
                            "count": model_count
                        })
                    );
                }
            }
        }
        
        result.insert("providers".to_string(), json!(providers));
        result.insert("summary".to_string(), json!({
            "shown_providers": shown_families,
            "shown_models": shown_models,
            "total_providers": total_families,
            "total_models": total_models,
            "filtered": filter.is_some()
        }));
        
        if let Some(f) = filter {
            result.insert("filter".to_string(), json!(f));
        }
        
        println!("{}", serde_json::to_string_pretty(&json!(result)).unwrap());
        
    } else {
        // Original text output with broken pipe handling
        macro_rules! print_or_exit {
            ($($arg:tt)*) => {
                if let Err(e) = writeln!(io::stdout(), $($arg)*) {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        std::process::exit(0);
                    } else {
                        panic!("Failed writing to stdout: {}", e);
                    }
                }
            };
        }
        
        if filter.is_some() {
            print_or_exit!("Available models by provider (filtered):");
        } else {
            print_or_exit!("Available models by provider:");
        }
        print_or_exit!();
        
        let mut shown_families = 0;
        let mut shown_models = 0;
        
        for family in &families {
            if family_matches(family) {
                let models = factory.list_models_in_family(family);
                
                if !models.is_empty() {
                    print_or_exit!("📦 {} ({} models)", family, models.len());
                    shown_families += 1;
                    
                    let mut sorted_models = models;
                    sorted_models.sort();
                    
                    for model in sorted_models {
                        print_or_exit!("  └─ {}", model);
                        shown_models += 1;
                    }
                    print_or_exit!();
                }
            }
        }
        
        if filter.is_some() {
            print_or_exit!("Summary: {} models across {} providers shown (filtered from {} models across {} providers)", 
                          shown_models, shown_families, total_models, total_families);
        } else {
            print_or_exit!("Summary: {} models across {} providers", total_models, total_families);
        }
    }
}

fn list_services<T: ConfigProvider>(factory: &ClientFactory<T>, json_output: bool) {
    use std::io::{self, Write};
    use serde_json::json;
    
    let services = factory.list_available_services();
    let service_count = services.len();
    
    if json_output {
        let result = json!({
            "services": services.into_iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "count": service_count
        });
        
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else {
        // Helper macro to handle broken pipe errors
        macro_rules! print_or_exit {
            ($($arg:tt)*) => {
                if let Err(e) = writeln!(io::stdout(), $($arg)*) {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        std::process::exit(0);
                    } else {
                        panic!("Failed writing to stdout: {}", e);
                    }
                }
            };
        }
        
        print_or_exit!("Available services:");
        let services = factory.list_available_services();
        for service in services {
            print_or_exit!("  {}", service);
        }
        print_or_exit!();
        print_or_exit!("Total: {} services", service_count);
    }
}

async fn ask_model<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
    prompt: &str,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::json;
    let client = match factory.create_client(model_id) {
        Ok(c) => c,
        Err(e) => {
            if json_output {
                println!("{}", json!({
                    "error": e.to_string(),
                    "model": model_id,
                    "type": "client_creation_failed"
                }));
            } else {
                eprintln!("Failed to create client: {}", e);
            }
            return Err(Box::new(e));
        }
    };
    
    if !json_output {
        println!("🤖 Model: {}", model_id);
        println!("💭 Prompt: {}", prompt);
        println!("📝 Response:");
        println!();
    }
    
    let request = CompletionRequest::text("user", prompt);
    
    // Check if verbose mode is enabled
    let verbose = env::var("RUST_LOG").unwrap_or_default().contains("debug") || 
                  env::args().any(|arg| arg == "--verbose" || arg == "-v");
    
    match client.complete(&request).await {
        Ok(response) => {
            if json_output {
                let mut result = json!({
                    "model": model_id,
                    "prompt": prompt,
                    "response": response.content,
                    "success": true
                });
                
                if let Some(usage) = &response.usage {
                    result["usage"] = json!({
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                        "total_tokens": usage.total_tokens
                    });
                }
                
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                if verbose {
                    println!("🔍 Debug: Response received successfully");
                    if let Some(usage) = &response.usage {
                        println!("🔍 Debug: Token usage - Input: {}, Output: {}", 
                               usage.input_tokens, usage.output_tokens);
                    }
                }
                println!("{}", response.content);
            }
        },
        Err(e) => {
            let error_str = e.to_string();
            
            if json_output {
                let mut error_type = "unknown_error";
                let mut hint = None;
                
                if error_str.contains("JsonPath") {
                    error_type = "json_path_error";
                    hint = Some("API response structure doesn't match expected format");
                } else if error_str.contains("401") {
                    error_type = "auth_error";
                    hint = Some("Check your API key for this provider");
                } else if error_str.contains("404") {
                    error_type = "not_found";
                    hint = Some("Model might not exist or be available");
                } else if error_str.contains("429") {
                    error_type = "rate_limit";
                    hint = Some("Rate limit exceeded. Try again in a moment");
                }
                
                let mut result = json!({
                    "model": model_id,
                    "prompt": prompt,
                    "success": false,
                    "error": error_str,
                    "type": error_type
                });
                
                if let Some(h) = hint {
                    result["hint"] = json!(h);
                }
                
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                eprintln!("❌ Error: {}", e);
                if verbose {
                    eprintln!("🔍 Debug: Full error details: {:?}", e);
                }
                
                // Try to provide more helpful error context
                match error_str.as_str() {
                    s if s.contains("JsonPath") => {
                        eprintln!("💡 Hint: This is a JSON path parsing error. The API response structure doesn't match the expected format.");
                        eprintln!("💡 Common causes:");
                        eprintln!("   - API provider changed their response format");
                        eprintln!("   - Wrong service configuration for this model");
                        eprintln!("   - Model returned an error response in unexpected format");
                        eprintln!("💡 Try using verbose mode to see the raw response: `cargo run --bin cllient -- --verbose ask {} \"{}\"`", model_id, prompt);
                        eprintln!("💡 Or try streaming mode instead: `cargo run --bin cllient -- stream {} \"{}\"`", model_id, prompt);
                    },
                    s if s.contains("401") => {
                        eprintln!("💡 Hint: Check your API key for this provider");
                    },
                    s if s.contains("404") => {
                        eprintln!("💡 Hint: Model '{}' might not exist or be available", model_id);
                    },
                    s if s.contains("429") => {
                        eprintln!("💡 Hint: Rate limit exceeded. Try again in a moment");
                    },
                    _ => {}
                }
            }
            
            return Err(Box::new(e));
        }
    }
    
    Ok(())
}


async fn stream_model<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
    prompt: &str,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = factory.create_client(model_id)?;
    
    if json_output {
        // Stream JSON structure as it's being built!
        let mut json_obj = StreamingJsonObject::new()?;

        json_obj.field_string("model", model_id)?;
        json_obj.field_string("prompt", prompt)?;

        let request = CompletionRequest::text("user", prompt).with_streaming(true);
        let mut stream = client.complete_stream(&request).await?;

        // Start streaming the response field
        let mut response_writer = json_obj.field_streaming_string("response")?;

        let mut error_occurred = false;
        let mut error_message = String::new();

        while let Some(event_result) = FuturesStreamExt::next(&mut stream).await {
            match event_result {
                Ok(StreamEvent::Content(chunk)) => {
                    // Stream each chunk directly to the JSON output!
                    response_writer.write_chunk(&chunk)?;
                },
                Ok(_) => {}, // Ignore other events for now
                Err(e) => {
                    error_occurred = true;
                    error_message = e.to_string();
                    break;
                }
            }
        }

        // Close the streaming response field
        response_writer.close()?;

        // Add remaining fields
        json_obj.field_bool("streamed", true)?;

        if error_occurred {
            json_obj.field_bool("success", false)?;
            json_obj.field_string("error", &error_message)?;
        } else {
            json_obj.field_bool("success", true)?;
        }

        // Close the JSON object
        json_obj.close()?;
    } else {
        // Original streaming output
        println!("🤖 Model: {}", model_id);
        println!("💭 Prompt: {}", prompt);
        println!("📡 Streaming response:");
        println!();
        
        let request = CompletionRequest::text("user", prompt).with_streaming(true);
        let mut stream = client.complete_stream(&request).await?;
        
        while let Some(event_result) = FuturesStreamExt::next(&mut stream).await {
            match event_result {
                Ok(StreamEvent::Content(chunk)) => {
                    print!("{}", chunk);
                    io::stdout().flush()?;
                },
                Ok(_) => {}, // Ignore other events for now
                Err(e) => {
                    eprintln!("\nError: {}", e);
                    break;
                }
            }
        }
        
        println!("\n");
    }
    
    Ok(())
}

async fn interactive_chat<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = factory.create_client(model_id)?;
    
    println!("🤖 Starting interactive chat with {}", model_id);
    println!("💡 Type 'quit' to exit, 'clear' to clear history");
    println!();
    
    let mut history = Vec::new();
    
    loop {
        print!("You: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        if input == "quit" {
            break;
        }
        
        if input == "clear" {
            history.clear();
            println!("🗑️ History cleared");
            continue;
        }
        
        // Add user message to history
        history.push(serde_json::json!({
            "role": "user",
            "content": input
        }));
        
        print!("Assistant: ");
        io::stdout().flush()?;
        
        let request = CompletionRequest::text("user", input).with_streaming(true);
        match client.complete_stream(&request).await {
            Ok(mut stream) => {
                let mut response_content = String::new();
                
                while let Some(event_result) = FuturesStreamExt::next(&mut stream).await {
                    match event_result {
                        Ok(StreamEvent::Content(chunk)) => {
                            print!("{}", chunk);
                            response_content.push_str(&chunk);
                            io::stdout().flush()?;
                        },
                        Ok(_) => {}, // Ignore other events for now
                        Err(e) => {
                            eprintln!("\nError: {}", e);
                            break;
                        }
                    }
                }
                
                println!("\n");
                
                // Add assistant response to history
                history.push(serde_json::json!({
                    "role": "assistant",
                    "content": response_content
                }));
            },
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
    
    println!("👋 Goodbye!");
    Ok(())
}

async fn compare_models<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_ids: &[&str],
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Comparing models on prompt: {}", prompt);
    println!();
    
    // Check if verbose mode is enabled
    let verbose = env::var("RUST_LOG").unwrap_or_default().contains("debug") || 
                  env::args().any(|arg| arg == "--verbose" || arg == "-v");
    
    for model_id in model_ids {
        println!("🤖 {} ", model_id);
        println!("{}", "─".repeat(50));
        
        if verbose {
            println!("🔍 Debug: Creating client for model: {}", model_id);
        }
        
        match factory.create_client(model_id) {
            Ok(client) => {
                if verbose {
                    println!("🔍 Debug: Client created successfully, sending request...");
                }
                
                let request = CompletionRequest::text("user", prompt);
                match client.complete(&request).await {
                    Ok(response) => {
                        if verbose {
                            println!("🔍 Debug: Response received successfully");
                            if let Some(usage) = &response.usage {
                                println!("🔍 Debug: Token usage - Input: {}, Output: {}", 
                                       usage.input_tokens, usage.output_tokens);
                            }
                        }
                        println!("{}", response.content);
                    },
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                        if verbose {
                            eprintln!("🔍 Debug: Full error details: {:?}", e);
                        }
                        
                        // Provide helpful hints
                        match e.to_string().as_str() {
                            s if s.contains("JsonPath") => {
                                eprintln!("💡 Hint: JSON path parsing error - API response format mismatch");
                                eprintln!("💡 Try: cargo run --bin cllient -- --verbose ask {} \"test prompt\" for debug info", model_id);
                            },
                            s if s.contains("401") => {
                                eprintln!("💡 Hint: Check API key for this provider");
                            },
                            s if s.contains("404") => {
                                eprintln!("💡 Hint: Model '{}' might not be available", model_id);
                            },
                            _ => {}
                        }
                    }
                }
            },
            Err(e) => {
                eprintln!("❌ Failed to create client: {}", e);
                if verbose {
                    eprintln!("🔍 Debug: Client creation error details: {:?}", e);
                }
            }
        }
        
        println!();
    }
    
    Ok(())
}

async fn debug_raw_response<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Debug mode: Analyzing API response for model '{}'", model_id);
    println!("💭 Prompt: {}", prompt);
    println!("{}", "=".repeat(60));
    
    let client = factory.create_client(model_id)?;
    let request = CompletionRequest::text("user", prompt);
    
    println!("📡 Sending request...");
    
    match client.complete(&request).await {
        Ok(response) => {
            println!("✅ Request succeeded!");
            println!("📝 Content: {}", response.content);
            
            if let Some(usage) = &response.usage {
                println!("📊 Usage:");
                println!("   Input tokens: {}", usage.input_tokens);
                println!("   Output tokens: {}", usage.output_tokens);
                if let Some(total) = usage.total_tokens {
                    println!("   Total tokens: {}", total);
                }
            }
        },
        Err(e) => {
            println!("❌ Request failed with error: {}", e);
            println!("🔍 Error type: {:?}", e);
            
            // Provide specific troubleshooting based on error type
            let error_str = e.to_string();
            
            if error_str.contains("JsonPath") {
                println!("\n🔧 JSON Path Error Troubleshooting:");
                println!("   This indicates the API response structure doesn't match expectations.");
                println!("   Common causes:");
                println!("   - API response format changed");
                println!("   - Wrong service configuration for this model");
                println!("   - Model might not be compatible with this service");
                println!("\n💡 Try:");
                println!("   1. Use streaming mode: `cargo run --bin cllient -- stream {} \"{}\"`", model_id, prompt);
                println!("   2. Check if model exists: `cargo run --bin cllient -- list | grep {}`", model_id);
                println!("   3. Enable debug logs: `RUST_LOG=debug cargo run --bin cllient -- ask {} \"{}\"`", model_id, prompt);
            } else if error_str.contains("401") {
                println!("\n🔧 Authentication Error (401):");
                println!("   Your API key is missing or invalid.");
                println!("   Check your .env file or environment variables.");
            } else if error_str.contains("404") {
                println!("\n🔧 Not Found Error (404):");
                println!("   The model '{}' was not found.", model_id);
                println!("   Check available models: `cargo run --bin cllient -- list`");
            } else if error_str.contains("429") {
                println!("\n🔧 Rate Limit Error (429):");
                println!("   Too many requests. Wait a moment and try again.");
            } else {
                println!("\n🔧 General troubleshooting:");
                println!("   1. Check your internet connection");
                println!("   2. Verify API keys are set correctly");
                println!("   3. Try a different model");
            }
        }
    }
    
    Ok(())
}