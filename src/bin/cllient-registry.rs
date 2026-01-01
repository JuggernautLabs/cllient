//! CLI tool for validating and inspecting the cllient registry.
//!
//! Provides commands for:
//! - Validating configs (schema, cross-references, semantic)
//! - Inspecting the bidirectional index
//! - Finding orphan services and broken references
//! - Searching for models

use clap::{Parser, Subcommand, ValueEnum};
use cllient::{
    ModelRegistry, ValidationLevel, ValidationReport, VerificationStatus,
};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "cllient-registry")]
#[command(about = "Validate and inspect the cllient model registry")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(long, short, global = true, default_value = "table")]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate registry configurations
    Validate {
        /// Validation levels to run
        #[arg(long, short, value_delimiter = ',', default_value = "all")]
        level: Vec<LevelArg>,

        /// Validate a specific model only
        #[arg(long)]
        model: Option<String>,

        /// Validate a specific service only
        #[arg(long)]
        service: Option<String>,
    },

    /// Show registry index statistics
    Stats,

    /// Show the full index (service→model mappings)
    Index,

    /// List orphan services (services with no models)
    Orphans,

    /// List broken references (models referencing non-existent services)
    Broken,

    /// List all models for a service
    ModelsFor {
        /// Service name
        service: String,
    },

    /// Get the service for a model
    ServiceFor {
        /// Model ID
        model: String,
    },

    /// Search for models
    Search {
        /// Search query
        query: String,

        /// Use fuzzy matching
        #[arg(long, short = 'z')]
        fuzzy: bool,

        /// Filter by verified status only
        #[arg(long)]
        verified: bool,

        /// Filter by service
        #[arg(long, short)]
        service: Option<String>,

        /// Filter by family
        #[arg(long, short = 'F')]
        family: Option<String>,

        /// Limit number of results
        #[arg(long, short, default_value = "20")]
        limit: usize,
    },

    /// List all services with model counts
    Services,

    /// List all families with model counts
    Families,

    /// Verify services/models by testing API connectivity
    Verify {
        /// Verify a specific service
        #[arg(long, short)]
        service: Option<String>,

        /// Verify a specific model
        #[arg(long, short)]
        model: Option<String>,

        /// Verify all unverified services
        #[arg(long)]
        all_services: bool,

        /// Verify all unverified models for verified services
        #[arg(long)]
        all_models: bool,

        /// Maximum number of models to test per service
        #[arg(long, default_value = "3")]
        max_per_service: usize,

        /// Dry run - show what would be tested without testing
        #[arg(long)]
        dry_run: bool,

        /// Update config files with results
        #[arg(long)]
        update_configs: bool,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Yaml,
}

#[derive(Clone, PartialEq, Eq)]
enum LevelArg {
    Schema,
    CrossRef,
    Semantic,
    Live,
    All,
}

impl std::str::FromStr for LevelArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "schema" => Ok(LevelArg::Schema),
            "crossref" | "cross-ref" | "cross_ref" => Ok(LevelArg::CrossRef),
            "semantic" => Ok(LevelArg::Semantic),
            "live" => Ok(LevelArg::Live),
            "all" => Ok(LevelArg::All),
            _ => Err(format!("Unknown validation level: {}", s)),
        }
    }
}

