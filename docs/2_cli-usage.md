# CLI Usage Guide

Complete reference for using cllient from the command line.

> **Source Code**: The CLI is implemented in [`src/bin/cllient.rs`](../src/bin/cllient.rs)

## Installation

```bash
# Install from source
cargo install --path .

# Or run directly during development
cargo run --bin cllient -- <command>
```

## Basic Commands

### `list` - Show Available Models

List all configured models grouped by provider.

> **Implementation**: [`list command handler`](../src/bin/cllient.rs)

```bash
# List all configured models grouped by provider
cllient list

# Filter by provider name(s)
cllient list claude
cllient list "gpt,claude,deepseek"

# Use regex patterns
cllient list "o[0-9]"           # o1, o3, o4 models
cllient list ".*-4.*"           # models with '4' in family
cllient list "^(gpt|claude)$"   # exact matches

# JSON output (default when --pretty is not used)
cllient list
cllient list claude

# With verbose output
cllient --verbose list
```

### `list-services` - Show Available Services

List all configured services (HTTP providers).

> **Service Configs**: [`config/service/`](../config/service/)

```bash
# List all configured services
cllient list-services

# JSON output (default)
cllient list-services
```

### `ask` - Single Completion Request

Send a single prompt and get a complete response.

> **Request Types**: [`CompletionRequest`](../src/types.rs)

```bash
# Basic usage
cllient ask <model> "<prompt>"

# Examples
cllient ask gpt-4o-mini "What is the capital of France?"
cllient ask claude-3-opus-20240229 "Explain machine learning"
cllient ask deepseek-chat "Write a Python function to sort a list"

# JSON output (default)
cllient ask gpt-4o-mini "What is 2+2?"
```

### `stream` - Real-time Streaming Response

Get responses in real-time as they're generated. The stream command supports two output modes: JSON (default) and Pretty (human-readable).

> **Streaming Implementation**: [`src/streaming/`](../src/streaming/)
> **JSON Output**: [`src/streaming_json.rs`](../src/streaming_json.rs)

#### JSON Mode (Default) - Streaming JSON Structure

The default output mode streams a valid JSON structure as it's being built:

```bash
# Default JSON streaming - see the response field populate in real-time
cllient stream deepseek-chat "Count to 5"

# Output (streaming as it arrives):
{
  "model": "deepseek-chat",
  "prompt": "Count to 5",
  "response": "1, 2, 3, 4, 5",
  "streamed": true,
  "success": true
}
```

The JSON structure is output incrementally - you'll see the response field being filled character-by-character as tokens arrive from the LLM.

#### Pretty Mode - Human-Readable Output

For interactive terminal use, add the `--pretty` flag for formatted output with emojis:

```bash
# Human-readable output with decorations
cllient --pretty stream gpt-4-turbo "Write a haiku about code"

# Output:
🤖 Model: gpt-4-turbo
💭 Prompt: Write a haiku about code
📡 Streaming response:

Lines of logic flow
Silent commands shape the world
Code breathes life anew
```

#### Examples

```bash
# Long-form responses work great with streaming
cllient stream gpt-4-turbo "Write a detailed essay on climate change"
cllient stream claude-3-5-sonnet-20241022 "Create a recipe for chocolate cake"
cllient stream deepseek-coder "Explain how HTTP works"

# JSON mode for automation/parsing
cllient stream claude-3-haiku "Count to 5" | jq -r '.response'

# Pretty mode for terminal viewing
cllient --pretty stream deepseek-chat "Tell me a joke"
```

### `chat` - Interactive Conversation

Start an interactive chat session with a model.

> **Chat Implementation**: [`src/chat.rs`](../src/chat.rs)

```bash
# Start interactive chat
cllient chat <model>

# Examples
cllient chat gpt-4o-mini
cllient chat claude-3-haiku-20240307
cllient chat deepseek-chat

# In chat mode:
# - Type messages and press Enter
# - Type 'quit' to exit
# - Type 'clear' to reset conversation history
```

### `compare` - Compare Multiple Models

Compare responses from multiple models with the same prompt.

```bash
# Compare 2+ models with same prompt
cllient compare <model1,model2,...> "<prompt>"

# Examples
cllient compare gpt-4o-mini,claude-3-haiku-20240307 "What makes a good API?"
cllient compare gpt-3.5-turbo,claude-3-opus-20240229,deepseek-chat "Explain recursion"
cllient compare gpt-4-turbo,claude-3-5-sonnet-20241022 "Write a haiku about programming"
```

