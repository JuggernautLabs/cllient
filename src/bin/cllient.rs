use clap::{Parser, Subcommand};
use cllient::{
    ClientFactory, CompletionRequest, ConfigLoader, ConfigProvider, EmbeddedClientFactory,
    EmbeddedConfigLoader, FileBasedClientFactory, HttpClient, ClientError, LowLevelClient,
};
use cllient::streaming::StreamEvent;
use cllient::streaming_json::StreamingJsonObject;
use futures::StreamExt as FuturesStreamExt;
use std::env;
use std::io::{self, Write};
use std::sync::OnceLock;

// ============================================================================
// CLI Definition
// ============================================================================

/// CLLient - Configurable LLM Client
///
/// A config-driven LLM client that provides a unified interface to multiple
/// LLM providers (Anthropic, OpenAI, Google, DeepSeek, etc.).
#[derive(Parser)]
#[command(name = "cllient")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable debug logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Human-readable output with decorations
    #[arg(long, global = true)]
    pretty: bool,

    /// Output only LLM response (no formatting, for piping)
    #[arg(long, global = true)]
    clean: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available models
    #[command(visible_alias = "ls")]
    List {
        /// Regex pattern to filter models by family name
        filter: Option<String>,
    },

    /// List available services
    ListServices,

    /// Single completion request
    Ask {
        /// Model ID (e.g., gpt-4o-mini, claude-3-haiku-20240307)
        model: String,
        /// The prompt to send to the model
        prompt: String,
    },

    /// Streaming completion request
    Stream {
        /// Model ID (e.g., gpt-4o-mini, deepseek-chat)
        model: String,
        /// The prompt to send to the model
        prompt: String,
    },

    /// Interactive chat session
    Chat {
        /// Model ID to chat with
        model: String,
    },

    /// Compare multiple models on the same prompt
    Compare {
        /// Comma-separated list of model IDs
        models: String,
        /// The prompt to send to all models
        prompt: String,
    },

    /// Debug API response issues
    DebugResponse {
        /// Model ID to test
        model: String,
        /// Test prompt
        prompt: String,
    },
}

// ============================================================================
// Global Config
// ============================================================================

static CONFIG: OnceLock<OutputConfig> = OnceLock::new();

#[derive(Debug, Clone)]
struct OutputConfig {
    verbose: bool,
    json_output: bool,
    clean_output: bool,
}

impl OutputConfig {
    fn get() -> &'static OutputConfig {
        CONFIG.get().expect("Config not initialized")
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let cli = Cli::parse();

    CONFIG
        .set(OutputConfig {
            verbose: cli.verbose,
            json_output: !cli.pretty && !cli.clean,
            clean_output: cli.clean,
        })
        .expect("Failed to initialize config");

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    if let Ok(config_path) = env::var("CLLIENT_CONFIG_DIR") {
        let loader = ConfigLoader::new(&config_path)?;
        run_command(cli.command, FileBasedClientFactory::new(loader)).await
    } else {
        let loader = EmbeddedConfigLoader::new()?;
        run_command(cli.command, EmbeddedClientFactory::new(loader)).await
    }
}

async fn run_command<T: ConfigProvider>(
    command: Commands,
    factory: ClientFactory<T>,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Commands::List { filter } => list_models(&factory, filter.as_deref()),
        Commands::ListServices => list_services(&factory),
        Commands::Ask { model, prompt } => ask_model(&factory, &model, &prompt).await?,
        Commands::Stream { model, prompt } => stream_model(&factory, &model, &prompt).await?,
        Commands::Chat { model } => interactive_chat(&factory, &model).await?,
        Commands::Compare { models, prompt } => {
            let model_list: Vec<&str> = models.split(',').collect();
            compare_models(&factory, &model_list, &prompt).await?
        }
        Commands::DebugResponse { model, prompt } => {
            debug_raw_response(&factory, &model, &prompt).await?
        }
    }
    Ok(())
}

// ============================================================================
// Output Helpers
// ============================================================================

macro_rules! print_or_exit {
    ($($arg:tt)*) => {
        if let Err(e) = writeln!(io::stdout(), $($arg)*) {
            if e.kind() == io::ErrorKind::BrokenPipe {
                std::process::exit(0);
            }
            panic!("Failed writing to stdout: {}", e);
        }
    };
}

