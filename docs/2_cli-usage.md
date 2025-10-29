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

# JSON output
cllient --json list
cllient --json list claude

# With verbose output
cllient --verbose list
```

### `list-services` - Show Available Services

List all configured services (HTTP providers).

> **Service Configs**: [`config/service/`](../config/service/)

```bash
# List all configured services
cllient list-services

# JSON output
cllient --json list-services
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

# JSON output
cllient --json ask gpt-4o-mini "What is 2+2?"
```

### `stream` - Real-time Streaming Response

Get responses in real-time as they're generated.

> **Streaming Implementation**: [`src/streaming/`](../src/streaming/)

```bash
# Basic usage  
cllient stream <model> "<prompt>"

# Examples - great for longer responses
cllient stream gpt-4-turbo "Write a detailed essay on climate change"
cllient stream claude-3-5-sonnet-20241022 "Create a recipe for chocolate cake"
cllient stream deepseek-coder "Explain how HTTP works"

# JSON streaming output
cllient --json stream claude-3-haiku "Count to 5"
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

## CLI Flags and Options

### Global Flags

```bash
# Enable verbose/debug logging
cllient --verbose list
cllient -v ask gpt-4o-mini "Hello"

# Output in JSON format
cllient --json list
cllient --json ask gpt-4o-mini "Hello"
cllient --json stream claude-3-haiku-20240307 "Count to 5"

# Combine flags
cllient --verbose --json list claude
```

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

**Next**: [3. API Reference](3_api-reference.md) | [4. Configuration](4_configuration.md)