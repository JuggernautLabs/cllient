# API Reference

Complete reference for all three API levels provided by cllient.

## Overview

cllient provides three distinct API layers for different use cases:

1. **[CLI API](#cli-api)** - Command-line interface for direct usage
2. **[Runtime API](#runtime-api)** - High-level Rust API for applications
3. **[Low-Level Client API](#low-level-client-api)** - Direct HTTP client for advanced usage

---

## CLI API

> **Source**: [`src/bin/cllient.rs`](../src/bin/cllient.rs)

The command-line interface provides direct access to all cllient functionality:

```bash
# List all available models
cllient list

# Stream responses with real-time output
cllient stream <model-id> "<prompt>"

# Single completion request
cllient ask <model-id> "<prompt>"

# Interactive chat session
cllient chat <model-id>

# Compare responses across models
cllient compare <model1,model2> "<prompt>"
```

**Complete CLI documentation**: [2. CLI Usage Guide](2_cli-usage.md)

---

## Runtime API

> **Source**: [`src/runtime.rs`](../src/runtime.rs) | **Main Struct**: [`ModelRegistry`](../src/runtime.rs#L12)

High-level API for embedding cllient in Rust applications. Provides a fluent interface for model selection and request building.

### Basic Usage

```rust
use cllient::{ModelRegistry, ClientError};

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    // Initialize with embedded configurations
    let registry = ModelRegistry::new()?;
    
    // Use specific model
    let response = registry
        .from_id("gpt-4o-mini")?
        .prompt("Hello, world!")
        .complete()
        .await?;
    
    println!("Response: {}", response.content);
    Ok(())
}
```

### Model Selection Methods

> **Implementation**: [`ModelRegistry methods`](../src/runtime.rs#L16)

#### `from_id(model_id)` - Use Specific Model

```rust
let response = registry
    .from_id("claude-3-haiku-20240307")?
    .prompt("Explain quantum computing")
    .complete()
    .await?;
```

#### `use_cheapest(pattern)` - Select Cheapest Matching Model

```rust
// Find cheapest Claude model
let response = registry
    .use_cheapest("claude-*")?
    .prompt("Write a haiku")
    .complete()
    .await?;

// Find cheapest GPT model
let response = registry
    .use_cheapest("gpt-*")?
    .prompt("Explain AI")
    .complete()
    .await?;
```

#### `use_fastest(pattern)` - Select Fastest Matching Model

```rust
let response = registry
    .use_fastest("deepseek-*")?
    .prompt("Quick calculation: 2+2")
    .complete()
    .await?;
```

### Request Building

> **Source**: [`RequestBuilder`](../src/types.rs)

The Runtime API returns a `RequestBuilder` that allows method chaining:

```rust
let response = registry
    .from_id("gpt-4o-mini")?
    .prompt("Explain machine learning")
    .temperature(0.7)
    .max_tokens(1000)
    .complete()
    .await?;
```

### Streaming Responses

```rust
let mut stream = registry
    .from_id("deepseek-chat")?
    .prompt("Count to 10")
    .stream()
    .await?;

while let Some(chunk) = stream.next().await {
    match chunk? {
        StreamChunk::Content(text) => print!("{}", text),
        StreamChunk::Done => break,
        StreamChunk::Error(err) => eprintln!("Error: {}", err),
    }
}
```

### Model Information and Discovery

#### `list_models()` - Get All Available Models

```rust
let models = registry.list_models();
println!("Available models: {:#?}", models);
```

#### `list_models_matching(pattern)` - Filter Models by Pattern

```rust
// Get all Claude models
let claude_models = registry.list_models_matching("claude-*")?;

// Get all reasoning models
let reasoning_models = registry.list_models_matching("o[0-9]")?;
```

#### `get_model_info(model_id)` - Get Model Details

```rust
let model_info = registry.get_model_info("gpt-4o-mini")?;
println!("Context window: {}", model_info.capabilities.context_window);
println!("Input cost: ${} per 1K tokens", model_info.pricing.input_per_1k_tokens);
```

### Error Handling

> **Source**: [`ClientError`](../src/error.rs)

```rust
use cllient::{ModelRegistry, ClientError};

match registry.from_id("invalid-model") {
    Ok(builder) => { /* ... */ },
    Err(ClientError::ModelNotFound(model)) => {
        eprintln!("Model '{}' not found", model);
    },
    Err(e) => eprintln!("Error: {}", e),
}
```

---

## Low-Level Client API

> **Source**: [`src/client.rs`](../src/client.rs)

For advanced users needing direct control over HTTP requests and responses.

### Basic Usage

```rust
use cllient::{HttpClient, CompletionRequest, Message, Content, Role};

// Create client with service configuration
let client = HttpClient::new(service_config);

// Build request manually
let request = CompletionRequest {
    model_id: "gpt-4o-mini".to_string(),
    messages: vec![
        Message {
            role: Role::User,
            content: vec![Content::Text { 
                text: "Hello, world!".to_string() 
            }],
        }
    ],
    stream: false,
    max_tokens: Some(100),
    temperature: Some(0.7),
};

// Send request
let response = client.complete(&request).await?;
```

### Streaming with Low-Level API

```rust
let mut stream = client.complete_stream(&request).await?;

while let Some(chunk) = stream.next().await {
    match chunk? {
        StreamChunk::Content(text) => print!("{}", text),
        StreamChunk::Done => break,
        StreamChunk::Error(err) => eprintln!("Error: {}", err),
    }
}
```

---

## Type Definitions

> **Source**: [`src/types.rs`](../src/types.rs)

### Core Request Types

```rust
pub struct CompletionRequest {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub stream: bool,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

pub enum Role {
    User,
    Assistant, 
    System,
}
```

### Content Types

```rust
pub enum ContentBlock {
    Text { text: String },
    Image { 
        url: String, 
        detail: Option<String> 
    },
    Document { 
        data: Vec<u8>, 
        media_type: String 
    },
}
```

### Response Types

```rust
pub struct CompletionResponse {
    pub content: String,
    pub usage: Option<Usage>,
    pub model: String,
}

pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}
```

### Configuration Types

> **Source**: [`src/config.rs`](../src/config.rs)

```rust
pub struct ModelConfig {
    pub model: ModelInfo,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
}

pub struct Capabilities {
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub vision: bool,
    pub streaming: bool,
    pub function_calling: bool,
}

pub struct Pricing {
    pub currency: String,
    pub input_per_1k_tokens: f64,
    pub output_per_1k_tokens: f64,
}
```

---

## Multimodal Content Support

All APIs support multimodal content through the unified content system:

### Text Content

```rust
Content::Text { text: "Hello, world!".to_string() }
```

### Image Content

```rust
// Base64 encoded image
Content::Image { 
    url: "data:image/jpeg;base64,/9j/4AAQ...".to_string(),
    detail: Some("high".to_string())
}

// Image URL
Content::Image { 
    url: "https://example.com/image.jpg".to_string(),
    detail: None
}
```

### Document Content

```rust
Content::Document { 
    data: document_bytes,
    media_type: "application/pdf".to_string()
}
```

---

## Streaming JSON Output API

> **Source**: [`src/streaming_json.rs`](../src/streaming_json.rs) | **Examples**: [`examples/test_streaming_json.rs`](../examples/test_streaming_json.rs)

Low-level API for outputting JSON structures incrementally to stdout. Useful for building streaming responses in custom applications.

### StreamingJsonObject

A state machine that outputs JSON object structures as they're being built:

```rust
use cllient::streaming_json::StreamingJsonObject;

let mut json = StreamingJsonObject::new()?;

// Write static fields
json.field_string("model", "gpt-4o-mini")?;
json.field_bool("streaming", true)?;

// Close the object
json.close()?;

// Output (immediate):
// {
//   "model": "gpt-4o-mini",
//   "streaming": true
// }
```

### Streaming String Fields

For dynamic content that arrives incrementally (like LLM responses):

```rust
let mut json = StreamingJsonObject::new()?;
json.field_string("model", "deepseek-chat")?;

// Start streaming field
let mut response = json.field_streaming_string("response")?;

// Write chunks as they arrive
response.write_chunk("Hello")?;
response.write_chunk(" ")?;
response.write_chunk("world")?;

// Close the field
response.close()?;

json.field_bool("success", true)?;
json.close()?;

// Output (streams incrementally):
// {
//   "model": "deepseek-chat",
//   "response": "Hello world",
//   "success": true
// }
```

### Full Example with LLM Streaming

```rust
use cllient::streaming_json::StreamingJsonObject;
use futures::StreamExt;

async fn stream_response(model_id: &str, prompt: &str) -> std::io::Result<()> {
    let mut json = StreamingJsonObject::new()?;

    json.field_string("model", model_id)?;
    json.field_string("prompt", prompt)?;

    // Get LLM stream
    let client = create_client(model_id)?;
    let mut stream = client.complete_stream(&request).await?;

    // Start streaming response field
    let mut response_writer = json.field_streaming_string("response")?;

    // Stream each chunk to JSON output
    while let Some(chunk) = stream.next().await {
        response_writer.write_chunk(&chunk?)?;
    }

    response_writer.close()?;
    json.field_bool("success", true)?;
    json.close()?;

    Ok(())
}
```

### JSON Escaping

The API automatically handles JSON special characters:

```rust
response.write_chunk("Line 1\n")?;           // → "Line 1\n"
response.write_chunk("Quote: \"hello\"")?;  // → "Quote: \"hello\""
response.write_chunk("Path: C:\\file")?;    // → "Path: C:\\file"
```

### API Methods

#### `StreamingJsonObject`

| Method | Description |
|--------|-------------|
| `new()` | Initialize and output opening `{` |
| `field_string(key, value)` | Write a string field |
| `field_bool(key, value)` | Write a boolean field |
| `field_streaming_string(key)` | Start a streaming string field |
| `close()` | Output closing `}` |

#### `StreamingJsonString`

| Method | Description |
|--------|-------------|
| `write_chunk(chunk)` | Append text to the field value |
| `close()` | Output closing quote |

### Tracing Support

All operations are instrumented with `tracing`:

```bash
# Enable debug logging
RUST_LOG=cllient::streaming_json=debug cargo run
RUST_LOG=trace cargo run  # For chunk-level details
```

---

## Provider Integration

> **Service Configs**: [`config/service/`](../config/service/)

### Supported Providers

| Provider | Endpoint | Authentication | Streaming |
|----------|----------|----------------|-----------|
| **OpenAI** | `https://api.openai.com/v1/chat/completions` | Bearer Token | ✅ SSE |
| **Anthropic** | `https://api.anthropic.com/v1/messages` | API Key | ✅ SSE |
| **DeepSeek** | `https://api.deepseek.com/chat/completions` | Bearer Token | ✅ SSE |
| **Google** | Gemini API endpoints | API Key | ✅ SSE |
| **Azure OpenAI** | Azure endpoints | API Key | ✅ SSE |

### Adding New Providers

See [4. Configuration Guide](4_configuration.md#adding-providers) for details on adding new LLM providers.

---

## Testing

> **Test Sources**: [`tests/integration/`](../tests/integration/)

### API Testing

```bash
# Test CLI API
cargo test --test cli_api_tests

# Test Runtime API  
cargo test --test runtime_api_tests

# Test Low-level Client API
cargo test --test low_level_client_tests

# Test all APIs
cargo test --tests
```

### Example Test Usage

```rust
#[tokio::test]
async fn test_runtime_api() {
    let registry = ModelRegistry::new().unwrap();
    
    let response = registry
        .from_id("gpt-4o-mini").unwrap()
        .prompt("Hello, world!")
        .complete()
        .await
        .unwrap();
    
    assert!(!response.content.is_empty());
}
```

---

**Next**: [4. Configuration](4_configuration.md) | [Examples](examples/)