fn classify_error(error_str: &str) -> (&'static str, Option<&'static str>) {
    if error_str.contains("JsonPath") {
        ("json_path_error", Some("API response structure doesn't match expected format"))
    } else if error_str.contains("401") {
        ("auth_error", Some("Check your API key for this provider"))
    } else if error_str.contains("404") {
        ("not_found", Some("Model might not exist or be available"))
    } else if error_str.contains("429") {
        ("rate_limit", Some("Rate limit exceeded. Try again in a moment"))
    } else {
        ("unknown_error", None)
    }
}

// ============================================================================
// List Commands
// ============================================================================

fn list_models<T: ConfigProvider>(factory: &ClientFactory<T>, filter: Option<&str>) {
    let config = OutputConfig::get();
    let filters = parse_filters(filter, config);
    let families = factory.list_families();
    let total_models = factory.list_available_models().len();

    if config.json_output {
        print_models_json(factory, &families, &filters, filter, total_models);
    } else {
        print_models_pretty(factory, &families, &filters, filter, total_models);
    }
}

fn parse_filters(filter: Option<&str>, config: &OutputConfig) -> Option<Vec<regex::Regex>> {
    use regex::Regex;
    filter.map(|f| {
        f.split(',')
            .filter_map(|pattern| match Regex::new(pattern.trim()) {
                Ok(regex) => Some(regex),
                Err(e) => {
                    if !config.json_output {
                        eprintln!("Warning: Invalid regex pattern '{}': {}", pattern, e);
                    }
                    None
                }
            })
            .collect()
    })
}

fn family_matches(family: &str, filters: &Option<Vec<regex::Regex>>) -> bool {
    filters.as_ref().map_or(true, |f| f.iter().any(|r| r.is_match(family)))
}

fn print_models_json<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    families: &[String],
    filters: &Option<Vec<regex::Regex>>,
    filter: Option<&str>,
    total_models: usize,
) {
    use serde_json::json;
    use std::collections::BTreeMap;

    let mut providers = BTreeMap::new();
    let mut shown_families = 0;
    let mut shown_models = 0;

    for family in families {
        if family_matches(family, filters) {
            let mut models = factory.list_models_in_family(family);
            if !models.is_empty() {
                models.sort();
                shown_families += 1;
                shown_models += models.len();
                providers.insert(family.clone(), json!({
                    "models": models.into_iter().map(|m| m.to_string()).collect::<Vec<_>>(),
                    "count": shown_models
                }));
            }
        }
    }

    let mut result = serde_json::Map::new();
    result.insert("providers".into(), json!(providers));
    result.insert("summary".into(), json!({
        "shown_providers": shown_families,
        "shown_models": shown_models,
        "total_providers": families.len(),
        "total_models": total_models,
        "filtered": filter.is_some()
    }));
    if let Some(f) = filter {
        result.insert("filter".into(), json!(f));
    }

    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}

fn print_models_pretty<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    families: &[String],
    filters: &Option<Vec<regex::Regex>>,
    filter: Option<&str>,
    total_models: usize,
) {
    if filter.is_some() {
        print_or_exit!("Available models by provider (filtered):");
    } else {
        print_or_exit!("Available models by provider:");
    }
    print_or_exit!();

    let mut shown_families = 0;
    let mut shown_models = 0;

    for family in families {
        if family_matches(family, filters) {
            let mut models = factory.list_models_in_family(family);
            if !models.is_empty() {
                print_or_exit!("📦 {} ({} models)", family, models.len());
                shown_families += 1;
                models.sort();
                for model in models {
                    print_or_exit!("  └─ {}", model);
                    shown_models += 1;
                }
                print_or_exit!();
            }
        }
    }

    if filter.is_some() {
        print_or_exit!(
            "Summary: {} models across {} providers shown (filtered from {} models across {} providers)",
            shown_models, shown_families, total_models, families.len()
        );
    } else {
        print_or_exit!("Summary: {} models across {} providers", total_models, families.len());
    }
}

fn list_services<T: ConfigProvider>(factory: &ClientFactory<T>) {
    let config = OutputConfig::get();
    let services = factory.list_available_services();

    if config.json_output {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "services": services.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "count": services.len()
        })).unwrap());
    } else {
        print_or_exit!("Available services:");
        for service in &services {
            print_or_exit!("  {}", service);
        }
        print_or_exit!();
        print_or_exit!("Total: {} services", services.len());
    }
}

// ============================================================================
// Ask Command
// ============================================================================

async fn ask_model<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = OutputConfig::get();
    let client = create_client_or_report(factory, model_id, config)?;

    if !config.json_output && !config.clean_output {
        println!("🤖 Model: {}", model_id);
        println!("💭 Prompt: {}", prompt);
        println!("📝 Response:\n");
    }

    let request = CompletionRequest::text("user", prompt);
    match client.complete(&request).await {
        Ok(response) => print_ask_success(model_id, prompt, &response, config),
        Err(e) => {
            print_ask_error(model_id, prompt, &e, config);
            return Err(Box::new(e));
        }
    }
    Ok(())
}

