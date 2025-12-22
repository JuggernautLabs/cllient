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
│                        ▼                                         │
│  ┌─────────────────────────────────────────┐                     │
│  │           HTTP + SSE Streaming          │                     │
│  │   Provider-specific parsers (OpenAI,    │                     │
│  │   Anthropic, Google) extract content    │                     │
│  └─────────────────────────────────────────┘                     │
│                        │                                         │
│                        ▼                                         │
│  ┌─────────────────────────────────────────┐                     │
│  │        Streaming JSON Output            │                     │
│  │   Valid JSON emitted incrementally -    │                     │
│  │   watch the response build in real-time │                     │
│  └─────────────────────────────────────────┘                     │
└──────────────────────────────────────────────────────────────────┘
```

> **Experimental**: Proof-of-concept with 3 tested providers (OpenAI, Anthropic, DeepSeek). 242 additional models available via OpenRouter but not validated. Not production-ready.

## Features

- **Config-driven** - Add providers via YAML, no code changes
- **SSE streaming** - Real-time token streaming with provider-specific parsers
- **Streaming JSON output** - Emit valid JSON incrementally as tokens arrive (pipe-friendly)
- **339 model configs** - 97 direct integrations + 242 via OpenRouter
- **Model verification tracking** - Know which models are tested vs auto-generated
- **Registry export** - Single-call access to full registry for RPC/integrations
- **CLI + library** - Use from command line or as a Rust crate
- **Embedded configs** - Ship as a single binary with all configs baked in

## Limitations

- Most configs are untested (especially OpenRouter models)
- Minimal error handling
- YAML templates can be fragile
- Research project, not production software

## Quick Start

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
# List available models (alias: ls)
cllient list

# Simple completion (JSON output by default)
cllient ask gpt-4o-mini "What is Rust programming?"

# Streaming - outputs valid JSON incrementally as tokens arrive
cllient stream deepseek-chat "Tell me a story"

# Human-readable output with decorations
cllient --pretty stream deepseek-chat "Tell me a story"

# Clean output for piping (just the response, no JSON)
cllient --clean ask gpt-4o-mini "Hello" | wc -w

# Interactive chat
cllient chat claude-3-haiku-20240307

# Compare models
cllient compare gpt-4o-mini,claude-3-haiku "Explain quantum computing"

# Get help on any command
cllient --help
cllient ask --help
```

### CLI Reference

```
cllient [OPTIONS] <COMMAND>

Commands:
  list (ls)       List available models
  list-services   List available services/providers
  ask             Single completion request
  stream          Streaming completion request
  chat            Interactive chat session
  compare         Compare multiple models on same prompt
  debug-response  Debug API response issues

Options:
  -v, --verbose   Enable debug logging
      --pretty    Human-readable output with decorations
      --clean     Raw response only (for piping)
  -h, --help      Print help (works on all subcommands)
  -V, --version   Print version
```

### Programmatic Usage

```rust
use cllient::{ModelRegistry, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    // Simple single message
    let response = registry
        .from_id("gpt-4o-mini")?
        .prompt("Hello, world!")
        .send()
        .await?;

    // Multi-turn conversation
    let response = registry
        .from_id("claude-3-haiku-20240307")?
        .messages(vec![
            Message::system("You are a helpful assistant"),
            Message::user("What is 2+2?"),
            Message::assistant("4"),
            Message::user("And 4+4?"),
        ])
        .send()
        .await?;

    // Use cheapest model matching pattern
    let response = registry
        .use_cheapest("claude-*")?
        .prompt("Explain AI")
        .send()
        .await?;

    println!("Response: {}", response.content);
    Ok(())
}
```

### Streaming

The library provides real-time streaming via `StreamEvent`. Each event represents a piece of the response as it arrives:

```rust
use cllient::{ClientFactory, EmbeddedConfigLoader, EmbeddedClientFactory, CompletionRequest};
use cllient::streaming::StreamEvent;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loader = EmbeddedConfigLoader::new()?;
    let factory = EmbeddedClientFactory::new(loader);
    let client = factory.create_client("gpt-4o-mini")?;

    let request = CompletionRequest::text("user", "Tell me a story")
        .with_streaming(true);

    let mut stream = client.complete_stream(&request).await?;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::Content(text) => print!("{}", text),  // Token chunk
            StreamEvent::Start => println!("--- stream started ---"),
            StreamEvent::Finish(reason) => println!("\n--- done: {:?} ---", reason),
            StreamEvent::Usage { input_tokens, output_tokens, .. } => {
                println!("Tokens: in={:?}, out={:?}", input_tokens, output_tokens);
            }
            StreamEvent::Error(e) => eprintln!("Error: {}", e),
            _ => {}
        }
    }
    Ok(())
}
```

**`StreamEvent` variants:**

| Variant | Description |
|---------|-------------|
| `Content(String)` | A chunk of text content (the main output) |
| `Start` | Stream has started |
| `Finish(Option<String>)` | Stream complete, with optional finish reason |
| `Usage { input_tokens, output_tokens, total_tokens }` | Token usage stats |
| `Role(String)` | Role information (usually at start) |
| `Error(String)` | An error occurred |
| `Raw(String)` | Raw event data (for debugging) |

### Typed JSON Extraction

Use `StreamEventExt::parse_json<T>()` to extract typed data from streaming responses. This is useful when the LLM returns structured JSON (tool calls, structured output, etc.):

```rust
use cllient::streaming::{StreamEventExt, StreamItem};
use serde::Deserialize;
use schemars::JsonSchema;
use futures::StreamExt;

#[derive(Deserialize, JsonSchema)]
struct ToolCall {
    name: String,
    args: serde_json::Value,
}

let stream = client.complete_stream(&request).await?;
let mut typed_stream = stream.parse_json::<ToolCall>();

while let Some(item) = typed_stream.next().await {
    match item {
        StreamItem::Data(tool) => println!("Tool call: {}", tool.name),
        StreamItem::Text(t) => print!("{}", t.text),  // Non-JSON text
        StreamItem::Token(tok) => print!("{}", tok),  // Raw tokens
    }
}
```

**`StreamItem<T>` variants:**

| Variant | Description |
|---------|-------------|
| `Data(T)` | Successfully parsed JSON matching your type |
| `Text(TextContent)` | Plain text or JSON that doesn't match type T |
| `Token(String)` | Individual token for real-time display |

## Documentation

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
- **[Registry Export API](docs/architecture/16680999631625794559_registry-export.md)** - RPC-friendly registry access and model verification
- **[Complete Documentation Hub](docs/)** - All guides and references

## Architecture

The core idea is **"unassociated truth"** - services and models are separate entities that can be mixed:

```
Traditional:  gpt-4 → hardcoded to OpenAI API
cllient:      gpt-4 → { OpenAI, Azure, OpenRouter }
              Model configs reference service configs
              Service configs are HTTP templates
```

**How it works:**

1. **Service configs** (`config/service/*.yaml`) define HTTP request templates using Handlebars:
   ```yaml
   http:
     request: |
       POST /v1/chat/completions HTTP/1.1
       Authorization: Bearer ${OPENAI_API_KEY}
       {"model": "{{model_id}}", "messages": {{json messages}}}
   ```

2. **Model configs** (`config/family/*/*.yaml`) reference a service and add metadata:
   ```yaml
   model:
     id: gpt-4o-mini
     service: openai  # Points to config/service/openai.yaml
   capabilities:
     context_window: 128000
   ```

3. **Runtime** loads configs, renders templates, makes HTTP requests