fn main() -> ExitCode {
    // Load .env file for API keys
    dotenv::dotenv().ok();

    let cli = Cli::parse();

    // Use permissive loading so we can inspect broken configs
    let registry = match ModelRegistry::new_permissive() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to load registry: {}", e);
            return ExitCode::from(2);
        }
    };

    match cli.command {
        Commands::Validate { level, model, service } => {
            cmd_validate(&registry, &level, model.as_deref(), service.as_deref(), cli.format)
        }
        Commands::Stats => cmd_stats(&registry, cli.format),
        Commands::Index => cmd_index(&registry, cli.format),
        Commands::Orphans => cmd_orphans(&registry, cli.format),
        Commands::Broken => cmd_broken(&registry, cli.format),
        Commands::ModelsFor { service } => cmd_models_for(&registry, &service, cli.format),
        Commands::ServiceFor { model } => cmd_service_for(&registry, &model, cli.format),
        Commands::Search { query, fuzzy, verified, service, family, limit } => {
            cmd_search(&registry, &query, fuzzy, verified, service.as_deref(), family.as_deref(), limit, cli.format)
        }
        Commands::Services => cmd_services(&registry, cli.format),
        Commands::Families => cmd_families(&registry, cli.format),
        Commands::Verify { service, model, all_services, all_models, max_per_service, dry_run, update_configs } => {
            cmd_verify(&registry, service.as_deref(), model.as_deref(), all_services, all_models, max_per_service, dry_run, update_configs, cli.format)
        }
    }
}

fn levels_from_args(args: &[LevelArg]) -> Vec<ValidationLevel> {
    let mut levels = Vec::new();
    for arg in args {
        match arg {
            LevelArg::Schema => levels.push(ValidationLevel::Schema),
            LevelArg::CrossRef => levels.push(ValidationLevel::CrossRef),
            LevelArg::Semantic => levels.push(ValidationLevel::Semantic),
            LevelArg::Live => levels.push(ValidationLevel::Live),
            LevelArg::All => {
                return vec![
                    ValidationLevel::Schema,
                    ValidationLevel::CrossRef,
                    ValidationLevel::Semantic,
                ];
            }
        }
    }
    if levels.is_empty() {
        vec![
            ValidationLevel::Schema,
            ValidationLevel::CrossRef,
            ValidationLevel::Semantic,
        ]
    } else {
        levels
    }
}

fn cmd_validate(
    registry: &ModelRegistry,
    level_args: &[LevelArg],
    model: Option<&str>,
    service: Option<&str>,
    format: OutputFormat,
) -> ExitCode {
    let levels = levels_from_args(level_args);

    let report: ValidationReport = if let Some(model_id) = model {
        registry.validate_model(model_id, &levels)
    } else if let Some(service_name) = service {
        registry.validate_service(service_name, &levels)
    } else {
        registry.validate(&levels)
    };

    match format {
        OutputFormat::Table => {
            println!("{}", report);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&report).unwrap());
        }
    }

    if report.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_stats(registry: &ModelRegistry, format: OutputFormat) -> ExitCode {
    let stats = registry.index_stats();

    match format {
        OutputFormat::Table => {
            println!("Registry Statistics");
            println!("===================");
            println!("Services:          {}", stats.total_services);
            println!("  Verified:        {}", stats.verified_services);
            println!("  Unverified:      {}", stats.unverified_services);
            println!("Models:            {}", stats.total_models);
            println!("  Verified:        {}", stats.verified_models);
            println!("  Unverified:      {}", stats.unverified_models);
            println!("Families:          {}", stats.total_families);
            println!("Orphan Services:   {}", stats.orphan_services);
            println!("Broken References: {}", stats.broken_refs);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&stats).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&stats).unwrap());
        }
    }

    ExitCode::SUCCESS
}

fn cmd_index(registry: &ModelRegistry, format: OutputFormat) -> ExitCode {
    let index = registry.index();

    match format {
        OutputFormat::Table => {
            println!("Service → Models Index");
            println!("======================");
            for service in index.all_services() {
                let models = index.models_for_service(service);
                println!("\n{} ({} models):", service, models.len());
                for model in models {
                    println!("  - {}", model);
                }
            }
        }
        OutputFormat::Json => {
            let mut map = std::collections::HashMap::new();
            for service in index.all_services() {
                map.insert(service.clone(), index.models_for_service(service).to_vec());
            }
            println!("{}", serde_json::to_string_pretty(&map).unwrap());
        }
        OutputFormat::Yaml => {
            let mut map = std::collections::HashMap::new();
            for service in index.all_services() {
                map.insert(service.clone(), index.models_for_service(service).to_vec());
            }
            println!("{}", serde_yaml::to_string(&map).unwrap());
        }
    }

    ExitCode::SUCCESS
}

