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
        .send()
        .await?;

    println!("Response: {}", response.content);
    Ok(())
}
```

### Registry Constructors

```rust
// Standard constructor (validates cross-references)
let registry = ModelRegistry::new()?;

// Permissive constructor (allows broken references for debugging)
let registry = ModelRegistry::new_permissive()?;

// With custom config provider
let registry = ModelRegistry::with_provider(my_provider)?;

// With custom provider, permissive
let registry = ModelRegistry::with_provider_permissive(my_provider);
```

### Model Selection Methods

> **Implementation**: [`ModelRegistry methods`](../src/runtime.rs#L22)

#### `from_id(model_id)` - Use Specific Model

```rust
let response = registry
    .from_id("claude-3-haiku-20240307")?
    .prompt("Explain quantum computing")
    .send()
    .await?;
```

#### `use_cheapest(pattern)` - Select Cheapest Matching Model

```rust
// Find cheapest Claude model
let response = registry
    .use_cheapest("claude-*")?
    .prompt("Write a haiku")
    .send()
    .await?;

// Find cheapest GPT model
let response = registry
    .use_cheapest("gpt-*")?
    .prompt("Explain AI")
    .send()
    .await?;
```

#### `use_fastest(pattern)` - Select Fastest Matching Model

```rust
let response = registry
    .use_fastest("deepseek-*")?
    .prompt("Quick calculation: 2+2")
    .send()
    .await?;
```

#### `use_best_quality(pattern)` - Select Highest Quality Matching Model

```rust
let response = registry
    .use_best_quality("claude-*")?
    .prompt("Write a detailed analysis")
    .send()
    .await?;
```

### Request Building

> **Source**: [`RequestBuilder`](../src/types.rs)

The Runtime API returns a `RequestBuilder` that allows method chaining:

```rust
let response = registry
    .from_id("gpt-4o-mini")?
    .system("You are a helpful assistant")
    .prompt("Explain machine learning")
    .temperature(0.7)
    .max_tokens(1000)
    .top_p(0.9)
    .send()
    .await?;
```

#### RequestBuilder Methods

| Method | Description |
|--------|-------------|
| `system(prompt)` | Set system prompt |
| `prompt(text)` | Set user message (convenience for single turn) |
| `temperature(f64)` | Set temperature parameter |
| `max_tokens(u32)` | Set max output tokens |
| `top_p(f64)` | Set top_p parameter |
| `parameter(key, value)` | Set any custom parameter |
| `append_message(Message)` | Add a message to the conversation |
| `messages(Vec<Message>)` | Set all messages at once |
| `send()` | Send request (requires messages set) |
| `send_text(text)` | Set message and send in one call |
| `send_multimodal(blocks)` | Send multimodal content |
| `stream()` | Start streaming (requires messages set) |
| `stream_text(text)` | Set message and stream in one call |

#### Multi-turn Conversations

```rust
let response = registry
    .from_id("gpt-4o-mini")?
    .append_message(Message::user("What is 2+2?"))
    .append_message(Message::assistant("4"))
    .append_message(Message::user("And what's 4+4?"))
    .send()
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

#### `list_families()` / `list_models_in_family(family)` - Family Navigation

```rust
// List all model families
let families = registry.list_families();

// List models in a specific family
let gpt_models = registry.list_models_in_family("gpt");
```

#### `list_services()` / `get_service(name)` - Service Access

```rust
// List all available services
let services = registry.list_services();

// Get specific service configuration
let anthropic = registry.get_service("anthropic")?;
```

#### Verification Status Methods

```rust
// List only verified models
let verified = registry.list_verified_models();

// Check if a model is verified
if registry.is_verified("gpt-4o-mini") {
    println!("Model is verified");
}

// List models by status
use cllient::VerificationStatus;
let broken = registry.list_models_by_status(VerificationStatus::Broken);
```

### Query API

> **Source**: [`src/query.rs`](../src/query.rs)

The fluent query API provides powerful filtering capabilities:

```rust
let models = registry.query()
    .service("openai")
    .verified()
    .with_vision()
    .context_min(100_000)
    .max_input_price(0.01)
    .order_by_price_asc()
    .limit(10)
    .list();
```

