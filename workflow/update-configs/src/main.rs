use anyhow::{anyhow, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "update-configs")]
#[command(about = "Update model config files with pricing data from OpenRouter")]
struct Args {
    /// JSON file containing OpenRouter model data
    #[arg(short, long)]
    input: PathBuf,
    
    /// Config directory to update
    #[arg(short, long, default_value = "../../config/family")]
    config_dir: PathBuf,
    
    /// Dry run - show what would be updated without making changes
    #[arg(short, long)]
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    name: String,
    canonical_slug: String,
    context_length: Option<u64>,
    input_cost: f64,
    output_cost: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelConfig {
    model: HashMap<String, Value>,
    capabilities: Option<HashMap<String, Value>>,
    pricing: Option<HashMap<String, Value>>,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Read and parse the OpenRouter JSON data
    let json_data = fs::read_to_string(&args.input)?;
    let openrouter_models: Vec<OpenRouterModel> = serde_json::from_str(&json_data)?;
    
    println!("Loaded {} models from OpenRouter data", openrouter_models.len());
    
    // Create a lookup map by multiple possible identifiers
    let mut model_lookup: HashMap<String, &OpenRouterModel> = HashMap::new();
    for model in &openrouter_models {
        // Original ID: openai/gpt-4o
        model_lookup.insert(model.id.clone(), model);
        
        // Canonical slug: openai-gpt-4o
        model_lookup.insert(model.canonical_slug.clone(), model);
        
        // Strip provider prefix: gpt-4o (for local configs)
        if let Some(model_name) = model.id.split('/').nth(1) {
            model_lookup.insert(model_name.to_string(), model);
        }
        
        // Also try with dashes replacing colons for variants
        let dash_variant = model.id.replace(':', "-");
        model_lookup.insert(dash_variant.clone(), model);
        if let Some(model_name) = dash_variant.split('/').nth(1) {
            model_lookup.insert(model_name.to_string(), model);
        }
    }
    
    // Find all config files
    let config_files = find_config_files(&args.config_dir)?;
    println!("Found {} config files", config_files.len());
    
    let mut updated_count = 0;
    let mut errors = Vec::new();
    
    // Process each config file
    for config_path in config_files {
        match update_config_file(&config_path, &model_lookup, args.dry_run) {
            Ok(updated) => {
                if updated {
                    updated_count += 1;
                    println!("✓ Updated: {}", config_path.display());
                }
            }
            Err(e) => {
                let error_msg = format!("✗ Error updating {}: {}", config_path.display(), e);
                eprintln!("{}", error_msg);
                errors.push(error_msg);
            }
        }
    }
    
    println!("\nSummary:");
    println!("  Updated: {} files", updated_count);
    if !errors.is_empty() {
        println!("  Errors: {} files", errors.len());
        for error in &errors {
            println!("    {}", error);
        }
    }
    
    if args.dry_run {
        println!("  (Dry run - no files were actually modified)");
    }
    
    Ok(())
}

fn find_config_files(config_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut config_files = Vec::new();
    
    for entry in WalkDir::new(config_dir) {
        let entry = entry?;
        if entry.file_type().is_file() && 
           entry.path().extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
            config_files.push(entry.path().to_path_buf());
        }
    }
    
    Ok(config_files)
}

fn update_config_file(
    config_path: &Path, 
    model_lookup: &HashMap<String, &OpenRouterModel>,
    dry_run: bool
) -> Result<bool> {
    // Read the existing config
    let config_content = fs::read_to_string(config_path)?;
    let mut config: ModelConfig = serde_yaml::from_str(&config_content)?;
    
    // Get the model ID from the config
    let model_id = config.model.get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("No model.id found in config"))?;
    
    // Try to find the model in our lookup
    // First try the direct ID, then try converting to canonical slug format
    let canonical_slug = model_id.replace("/", "-");
    let openrouter_model = model_lookup.get(model_id)
        .or_else(|| model_lookup.get(&canonical_slug))
        .ok_or_else(|| anyhow!("Model {} not found in OpenRouter data", model_id))?;
    
    let mut updated = false;
    
    // Update pricing information
    if let Some(pricing) = config.pricing.as_mut() {
        // Convert per-1M-token costs to per-1K-token costs
        let input_per_1k = openrouter_model.input_cost / 1000.0;
        let output_per_1k = openrouter_model.output_cost / 1000.0;
        
        let current_input = pricing.get("input_per_1k_tokens")
            .and_then(|v| v.as_f64());
        let current_output = pricing.get("output_per_1k_tokens")
            .and_then(|v| v.as_f64());
        
        if current_input != Some(input_per_1k) {
            pricing.insert("input_per_1k_tokens".to_string(), Value::from(input_per_1k));
            updated = true;
            if !dry_run {
                println!("  Updated input pricing: {:.6} -> {:.6}", 
                    current_input.unwrap_or(0.0), input_per_1k);
            }
        }
        
        if current_output != Some(output_per_1k) {
            pricing.insert("output_per_1k_tokens".to_string(), Value::from(output_per_1k));
            updated = true;
            if !dry_run {
                println!("  Updated output pricing: {:.6} -> {:.6}", 
                    current_output.unwrap_or(0.0), output_per_1k);
            }
        }
    }
    
    // Update context window if available
    if let Some(context_length) = openrouter_model.context_length {
        if let Some(capabilities) = config.capabilities.as_mut() {
            let current_context = capabilities.get("context_window")
                .and_then(|v| v.as_u64());
            
            if current_context != Some(context_length) {
                capabilities.insert("context_window".to_string(), Value::from(context_length));
                updated = true;
                if !dry_run {
                    println!("  Updated context window: {} -> {}", 
                        current_context.unwrap_or(0), context_length);
                }
            }
        }
    }
    
    // Write back the updated config if changes were made
    if updated && !dry_run {
        let updated_content = serde_yaml::to_string(&config)?;
        fs::write(config_path, updated_content)?;
    }
    
    if updated && dry_run {
        println!("  Would update: {}", config_path.display());
    }
    
    Ok(updated)
}