### `debug-response` - Debug API Response Issues

Debug and analyze API responses when troubleshooting issues with specific models.

> **Implementation**: [`debug_raw_response handler`](../src/bin/cllient.rs)

```bash
# Debug a specific model's API response
cllient debug-response <model> "<prompt>"

# Examples
cllient debug-response gpt-4o-mini "Say hello"
cllient debug-response claude-3-haiku-20240307 "Test"

# Output includes:
# - Request status (success/failure)
# - Response content
# - Token usage statistics
# - Detailed error analysis with troubleshooting hints
```

This command provides detailed diagnostics including:
- JSON path errors (API response structure mismatches)
- Authentication errors (401)
- Not found errors (404)
- Rate limit errors (429)
- General connectivity troubleshooting

## CLI Flags and Options

### Global Flags

```bash
# Enable verbose/debug logging
cllient --verbose list
cllient -v ask gpt-4o-mini "Hello"

# Human-readable pretty output (with emojis)
cllient --pretty list
cllient --pretty ask gpt-4o-mini "Hello"
cllient --pretty stream claude-3-haiku-20240307 "Count to 5"

# Clean output (LLM response only, no formatting - ideal for piping)
cllient --clean ask gpt-4o-mini "Hello"
cllient --clean stream deepseek-chat "Count to 5" | wc -w

# Combine flags
cllient --verbose --pretty list claude
```

**Output Modes:**
- Default: JSON output (structured, machine-readable)
- `--pretty`: Human-readable output with emoji decorators
- `--clean`: Raw LLM response only (no formatting, perfect for piping)

**Note**: `--pretty` and `--clean` are mutually exclusive. If neither is specified, JSON output is used.

### Environment Variables

> **Config Loading**: [`src/config.rs`](../src/config.rs)

```bash
# Use custom config directory
CLLIENT_CONFIG_DIR=/path/to/configs cllient list

# Set API keys
OPENAI_API_KEY=your_key cllient ask gpt-4o-mini "Hello"
ANTHROPIC_API_KEY=your_key cllient ask claude-3-haiku-20240307 "Hello"
DEEPSEEK_API_KEY=your_key cllient ask deepseek-chat "Hello"

# Debug logging
RUST_LOG=debug cllient --verbose stream gpt-4o-mini "Hello"
```

## Advanced Usage

### Piping and Redirection

```bash
# Pipe input from file
cat prompt.txt | cllient stream gpt-4o-mini

# Save output to file
cllient ask claude-3-opus-20240229 "Explain AI" > response.txt

# Chain with other commands
echo "Summarize this: $(cat article.txt)" | cllient ask gpt-4-turbo
```

### Batch Processing

```bash
# Process multiple prompts
for prompt in "What is AI?" "Explain ML" "Define NLP"; do
  echo "Q: $prompt"
  cllient ask deepseek-chat "$prompt"
  echo "---"
done

# Compare models on multiple topics
models="gpt-4o-mini,claude-3-haiku-20240307"
for topic in "AI" "ML" "Blockchain"; do
  cllient compare $models "Explain $topic briefly"
done
```

### Error Handling and Debugging

```bash
# Check if model exists
cllient ask invalid-model "test" || echo "Model not found"

# Debug API calls
RUST_LOG=debug cllient --verbose stream gpt-4o-mini "Hello"

# Test connectivity
cllient ask deepseek-chat "Say OK" && echo "✓ Working"
```

## Production Usage

### Shell Script Integration

```bash
#!/bin/bash
# ask_ai.sh - Simple AI assistant script

MODEL=${1:-"deepseek-chat"}
PROMPT="$2"

if [ -z "$PROMPT" ]; then
    echo "Usage: $0 [model] <prompt>"
    echo "Available models:"
    cllient list | grep "  " | head -10
    exit 1
fi

cllient ask "$MODEL" "$PROMPT"
```

### Configuration Management

```bash
# Use different config sets
export CLLIENT_CONFIG_DIR="./configs/production"
cllient list

export CLLIENT_CONFIG_DIR="./configs/development"  
cllient ask gpt-4o-mini "test"
```