fn create_client_or_report<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
    config: &OutputConfig,
) -> Result<HttpClient, ClientError> {
    factory.create_client(model_id).map_err(|e| {
        if config.clean_output {
            eprintln!("Failed to create client: {}", e);
        } else if config.json_output {
            println!("{}", serde_json::json!({
                "error": e.to_string(),
                "model": model_id,
                "type": "client_creation_failed"
            }));
        } else {
            eprintln!("Failed to create client: {}", e);
        }
        e
    })
}

fn print_ask_success(
    model_id: &str,
    prompt: &str,
    response: &cllient::CompletionResponse,
    config: &OutputConfig,
) {
    use serde_json::json;

    if config.clean_output {
        print!("{}", response.content);
    } else if config.json_output {
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
        if config.verbose {
            println!("🔍 Debug: Response received successfully");
            if let Some(usage) = &response.usage {
                println!("🔍 Debug: Token usage - Input: {}, Output: {}",
                    usage.input_tokens, usage.output_tokens);
            }
        }
        println!("{}", response.content);
    }
}

fn print_ask_error(
    model_id: &str,
    prompt: &str,
    error: &ClientError,
    config: &OutputConfig,
) {
    use serde_json::json;
    let error_str = error.to_string();
    let (error_type, hint) = classify_error(&error_str);

    if config.clean_output {
        eprintln!("Error: {}", error);
    } else if config.json_output {
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
        eprintln!("❌ Error: {}", error);
        if config.verbose {
            eprintln!("🔍 Debug: Full error details: {:?}", error);
        }
        print_error_hints(&error_str, model_id, prompt);
    }
}

fn print_error_hints(error_str: &str, model_id: &str, prompt: &str) {
    if error_str.contains("JsonPath") {
        eprintln!("💡 Hint: JSON path parsing error. The API response structure doesn't match.");
        eprintln!("💡 Try: cllient -v ask {} \"{}\"", model_id, prompt);
        eprintln!("💡 Or try streaming: cllient stream {} \"{}\"", model_id, prompt);
    } else if error_str.contains("401") {
        eprintln!("💡 Hint: Check your API key for this provider");
    } else if error_str.contains("404") {
        eprintln!("💡 Hint: Model '{}' might not exist or be available", model_id);
    } else if error_str.contains("429") {
        eprintln!("💡 Hint: Rate limit exceeded. Try again in a moment");
    }
}

// ============================================================================
// Stream Command
// ============================================================================

async fn stream_model<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = OutputConfig::get();
    let client = factory.create_client(model_id)?;
    let request = CompletionRequest::text("user", prompt).with_streaming(true);

    if config.clean_output {
        stream_clean(&client, &request).await
    } else if config.json_output {
        stream_json(&client, &request, model_id, prompt).await
    } else {
        stream_pretty(&client, &request, model_id, prompt).await
    }
}

