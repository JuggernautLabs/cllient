# LLM Client System - Implementation Plan

## Overview

This document outlines the plan for building a runtime-configurable LLM client system that separates "unassociated truths" (services and models) from their associations, allowing for maximum flexibility and extensibility.

## Architecture

### Core Concepts

1. **Unassociated Truth**: Services (Anthropic, OpenAI) and Models (Claude, GPT) exist independently
2. **Runtime Configuration**: YAML files define all service and model properties
3. **HTTP Transparency**: Service configs use literal HTTP request templates
4. **Rust Core**: Low-level protocol handling and streaming via PyO3 bindings
5. **Type Safety**: Strongly typed clients can be generated from configurations

### Directory Structure

```
config/
├── service/
│   ├── anthropic.yaml      # Anthropic service configuration
│   ├── openai.yaml         # OpenAI service configuration
│   └── deepseek.yaml       # DeepSeek service configuration
└── family/
    ├── claude/
    │   ├── claude-3-opus.yaml
    │   ├── claude-3-sonnet.yaml
    │   └── claude-3-haiku.yaml
    └── gpt/
        ├── gpt-4-turbo.yaml
        ├── gpt-4o.yaml
        └── gpt-3.5-turbo.yaml
```

## Configuration Schema

### Service Configuration (service/*.yaml)

Services define HTTP communication patterns using literal HTTP templates:

```yaml
service:
  name: "Service Name"
  base_url: "https://api.example.com"

http:
  request: |
    POST /v1/endpoint HTTP/1.1
    Host: api.example.com
    Content-Type: application/json
    Authorization: Bearer ${API_KEY}
    
    {
      "model": "${model_id}",
      "messages": ${messages},
      "temperature": ${temperature},
      "stream": ${stream}
    }

streaming:
  format: "text/event-stream"
  parser: "service_sse_parser"
  events:
    - type: "content"
      extract: "delta.text"

response:
  success_codes: [200]
  extract:
    content: "content[0].text"
    usage:
      input: "usage.input_tokens"
      output: "usage.output_tokens"

message_builder: "service_message_format"
```

### Model Configuration (family/*/*.yaml)

Models define capabilities, constraints, and service associations:

```yaml
model:
  id: "model-identifier"
  family: "model-family"
  name: "Human Readable Name"
  service: "service-name"

capabilities:
  context_window: 200000
  max_output_tokens: 4096
  vision: true
  functions: true
  streaming: true

pricing:
  currency: "USD"
  input_per_1k: 0.015
  output_per_1k: 0.075

constraints:
  max_images_per_message: 20
  supported_image_formats: ["jpeg", "jpg", "png", "gif", "webp"]

defaults:
  temperature: 1.0
  max_tokens: 4096
```

## Rust Implementation

### Core Traits and Structures

```rust
// Low-level client trait (inspired by semantic-query)
pub trait LowLevelClient: Send + Sync {
    fn ask_raw(&self, prompt: &str) -> Result<String>;
    fn stream_raw(&self, prompt: &str) -> Result<Box<dyn Stream<Item = String>>>;
}

// Service configuration
pub struct ServiceConfig {
    pub name: String,
    pub http_template: String,
    pub response_parser: String,
    pub stream_parser: String,
}

// Model configuration
pub struct ModelConfig {
    pub id: String,
    pub family: String,
    pub service: String,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
}

// Client factory
pub struct ClientFactory {
    services: HashMap<String, ServiceConfig>,
    models: HashMap<String, ModelConfig>,
}
```

### Key Components

1. **Configuration Loader**: Parse YAML files into Rust structures
2. **HTTP Client**: Process templates and make requests
3. **Stream Processor**: Handle SSE and other streaming formats
4. **Message Builders**: Format messages for different services
5. **PyO3 Bindings**: Expose Rust functionality to Python

## Python API

### Basic Usage

```python
import llm_client

# Create client from model ID
client = llm_client.create("claude-3-opus")

# Simple completion
response = client.chat("Hello, how are you?")

# Streaming
for chunk in client.stream("Tell me a story"):
    print(chunk, end="")

# With system prompt
response = client.chat(
    "Explain quantum computing",
    system="You are a physics teacher"
)
```

### Advanced Usage

```python
# Custom configuration directory
config = llm_client.load_config("./my_configs")
client = llm_client.create("custom-model", config=config)

# Runtime service definition
custom_service = {
    "name": "CustomLLM",
    "http": {
        "request": "POST https://custom.api/chat\n..."
    }
}
client = llm_client.create_from_config(
    service=custom_service,
    model={"id": "custom-1", "service": "CustomLLM"}
)

# Strongly typed client generation
from llm_client.typed import generate_client
ClaudeClient = generate_client("claude-3-opus")
client = ClaudeClient(api_key="...")
```

## Implementation Phases

### Phase 1: Core Infrastructure ✅ COMPLETED
- [x] Define YAML schemas
- [x] Create example configurations
- [x] Implement configuration loader in Rust
- [x] Basic HTTP client with template processing
- [x] Environment variable support (.env files)
- [x] CLI binary with model listing and execution