### Performance Testing

```bash
# Test response times
time cllient ask deepseek-chat "What is 2+2?"

# Test streaming vs non-streaming
time cllient ask gpt-4o-mini "Count to 10"
time cllient stream gpt-4o-mini "Count to 10"
```

## Available Models

### Popular Models by Provider

#### OpenAI
- `gpt-4o`, `gpt-4o-mini` - Latest multimodal models
- `gpt-4`, `gpt-3.5-turbo` - Classic models
- `o1`, `o1-mini` - Reasoning models (no streaming)
- `gpt-4.1-nano` - Ultra low-cost option

#### Anthropic
- `claude-3-opus-20240229` - Highest capability
- `claude-3-5-sonnet-20241022` - Best balance
- `claude-3-5-haiku-20241022` - Fast and cheap
- `claude-3-haiku-20240307` - Ultra low-cost

#### DeepSeek
- `deepseek-chat` - General purpose ($0.014/$0.028 per 1M tokens)
- `deepseek-coder` - Code-specialized
- `deepseek-v3` - Latest version

> **All Models**: Run `cllient list` to see all 330+ available models
> 
> **Model Configs**: See [`config/family/`](../config/family/) for detailed model configurations

## Troubleshooting

### Common Issues

**Command not found**: Ensure cllient is installed (`cargo install --path .`)

**API key errors**: Check your `.env` file or environment variables

**Model not found**: Use `cllient list` to see available models

**Network issues**: Use `--verbose` flag for debugging

**JSON parsing errors**: Ensure you're using `--json` flag correctly

### Getting Help

```bash
# Show all available commands
cllient --help

# Show help for specific command
cllient ask --help
cllient stream --help
```

---

## cllient-registry - Registry Validation and Inspection

The `cllient-registry` CLI provides tools for validating and inspecting the model registry.

> **Source Code**: [`src/bin/cllient-registry.rs`](../src/bin/cllient-registry.rs)

### Installation

```bash
cargo install --path . --bin cllient-registry
```

### Global Options

```bash
# Output format (table, json, yaml)
cllient-registry --format json <command>
cllient-registry -f yaml <command>
```

### `validate` - Validate Registry Configurations

Run validation checks on registry configurations.

```bash
# Validate all configs with all levels
cllient-registry validate

# Validate with specific levels
cllient-registry validate --level schema
cllient-registry validate --level crossref
cllient-registry validate --level semantic
cllient-registry validate --level live
cllient-registry validate --level schema,crossref

# Validate a specific model
cllient-registry validate --model gpt-4o-mini

# Validate a specific service
cllient-registry validate --service openai

# JSON output for CI/CD
cllient-registry --format json validate
```

**Validation Levels:**
- `schema` - YAML schema validation
- `crossref` - Cross-reference validation (model-service links)
- `semantic` - Semantic validation (consistent naming, pricing)
- `live` - Live API testing
- `all` - All levels except live (default)

### `stats` - Show Registry Statistics

Display registry index statistics.

```bash
cllient-registry stats

# Output:
# Registry Statistics
# ===================
# Services:          15
#   Verified:        8
#   Unverified:      7
# Models:            330
#   Verified:        45
#   Unverified:      285
# Families:          25
# Orphan Services:   0
# Broken References: 0
```

### `index` - Show Service-Model Mappings

Display the full bidirectional index.

```bash
cllient-registry index

# JSON output
cllient-registry --format json index
```

### `orphans` - List Orphan Services

Find services that have no models configured.

```bash
cllient-registry orphans
```

### `broken` - List Broken References

Find models that reference non-existent services.

```bash
cllient-registry broken
```

### `models-for` - List Models for a Service

Get all models that use a specific service.

```bash
cllient-registry models-for openai
cllient-registry models-for anthropic
cllient-registry --format json models-for deepseek
```

### `service-for` - Get Service for a Model

Find which service a model uses.

```bash
cllient-registry service-for gpt-4o-mini
cllient-registry service-for claude-3-opus-20240229
```

### `search` - Search for Models

Search for models with various filters.

```bash
# Basic search
cllient-registry search gpt

# Fuzzy matching
cllient-registry search --fuzzy claude

# Filter by verified status
cllient-registry search --verified gpt

# Filter by service
cllient-registry search --service openai gpt

# Filter by family
cllient-registry search --family claude opus

# Limit results
cllient-registry search --limit 10 gpt

# Combined filters
cllient-registry search --verified --service openai --limit 5 gpt
```

