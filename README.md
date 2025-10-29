# cllient - Runtime-Configurable LLM Client

> **Foreword**: This whole project, including the readme, was generated with concerningly little human oversight. I have tested that core features work for 3 families: openai, claude, and deepseek. That being said I believe that what is built here, a configuration driven llm client library, can serve as the basis for type-safe clients, agents, and more.

A flexible, high-performance LLM client system written in Rust that separates services (OpenAI, Anthropic, etc.) from models (GPT-4, Claude, etc.) for maximum flexibility and runtime configuration.

## ✨ Key Features

- 🚀 **330+ Pre-configured Models** - OpenAI, Anthropic, DeepSeek, Google, and 70+ providers
- 📡 **Real-time Streaming** - Token-level streaming with Server-Sent Events (SSE)
- 💰 **Cost Optimization** - Built-in pricing data with cheapest/fastest model selection
- 🔧 **Zero Code Changes** - Add providers via YAML configuration
- 🎯 **Multiple APIs** - CLI, high-level Runtime API, and low-level HTTP client
- 📦 **Self-contained** - Embedded configurations compiled into binary

## 🚀 Quick Start

### Installation

```bash
# Install from source
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
cllient ask gpt-4o-mini "What is Rust programming?"

# Real-time streaming
cllient stream deepseek-chat "Tell me a story about robots"

# Interactive chat
cllient chat claude-3-haiku-20240307

# Compare models
cllient compare gpt-4o-mini,claude-3-haiku "Explain quantum computing"
```

### Programmatic Usage

```rust
use cllient::ModelRegistry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;
    
    // Use specific model
    let response = registry
        .from_id("gpt-4o-mini")?
        .prompt("Hello, world!")
        .complete()
        .await?;
    
    // Use cheapest model matching pattern  
    let response = registry
        .use_cheapest("claude-*")?
        .prompt("Explain AI")
        .complete()
        .await?;
    
    println!("Response: {}", response.content);
    Ok(())
}
```

## 📚 Documentation

### Getting Started
- **[1. Installation & Setup](docs/1_installation.md)** - Prerequisites, installation, environment setup
- **[2. CLI Usage Guide](docs/2_cli-usage.md)** - Complete command-line reference
- **[Examples: Basic Usage](docs/examples/1_basic-usage.md)** - Essential usage patterns

### Core Guides  
- **[3. API Reference](docs/3_api-reference.md)** - Runtime API, CLI API, Low-level Client
- **[4. Configuration](docs/4_configuration.md)** - Services, models, environment variables
- **[5. Architecture](docs/5_architecture.md)** - "Unassociated truth" design principles

### Advanced Usage
- **[Examples](docs/examples/)** - Real-world usage patterns and scripts
- **[6. Development](docs/6_development.md)** - Contributing, testing, workflow automation
- **[Complete Documentation Hub](docs/)** - All guides and references

## 🏗️ Architecture Overview

cllient uses a **flexible architecture** where services and models exist independently:

```
Traditional: gpt-4 → OpenAI only ❌
cllient:     gpt-4 → {OpenAI, Azure, OpenRouter} ✅

Traditional: Code changes for new providers ❌  
cllient:     YAML configuration only ✅
```

**Core Components** ([5. Architecture Guide](docs/5_architecture.md)):
- **[ModelRegistry](src/runtime.rs#L12)** - High-level model selection and request building
- **[Configuration System](src/config.rs)** - Embedded + external YAML configs
- **[HTTP Client](src/client.rs)** - Provider communication with streaming
- **[Template Engine](src/template.rs)** - Handlebars request templating

## 🔧 Configuration

### Available Models

- **OpenAI**: `gpt-4o`, `gpt-4o-mini`, `o1`, `o1-mini` (330+ total models)
- **Anthropic**: `claude-3-opus`, `claude-3-5-sonnet`, `claude-3-haiku`  
- **DeepSeek**: `deepseek-chat`, `deepseek-coder`, `deepseek-v3`
- **Google**: `gemini-2.0-flash`, `gemini-pro`, `gemma` variants
- **+70 providers**: Meta, Microsoft, Cohere, X.AI, Perplexity, etc.

### Environment Variables

```bash
# Core providers (add to .env)
OPENAI_API_KEY=your_openai_key
ANTHROPIC_API_KEY=your_anthropic_key  
DEEPSEEK_API_KEY=your_deepseek_key

# Optional: Custom config directory
CLLIENT_CONFIG_DIR=/path/to/custom/configs
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
    
    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "stream": {{stream}}
    }
```

**Complete Configuration Guide**: [4. Configuration Documentation](docs/4_configuration.md)

## 🛠️ Development

### Build & Test

```bash
# Build project
make build

# Run tests  
make test

# See all commands
make help
```

### Model Management

```bash
# Fetch latest models from providers
make models-openai
make models-anthropic  
make models-all

# Pricing analysis
make pricing-cheapest
make cost-analysis
```

**Complete Development Guide**: [6. Development Documentation](docs/6_development.md)

## 📈 Roadmap

- [x] Core Rust implementation with streaming
- [x] 330+ models across 70+ providers  
- [x] Configuration-driven architecture
- [x] Cost tracking and optimization
- [x] Comprehensive CLI and APIs
- [ ] Python bindings (PyO3)
- [ ] TypeScript type generation
- [ ] Advanced retry logic and caching
- [ ] Local model support

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make changes and test: `make test && make lint`
4. Commit changes: `make commit`
5. Submit a pull request

**See**: [6. Development Guide](docs/6_development.md) for detailed contribution guidelines.

## 📄 License

This project is licensed under the terms specified in the [LICENSE](LICENSE) file.

## 🔗 Source Code Structure

- **[`src/bin/cllient.rs`](src/bin/cllient.rs)** - CLI implementation
- **[`src/runtime.rs`](src/runtime.rs)** - High-level Runtime API  
- **[`src/client.rs`](src/client.rs)** - HTTP client and streaming
- **[`src/config.rs`](src/config.rs)** - Configuration system
- **[`config/`](config/)** - Service and model configurations
- **[`docs/`](docs/)** - Complete documentation

---

**Quick Links**: [📖 Documentation](docs/) | [🚀 Examples](docs/examples/) | [⚙️ Configuration](docs/4_configuration.md) | [🏗️ Architecture](docs/5_architecture.md)