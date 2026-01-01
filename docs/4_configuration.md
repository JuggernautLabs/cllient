# Configuration Guide

Complete guide to configuring cllient's services, models, and runtime behavior.

> **Source**: [`src/config.rs`](../src/config.rs) | **Embedded Loader**: [`src/embedded_config.rs`](../src/embedded_config.rs)

## 📋 Navigation

### Quick Access
- **[Quick Start](#quick-start)** - Get running in 5 minutes
- **[Environment Setup](#environment-setup)** - API keys and configuration
- **[Provider Examples](#provider-examples)** - OpenAI, Anthropic, DeepSeek setup
- **[Troubleshooting](#troubleshooting)** - Common issues and solutions

### Core Configuration
- **[Configuration System Overview](#configuration-system-overview)** - How the system works
- **[Service Configuration](#service-configuration)** - HTTP communication with providers
- **[Model Configuration](#model-configuration)** - Model capabilities and pricing
- **[Customization](#customization)** - Adding your own services and models

### Advanced Topics
- **[Advanced Configuration](#advanced-configuration)** - Custom SSE parsers and message unpackers
- **[Reference](#reference)** - Complete environment variables and file locations

### By Use Case
- **New User** → [Quick Start](#quick-start) → [Provider Examples](#provider-examples)
- **Custom Provider** → [Adding New Providers](#adding-new-providers) → [Advanced Configuration](#advanced-configuration)
- **Troubleshooting** → [Troubleshooting](#troubleshooting) → [Reference](#reference)
- **Development** → [Customization](#customization) → [Advanced Configuration](#advanced-configuration)

---

## Quick Start

### Installation Check

```bash
# Verify cllient is installed
cllient --version

# Check available models
cllient list | head -5
```

### Basic API Key Setup

```bash
# Copy environment template
cp .env.example .env

# Edit with your API keys
# Required for basic functionality:
OPENAI_API_KEY=sk-proj-your-openai-key-here
ANTHROPIC_API_KEY=sk-ant-api03-your-anthropic-key-here
DEEPSEEK_API_KEY=sk-your-deepseek-key-here
```

### Test Your Setup

```bash
# Test each provider
cllient ask gpt-4o-mini "Hello!" && echo "✅ OpenAI working"
cllient ask claude-3-haiku-20240307 "Hello!" && echo "✅ Anthropic working"  
cllient ask deepseek-chat "Hello!" && echo "✅ DeepSeek working"
```

---

## Configuration System Overview

### How It Works

cllient uses a **layered configuration system** where settings override in this order:

```
1. Embedded configs (339 models across 57 families and 9 providers built into binary)
2. External configs (custom YAML files)  
3. Environment variables (API keys, runtime settings)
```

```bash
# Use custom configuration directory
export CLLIENT_CONFIG_DIR="/path/to/custom/configs"
cllient list
```

### Service vs Model Configs

**Services** define **HOW** to communicate with providers:
- HTTP request templates
- Authentication methods
- Response parsing rules

**Models** define **WHAT** each model can do:
- Context window size
- Capabilities (vision, streaming, etc.)
- Pricing information
- Which service to use

### Override Behavior

Later configurations override earlier ones:

```bash
# Built-in model: gpt-4o-mini (embedded)
# Your custom: gpt-4o-mini.yaml (external) → Overrides built-in
# Runtime: OPENAI_API_KEY (environment) → Overrides both
```

---

## Environment Setup

### API Keys (.env file)

> **Template**: [`.env.example`](../.env.example)

```bash
# Core providers
OPENAI_API_KEY=sk-proj-your-openai-key-here
ANTHROPIC_API_KEY=sk-ant-api03-your-anthropic-key-here
DEEPSEEK_API_KEY=sk-your-deepseek-key-here

# Additional providers (optional)
GOOGLE_AI_API_KEY=your-google-ai-key-here
COHERE_API_KEY=your-cohere-key-here
AZURE_OPENAI_API_KEY=your-azure-key-here
AZURE_OPENAI_ENDPOINT=https://your-resource.openai.azure.com

# Configuration options
CLLIENT_CONFIG_DIR=/path/to/custom/configs
RUST_LOG=info
```

### Custom Config Directory

```bash
# Create custom config structure
mkdir -p ~/.config/cllient/{service,family}

# Set environment variable
export CLLIENT_CONFIG_DIR="$HOME/.config/cllient"

# Make permanent (add to ~/.bashrc or ~/.zshrc)
echo 'export CLLIENT_CONFIG_DIR="$HOME/.config/cllient"' >> ~/.bashrc
```

### Environment Variables Reference

| Variable | Description | Example |
|----------|-------------|---------|
| `CLLIENT_CONFIG_DIR` | Custom config directory | `/home/user/.config/cllient` |
| `RUST_LOG` | Logging level | `info`, `debug`, `trace` |
| `{PROVIDER}_API_KEY` | Provider API keys | `OPENAI_API_KEY=sk-...` |

---

## Service Configuration

> **Location**: [`config/service/`](../config/service/) | **Template Processing**: [`src/template.rs`](../src/template.rs)

### What Are Services

Services define the **HTTP communication layer** with LLM providers. Each service specifies:

- **Base URL** and **endpoints**
- **Authentication** methods  
- **Request templates** with variables
- **Response parsing** rules
- **Streaming** configuration

### Service YAML Structure

```yaml
service:
  name: ProviderName
  base_url: https://api.provider.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Host: api.provider.com
    Authorization: Bearer ${PROVIDER_API_KEY}
    Content-Type: application/json
    
    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "stream": {{stream}},
      "temperature": {{temperature}},
      "max_tokens": {{max_tokens}}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

response:
  success_codes: [200]
  extract:
    content: choices[0].message.content
    usage: usage

message_builder: openai
```

### Common Service Examples

#### OpenAI Service
> **File**: [`config/service/openai.yaml`](../config/service/openai.yaml)

```yaml
service:
  name: OpenAI
  base_url: https://api.openai.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Host: api.openai.com
    Authorization: Bearer ${OPENAI_API_KEY}
    Content-Type: application/json

    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "temperature": {{temperature}},
      "max_tokens": {{max_tokens}},
      "stream": {{stream}}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

message_builder: openai
```

#### Anthropic Service
> **File**: [`config/service/anthropic.yaml`](../config/service/anthropic.yaml)

```yaml
service:
  name: Anthropic
  base_url: https://api.anthropic.com

http:
  request: |
    POST /v1/messages HTTP/1.1
    Host: api.anthropic.com
    x-api-key: ${ANTHROPIC_API_KEY}
    anthropic-version: 2023-06-01
    Content-Type: application/json

    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "max_tokens": {{max_tokens}},
      "stream": {{stream}}
    }

streaming:
  format: text/event-stream
  parser: anthropic_sse

message_builder: anthropic
```

### Template Variables Reference

> **Handlebars Processing**: [`src/template.rs`](../src/template.rs)

| Variable | Type | Description | Example |
|----------|------|-------------|---------|
| `{{model_id}}` | String | Model identifier | `"gpt-4o-mini"` |
| `{{json messages}}` | JSON | Serialized messages | `[{"role":"user","content":"Hi"}]` |
| `{{stream}}` | Boolean | Streaming enabled | `true`, `false` |
| `{{temperature}}` | Number | Response randomness | `0.7` |
| `{{max_tokens}}` | Number | Output limit | `1000` |
| `${API_KEY}` | Environment | Environment variable | `${OPENAI_API_KEY}` |

---

## Model Configuration

> **Location**: [`config/family/`](../config/family/) | **Loading**: [`src/config.rs`](../src/config.rs)

### What Are Models

Models define the **capabilities and properties** of specific LLM models:

- **Service association** (which HTTP service to use)
- **Capabilities** (context window, vision, streaming, etc.)
- **Pricing** information (input/output costs)
- **Default parameters** (temperature, max_tokens, etc.)

### Model YAML Structure

```yaml
model:
  id: model-identifier
  family: model-family  
  name: Human Readable Name
  service: service-name
  status: verified

capabilities:
  context_window: 128000
  max_output_tokens: 4096
  vision: true
  streaming: true
  functions: true
  json_mode: false
  system_prompt: true
  multimodal: true

pricing:
  currency: USD
  input_per_1k_tokens: 0.0025
  output_per_1k_tokens: 0.01
  cached_input_per_1k_tokens: 0.00125

defaults:
  temperature: 1.0
  max_tokens: 4096
  top_p: 1.0
```

### Example: Claude 3 Haiku

> **File**: [`config/family/claude/claude-3-haiku-20240307.yaml`](../config/family/claude/claude-3-haiku-20240307.yaml)

```yaml
model:
  id: claude-3-haiku-20240307
  family: claude
  name: Claude 3 Haiku
  service: anthropic
  status: verified

capabilities:
  context_window: 200000
  max_output_tokens: 4096
  vision: true
  functions: true
  streaming: true
  multimodal: true

pricing:
  currency: USD
  input_per_1k_tokens: 0.00025
  output_per_1k_tokens: 0.00125

defaults:
  temperature: 1.0
  max_tokens: 4096
```

### Capability Flags

| Flag | Description |
|------|-------------|
| `vision` | Supports image input |
| `functions` | Supports function calling |
| `streaming` | Supports real-time streaming |
| `json_mode` | Supports JSON output mode |
| `system_prompt` | Supports system prompts |
| `multimodal` | Supports multiple content types |

### Pricing Information

All pricing is in **USD per 1,000 tokens**:

```yaml
pricing:
  currency: USD
  input_per_1k_tokens: 0.00025    # $0.00025 per 1K input tokens
  output_per_1k_tokens: 0.00125   # $0.00125 per 1K output tokens
  cached_input_per_1k_tokens: 0.000125  # Cached input (if supported)
```

---

## Customization

### Adding Your Own Service

Create `~/.config/cllient/service/myservice.yaml`:

```yaml
service:
  name: MyCustomService
  base_url: https://api.myservice.com

http:
  request: |
    POST /v1/completions HTTP/1.1
    Host: api.myservice.com
    Authorization: Bearer ${MY_SERVICE_API_KEY}
    
    {
      "model": "{{model_id}}",
      "prompt": "{{prompt}}",
      "stream": {{stream}}
    }

streaming:
  format: text/event-stream
  parser: openai_sse

message_builder: openai
```

### Adding Your Own Models

Create `~/.config/cllient/family/custom/my-model.yaml`:

```yaml
model:
  id: my-custom-model
  family: custom
  name: My Custom Model
  service: myservice

capabilities:
  context_window: 32000
  max_output_tokens: 2048
  streaming: true

pricing:
  currency: USD
  input_per_1k_tokens: 0.001
  output_per_1k_tokens: 0.002
```

### Testing Custom Configs

```bash
# Set environment variable for your service
export MY_SERVICE_API_KEY="your-api-key"

# List models (should include your custom model)
cllient list custom

# Test your custom model
cllient ask my-custom-model "Hello world"
```

---

## Provider Examples

### OpenAI Setup

```bash
# 1. Get API key from https://platform.openai.com/api-keys
# 2. Add to .env file
echo "OPENAI_API_KEY=sk-proj-your-key-here" >> .env

# 3. Test
cllient ask gpt-4o-mini "What is 2+2?"
cllient list gpt
```

### Anthropic Setup

```bash
# 1. Get API key from https://console.anthropic.com/
# 2. Add to .env file  
echo "ANTHROPIC_API_KEY=sk-ant-api03-your-key-here" >> .env

# 3. Test
cllient ask claude-3-haiku-20240307 "What is 2+2?"
cllient list claude
```

### DeepSeek Setup

```bash
# 1. Get API key from https://platform.deepseek.com/api_keys
# 2. Add to .env file
echo "DEEPSEEK_API_KEY=sk-your-key-here" >> .env

# 3. Test
cllient ask deepseek-chat "What is 2+2?"
cllient list deepseek
```

### Adding New Providers

**Step 1**: Create service configuration
```bash
# Create service config
cat > ~/.config/cllient/service/newprovider.yaml << EOF
service:
  name: NewProvider
  base_url: https://api.newprovider.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Authorization: Bearer \${NEWPROVIDER_API_KEY}
    
    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "stream": {{stream}}
    }
EOF
```

**Step 2**: Add model configurations
```bash
# Create model config
cat > ~/.config/cllient/family/newprovider/model1.yaml << EOF
model:
  id: newprovider-model-1
  service: newprovider

capabilities:
  context_window: 8192
  streaming: true

pricing:
  currency: USD
  input_per_1k_tokens: 0.002
  output_per_1k_tokens: 0.004
EOF
```

**Step 3**: Set environment and test
```bash
export NEWPROVIDER_API_KEY="your-api-key"
cllient list newprovider
cllient ask newprovider-model-1 "Test message"
```

---

## Advanced Configuration

This section covers **configuration-driven extensibility** - the ability to integrate new LLM providers, custom streaming formats, and unique message protocols **without writing any Rust code**. These features enable cllient to work with any LLM API through pure YAML configuration.


### Custom SSE Parsers

> **Source**: [`src/streaming/sse/providers/`](../src/streaming/sse/providers/) | **Parser Implementation**: [`src/streaming/parsers.rs`](../src/streaming/parsers.rs)

**SSE (Server-Sent Events) parsers** handle real-time streaming responses from LLM providers. Advanced SSE configuration enables support for **any streaming format** through declarative rules.

#### Understanding SSE Formats

Different providers use different SSE streaming formats:

**OpenAI/DeepSeek Format**:
```
data: {"choices":[{"delta":{"content":"hello"}}]}
```

**Anthropic Format**:
```
data: {"delta":{"text":"hello"}}
```

**Google Format**:
```
data: {"candidates":[{"content":{"parts":[{"text":"hello"}]}}]}
```

#### Built-in Parser Types

Available in service configurations:

| Parser | Provider | JSON Path |
|--------|----------|-----------|
| `openai_sse` | OpenAI, DeepSeek | `choices[0].delta.content` |
| `anthropic_sse` | Anthropic | `delta.text` |
| `google_sse` | Google Gemini | `candidates[0].content.parts[0].text` |

#### Creating Custom SSE Parser

**Step 1**: Analyze the provider's SSE format
```bash
# Test the provider's streaming endpoint
curl -N -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"model-name","messages":[{"role":"user","content":"hello"}],"stream":true}' \
  https://api.newprovider.com/v1/chat/completions
```

**Step 2**: Create custom parser configuration

For a hypothetical provider with this format:
```
data: {"response":{"message":{"text":"hello"}}}
```

Add to your service config:
```yaml
service:
  name: NewProvider
  base_url: https://api.newprovider.com

streaming:
  format: text/event-stream
  parser: custom_newprovider_sse
  
  # Custom parser configuration
  custom_parser:
    content_path: "response.message.text"
    done_marker: "[DONE]"
    error_path: "error.message"
    usage_path: "response.usage"
```

**Step 3**: Configure JSON extraction paths

```yaml
streaming:
  parser: custom_newprovider_sse
  extraction_rules:
    # Primary content extraction
    content:
      path: "response.message.text"
      type: "string"
    
    # Completion detection
    done:
      path: "response.finished"
      type: "boolean"
      when: true
    
    # Error handling
    error:
      path: "error"
      type: "object"
      when_present: true
    
    # Usage statistics
    usage:
      path: "response.usage"
      type: "object"
      when_present: true
```

#### Advanced SSE Parser Configuration

**Complex nested extraction**:
```yaml
streaming:
  parser: advanced_custom_sse
  extraction_rules:
    content:
      # Handle multiple content types
      paths:
        - "choices[0].delta.content"          # Text content
        - "choices[0].delta.tool_calls"       # Function calls
        - "choices[0].delta.function_call"    # Legacy function calls
      
    metadata:
      # Extract additional metadata
      model: "model"
      created: "created"
      id: "id"
      
    finish_reason:
      path: "choices[0].finish_reason"
      done_values: ["stop", "length", "function_call"]
```

**Error handling and retries**:
```yaml
streaming:
  parser: robust_custom_sse
  error_handling:
    retry_on_parse_error: true
    max_retries: 3
    backoff_ms: [100, 500, 1000]
    
    # Handle malformed JSON
    skip_invalid_json: true
    log_parse_errors: true
    
    # Timeout configuration
    chunk_timeout_ms: 5000
    total_timeout_ms: 30000
```


### Configuration-Driven Parser Development

#### Parser Registration

Add new parsers without code changes by registering them in configuration:

```yaml
# config/parsers/custom_parsers.yaml
parsers:
  custom_newprovider_sse:
    type: "sse"
    implementation: "json_path"
    config:
      content_path: "response.message.text"
      done_marker: "[DONE]"
      error_path: "error.message"
```

#### Testing Custom Parsers

```bash
# Test SSE parser
echo 'data: {"response":{"message":{"text":"hello"}}}' | \
  cllient test-parser custom_newprovider_sse

# Integration test
cllient ask custom-model "Hello" --debug-parser
```

#### Parser Development Workflow

**1. Analyze Provider Format**:
```bash
# Capture actual API responses
curl -v -N [provider-endpoint] > provider_responses.jsonl

# Analyze patterns
grep "data:" provider_responses.jsonl | head -10
```

**2. Create Parser Configuration**:
```yaml
# Start with basic config
streaming:
  parser: custom_provider_sse
  content_path: "data.message.content"
```

**3. Test and Iterate**:
```bash
# Test with real provider
RUST_LOG=cllient::streaming=debug cllient stream custom-model "test"

# Verify parsing
cllient --json stream custom-model "test" | jq '.content'
```

**4. Handle Edge Cases**:
```yaml
streaming:
  parser: custom_provider_sse
  edge_cases:
    empty_content: "skip"      # or "placeholder", "error"
    malformed_json: "log"      # or "skip", "error"
    missing_fields: "default"  # or "error", "skip"
    
  fallbacks:
    - parser: "openai_sse"     # Fallback parser
    - content_path: "text"     # Alternative extraction
```

---

## Reference

### Complete Environment Variables

```bash
# Core Provider API Keys
OPENAI_API_KEY=sk-proj-...
ANTHROPIC_API_KEY=sk-ant-api03-...
DEEPSEEK_API_KEY=sk-...
GOOGLE_AI_API_KEY=...
COHERE_API_KEY=...

# Azure OpenAI
AZURE_OPENAI_API_KEY=...
AZURE_OPENAI_ENDPOINT=https://....openai.azure.com

# cllient Configuration
CLLIENT_CONFIG_DIR=/path/to/custom/configs

# Logging and Debug
RUST_LOG=info                    # error, warn, info, debug, trace
CLLIENT_VERBOSE=true            # Enable verbose output
```

### Configuration File Locations

```
# Embedded (built-in)
<binary>/config/service/*.yaml
<binary>/config/family/**/*.yaml

# External (custom)
$CLLIENT_CONFIG_DIR/service/*.yaml
$CLLIENT_CONFIG_DIR/family/**/*.yaml

# Default external location
~/.config/cllient/service/*.yaml
~/.config/cllient/family/**/*.yaml
```

### Common Patterns

**Multiple environments**:
```bash
# Development
export CLLIENT_CONFIG_DIR="./configs/dev"

# Production  
export CLLIENT_CONFIG_DIR="./configs/prod"

# Testing
export CLLIENT_CONFIG_DIR="./configs/test"
```

**Service variants**:
```bash
# config/service/openai-dev.yaml    (development endpoint)
# config/service/openai-prod.yaml   (production endpoint)
# config/service/openai-azure.yaml  (Azure OpenAI)
```

---

## Troubleshooting

### Configuration Not Loading

**Check configuration directory**:
```bash
echo $CLLIENT_CONFIG_DIR
ls -la $CLLIENT_CONFIG_DIR/service/
ls -la $CLLIENT_CONFIG_DIR/family/
```

**Validate YAML syntax**:
```bash
# Test YAML file
python -c "import yaml; yaml.safe_load(open('config.yaml'))" && echo "✅ Valid YAML"
```

### API Key Issues

**Check environment variables**:
```bash
echo $OPENAI_API_KEY | head -c 20      # Should start with sk-proj- or sk-
echo $ANTHROPIC_API_KEY | head -c 20   # Should start with sk-ant-
echo $DEEPSEEK_API_KEY | head -c 10    # Should start with sk-
```

**Test API connectivity**:
```bash
# Test with curl
curl -H "Authorization: Bearer $OPENAI_API_KEY" https://api.openai.com/v1/models
```

### Model Not Found

**List available models**:
```bash
cllient list | grep -i "model-name"
cllient list provider-name
```

**Check model configuration**:
```bash
cllient --json list | jq '.[] | select(.id == "model-name")'
```

### Getting Help

**Enable debug logging**:
```bash
RUST_LOG=debug cllient --verbose ask gpt-4o-mini "test"
```

**Check specific components**:
```bash
RUST_LOG=cllient::config=debug cllient list
RUST_LOG=cllient::client=debug cllient ask model "test"
```

**Verify installation**:
```bash
cllient --version
cllient list-services
cllient list | head -5
```

---

**Next**: [5. Architecture](5_architecture.md) | [3. API Reference](3_api-reference.md)