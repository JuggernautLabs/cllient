# cllient

A config-driven LLM client in Rust. Define providers and models in YAML instead of code.

```
┌──────────────────────────────────────────────────────────────────┐
│  YAML Configs                                                    │
│  ┌─────────────────┐    ┌─────────────────┐                      │
│  │ service/        │    │ family/         │                      │
│  │  openai.yaml    │◄───│  gpt/4o.yaml    │  model references    │
│  │  anthropic.yaml │◄───│  claude/*.yaml  │  service by name     │
│  │  deepseek.yaml  │◄───│  deepseek/*.yaml│                      │
│  └─────────────────┘    └─────────────────┘                      │
│          │                      │                                │
│          ▼                      ▼                                │
│  ┌─────────────────────────────────────────┐                     │
│  │           ModelRegistry                 │                     │
│  │   - Loads configs (embedded or files)   │                     │
│  │   - Renders Handlebars templates        │                     │
│  │   - Substitutes env vars (API keys)     │                     │
│  └─────────────────────────────────────────┘                     │
│                        │                                         │
│          ┌─────────────┼─────────────┐                           │
│          ▼             ▼             ▼                           │
│   ┌────────────┐ ┌───────────┐ ┌──────────┐                      │
│   │  Library   │ │    CLI    │ │  Plugin  │                      │
│   │   Usage    │ │   Tool    │ │   Hub    │                      │
│   └────────────┘ └───────────┘ └──────────┘                      │
└──────────────────────────────────────────────────────────────────┘
```

## Features

- **Config-driven** - Add providers via YAML, no code changes
- **SSE streaming** - Real-time token streaming with provider-specific parsers
- **Streaming JSON output** - Emit valid JSON incrementally as tokens arrive
- **339 model configs** - 97 direct integrations + 242 via OpenRouter
- **Plugin system** - Expose as a substrate Activation for hub integration
- **Query API** - Fluent filter builder for model discovery
- **Validation** - Schema, cross-ref, and semantic config validation
- **Registry export** - Single-call access to full registry for RPC/integrations
- **CLI + library** - Use from command line or as a Rust crate
- **Embedded configs** - Ship as a single binary with all configs baked in

## Quick Start

### Installation

```bash
cargo install --path .

# Set up API keys
cp .env.example .env
# Edit .env with your API keys
```

### Basic Usage

```bash
# List available models
cllient list

# Simple completion
cllient ask gpt-4o-mini "What is Rust?"

# Streaming output
cllient stream deepseek-chat "Tell me a story"

# Interactive chat
cllient chat claude-3-haiku-20240307
```

### Library Usage

```rust
use cllient::{ModelRegistry, Message};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let registry = ModelRegistry::new()?;

    // Simple completion
    let response = registry
        .from_id("gpt-4o-mini")?
        .prompt("Hello!")
        .send()
        .await?;

    println!("{}", response.content);

    // Query API - find models by criteria
    let cheap_claude = registry.query()
        .family("claude")
        .verified()
        .cheapest();

    Ok(())
}
```

## Plugin System

cllient can expose itself as a **substrate Activation** for integration with hub-based systems.

### Feature Flags

```toml
[dependencies]
cllient = { version = "0.2", features = ["plugin"] }  # Activation only
cllient = { version = "0.2", features = ["hub"] }     # + standalone serve
```

| Feature | What You Get | Dependencies |
|---------|--------------|--------------|
| (none) | `ModelRegistry` library | minimal |
| `plugin` | + `CllientActivation` | hub-core, hub-macro, jsonrpsee |
| `hub` | + `serve()` method | plugin deps |

### Plugin Methods

The `CllientActivation` exposes these streaming RPC methods:

| Method | Parameters | Returns |
|--------|------------|---------|
| `complete` | model, prompt, system?, max_tokens?, temperature? | `Stream<CompletionEvent>` |
| `models` | - | `Stream<ModelEvent>` |
| `services` | - | `Stream<ServiceEvent>` |
| `verify` | targets? | `Stream<VerifyEvent>` |
| `query` | service?, family?, verified? | `Stream<QueryEvent>` |

### Usage Examples

```rust
use cllient::{ModelRegistry, CllientActivation};
use hub_core::Plexus;

// Option 1: Register with external Plexus
let registry = ModelRegistry::new()?;
let plugin = registry.into_plugin();
let plexus = Plexus::new()
    .register(plugin)
    .register(other_activation);

// Option 2: Create Plexus from registry
let plexus = ModelRegistry::new()?.into_plexus();

// Option 3: Standalone hub server (requires "hub" feature)
ModelRegistry::new()?
    .serve("127.0.0.1:8080")
    .await?;
```

