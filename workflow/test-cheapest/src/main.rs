use std::process::Command;
use clap::Parser;
use colored::*;

#[derive(Parser)]
#[command(name = "test-cheapest-models")]
#[command(about = "Test the cheapest models from Anthropic, OpenAI, and DeepSeek")]
struct Args {
    /// The prompt to send to all models
    #[arg(default_value = "Hello! Please introduce yourself and tell me your strengths.")]
    prompt: String,
    
    /// Only test specific provider (anthropic, openai, or deepseek)
    #[arg(short, long)]
    provider: Option<String>,
    
    /// Show full output including compilation messages
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Define working models for each provider (using original config IDs that have defaults)
    let models = vec![
        ("Anthropic Claude 3.5 Sonnet", "claude-3-5-sonnet-20241022", "$0.003/1K input", "anthropic"),
        ("OpenAI GPT-4o", "gpt-4o", "$0.0025/1K input", "openai"), 
        ("DeepSeek Chat", "deepseek-chat", "$0.00014/1K input", "deepseek"),
    ];
    
    println!("{} {}", "🚀".yellow(), format!("Testing cheapest models with prompt: \"{}\"", args.prompt).bold());
    println!("{}", "=".repeat(80).blue());
    
    for (display_name, model_id, pricing, provider) in &models {
        // Skip if filtering by provider
        if let Some(ref provider_filter) = args.provider {
            if provider != &provider_filter.as_str() {
                continue;
            }
        }
        
        println!();
        println!("{} {} {}", "🤖".cyan(), display_name.bold(), format!("({})", pricing).green());
        println!("{} {}", "📋".yellow(), format!("Model ID: {}", model_id).italic());
        println!("{}", "─".repeat(60).blue());
        
        // Call the cllient binary from the project root
        let output = Command::new("cargo")
            .args(&["run", "--bin", "cllient", "--", "stream", model_id, &args.prompt])
            .current_dir("../../") // Go up to project root
            .output();
            
        match output {
            Ok(result) => {
                if result.status.success() {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    
                    if args.verbose {
                        println!("{}", stdout);
                        if !stderr.is_empty() {
                            println!("{}", stderr);
                        }
                    } else {
                        // Print the output, filtering out cargo compilation messages
                        let lines: Vec<&str> = stdout.lines().collect();
                        let mut in_response = false;
                        
                        for line in lines {
                            if line.starts_with("📡 Streaming response:") {
                                in_response = true;
                                println!("{}", line.bold());
                            } else if in_response && !line.starts_with("    Finished") && !line.starts_with("     Running") {
                                println!("{}", line);
                            } else if line.starts_with("🤖") || line.starts_with("💭") {
                                println!("{}", line.bold());
                            }
                        }
                        
                        // Print any important stderr content (excluding compilation info)
                        if !stderr.is_empty() && !stderr.contains("Finished") && !stderr.contains("Compiling") {
                            println!("{} {}", "⚠️".yellow(), stderr.trim().red());
                        }
                    }
                    println!("{}", "✅ Success".green());
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    println!("{} {}", "❌".red(), format!("Failed to call model: {}", stderr.trim()).red());
                }
            }
            Err(e) => {
                println!("{} {}", "❌".red(), format!("Failed to execute command: {}", e).red());
            }
        }
        
        println!("\n{}", "=".repeat(80).blue());
    }
    
    println!("\n{}", "✅ Testing complete!".green().bold());
    
    Ok(())
}