fn cmd_orphans(registry: &ModelRegistry, format: OutputFormat) -> ExitCode {
    let orphans = registry.orphan_services();

    match format {
        OutputFormat::Table => {
            if orphans.is_empty() {
                println!("No orphan services found.");
            } else {
                println!("Orphan Services ({}):", orphans.len());
                for service in orphans {
                    println!("  - {}", service);
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&orphans).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&orphans).unwrap());
        }
    }

    ExitCode::SUCCESS
}

fn cmd_broken(registry: &ModelRegistry, format: OutputFormat) -> ExitCode {
    let broken = registry.broken_refs();

    match format {
        OutputFormat::Table => {
            if broken.is_empty() {
                println!("No broken references found.");
            } else {
                println!("Broken References ({}):", broken.len());
                for br in broken {
                    println!("  - Model '{}' → missing service '{}'", br.model_id, br.referenced_service);
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&broken).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&broken).unwrap());
        }
    }

    if broken.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_models_for(registry: &ModelRegistry, service: &str, format: OutputFormat) -> ExitCode {
    let models = registry.models_for_service(service);

    match format {
        OutputFormat::Table => {
            if models.is_empty() {
                println!("No models found for service '{}'", service);
            } else {
                println!("Models for service '{}' ({}):", service, models.len());
                for model in models {
                    println!("  - {}", model);
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&models).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&models).unwrap());
        }
    }

    ExitCode::SUCCESS
}

fn cmd_service_for(registry: &ModelRegistry, model: &str, format: OutputFormat) -> ExitCode {
    let service = registry.service_for_model(model);

    match format {
        OutputFormat::Table => {
            if let Some(s) = service {
                println!("Model '{}' uses service '{}'", model, s);
            } else {
                println!("Model '{}' not found", model);
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&service).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&service).unwrap());
        }
    }

    if service.is_some() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_search(
    registry: &ModelRegistry,
    query: &str,
    fuzzy: bool,
    verified: bool,
    service: Option<&str>,
    family: Option<&str>,
    limit: usize,
    format: OutputFormat,
) -> ExitCode {
    let mut q = registry.query();

    if fuzzy {
        q = q.fuzzy(query);
    } else {
        q = q.model_id_like(&format!("*{}*", query));
    }

    if verified {
        q = q.verified();
    }

    if let Some(s) = service {
        q = q.service(s);
    }

    if let Some(f) = family {
        q = q.family(f);
    }

    q = q.limit(limit);

    let models: Vec<&str> = q.list();

    match format {
        OutputFormat::Table => {
            if models.is_empty() {
                println!("No models found matching '{}'", query);
            } else {
                println!("Models matching '{}' ({} results):", query, models.len());
                for model in &models {
                    // Get additional info
                    if let Ok(config) = registry.get_model_info(model) {
                        let status = match config.model.status {
                            VerificationStatus::Verified => "✓",
                            VerificationStatus::Unverified => "?",
                            VerificationStatus::Broken => "✗",
                            VerificationStatus::Deprecated => "⚠",
                            VerificationStatus::Error => "!",
                            VerificationStatus::NotFound => "∅",
                        };
                        println!("  {} {} (service: {}, family: {})",
                            status, model, config.model.service, config.model.family);
                    } else {
                        println!("  - {}", model);
                    }
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&models).unwrap());
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&models).unwrap());
        }
    }

    ExitCode::SUCCESS
}