### Hub Binary

With the `hub` feature, a standalone binary is available:

```bash
# Build the hub binary
cargo build --features hub --bin cllient-hub

# Run it
./target/debug/cllient-hub --bind 127.0.0.1:8080
```

## Query API

Fluent builder for filtering models:

```rust
let registry = ModelRegistry::new()?;

// Find all verified Claude models with vision
let models = registry.query()
    .family("claude")
    .verified()
    .with_vision()
    .list();

// Get cheapest model with 100k+ context
let model = registry.query()
    .verified()
    .context_min(100_000)
    .cheapest();

// Fuzzy search
let models = registry.query()
    .service("openai")
    .fuzzy("gpt turbo")
    .list();
```

## Validation

Registry validation at multiple levels:

```rust
use cllient::{ModelRegistry, ValidationLevel};

let registry = ModelRegistry::new()?;

// Full validation
let report = registry.validate(&[
    ValidationLevel::Schema,    // YAML structure
    ValidationLevel::CrossRef,  // Service references
    ValidationLevel::Semantic,  // Logic constraints
]);

// Quick check
if !registry.is_valid() {
    eprintln!("Registry has issues");
}
```

CLI validation tool:

```bash
# Validate all configs
cllient-registry validate

# Check specific model
cllient-registry validate --model gpt-4o

# List orphaned services
cllient-registry orphans

# Show registry stats
cllient-registry stats
```

## Streaming

```rust
use cllient::streaming::StreamEvent;
use futures::StreamExt;

let mut stream = registry
    .from_id("gpt-4o-mini")?
    .stream_text("Tell me a story")
    .await?;

while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::Content(text) => print!("{}", text),
        StreamEvent::Finish(reason) => println!("\nDone: {:?}", reason),
        StreamEvent::Usage { input_tokens, output_tokens, .. } => {
            println!("Tokens: {:?} in, {:?} out", input_tokens, output_tokens);
        }
        _ => {}
    }
}
```

## Configuration

### Environment Variables

```bash
# Core providers
OPENAI_API_KEY=your_key
ANTHROPIC_API_KEY=your_key
DEEPSEEK_API_KEY=your_key
GOOGLE_API_KEY=your_key

# OpenRouter (242 additional models)
OPEN_ROUTER_API_KEY=your_key
```

### Adding Providers

```yaml
# config/service/newprovider.yaml
service:
  name: NewProvider
  base_url: https://api.newprovider.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Authorization: Bearer ${NEWPROVIDER_API_KEY}

    {"model": "{{model_id}}", "messages": {{json messages}}}
```

## Architecture

**Config-driven design**: Services and models are separate entities defined in YAML.

```
Traditional:  gpt-4 → hardcoded to OpenAI API
cllient:      gpt-4 → { OpenAI, Azure, OpenRouter }
```

**Core components:**
- `ModelRegistry` - High-level API for model selection and requests
- `CllientActivation` - Plugin wrapper for hub integration
- Service configs - HTTP templates with Handlebars
- Model configs - Capabilities, pricing, constraints

## Project Structure

```
src/
├── lib.rs              # Public exports
├── runtime.rs          # ModelRegistry
├── plugin.rs           # CllientActivation (feature: plugin)
├── events.rs           # Streaming event types (feature: plugin)
├── query.rs            # Fluent query builder
├── validation.rs       # Config validation
├── registry_index.rs   # Bidirectional indexes
├── config.rs           # YAML parsing
├── client.rs           # HTTP + streaming
├── streaming/          # SSE parsers
└── bin/
    ├── cllient.rs      # Main CLI
    ├── cllient-registry.rs  # Validation tool
    └── cllient-hub.rs  # Hub server (feature: hub)

config/
├── service/            # Provider HTTP templates
└── family/             # Model configurations
```

## Development

```bash
# Build
cargo build

# Test
cargo test

# Build with plugin support
cargo build --features plugin

# Build hub binary
cargo build --features hub
```

## Status

**Working:**
- Core library with SSE streaming
- Config-driven architecture
- CLI tools
- Plugin/hub integration
- Query API and validation

**Tested providers:** OpenAI, Anthropic, DeepSeek

**Untested:** 242 OpenRouter models, Google, Azure

## License

MIT OR Apache-2.0

---

**Links**: [Documentation](docs/) | [Examples](docs/examples/) | [Configuration](docs/4_configuration.md)