### `services` - List All Services

List all services with their model counts and verification status.

```bash
cllient-registry services

# Output:
# Services (15):
#   ✓ openai                        85 models
#   ✓ anthropic                     24 models
#   ? deepseek                      12 models
```

**Status indicators:**
- `✓` - Verified
- `?` - Unverified
- `✗` - Broken
- `⚠` - Deprecated
- `!` - Error
- `∅` - Not Found

### `families` - List All Families

List all model families with their model counts.

```bash
cllient-registry families

# Output:
# Families (25):
#   gpt                              45 models
#   claude                           24 models
#   deepseek                         12 models
```

### `verify` - Verify Services/Models

Test API connectivity to verify services and models work correctly.

```bash
# Verify a specific service
cllient-registry verify --service openai

# Verify a specific model
cllient-registry verify --model gpt-4o-mini

# Verify all unverified services
cllient-registry verify --all-services

# Verify all unverified models (for verified services)
cllient-registry verify --all-models

# Limit models tested per service
cllient-registry verify --all-services --max-per-service 5

# Dry run - show what would be tested
cllient-registry verify --all-services --dry-run

# Update config files with verification results
cllient-registry verify --service openai --update-configs
```

---

## cllient-hub - Hub Server

The `cllient-hub` CLI runs a standalone hub server that exposes the ModelRegistry over JSON-RPC.

> **Source Code**: [`src/bin/cllient-hub.rs`](../src/bin/cllient-hub.rs)

### Installation

```bash
cargo install --path . --bin cllient-hub
```

### Usage

```bash
# Start with default settings (127.0.0.1:8080)
cllient-hub

# Bind to a specific address
cllient-hub --bind 127.0.0.1:8080
cllient-hub --bind 0.0.0.0:3000

# Use custom config directory (not yet fully supported)
cllient-hub --config ./my-configs

# Only load verified models
cllient-hub --verified-only
```

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--bind` | `-b` | Address to bind the server to | `127.0.0.1:8080` |
| `--config` | `-c` | Config directory (uses embedded if not specified) | embedded |
| `--verified-only` | | Only load verified models | false |

### Startup Output

```
cllient-hub starting
  Bind address: 127.0.0.1:8080
  Models: 330 total, 45 verified
  Services: 15

  openai (85 models)
  anthropic (24 models)
  deepseek (12 models)
  ...
```

---

## cllient-utils - Utility Commands

The `cllient-utils` CLI provides utility commands for testing and debugging.

> **Source Code**: [`src/bin/cllient-utils.rs`](../src/bin/cllient-utils.rs)

### Installation

```bash
cargo install --path . --bin cllient-utils
```

### `test-family` - Test All Models in a Family

Test all models belonging to a specific family.

```bash
# Test all models in a family
cllient-utils test-family claude
cllient-utils test-family gpt
cllient-utils test-family deepseek

# Test and update config files with status
cllient-utils test-family claude --update-status
```

**Features:**
- Runs tests in parallel (up to 5 concurrent requests)
- Tests each model with a simple "respond with just hi" prompt
- Reports success/failure status for each model
- Optionally updates model config files with verification status

**Status values when using `--update-status`:**
- `verified` - Model responded successfully
- `auth_error` - Authentication failure
- `not_found` - Model not found or deprecated
- `access_restricted` - Organization verification required
- `special_requirements` - Model requires special input (e.g., audio)
- `endpoint_error` - Unsupported parameter or endpoint issue
- `error` - General error

### `debug-families` - Show Available Families

Debug command to show all available model families and their models.

```bash
cllient-utils debug-families

# Output:
# Available families:
#   - gpt
#     Models (45): ["gpt-4o", "gpt-4o-mini", ...]
#   - claude
#     Models (24): ["claude-3-opus-20240229", ...]
#
# Looking for claude models:
# Claude family models: ["claude-3-opus-20240229", ...]
#
# All models (first 10):
#   1: gpt-4o (family: gpt)
#   2: gpt-4o-mini (family: gpt)
#   ...
```

---

**Next**: [3. API Reference](3_api-reference.md) | [4. Configuration](4_configuration.md)