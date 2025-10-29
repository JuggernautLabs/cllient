# Architecture Guide

Deep dive into cllient's flexible architecture and core components.

## Overview

cllient implements a unique **configuration-driven architecture** that fundamentally separates:

1. **Services** (HTTP providers) - How to communicate with APIs
2. **Models** (LLM capabilities) - What models can do
3. **Associations** (runtime bindings) - Which models use which services

This separation enables maximum flexibility without code changes.

---

## Core Principles

### Service-Model Separation

Traditional LLM clients hardcode model-to-provider relationships:

```
❌ Traditional: gpt-4 → OpenAI only
❌ Traditional: claude → Anthropic only  
❌ Traditional: Code changes needed for new providers
```

cllient separates these concerns:

```
✅ cllient: gpt-4 → {openai, azure, openrouter}
✅ cllient: claude → {anthropic, openrouter, bedrock}
✅ cllient: Configuration-driven associations
```

### Benefits

- **Runtime Flexibility**: Switch providers without rebuilding
- **Provider Agnostic**: Same model via different APIs
- **HTTP Transparency**: See exactly what requests are sent
- **Zero Code Changes**: Add providers via YAML config

---

## System Components

> **Main Library**: [`src/lib.rs`](../src/lib.rs)

### 1. Configuration System

> **Source**: [`src/config.rs`](../src/config.rs) | **Embedded**: [`src/embedded_config.rs`](../src/embedded_config.rs)

```
Configuration Hierarchy:
┌─────────────────────────┐
│ Embedded Configurations │ ← 330+ models compiled in
├─────────────────────────┤
│ External YAML Files     │ ← Custom overrides  
├─────────────────────────┤
│ Environment Variables   │ ← API keys, runtime config
└─────────────────────────┘
```

**Key Components**:
- **[ConfigProvider Trait](../src/config.rs)** - Abstraction for config sources
- **[EmbeddedConfigLoader](../src/embedded_config.rs)** - Built-in configurations via `rust-embed`
- **[FileBasedConfigLoader](../src/config.rs)** - External YAML configurations

### 2. Runtime Layer