fn cmd_services(registry: &ModelRegistry, format: OutputFormat) -> ExitCode {
    use cllient::VerificationStatus as VS;
    let index = registry.index();
    let counts = index.model_counts_by_service();

    match format {
        OutputFormat::Table => {
            println!("Services ({}):", counts.len());
            let mut items: Vec<_> = counts.into_iter().collect();
            items.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending
            for (service, count) in items {
                let status = index.service_status(service).unwrap_or(&VS::Unverified);
                let status_char = match status {
                    VS::Verified => "✓",
                    VS::Unverified => "?",
                    VS::Broken => "✗",
                    VS::Deprecated => "⚠",
                    VS::Error => "!",
                    VS::NotFound => "∅",
                };
                println!("  {} {:28} {:>5} models", status_char, service, count);
            }
        }
        OutputFormat::Json => {
            // Build a richer structure for JSON output
            let mut services_info: Vec<serde_json::Value> = Vec::new();
            for (service, count) in &counts {
                let status = index.service_status(service)
                    .map(|s| format!("{:?}", s).to_lowercase())
                    .unwrap_or_else(|| "unverified".to_string());
                services_info.push(serde_json::json!({
                    "name": service,
                    "status": status,
                    "model_count": count
                }));
            }
            println!("{}", serde_json::to_string_pretty(&services_info).unwrap());
        }
        OutputFormat::Yaml => {
            let mut services_info: Vec<serde_json::Value> = Vec::new();
            for (service, count) in &counts {
                let status = index.service_status(service)
                    .map(|s| format!("{:?}", s).to_lowercase())
                    .unwrap_or_else(|| "unverified".to_string());
                services_info.push(serde_json::json!({
                    "name": service,
                    "status": status,
                    "model_count": count
                }));
            }
            println!("{}", serde_yaml::to_string(&services_info).unwrap());
        }
    }

    ExitCode::SUCCESS
}

fn cmd_families(registry: &ModelRegistry, format: OutputFormat) -> ExitCode {
    let families = registry.list_families();
    let mut family_counts: Vec<(String, usize)> = families
        .iter()
        .map(|f| (f.clone(), registry.list_models_in_family(f).len()))
        .collect();
    family_counts.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending

    match format {
        OutputFormat::Table => {
            println!("Families ({}):", family_counts.len());
            for (family, count) in &family_counts {
                println!("  {:30} {:>5} models", family, count);
            }
        }
        OutputFormat::Json => {
            let map: std::collections::HashMap<_, _> = family_counts.into_iter().collect();
            println!("{}", serde_json::to_string_pretty(&map).unwrap());
        }
        OutputFormat::Yaml => {
            let map: std::collections::HashMap<_, _> = family_counts.into_iter().collect();
            println!("{}", serde_yaml::to_string(&map).unwrap());
        }
    }

    ExitCode::SUCCESS
}

/// The verification prompt - a simple creative task
const VERIFY_PROMPT: &str = "10 word poem";

/// Result of a verification test
#[derive(Debug, Clone)]
struct VerifyResult {
    target: String,
    target_type: &'static str, // "service" or "model"
    success: bool,
    error: Option<String>,
    response: Option<String>, // The model's response (poem)
    response_time_ms: Option<u64>,
}