async fn stream_clean(
    client: &HttpClient,
    request: &CompletionRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = client.complete_stream(request).await?;
    while let Some(event) = FuturesStreamExt::next(&mut stream).await {
        match event {
            Ok(StreamEvent::Content(chunk)) => {
                print!("{}", chunk);
                io::stdout().flush()?;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }
    println!();
    Ok(())
}

async fn stream_json(
    client: &HttpClient,
    request: &CompletionRequest,
    model_id: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_obj = StreamingJsonObject::new()?;
    json_obj.field_string("model", model_id)?;
    json_obj.field_string("prompt", prompt)?;

    let mut stream = client.complete_stream(request).await?;
    let mut response_writer = json_obj.field_streaming_string("response")?;
    let mut error: Option<String> = None;

    while let Some(event) = FuturesStreamExt::next(&mut stream).await {
        match event {
            Ok(StreamEvent::Content(chunk)) => response_writer.write_chunk(&chunk)?,
            Ok(_) => {}
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
    }

    response_writer.close()?;
    json_obj.field_bool("streamed", true)?;

    if let Some(err) = error {
        json_obj.field_bool("success", false)?;
        json_obj.field_string("error", &err)?;
    } else {
        json_obj.field_bool("success", true)?;
    }

    json_obj.close()?;
    Ok(())
}

async fn stream_pretty(
    client: &HttpClient,
    request: &CompletionRequest,
    model_id: &str,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Model: {}", model_id);
    println!("💭 Prompt: {}", prompt);
    println!("📡 Streaming response:\n");

    let mut stream = client.complete_stream(request).await?;
    while let Some(event) = FuturesStreamExt::next(&mut stream).await {
        match event {
            Ok(StreamEvent::Content(chunk)) => {
                print!("{}", chunk);
                io::stdout().flush()?;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("\nError: {}", e);
                break;
            }
        }
    }
    println!("\n");
    Ok(())
}

// ============================================================================
// Chat Command
// ============================================================================

async fn interactive_chat<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = factory.create_client(model_id)?;

    println!("🤖 Starting interactive chat with {}", model_id);
    println!("💡 Type 'quit' to exit, 'clear' to clear history\n");

    let mut history = Vec::new();

    loop {
        print!("You: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "" => continue,
            "quit" => break,
            "clear" => {
                history.clear();
                println!("🗑️ History cleared");
                continue;
            }
            _ => {}
        }

        history.push(serde_json::json!({"role": "user", "content": input}));
        print!("Assistant: ");
        io::stdout().flush()?;

        let response = stream_chat_response(&client, input).await;
        history.push(serde_json::json!({"role": "assistant", "content": response}));
    }

    println!("👋 Goodbye!");
    Ok(())
}

async fn stream_chat_response(
    client: &HttpClient,
    input: &str,
) -> String {
    let request = CompletionRequest::text("user", input).with_streaming(true);
    let mut response_content = String::new();

    match client.complete_stream(&request).await {
        Ok(mut stream) => {
            while let Some(event) = FuturesStreamExt::next(&mut stream).await {
                match event {
                    Ok(StreamEvent::Content(chunk)) => {
                        print!("{}", chunk);
                        response_content.push_str(&chunk);
                        let _ = io::stdout().flush();
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("\nError: {}", e);
                        break;
                    }
                }
            }
            println!("\n");
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    response_content
}

// ============================================================================
// Compare Command
// ============================================================================

async fn compare_models<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_ids: &[&str],
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = OutputConfig::get();
    println!("🔍 Comparing models on prompt: {}\n", prompt);

    for model_id in model_ids {
        println!("🤖 {} ", model_id);
        println!("{}", "─".repeat(50));
        compare_single_model(factory, model_id, prompt, config).await;
        println!();
    }

    Ok(())
}

async fn compare_single_model<T: ConfigProvider>(
    factory: &ClientFactory<T>,
    model_id: &str,
    prompt: &str,
    config: &OutputConfig,
) {
    if config.verbose {
        println!("🔍 Debug: Creating client for model: {}", model_id);
    }

    let client = match factory.create_client(model_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Failed to create client: {}", e);
            return;
        }
    };

    if config.verbose {
        println!("🔍 Debug: Client created, sending request...");
    }

    let request = CompletionRequest::text("user", prompt);
    match client.complete(&request).await {
        Ok(response) => {
            if config.verbose {
                println!("🔍 Debug: Response received");
                if let Some(u) = &response.usage {
                    println!("🔍 Debug: Tokens - In: {}, Out: {}", u.input_tokens, u.output_tokens);
                }
            }
            println!("{}", response.content);
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            print_error_hints(&e.to_string(), model_id, prompt);
        }
    }
}

// ============================================================================
// Debug Command
// ============================================================================

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
        Ok(response) => print_debug_success(&response),
        Err(e) => print_debug_error(&e, model_id, prompt),
    }

    Ok(())
}

fn print_debug_success(response: &cllient::CompletionResponse) {
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
}

fn print_debug_error(error: &ClientError, model_id: &str, prompt: &str) {
    println!("❌ Request failed with error: {}", error);
    println!("🔍 Error type: {:?}", error);

    let error_str = error.to_string();

    if error_str.contains("JsonPath") {
        println!("\n🔧 JSON Path Error Troubleshooting:");
        println!("   API response structure doesn't match expectations.");
        println!("\n💡 Try:");
        println!("   1. cllient stream {} \"{}\"", model_id, prompt);
        println!("   2. cllient list | grep {}", model_id);
        println!("   3. RUST_LOG=debug cllient ask {} \"{}\"", model_id, prompt);
    } else if error_str.contains("401") {
        println!("\n🔧 Authentication Error (401):");
        println!("   Your API key is missing or invalid.");
    } else if error_str.contains("404") {
        println!("\n🔧 Not Found Error (404):");
        println!("   The model '{}' was not found.", model_id);
        println!("   Check available models: cllient list");
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