> **Source**: [`src/runtime.rs`](../src/runtime.rs#L12)

The `ModelRegistry` provides the high-level API:

```rust
ModelRegistry
├── config_provider: Arc<dyn ConfigProvider>
├── from_id(model_id) → RequestBuilder
├── use_cheapest(pattern) → RequestBuilder  
├── use_fastest(pattern) → RequestBuilder
└── list_models() → Vec<String>
```

**Key Features**:
- **Model Selection**: Pattern matching, cost optimization
- **Request Building**: Fluent API for request construction
- **Configuration Management**: Loading and validation

### 3. HTTP Client Layer

> **Source**: [`src/client.rs`](../src/client.rs)

Low-level HTTP communication with LLM providers:

```rust
HttpClient
├── service_config: ServiceConfig
├── complete(request) → CompletionResponse
├── complete_stream(request) → StreamResponse
└── build_request(template, data) → HttpRequest
```

### 4. Template Engine

> **Source**: [`src/template.rs`](../src/template.rs)

Handlebars-based request templating with environment substitution:

```yaml
http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Authorization: Bearer ${OPENAI_API_KEY}
    
    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "stream": {{stream}}
    }
```

**Features**:
- **Variable Substitution**: `${ENV_VAR}` and `{{template_var}}`
- **JSON Helpers**: `{{json data}}` for proper serialization
- **HTTP Transparency**: See exact requests being sent

### 5. Streaming Infrastructure

> **Source**: [`src/streaming/`](../src/streaming/)

Real-time response processing via Server-Sent Events (SSE):

```
Streaming Pipeline:
Raw SSE → Provider Parser → Content Extractor → StreamChunk
```

**Components**:
- **[SSE Core](../src/streaming/sse/core/)** - Generic SSE processing
- **[Provider Parsers](../src/streaming/sse/providers/)** - OpenAI, Anthropic, etc.
- **[Content Extractors](../src/streaming/sse/extractors/)** - Extract text from responses
- **[JSON Utils](../src/streaming/json_utils.rs)** - JSON path extraction

---

## Data Flow Architecture

### Request Flow

```
1. CLI/API → ModelRegistry.from_id("gpt-4o-mini")
2. ModelRegistry → Load model config from embedded/external
3. ModelRegistry → Find associated service config  
4. RequestBuilder → Build CompletionRequest
5. HttpClient → Apply Handlebars template
6. HttpClient → Substitute environment variables
7. HttpClient → Send HTTP request to provider
8. StreamProcessor → Parse SSE response (if streaming)
9. Response → Return CompletionResponse
```

### Configuration Resolution

```
1. EmbeddedConfigLoader → Load built-in configs
2. FileBasedConfigLoader → Override with external configs
3. Environment → Substitute ${API_KEY} variables
4. Validation → Ensure service/model consistency
5. Runtime → Cache resolved configurations
```

---

## Core Types

> **Source**: [`src/types.rs`](../src/types.rs)

### Request Types

```rust
pub struct CompletionRequest {
    pub model_id: String,        // "gpt-4o-mini"
    pub messages: Vec<Message>,  // Conversation history
    pub stream: bool,           // Enable streaming
    pub max_tokens: Option<u32>, // Output limit
    pub temperature: Option<f32>, // Randomness
}

pub struct Message {
    pub role: Role,              // User/Assistant/System
    pub content: Vec<ContentBlock>, // Text/Image/Document
}

pub enum ContentBlock {
    Text { text: String },
    Image { url: String, detail: Option<String> },
    Document { data: Vec<u8>, media_type: String },
}
```

### Configuration Types

```rust
pub struct ModelConfig {
    pub model: ModelInfo,         // ID, family, service
    pub capabilities: Capabilities, // Context, vision, etc.
    pub pricing: Pricing,         // Cost per token
}

pub struct ServiceConfig {
    pub service: ServiceInfo,     // Name, base URL
    pub http: HttpConfig,        // Request template
    pub streaming: StreamConfig, // SSE configuration
    pub response: ResponseConfig, // Response extraction
}
```

---

## Provider Integration

### Service Definition

> **Example**: [`config/service/openai.yaml`](../config/service/openai.yaml)

Services define HTTP communication patterns:

```yaml
service:
  name: OpenAI
  base_url: https://api.openai.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Host: api.openai.com
    Authorization: Bearer ${OPENAI_API_KEY}
    
    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "stream": {{stream}}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  extract:
    content: choices[0].message.content
    usage: usage
```

### Model Association

> **Example**: [`config/family/openai/gpt-4o-mini.yaml`](../config/family/openai/gpt-4o-mini.yaml)

Models reference services by name:

```yaml
model:
  id: gpt-4o-mini
  family: gpt
  service: openai  # ← References service config

capabilities:
  context_window: 128000
  streaming: true
  vision: true

pricing:
  input_per_1k_tokens: 0.000150
  output_per_1k_tokens: 0.000600
```

---

## Streaming Architecture

> **Source**: [`src/streaming/`](../src/streaming/)

### SSE Processing Pipeline

```
HTTP Response Stream
│
├─ SSE Parser (provider-specific)
│   ├─ OpenAI format: data: {"choices":[{"delta":{"content":"text"}}]}
│   ├─ Anthropic format: data: {"delta":{"text":"content"}}  
│   └─ Google format: data: {"candidates":[{"content":{"parts":[{"text":"content"}]}}]}
│
├─ Content Extractor
│   ├─ Extract text content from JSON
│   ├─ Handle completion markers
│   └─ Process usage statistics
│
└─ Stream Chunks
    ├─ StreamChunk::Content(text)
    ├─ StreamChunk::Done
    └─ StreamChunk::Error(error)
```

### Provider-Specific Parsers

> **Implementation**: [`src/streaming/sse/providers/`](../src/streaming/sse/providers/)

Each provider has custom SSE parsing:

**OpenAI/DeepSeek Parser**:
```rust
// Parse: data: {"choices":[{"delta":{"content":"hello"}}]}
let content = json["choices"][0]["delta"]["content"].as_str()?;
```

**Anthropic Parser**:
```rust  
// Parse: data: {"delta":{"text":"hello"}}
let content = json["delta"]["text"].as_str()?;
```

**Google Parser**:
```rust
// Parse: data: {"candidates":[{"content":{"parts":[{"text":"hello"}]}}]}
let content = json["candidates"][0]["content"]["parts"][0]["text"].as_str()?;
```

---

## Configuration Architecture

### Embedded Configuration

> **Implementation**: [`src/embedded_config.rs`](../src/embedded_config.rs)

Uses `rust-embed` to compile configurations into the binary:

```rust
#[derive(RustEmbed)]
#[folder = "config/"]
struct ConfigAssets;

impl EmbeddedConfigLoader {
    fn load_service_configs(&self) -> Vec<ServiceConfig> {
        for file in ConfigAssets::iter() {
            if file.starts_with("service/") && file.ends_with(".yaml") {
                // Load and parse YAML
            }
        }
    }
}
```

**Benefits**:
- **Zero Dependencies**: No external config files required
- **Fast Loading**: Configs loaded from memory
- **Version Consistency**: Configs versioned with code

### External Configuration Override

> **Implementation**: [`src/config.rs`](../src/config.rs)

```rust
impl FileBasedConfigLoader {
    fn load_from_directory(&self, dir: &Path) -> Result<ConfigSet> {
        // Load service/*.yaml files
        // Load family/**/*.yaml files  
        // Override embedded configs
    }
}
```

---

## Error Handling

> **Source**: [`src/error.rs`](../src/error.rs)

Comprehensive error types for different failure modes:

```rust
pub enum ClientError {
    ModelNotFound(String),        // Invalid model ID
    ServiceNotFound(String),      // Invalid service name
    Config(ConfigError),          // Configuration errors
    Http(HttpError),             // Network errors
    Template(TemplateError),     // Template rendering errors
    Streaming(StreamingError),   // SSE parsing errors
    Auth(AuthError),             // Authentication errors
}
```

---

## Extension Points

### Adding New Providers

1. **Create Service Config**: Define HTTP communication
2. **Add Models**: Reference the service
3. **Custom Parser**: If needed for streaming
4. **Test Integration**: Validate functionality

### Custom Content Types

Extend `ContentBlock` enum for new content types:

```rust
pub enum ContentBlock {
    Text { text: String },
    Image { url: String, detail: Option<String> },
    Document { data: Vec<u8>, media_type: String },
    Audio { data: Vec<u8>, format: AudioFormat },    // ← New
    Video { url: String, timestamps: Vec<f64> },     // ← New
}
```

### Custom Message Builders

> **Implementation**: [`src/streaming/`](../src/streaming/)

Create provider-specific message formatting:

```rust
trait MessageBuilder {
    fn build_messages(&self, messages: &[Message]) -> serde_json::Value;
}
```

---

## Performance Characteristics

### Configuration Loading

- **Embedded**: ~1ms startup time
- **External**: ~10ms for 100 configs  
- **Caching**: Configs cached after first load

### HTTP Performance

- **Connection Reuse**: HTTP/2 connection pooling
- **Streaming**: Sub-100ms first token
- **Concurrency**: Async/await throughout

### Memory Usage

- **Embedded Configs**: ~500KB compiled size
- **Runtime**: ~10MB for loaded configs
- **Streaming**: ~1KB per active stream

---

## Security Architecture

### API Key Handling

- **Environment Variables**: Keys loaded from environment
- **Template Substitution**: `${VAR}` expanded at request time
- **No Storage**: Keys never persisted to disk
- **Scope Isolation**: Keys scoped to specific services

### HTTP Security

- **TLS Required**: All provider communication via HTTPS
- **Header Validation**: Proper authorization headers
- **Request Signing**: Provider-specific authentication
- **Error Sanitization**: No secrets in error messages

---

**Next**: [Examples](examples/) | [6. Development](6_development.md)