#[allow(clippy::too_many_arguments)]
fn cmd_verify(
    registry: &ModelRegistry,
    service: Option<&str>,
    model: Option<&str>,
    all_services: bool,
    all_models: bool,
    max_per_service: usize,
    dry_run: bool,
    update_configs: bool,
    format: OutputFormat,
) -> ExitCode {
    use cllient::VerificationStatus as VS;

    // Collect what to verify
    let mut services_to_verify: Vec<String> = Vec::new();
    let mut models_to_verify: Vec<String> = Vec::new();

    if let Some(svc) = service {
        services_to_verify.push(svc.to_string());
    }

    if let Some(mdl) = model {
        models_to_verify.push(mdl.to_string());
    }

    if all_services {
        // Get all unverified services that have models
        let index = registry.index();
        for svc_name in index.unverified_services() {
            if index.service_has_models(svc_name) {
                services_to_verify.push(svc_name.clone());
            }
        }
    }

    if all_models {
        // Get unverified models from verified services
        let index = registry.index();
        for svc_name in index.verified_services() {
            let models = index.models_for_service(svc_name);
            let mut count = 0;
            for model_id in models {
                if let Ok(cfg) = registry.get_model_info(model_id) {
                    if cfg.model.status == VS::Unverified && count < max_per_service {
                        models_to_verify.push(model_id.clone());
                        count += 1;
                    }
                }
            }
        }
    }

    // For each service, also pick models to verify
    let index = registry.index();
    for svc_name in &services_to_verify {
        let models = index.models_for_service(svc_name);
        // Pick up to max_per_service models, preferring cheapest
        let mut service_models: Vec<&String> = models.iter().collect();
        service_models.truncate(max_per_service);
        for model_id in service_models {
            if !models_to_verify.contains(model_id) {
                models_to_verify.push(model_id.clone());
            }
        }
    }

    if services_to_verify.is_empty() && models_to_verify.is_empty() {
        println!("Nothing to verify. Use --service, --model, --all-services, or --all-models.");
        return ExitCode::SUCCESS;
    }

    match format {
        OutputFormat::Table => {
            println!("Verification Plan");
            println!("=================");
            if !services_to_verify.is_empty() {
                println!("\nServices to verify ({}):", services_to_verify.len());
                for svc in &services_to_verify {
                    println!("  - {}", svc);
                }
            }
            if !models_to_verify.is_empty() {
                println!("\nModels to verify ({}):", models_to_verify.len());
                for mdl in &models_to_verify {
                    let svc = registry.service_for_model(mdl).unwrap_or("unknown");
                    println!("  - {} (service: {})", mdl, svc);
                }
            }
        }
        OutputFormat::Json => {
            let plan = serde_json::json!({
                "services": services_to_verify,
                "models": models_to_verify,
            });
            println!("{}", serde_json::to_string_pretty(&plan).unwrap());
        }
        OutputFormat::Yaml => {
            let plan = serde_json::json!({
                "services": services_to_verify,
                "models": models_to_verify,
            });
            println!("{}", serde_yaml::to_string(&plan).unwrap());
        }
    }

    if dry_run {
        println!("\n[Dry run - no tests executed]");
        return ExitCode::SUCCESS;
    }

    // Run verification tests
    println!("\nRunning verification tests...\n");

    let runtime = tokio::runtime::Runtime::new().expect("Failed to create async runtime");
    let results = runtime.block_on(run_verification_tests(registry, &models_to_verify));

    // Print results
    let mut all_passed = true;
    let mut passed_count = 0;
    let mut failed_count = 0;

    println!("Results:");
    println!("--------");
    for result in &results {
        let status = if result.success {
            passed_count += 1;
            "✓ PASS"
        } else {
            failed_count += 1;
            all_passed = false;
            "✗ FAIL"
        };

        let time_str = result.response_time_ms
            .map(|ms| format!(" ({}ms)", ms))
            .unwrap_or_default();

        println!("  {} {} [{}]{}", status, result.target, result.target_type, time_str);

        if let Some(poem) = &result.response {
            // Show the poem, truncating if too long
            let display = if poem.len() > 80 {
                format!("{}...", &poem[..77])
            } else {
                poem.clone()
            };
            println!("       \"{}\"", display);
        }

        if let Some(err) = &result.error {
            println!("       Error: {}", err);
        }
    }

    println!("\nSummary: {} passed, {} failed", passed_count, failed_count);

    // Update configs for passing models if requested
    if update_configs {
        let mut updated_count = 0;
        let mut update_errors = Vec::new();

        for result in &results {
            if result.success {
                if let Some(poem) = &result.response {
                    match update_model_config(&result.target, poem) {
                        Ok(path) => {
                            updated_count += 1;
                            println!("  Updated: {}", path.display());
                        }
                        Err(e) => {
                            update_errors.push(format!("{}: {}", result.target, e));
                        }
                    }
                }
            }
        }

        if updated_count > 0 {
            println!("\nUpdated {} config file(s) with verification status", updated_count);
        }

        if !update_errors.is_empty() {
            println!("\nConfig update errors:");
            for err in &update_errors {
                println!("  - {}", err);
            }
        }
    }

    if all_passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

async fn run_verification_tests(
    registry: &ModelRegistry,
    models: &[String],
) -> Vec<VerifyResult> {
    use std::time::Instant;

    let mut results = Vec::new();

    for model_id in models {
        let start = Instant::now();

        // Try to make a request with the verification prompt
        let result = match registry.from_id(model_id) {
            Ok(builder) => {
                match builder
                    .prompt(VERIFY_PROMPT)
                    .max_tokens(50)
                    .send()
                    .await
                {
                    Ok(response) => {
                        let elapsed = start.elapsed().as_millis() as u64;
                        // Check if we got content
                        if !response.content.is_empty() {
                            VerifyResult {
                                target: model_id.clone(),
                                target_type: "model",
                                success: true,
                                error: None,
                                response: Some(response.content.trim().to_string()),
                                response_time_ms: Some(elapsed),
                            }
                        } else {
                            VerifyResult {
                                target: model_id.clone(),
                                target_type: "model",
                                success: false,
                                error: Some("Empty response content".to_string()),
                                response: None,
                                response_time_ms: Some(elapsed),
                            }
                        }
                    }
                    Err(e) => {
                        let elapsed = start.elapsed().as_millis() as u64;
                        VerifyResult {
                            target: model_id.clone(),
                            target_type: "model",
                            success: false,
                            error: Some(format!("{}", e)),
                            response: None,
                            response_time_ms: Some(elapsed),
                        }
                    }
                }
            }
            Err(e) => VerifyResult {
                target: model_id.clone(),
                target_type: "model",
                success: false,
                error: Some(format!("Config error: {}", e)),
                response: None,
                response_time_ms: None,
            },
        };

        results.push(result);
    }

    results
}

/// Find the config file path for a model ID
fn find_model_config_path(model_id: &str) -> Option<PathBuf> {
    let config_dir = PathBuf::from("config/family");

    if !config_dir.exists() {
        return None;
    }

    for entry in WalkDir::new(&config_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "yaml" || ext == "yml")
                .unwrap_or(false)
        })
    {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            // Check if this file contains our model ID
            if content.contains(&format!("id: {}", model_id))
                || content.contains(&format!("id: \"{}\"", model_id))
            {
                return Some(entry.path().to_path_buf());
            }
        }
    }

    None
}