### Phase 2: Streaming Support ✅ COMPLETED
- [x] SSE parser implementation (integrated from semantic-query)
- [x] Stream trait and implementations
- [x] Advanced JSON structure detection
- [x] Token-level streaming with aggregation
- [x] Provider-specific SSE parsing (OpenAI, Claude, DeepSeek)

### Phase 3: Template Rendering & Configuration ✅ COMPLETED
- [x] Fixed Handlebars template syntax in service configs
- [x] Environment variable substitution (${UPPERCASE_VARS})
- [x] Template variable substitution ({{lowercase_vars}})
- [x] JSON helper for complex objects ({{json messages}})
- [x] HTTP request parsing and validation
- [x] Error handling for undefined template variables

### Phase 4: Multi-Provider Support ✅ COMPLETED  
- [x] Azure OpenAI service configuration
- [x] Avian (Meta Llama) service configuration
- [x] Multiple provider testing and validation
- [x] Comprehensive .gitignore for security
- [x] Environment management and API key protection

### Phase 5: Development Tooling ✅ COMPLETED
- [x] Comprehensive Makefile with 40+ commands
- [x] Model discovery APIs (OpenAI, Anthropic, DeepSeek)
- [x] Pricing analysis via Helicone API integration
- [x] Cost reporting and cheapest model identification
- [x] Provider testing and streaming validation
- [x] Configuration validation and environment checks
- [x] Development workflow automation (build, test, lint, commit)

### Phase 6: Python Bindings 🚧 PLANNED
- [ ] PyO3 wrapper for LowLevelClient
- [ ] Client factory implementation
- [ ] Configuration loading from Python
- [ ] Type stub generation

### Phase 7: Advanced Features 🚧 PLANNED
- [ ] Retry logic and rate limiting
- [ ] Request/response interceptors
- [ ] Caching layer
- [ ] Metrics and monitoring
- [ ] Additional provider configurations (AWS, full Azure coverage)
- [ ] Better error handling with specific messages

### Phase 8: Type Generation 🚧 PLANNED
- [ ] Runtime type information
- [ ] TypeScript/Python type generation
- [ ] OpenAPI schema generation
- [ ] Client SDK generation

## Design Decisions

### Why YAML?
- Human-readable configuration
- Multi-line string support for HTTP templates
- Widely supported across languages
- Clean syntax for nested structures

### Why HTTP Templates?
- Transparent - see exactly what's sent
- Easy to debug with curl
- No abstraction layers hiding details
- Supports any HTTP-based API

### Why Rust Core?
- Performance for streaming
- Memory safety
- Good Python interop via PyO3
- Strong type system

### Why Separate Services from Models?
- Services can support multiple models
- Models can potentially move between services
- Clean separation of concerns
- Easier to add new providers

## Success Criteria

1. **Extensibility**: Add new LLM providers without code changes
2. **Performance**: Stream processing with minimal latency
3. **Type Safety**: Optional strongly-typed clients
4. **Debugging**: Clear visibility into HTTP requests/responses
5. **Compatibility**: Work with any HTTP-based LLM API

## Future Considerations

- WebSocket support for bidirectional streaming
- gRPC protocol support
- Local model support (llama.cpp, etc.)
- Middleware system for plugins
- Request queuing and batching
- Multi-model routing and fallbacks

---

## Current Status Summary (Latest Update)

### ✅ What's Working
- **Core Infrastructure**: Full Rust implementation with streaming support
- **Template System**: Fixed Handlebars syntax, environment variables work perfectly
- **Multi-Provider Support**: DeepSeek, OpenAI, Anthropic, Azure, Avian configured
- **Streaming**: Server-Sent Events working for all providers
- **Development Tools**: Comprehensive Makefile with 40+ automation commands
- **Pricing Integration**: Live cost data from Helicone API
- **Model Discovery**: Automatic fetching of latest models from provider APIs

### 🎯 Key Achievements
1. **Template Bug Fix**: Resolved JSON parsing errors by converting `${var}` to `{{var}}` syntax
2. **Working Streaming**: `cargo run -- stream deepseek-chat "Hello"` works perfectly
3. **Cost Analysis**: `make pricing-cheapest` shows GPT-4.1-nano at $0.10/1M tokens as cheapest
4. **Provider APIs**: Direct integration with OpenAI, Anthropic, DeepSeek model APIs
5. **Development Workflow**: Complete build, test, lint, commit automation

### 🔧 Makefile Highlights
- `make pricing-cheapest` - Find cheapest models across all providers
- `make models-openai` - Fetch latest OpenAI models from API
- `make test-deepseek` - Test streaming with DeepSeek
- `make provider-costs PROVIDER=openai` - Provider-specific pricing
- `make env-check` - Validate API keys and environment setup

### 🚀 Ready for Next Phase
The system is now ready for Python bindings (PyO3) with a solid Rust foundation that:
- Handles runtime configuration via YAML
- Processes HTTP templates correctly
- Streams responses from multiple providers
- Integrates with pricing and model discovery APIs
- Provides comprehensive development tooling

### 📊 Architecture Validated
The "unassociated truth" principle is working:
- Services (OpenAI, Anthropic, DeepSeek) exist independently
- Models (GPT-4o, Claude-3, deepseek-chat) can be associated at runtime
- HTTP templates provide complete transparency
- Streaming works consistently across providers