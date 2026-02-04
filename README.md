# cllient

A config-driven LLM client library in Rust. Define providers and models in YAML instead of code.

[![Crates.io](https://img.shields.io/crates/v/cllient.svg)](https://crates.io/crates/cllient)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│  YAML Configs                                                            │
│  ┌────────────────────┐         ┌────────────────────┐                   │
│  │ config/service/    │         │ config/family/     │                   │
│  │   anthropic.yaml   │◄────────│   claude/*.yaml    │                   │
│  │   openai.yaml      │◄────────│   gpt/*.yaml       │  model references │
│  │   deepseek.yaml    │◄────────│   deepseek/*.yaml  │  service by name  │
│  │   google.yaml      │◄────────│   google/*.yaml    │                   │
│  │   openrouter.yaml  │◄────────│   (242 models)     │                   │
│  └────────────────────┘         └────────────────────┘                   │
│            │                              │                              │
│            ▼                              ▼                              │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │                        ModelRegistry                              │   │
│  │  - Loads configs (embedded in binary or from filesystem)          │   │
│  │  - Renders Handlebars templates with request data                 │   │
│  │  - Substitutes environment variables (API keys)                   │   │
│  │  - Provides fluent Query API for model discovery                  │   │
│  └───────────────────────────────────────────────────────────────────┘   │
│                                   │                                      │
│            ┌──────────────────────┼──────────────────────┐               │
│            ▼                      ▼                      ▼               │
│     ┌─────────────┐       ┌─────────────┐       ┌─────────────┐          │
│     │  Library    │       │   CLI       │       │   Plugin    │          │
│     │  (Rust API) │       │   Tools     │       │   Hub       │          │
│     └─────────────┘       └─────────────┘       └─────────────┘          │
└──────────────────────────────────────────────────────────────────────────┘
```

## Features

- **Config-driven** - Add providers via YAML, no code changes required
- **SSE streaming** - Real-time token streaming with provider-specific parsers
- **Streaming JSON** - Emit valid JSON incrementally as tokens arrive
- **339 model configs** - 97 direct integrations + 242 via OpenRouter
- **Plugin system** - Expose as a Plexus RPC activation for hub integration
- **Query API** - Fluent filter builder for model discovery
- **Validation** - Schema, cross-reference, and semantic config validation
- **Registry export** - Single-call access to full registry for RPC/integrations
- **CLI + library** - Use from command line or as a Rust crate
- **Embedded configs** - Ship as a single binary with all configs baked in

## Quick Start

### Installation

```bash
# Install from source
cargo install --path .

# Set up API keys
cp .env.example .env
# Edit .env with your API keys
```

### CLI Examples

```bash
# List all models
cllient list

# List models matching a pattern
cllient list "claude"

# Simple completion
cllient ask gpt-4o-mini "What is Rust?"

# Streaming output
cllient stream deepseek-chat "Tell me a story"

# Interactive chat session
cllient chat claude-3-haiku-20240307

# Compare models on the same prompt
cllient compare "gpt-4o-mini,claude-3-haiku-20240307" "Explain monads"
```

## Library Usage

### Basic Completion

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
    Ok(())
}
```

### Streaming

```rust
use cllient::ModelRegistry;
use cllient::streaming::StreamEvent;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let registry = ModelRegistry::new()?;

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
    Ok(())
}
```

### Query API

```rust
use cllient::ModelRegistry;

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

## CLI Reference

### cllient

Main CLI for interacting with LLM providers.

| Command | Description | Example |
|---------|-------------|---------|
| `list [filter]` | List available models | `cllient list "claude"` |
| `list-services` | List available services | `cllient list-services` |
| `ask <model> <prompt>` | Single completion | `cllient ask gpt-4o-mini "Hello"` |
| `stream <model> <prompt>` | Streaming completion | `cllient stream deepseek-chat "Story"` |
| `chat <model>` | Interactive chat | `cllient chat claude-3-haiku-20240307` |
| `compare <models> <prompt>` | Compare multiple models | `cllient compare "gpt-4o,claude-3" "Hi"` |

**Global flags:** `--verbose`, `--pretty`, `--clean`

### cllient-registry

Registry validation and inspection tool.

| Command | Description | Example |
|---------|-------------|---------|
| `validate` | Validate all configs | `cllient-registry validate` |
| `validate --model <id>` | Validate specific model | `cllient-registry validate --model gpt-4o` |
| `stats` | Show registry statistics | `cllient-registry stats` |
| `orphans` | List services with no models | `cllient-registry orphans` |
| `broken` | List broken references | `cllient-registry broken` |
| `search <query>` | Search for models | `cllient-registry search claude --verified` |
| `services` | List all services | `cllient-registry services` |

### cllient-hub

Standalone hub server (requires `hub` feature).

```bash
# Build
cargo build --features hub --bin cllient-hub

# Run
cllient-hub --bind 127.0.0.1:8080
```

| Flag | Description | Default |
|------|-------------|---------|
| `--bind` | Address to bind | `127.0.0.1:8080` |
| `--config` | Config directory | embedded configs |
| `--verified-only` | Only load verified models | false |

## Configuration

### Environment Variables

```bash
# Core providers
OPENAI_API_KEY=your_key
ANTHROPIC_API_KEY=your_key
DEEPSEEK_API_KEY=your_key
GOOGLE_API_KEY=your_key

# Azure (requires deployment configuration)
AZURE_API_KEY=your_key

# OpenRouter (provides 242 additional models)
OPEN_ROUTER_API_KEY=your_key

# Avian
AVIAN_API_KEY=your_key
```

### Adding a New Provider

Create a service config at `config/service/newprovider.yaml`:

```yaml
service:
  name: NewProvider
  base_url: https://api.newprovider.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Authorization: Bearer ${NEWPROVIDER_API_KEY}

    {"model": "{{model_id}}", "messages": {{json messages}}}
```

Then add model configs referencing this service in `config/family/`.

## Plugin System

cllient can expose itself as a **Plexus RPC activation** for integration with hub-based systems.

### Feature Flags

```toml
[dependencies]
cllient = { version = "0.2" }                          # Library only
cllient = { version = "0.2", features = ["plugin"] }  # + CllientActivation
cllient = { version = "0.2", features = ["hub"] }     # + standalone serve
```

| Feature | What You Get | Dependencies |
|---------|--------------|--------------|
| (none) | `ModelRegistry` library | minimal |
| `plugin` | + `CllientActivation` | hub-core, hub-macro, jsonrpsee |
| `hub` | + `serve()` method | plugin deps |

### RPC Methods

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

## Model Support

| Metric | Count |
|--------|-------|
| Total models | 339 |
| Model families | 57 |
| Service providers | 9 |
| Direct integrations | 97 |
| Via OpenRouter | 242 |

### Service Providers

| Provider | Config | API Key |
|----------|--------|---------|
| Anthropic | `anthropic.yaml` | `ANTHROPIC_API_KEY` |
| OpenAI | `openai.yaml` | `OPENAI_API_KEY` |
| OpenAI (completions) | `openai-completions.yaml` | `OPENAI_API_KEY` |
| OpenAI (responses) | `openai-responses.yaml` | `OPENAI_API_KEY` |
| Google | `google.yaml` | `GOOGLE_API_KEY` |
| DeepSeek | `deepseek.yaml` | `DEEPSEEK_API_KEY` |
| Azure | `azure.yaml` | `AZURE_API_KEY` |
| OpenRouter | `openrouter.yaml` | `OPEN_ROUTER_API_KEY` |
| Avian | `avian.yaml` | `AVIAN_API_KEY` |

### Tested Providers

- **Verified:** OpenAI, Anthropic, DeepSeek
- **Untested:** Google, Azure, 242 OpenRouter models

## Project Structure

```
cllient/
├── src/
│   ├── lib.rs              # Public exports
│   ├── runtime.rs          # ModelRegistry core
│   ├── plugin.rs           # CllientActivation (feature: plugin)
│   ├── events.rs           # Streaming event types (feature: plugin)
│   ├── query.rs            # Fluent query builder
│   ├── validation.rs       # Config validation
│   ├── registry_index.rs   # Bidirectional indexes
│   ├── config.rs           # YAML parsing
│   ├── client.rs           # HTTP + streaming
│   ├── streaming/          # SSE parsers
│   └── bin/
│       ├── cllient.rs          # Main CLI
│       ├── cllient-registry.rs # Validation tool
│       ├── cllient-hub.rs      # Hub server (feature: hub)
│       └── cllient-utils.rs    # Utility commands
│
├── config/
│   ├── service/            # Provider HTTP templates (9 files)
│   └── family/             # Model configurations (339 files in 57 families)
│
└── docs/
    ├── architecture/       # Design documents
    └── examples/           # Usage examples
```

## Development

```bash
# Build library
cargo build

# Build with plugin support
cargo build --features plugin

# Build hub binary
cargo build --features hub --bin cllient-hub

# Run tests
cargo test

# Run tests with all features
cargo test --all-features

# Validate registry
cargo run --bin cllient-registry -- validate

# Check registry stats
cargo run --bin cllient-registry -- stats
```

## Recent Changes

Major refactoring in commit `c0ef5a1`:
- Eliminated redundancy across codebase
- Improved code quality and organization
- See [docs/architecture/](docs/architecture/) for detailed design documents

## License

MIT OR Apache-2.0

---

[Documentation](docs/) | [Examples](docs/examples/) | [Configuration](docs/4_configuration.md)
