//! Standalone hub server for cllient
//!
//! Exposes ModelRegistry as a substrate Activation over JSON-RPC.
//!
//! # Usage
//!
//! ```bash
//! # Start with default embedded configs
//! cllient-hub --bind 127.0.0.1:8080
//!
//! # Start with custom config directory
//! cllient-hub --bind 127.0.0.1:8080 --config ./my-configs
//! ```

use clap::Parser;
use cllient::ModelRegistry;

#[derive(Parser)]
#[command(name = "cllient-hub")]
#[command(about = "Serve cllient ModelRegistry as a substrate hub")]
#[command(version)]
struct Args {
    /// Address to bind the server to
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    bind: String,

    /// Config directory (uses embedded configs if not specified)
    #[arg(short, long)]
    config: Option<String>,

    /// Only load verified models
    #[arg(long)]
    verified_only: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load registry
    let registry = if args.config.is_some() {
        eprintln!("Warning: --config flag not yet supported, using embedded configs");
        ModelRegistry::new()?
    } else {
        tracing::info!("Using embedded configs");
        ModelRegistry::new()?
    };

    // Print startup info
    let models = registry.list_models();
    let services = registry.list_services();
    let verified = registry.list_verified_models();

    println!("cllient-hub starting");
    println!("  Bind address: {}", args.bind);
    println!("  Models: {} total, {} verified", models.len(), verified.len());
    println!("  Services: {}", services.len());
    println!();

    // List services with model counts
    for service in services {
        let count = registry.models_for_service(service).len();
        println!("  {} ({} models)", service, count);
    }
    println!();

    // Start serving
    tracing::info!(bind = %args.bind, "Starting hub server");
    registry.serve(&args.bind).await
}