#### Query Filter Methods

| Method | Description |
|--------|-------------|
| `service(name)` | Filter by exact service name |
| `service_like(pattern)` | Filter by service pattern (glob-style) |
| `family(name)` | Filter by exact family name |
| `family_like(pattern)` | Filter by family pattern |
| `verified()` | Only verified models |
| `unverified()` | Only unverified models |
| `status(VerificationStatus)` | Filter by status |
| `with_vision()` | Models with vision capability |
| `with_streaming()` | Models with streaming capability |
| `with_functions()` | Models with function calling |
| `with_json_mode()` | Models with JSON mode |
| `with_system_prompt()` | Models with system prompt support |
| `with_multimodal()` | Models with multimodal capability |
| `max_price(input, output)` | Maximum price per 1k tokens |
| `max_input_price(price)` | Maximum input price |
| `max_output_price(price)` | Maximum output price |
| `context_min(tokens)` | Minimum context window |
| `context_max(tokens)` | Maximum context window |
| `fuzzy(query)` | Fuzzy search on model ID/name |
| `model_id(id)` | Exact model ID match |
| `model_id_like(pattern)` | Model ID pattern match |

#### Query Terminal Methods

| Method | Description |
|--------|-------------|
| `list()` | Return matching model IDs |
| `first()` | Return first matching model |
| `count()` | Return count of matches |
| `cheapest()` | Return cheapest matching model |
| `configs()` | Return full model configs |

#### Query Ordering

| Method | Description |
|--------|-------------|
| `order_by_price_asc()` | Cheapest first |
| `order_by_price_desc()` | Most expensive first |
| `order_by_context_desc()` | Largest context first |
| `limit(n)` | Limit results |

### Error Handling

> **Source**: [`ClientError`](../src/error.rs)

```rust
use cllient::{ModelRegistry, ClientError, ConfigError};

match registry.from_id("invalid-model") {
    Ok(builder) => { /* ... */ },
    Err(ClientError::Config(ConfigError::ModelNotFound(model))) => {
        eprintln!("Model '{}' not found", model);
    },
    Err(ClientError::Auth) => {
        eprintln!("Authentication failed");
    },
    Err(ClientError::RateLimit) => {
        eprintln!("Rate limit exceeded");
    },
    Err(e) => eprintln!("Error: {}", e),
}
```

#### ClientError Variants

| Variant | Description |
|---------|-------------|
| `Config(ConfigError)` | Configuration errors (model/service not found, etc.) |
| `Http(reqwest::Error)` | HTTP request errors |
| `YamlSerialization` | YAML parsing errors |
| `JsonSerialization` | JSON parsing errors |
| `Template` / `Render` | Template processing errors |
| `JsonPath(String)` | JSONPath extraction errors |
| `Io(std::io::Error)` | IO errors |
| `Stream(String)` | Streaming errors |
| `RateLimit` | Rate limit exceeded |
| `Auth` | Authentication failed |
| `InvalidModel(String)` | Invalid model specification |
| `InvalidService(String)` | Invalid service specification |
| `ValidationError(String)` | Validation errors |

#### ConfigError Variants

| Variant | Description |
|---------|-------------|
| `ServiceNotFound(String)` | Service not in registry |
| `ModelNotFound(String)` | Model not in registry |
| `InvalidPath(String)` | Invalid configuration path |
| `MissingField(String)` | Required field missing |
| `InvalidYaml(String)` | YAML parsing failed |
| `EnvVarNotFound(String)` | Environment variable missing |
| `BrokenReferences(Vec<String>)` | Models reference non-existent services |

---

## Low-Level Client API

> **Source**: [`src/client.rs`](../src/client.rs)

For advanced users needing direct control over HTTP requests and responses.

### ConfigProvider Trait

```rust
/// Trait for configuration providers (file-based or embedded)
pub trait ConfigProvider {
    fn get_service(&self, name: &str) -> Result<&ServiceConfig>;
    fn get_model(&self, id: &str) -> Result<&ModelConfig>;
    fn list_services(&self) -> Vec<&str>;
    fn list_models(&self) -> Vec<&str>;
    fn get_model_with_service(&self, model_id: &str) -> Result<(&ModelConfig, &ServiceConfig)>;
}
```