/// Update a model config file to mark it as verified with the poem response
fn update_model_config(model_id: &str, poem: &str) -> Result<PathBuf, String> {
    let path = find_model_config_path(model_id)
        .ok_or_else(|| format!("Could not find config file for model: {}", model_id))?;

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    // Update or add status field under model section
    let updated = if content.contains("status:") {
        // Replace existing status
        let re = regex::Regex::new(r"(?m)^(\s*)status:\s*\w+").unwrap();
        re.replace(&content, "${1}status: verified").to_string()
    } else {
        // Add status after the service line in model section
        let re = regex::Regex::new(r"(?m)^(\s*service:\s*.+)$").unwrap();
        re.replace(&content, format!("$1\n  status: verified")).to_string()
    };

    // Add or update verification section with the poem
    let poem_escaped = poem.replace("\"", "\\\"").replace("\n", " ");
    let verification_block = format!(
        "\nverification:\n  poem: \"{}\"\n  verified_at: \"{}\"\n",
        poem_escaped,
        chrono::Utc::now().format("%Y-%m-%d")
    );

    let final_content = if updated.contains("verification:") {
        // Replace existing verification section
        let re = regex::Regex::new(r"(?ms)^verification:.*?(?=^\w|\z)").unwrap();
        re.replace(&updated, &verification_block).to_string()
    } else {
        // Append verification section
        format!("{}{}", updated.trim_end(), verification_block)
    };

    fs::write(&path, final_content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

    Ok(path)
}
