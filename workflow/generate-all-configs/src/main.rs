use anyhow::{anyhow, Result};
use clap::Parser;
use handlebars::Handlebars;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "generate-all-configs")]
#[command(about = "Generate all model configs from OpenRouter data using mustache templates")]
struct Args {
    /// OpenRouter complete JSON file
    #[arg(short, long)]
    input: PathBuf,
    
    /// Output directory for generated configs
    #[arg(short, long, default_value = "../../config/family")]
    output_dir: PathBuf,
    
    /// Templates directory
    #[arg(short, long, default_value = "../../templates")]
    templates_dir: PathBuf,
    
    /// Dry run - show what would be generated without creating files
    #[arg(short, long)]
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    name: String,
    description: Option<String>,
    context_length: Option<u64>,
    architecture: Option<Architecture>,
    pricing: Pricing,
    top_provider: Option<TopProvider>,
}

#[derive(Debug, Deserialize)]
struct Architecture {
    modality: Option<String>,
    input_modalities: Option<Vec<String>>,
    output_modalities: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Pricing {
    prompt: f64,
    completion: f64,
    image: Option<String>,
    request: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TopProvider {
    max_completion_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ServiceMapping {
    service: String,
    family_mapping: HashMap<String, String>,
    has_defaults: bool,
    #[serde(default)]
    openai_options: bool,
    #[serde(default)]
    anthropic_options: bool,
    default_temperature: Option<f64>,
    default_max_tokens: Option<u64>,
    default_top_p: Option<f64>,
    default_frequency_penalty: Option<f64>,
    default_presence_penalty: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CapabilityRules {
    vision_capable_patterns: Vec<String>,
    function_capable_patterns: Vec<String>,
    json_mode_capable_patterns: Vec<String>,
    multimodal_indicators: Vec<String>,
    vision_input_modalities: Vec<String>,
    audio_input_modalities: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ModelContext {
    // Direct from OpenRouter
    id: String,
    name: String,
    context_length: u64,
    
    // Derived fields
    lab: String,
    family: String,
    service: String,
    canonical_slug: String,
    provider: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    version: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    variant: String,
    
    // Pricing (converted to per-1K)
    input_cost_per_1k: f64,
    output_cost_per_1k: f64,
    image_cost: Option<f64>,
    
    // Capabilities
    max_output_tokens: u64,
    multimodal: String,
    vision: String,
    functions: String,
    json_mode: String,
    
    // Service-specific options
    has_defaults: bool,
    openai_options: bool,
    anthropic_options: bool,
    default_temperature: Option<f64>,
    default_max_tokens: Option<u64>,
    default_top_p: Option<f64>,
    default_frequency_penalty: Option<f64>,
    default_presence_penalty: Option<f64>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Load OpenRouter data
    let json_data = fs::read_to_string(&args.input)?;
    let openrouter_models: Vec<OpenRouterModel> = serde_json::from_str(&json_data)?;
    
    // Load service mapping
    let service_mapping_path = args.templates_dir.join("service-mapping.json");
    let service_mapping_data = fs::read_to_string(&service_mapping_path)?;
    let service_mappings: HashMap<String, ServiceMapping> = serde_json::from_str(&service_mapping_data)?;
    
    // Load capability rules
    let capability_rules_path = args.templates_dir.join("capability-rules.json");
    let capability_rules_data = fs::read_to_string(&capability_rules_path)?;
    let capability_rules: CapabilityRules = serde_json::from_str(&capability_rules_data)?;
    
    // Setup handlebars
    let mut handlebars = Handlebars::new();
    let template_path = args.templates_dir.join("model-config-simple.yaml.mustache");
    let template_content = fs::read_to_string(&template_path)?;
    handlebars.register_template_string("model-config", template_content)?;
    
    println!("Loaded {} models from OpenRouter data", openrouter_models.len());
    
    let mut generated_count = 0;
    let mut errors = Vec::new();
    
    // Process each model
    for model in openrouter_models {
        match generate_model_config(&model, &service_mappings, &capability_rules, &handlebars, &args) {
            Ok(generated) => {
                if generated {
                    generated_count += 1;
                    if !args.dry_run {
                        println!("✓ Generated: {}", model.id);
                    } else {
                        println!("✓ Would generate: {}", model.id);
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("✗ Error generating {}: {}", model.id, e);
                eprintln!("{}", error_msg);
                errors.push(error_msg);
            }
        }
    }
    
    println!("\nSummary:");
    println!("  Generated: {} files", generated_count);
    if !errors.is_empty() {
        println!("  Errors: {} files", errors.len());
        for error in &errors {
            println!("    {}", error);
        }
    }
    
    if args.dry_run {
        println!("  (Dry run - no files were actually created)");
    }
    
    Ok(())
}

fn generate_model_config(
    model: &OpenRouterModel,
    service_mappings: &HashMap<String, ServiceMapping>,
    capability_rules: &CapabilityRules,
    handlebars: &Handlebars,
    args: &Args,
) -> Result<bool> {
    // Extract provider and lab from model ID, then remove lab from ID
    let provider = model.id.split('/').next().unwrap_or("unknown").to_string();
    let lab = provider.clone();
    let model_id_without_lab = model.id.strip_prefix(&format!("{}/", lab))
        .unwrap_or(&model.id)
        .to_string();
    
    // Get service mapping (with fallback to default)
    let service_config = service_mappings.get(&provider)
        .or_else(|| service_mappings.get("default"))
        .ok_or_else(|| anyhow!("No service mapping found for provider: {}", provider))?;
    
    // Create canonical slug
    let canonical_slug = model.id.replace('/', "-");
    
    // Derive family
    let family = derive_family(&model.id, service_config);
    
    // Extract version and variant
    let (version, variant) = extract_version_variant(&model.id);
    
    // Detect capabilities
    let has_vision = detect_capability(&model.id, &capability_rules.vision_capable_patterns) ||
        model.architecture.as_ref().map_or(false, |arch| {
            arch.input_modalities.as_ref().map_or(false, |modalities| {
                modalities.iter().any(|m| capability_rules.vision_input_modalities.contains(m))
            })
        });
    
    let has_multimodal = model.architecture.as_ref().map_or(false, |arch| {
        arch.modality.as_ref().map_or(false, |m| m != "text->text")
    }) || has_vision;
    
    let has_json_mode = detect_capability(&model.id, &capability_rules.json_mode_capable_patterns);
    let has_functions = detect_capability(&model.id, &capability_rules.function_capable_patterns);
    
    // Create template context
    let context = ModelContext {
        id: model_id_without_lab,
        name: model.name.clone(),
        context_length: model.context_length.unwrap_or(4096),
        
        lab,
        family,
        service: service_config.service.clone(),
        canonical_slug: canonical_slug.clone(),
        provider,
        version: version.unwrap_or_default(),
        variant: variant.unwrap_or_default(),
        
        input_cost_per_1k: model.pricing.prompt * 1000.0,
        output_cost_per_1k: model.pricing.completion * 1000.0,
        image_cost: model.pricing.image.as_ref().and_then(|s| s.parse::<f64>().ok()).filter(|&f| f > 0.0),
        
        max_output_tokens: model.top_provider.as_ref().and_then(|tp| tp.max_completion_tokens).unwrap_or(4096),
        multimodal: if has_multimodal { "true".to_string() } else { "false".to_string() },
        vision: if has_vision { "true".to_string() } else { "false".to_string() },
        functions: if has_functions { "true".to_string() } else { "false".to_string() },
        json_mode: if has_json_mode { "true".to_string() } else { "false".to_string() },
        
        has_defaults: service_config.has_defaults,
        openai_options: service_config.openai_options,
        anthropic_options: service_config.anthropic_options,
        default_temperature: service_config.default_temperature,
        default_max_tokens: service_config.default_max_tokens,
        default_top_p: service_config.default_top_p,
        default_frequency_penalty: service_config.default_frequency_penalty,
        default_presence_penalty: service_config.default_presence_penalty,
    };
    
    // Render template
    let rendered = handlebars.render("model-config", &context)?;
    
    if !args.dry_run {
        // Create output directory
        let output_dir = args.output_dir.join(&context.provider);
        fs::create_dir_all(&output_dir)?;
        
        // Write file
        let output_path = output_dir.join(format!("{}.yaml", canonical_slug));
        fs::write(output_path, rendered)?;
    }
    
    Ok(true)
}

fn derive_family(model_id: &str, service_config: &ServiceMapping) -> String {
    // Try to match against family mapping patterns
    for (pattern, family) in &service_config.family_mapping {
        if model_id.contains(pattern) {
            return family.clone();
        }
    }
    
    // Fallback: extract from model ID
    let model_part = model_id.split('/').nth(1).unwrap_or(model_id);
    let base_name = model_part.split('-').next().unwrap_or(model_part);
    base_name.to_lowercase()
}

fn extract_version_variant(model_id: &str) -> (Option<String>, Option<String>) {
    let model_part = model_id.split('/').nth(1).unwrap_or(model_id);
    
    // Extract version (e.g., gpt-4 -> version: "4")
    let version_regex = Regex::new(r"-([\d\.]+)(?:-|$)").unwrap();
    let version = version_regex.captures(model_part)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string());
    
    // Extract variant (e.g., gpt-4o -> variant: "omni", claude-3-opus -> variant: "opus")
    let variant = if model_part.contains("omni") || model_part.contains("-o") {
        Some("omni".to_string())
    } else if model_part.contains("opus") {
        Some("opus".to_string())
    } else if model_part.contains("sonnet") {
        Some("sonnet".to_string())
    } else if model_part.contains("haiku") {
        Some("haiku".to_string())
    } else if model_part.contains("mini") {
        Some("mini".to_string())
    } else if model_part.contains("nano") {
        Some("nano".to_string())
    } else if model_part.contains("turbo") {
        Some("turbo".to_string())
    } else {
        None
    };
    
    (version, variant)
}

fn detect_capability(model_id: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            model_id.starts_with(prefix)
        } else {
            model_id == pattern
        }
    })
}