### LowLevelClient Trait

```rust
#[async_trait]
pub trait LowLevelClient: Send + Sync {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse>;
    async fn complete_stream(&self, request: &CompletionRequest) -> Result<Stream>;
}
```

### HttpClient

```rust
use cllient::{HttpClient, CompletionRequest, ContentBlock};

// Create client from model ID (uses embedded configs)
let client = HttpClient::from_model_id(&config_provider, "gpt-4o-mini")?;

// Or create with explicit configs
let client = HttpClient::new(model_config, service_config)?;

// Build request
let request = CompletionRequest::text("user", "Hello, world!")
    .with_system_prompt("You are helpful".to_string())
    .with_parameter("temperature", 0.7)
    .with_parameter("max_tokens", 100);

// Send request
let response = client.complete(&request).await?;
println!("Response: {}", response.content);
```

### Streaming with Low-Level API

```rust
use futures::StreamExt;

let request = CompletionRequest::text("user", "Count to 10")
    .with_streaming(true);

let mut stream = client.complete_stream(&request).await?;

while let Some(chunk) = stream.next().await {
    match chunk {
        Ok(text) => print!("{}", text),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### ClientFactory

```rust
use cllient::{ClientFactory, EmbeddedConfigLoader};

let loader = EmbeddedConfigLoader::new()?;
let factory = ClientFactory::new(loader);

// Create clients for different models
let client1 = factory.create_client("gpt-4o-mini")?;
let client2 = factory.create_client("claude-3-haiku-20240307")?;

// List available resources
let models = factory.list_available_models();
let services = factory.list_available_services();
let families = factory.list_families();
```

---

## Type Definitions

> **Source**: [`src/types.rs`](../src/types.rs)

### Message Types

```rust
/// Builder for constructing messages with specific roles and content
pub struct Message {
    role: String,
    content: Vec<ContentBlock>,
}

impl Message {
    // Constructors
    pub fn user(text: &str) -> Self;
    pub fn assistant(text: &str) -> Self;
    pub fn system(text: &str) -> Self;
    pub fn custom(role: &str) -> Self;
    pub fn user_multimodal(content: Vec<ContentBlock>) -> Self;

    // Builder methods
    pub fn add_text(self, text: &str) -> Self;
    pub fn add_image(self, data: Vec<u8>, format: ImageFormat) -> Self;
    pub fn add_image_url(self, url: &str) -> Self;
    pub fn add_audio(self, data: Vec<u8>, format: AudioFormat, filename: Option<String>) -> Self;
    pub fn add_document(self, data: Vec<u8>, format: DocumentFormat, filename: Option<String>) -> Self;
    pub fn add_content(self, block: ContentBlock) -> Self;
}

/// Low-level message content (internal)
pub enum MessageContent {
    Text { role: String, content: String },
    Multimodal { role: String, content: Vec<ContentBlock> },
}
```

### Content Types

```rust
pub enum ContentBlock {
    /// Text content
    Text(String),
    /// Binary content (images, audio, documents)
    Binary {
        data: Vec<u8>,
        mime_type: String,
        filename: Option<String>,
    },
    /// URL reference
    Url {
        url: String,
        mime_type: Option<String>,
    },
}

impl ContentBlock {
    pub fn text(content: &str) -> Self;
    pub fn image(data: Vec<u8>, format: ImageFormat) -> Self;
    pub fn image_url(url: &str) -> Self;
    pub fn audio(data: Vec<u8>, format: AudioFormat, filename: Option<String>) -> Self;
    pub fn document(data: Vec<u8>, format: DocumentFormat, filename: Option<String>) -> Self;
    pub fn binary(data: Vec<u8>, mime_type: &str, filename: Option<String>) -> Self;
    pub fn url(url: &str, mime_type: Option<&str>) -> Self;
}

// Format enums
pub enum ImageFormat { Jpeg, Png, Gif, Webp }
pub enum AudioFormat { Wav, Mp3, M4a, Flac, Ogg }
pub enum DocumentFormat { Pdf, Txt, Csv, Doc, Docx }
```

### Request Types

```rust
pub struct CompletionRequest {
    pub messages: Vec<MessageContent>,
    pub system_prompt: Option<String>,
    pub parameters: HashMap<String, Value>,
    pub stream: bool,
}