**Why this is neat:**
- Add new providers by writing YAML, not Rust
- Same model can work through multiple services
- HTTP templates make debugging transparent (see exactly what's sent)
- Configs can be embedded in binary or loaded at runtime

**Core Components:**
- **[ModelRegistry](src/runtime.rs#L12)** - High-level API for model selection
- **[Configuration System](src/config.rs)** - Loads and validates YAML
- **[HTTP Client](src/client.rs)** - Renders templates, handles streaming
- **[Template Engine](src/template.rs)** - Handlebars + environment variables

## Configuration

### What's Actually Available

**Tested & Working (97 models across 5 direct integrations):**
- **OpenAI** (35 models): `gpt-4o`, `gpt-4o-mini`, `o1`, `o1-mini`, etc.
- **Anthropic** (12 models): `claude-3-opus`, `claude-3-5-sonnet`, `claude-3-haiku`
- **DeepSeek** (22 models): `deepseek-chat`, `deepseek-coder`, `deepseek-v3`
- **Google** (25 models): `gemini-2.0-flash`, `gemini-pro`, `gemma` variants
- **Legacy OpenAI** (3 models): Old completion endpoints

**Via OpenRouter (242 models, untested):**
- Meta, Microsoft, Cohere, X.AI, Perplexity, and 50+ others
- These are auto-generated configs that *should* work but haven't been validated
- Requires `OPEN_ROUTER_API_KEY` environment variable

### Environment Variables

```bash
# Tested providers (add to .env)
OPENAI_API_KEY=your_openai_key
ANTHROPIC_API_KEY=your_anthropic_key
DEEPSEEK_API_KEY=your_deepseek_key

# OpenRouter (gives access to 242 models)
OPEN_ROUTER_API_KEY=your_openrouter_key
HTTP_REFERER=http://localhost:3000  # Required by OpenRouter
X_TITLE=cllient                      # Optional app name

# Google (untested but configured)
GOOGLE_API_KEY=your_google_key

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

## Development

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

## Status

**Completed:**
- [x] Core Rust implementation with SSE streaming
- [x] Streaming JSON output (valid JSON emitted incrementally)
- [x] Config-driven architecture (YAML templates)
- [x] Basic CLI tool (ask, stream, chat, compare)
- [x] 3 tested providers (OpenAI, Anthropic, DeepSeek)
- [x] OpenRouter integration (242 auto-generated configs)
- [x] Pricing data scraping from OpenRouter

**Aspirational (Not Started):**
- [ ] Test the 242 OpenRouter models individually
- [ ] Validate Google/Azure/other direct integrations
- [ ] Python bindings (PyO3)
- [ ] TypeScript type generation
- [ ] Proper error handling and retry logic
- [ ] Production hardening
- [ ] Local model support (llama.cpp, etc.)

## Contributing

This is an experimental project, so contributions are welcome but come with caveats:

**Good first contributions:**
- Test one of the 242 OpenRouter models and report if it works
- Add a new direct provider integration (not through OpenRouter)
- Improve error handling for common failure modes
- Fix the brittle template system
- Add tests (there aren't many)

**Before contributing:**
1. Understand this is a research project, not production software
2. The codebase was mostly AI-generated and may have lurking issues
3. Check existing issues to see if someone's already working on it

**If you still want to help:**
1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make changes and test: `make test && make lint`
4. Submit a pull request with clear description of what you tested

**See**: [6. Development Guide](docs/6_development.md) for detailed workflows.

## Should I Use This?

**Use this if:**
- You want to experiment with the config-driven architecture idea
- You're researching LLM client design patterns
- You need quick access to OpenRouter's 242 models via one interface
- You're okay debugging YAML templates when they break
- You want to learn how SSE streaming works in Rust

**Don't use this if:**
- You need a production-ready LLM client (use the official SDKs)
- You want comprehensive error handling and retries
- You need 100% stability and uptime
- You're not comfortable with experimental software
- You need Python/JS bindings (they don't exist yet)

**Alternatives to consider:**
- Official SDKs: `openai`, `anthropic`, `google-generativeai` Python packages
- LangChain / LlamaIndex: More mature, battle-tested frameworks
- LiteLLM: Similar unified interface concept, but production-ready

## License

This project is licensed under the terms specified in the [LICENSE](LICENSE) file.

## Source Code

- **[`src/bin/cllient.rs`](src/bin/cllient.rs)** - CLI implementation
- **[`src/runtime.rs`](src/runtime.rs)** - High-level Runtime API  
- **[`src/client.rs`](src/client.rs)** - HTTP client and streaming
- **[`src/config.rs`](src/config.rs)** - Configuration system
- **[`config/`](config/)** - Service and model configurations
- **[`docs/`](docs/)** - Complete documentation

---

**Quick Links**: [Documentation](docs/) | [Examples](docs/examples/) | [Configuration](docs/4_configuration.md) | [Architecture](docs/5_architecture.md)

---

**Disclaimer**: This is experimental research software. For production use, consider [LiteLLM](https://github.com/BerriAI/litellm) or official provider SDKs.