impl CompletionRequest {
    pub fn new(messages: Vec<MessageContent>) -> Self;
    pub fn text(role: &str, content: &str) -> Self;
    pub fn multimodal(role: &str, content_blocks: Vec<ContentBlock>) -> Self;
    pub fn with_system_prompt(self, prompt: String) -> Self;
    pub fn with_parameter<T: Into<Value>>(self, key: &str, value: T) -> Self;
    pub fn with_streaming(self, stream: bool) -> Self;
}
```

### Response Types

```rust
pub struct CompletionResponse {
    pub content: String,
    pub role: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
    pub raw_response: serde_json::Value,
}

pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: Option<u32>,
}

impl Usage {
    pub fn calculate_cost(&self, input_cost_per_1k: f64, output_cost_per_1k: f64) -> f64;
}
```

### Configuration Types

> **Source**: [`src/config.rs`](../src/config.rs)

```rust
pub struct ModelConfig {
    pub model: ModelInfo,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
    pub constraints: Constraints,
    pub defaults: HashMap<String, serde_yaml::Value>,
    pub quality: HashMap<String, u8>,
    pub behaviors: HashMap<String, bool>,
    pub use_cases: Vec<String>,
}

pub struct Capabilities {
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub vision: bool,
    pub functions: bool,
    pub streaming: bool,
    pub json_mode: bool,
    pub system_prompt: bool,
    pub multimodal: bool,
}

pub struct Pricing {
    pub currency: Currency,
    pub input_per_1k_tokens: f64,
    pub output_per_1k_tokens: f64,
    pub cached_input_per_1k_tokens: Option<f64>,
}

pub enum VerificationStatus {
    Verified,
    Unverified,
    Broken,
    Deprecated,
    Error,
    NotFound,
}
```

---

## Multimodal Content Support

All APIs support multimodal content through the unified content system:

### Using Message Builder (Recommended)

```rust
use cllient::{Message, ImageFormat, DocumentFormat};

// Simple text message
let msg = Message::user("Hello, world!");

// Image with text
let image_data = std::fs::read("photo.jpg")?;
let msg = Message::user("What's in this image?")
    .add_image(image_data, ImageFormat::Jpeg);

// Image URL
let msg = Message::user("Describe this image")
    .add_image_url("https://example.com/image.jpg");

// Document with question
let pdf_data = std::fs::read("document.pdf")?;
let msg = Message::user("Summarize this document")
    .add_document(pdf_data, DocumentFormat::Pdf, Some("report.pdf".to_string()));

// Multiple content blocks
let msg = Message::user_multimodal(vec![
    ContentBlock::text("Compare these images:"),
    ContentBlock::image(image1_data, ImageFormat::Png),
    ContentBlock::image(image2_data, ImageFormat::Png),
]);
```

### Using ContentBlock Directly

```rust
use cllient::ContentBlock;

// Text content
let text = ContentBlock::text("Hello, world!");

// Binary image
let image = ContentBlock::image(image_bytes, ImageFormat::Jpeg);

// Image URL
let image_url = ContentBlock::image_url("https://example.com/image.jpg");

// Audio content
let audio = ContentBlock::audio(audio_bytes, AudioFormat::Mp3, Some("recording.mp3".to_string()));

// Document
let doc = ContentBlock::document(pdf_bytes, DocumentFormat::Pdf, Some("file.pdf".to_string()));

// Generic binary with custom MIME type
let binary = ContentBlock::binary(data, "application/custom", None);

// Generic URL
let url = ContentBlock::url("https://example.com/file", Some("application/octet-stream"));
```

### Loading from Files

```rust
use cllient::{ContentBlock, FromFile};

// Automatically detects format from extension
let content = ContentBlock::from_file("photo.jpg")?;
let content = ContentBlock::from_file("document.pdf")?;
let content = ContentBlock::from_file("audio.mp3")?;
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
        .send()
        .await
        .unwrap();
    
    assert!(!response.content.is_empty());
}
```

---

**Next**: [4. Configuration](4_configuration.md) | [Examples](examples/)