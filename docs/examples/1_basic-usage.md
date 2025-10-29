# Basic Usage Examples

Essential examples for getting started with cllient.

> **CLI Source**: [`src/bin/cllient.rs`](../../src/bin/cllient.rs) | **Runtime API**: [`src/runtime.rs`](../../src/runtime.rs)

## Installation & Setup

```bash
# Install cllient
cargo install --path .

# Set up environment
cp .env.example .env
# Edit .env with your API keys

# Test installation
cllient list | head -5
```

---

## CLI Examples

### Quick Start

```bash
# List available models
cllient list

# Simple question
cllient ask gpt-4o-mini "What is the capital of France?"

# Streaming response (watch text appear in real-time)
cllient stream deepseek-chat "Tell me a short story about robots"

# Interactive chat
cllient chat claude-3-haiku-20240307
```

### Model Selection

```bash
# List models by provider
cllient list claude
cllient list deepseek
cllient list "gpt"

# Use regex patterns
cllient list ".*-4o.*"        # GPT-4o variants
cllient list "claude-3-.*"    # Claude 3 family
cllient list "^deepseek"      # All DeepSeek models
```

### Common Tasks

```bash
# Code generation
cllient ask deepseek-coder "Write a Python function to calculate factorial"

# Text analysis  
cllient ask claude-3-opus-20240229 "Summarize this text: $(cat article.txt)"

# Creative writing
cllient stream gpt-4-turbo "Write a haiku about programming"

# Quick calculations
cllient ask gpt-4o-mini "What is 15% of 240?"
```

### Comparing Models

```bash
# Compare reasoning across models
cllient compare gpt-4o-mini,claude-3-haiku-20240307 "Explain the difference between AI and ML"

# Test code generation quality
cllient compare deepseek-coder,gpt-4o-mini "Write a binary search in Python"

# Creative writing comparison
cllient compare gpt-4-turbo,claude-3-opus-20240229 "Write a limerick about coffee"
```

---

## Rust API Examples

> **Runtime API**: [`ModelRegistry`](../../src/runtime.rs#L12)

### Basic Usage

```rust
use cllient::{ModelRegistry, ClientError};

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    let registry = ModelRegistry::new()?;
    
    // Simple completion
    let response = registry
        .from_id("gpt-4o-mini")?
        .prompt("Hello, world!")
        .complete()
        .await?;
    
    println!("Response: {}", response.content);
    Ok(())
}
```

### Smart Model Selection

```rust
use cllient::ModelRegistry;

#[tokio::main] 
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;
    
    // Use cheapest Claude model
    let cheap_response = registry
        .use_cheapest("claude-*")?
        .prompt("Simple question: what is 2+2?")
        .complete()
        .await?;
    
    // Use fastest model for quick tasks
    let fast_response = registry
        .use_fastest(".*")?
        .prompt("Quick calculation")
        .complete()
        .await?;
    
    println!("Cheap: {}", cheap_response.content);
    println!("Fast: {}", fast_response.content);
    Ok(())
}
```

### Streaming Responses

```rust
use cllient::ModelRegistry;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;
    
    let mut stream = registry
        .from_id("deepseek-chat")?
        .prompt("Count from 1 to 10")
        .stream()
        .await?;
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        print!("{}", chunk.content);
        std::io::Write::flush(&mut std::io::stdout())?;
    }
    
    Ok(())
}
```

### Configuration with Parameters

```rust
use cllient::ModelRegistry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;
    
    let response = registry
        .from_id("claude-3-opus-20240229")?
        .prompt("Write a creative story")
        .temperature(0.8)      // More creative
        .max_tokens(1000)      // Longer response
        .complete()
        .await?;
    
    println!("Story: {}", response.content);
    Ok(())
}
```

### Error Handling

```rust
use cllient::{ModelRegistry, ClientError};

#[tokio::main]
async fn main() {
    let registry = ModelRegistry::new().unwrap();
    
    match registry.from_id("invalid-model") {
        Ok(builder) => {
            // Use builder...
        },
        Err(ClientError::ModelNotFound(model)) => {
            eprintln!("Model '{}' not found. Available models:", model);
            for model in registry.list_models().iter().take(5) {
                eprintln!("  {}", model);
            }
        },
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Model Information

```rust
use cllient::ModelRegistry;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;
    
    // Get model details
    let model_info = registry.get_model_info("gpt-4o-mini")?;
    
    println!("Model: {}", model_info.model.name);
    println!("Context window: {}", model_info.capabilities.context_window);
    println!("Input cost: ${:.6} per 1K tokens", model_info.pricing.input_per_1k_tokens);
    println!("Output cost: ${:.6} per 1K tokens", model_info.pricing.output_per_1k_tokens);
    
    // List cheap models
    let cheap_models = registry.list_models_matching(".*")?
        .into_iter()
        .filter_map(|id| registry.get_model_info(&id).ok())
        .filter(|info| info.pricing.input_per_1k_tokens < 0.001)
        .take(5)
        .collect::<Vec<_>>();
    
    println!("\nCheap models (< $0.001 per 1K tokens):");
    for model in cheap_models {
        println!("  {} - ${:.6}", model.model.id, model.pricing.input_per_1k_tokens);
    }
    
    Ok(())
}
```

---

## JSON Output Examples

### CLI JSON Output

```bash
# Get model info as JSON
cllient --json list claude | jq '.[0]'

# JSON response from completion
cllient --json ask gpt-4o-mini "What is 2+2?" | jq '.content'

# Streaming with JSON
cllient --json stream deepseek-chat "Count to 5"
```

### Processing JSON with jq

```bash
# Find cheapest models
cllient --json list | jq 'map(select(.pricing.input_per_1k_tokens < 0.001)) | .[].id'

# Models with vision support
cllient --json list | jq 'map(select(.capabilities.vision == true)) | .[].id'

# Sort by context window
cllient --json list | jq 'sort_by(.capabilities.context_window) | reverse | .[0:5] | .[].id'
```

---

## Environment Configuration

### API Key Setup

```bash
# Option 1: Environment variables
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export DEEPSEEK_API_KEY="sk-..."

# Option 2: .env file
echo "OPENAI_API_KEY=sk-..." >> .env
echo "ANTHROPIC_API_KEY=sk-ant-..." >> .env
echo "DEEPSEEK_API_KEY=sk-..." >> .env

# Test configuration
cllient ask gpt-4o-mini "test" && echo "✓ OpenAI working"
cllient ask claude-3-haiku-20240307 "test" && echo "✓ Anthropic working"
cllient ask deepseek-chat "test" && echo "✓ DeepSeek working"
```

### Custom Configuration

```bash
# Use custom config directory
export CLLIENT_CONFIG_DIR="$HOME/.config/cllient"
mkdir -p $CLLIENT_CONFIG_DIR/{service,family}

# Create custom model
cat > $CLLIENT_CONFIG_DIR/family/custom/my-model.yaml << EOF
model:
  id: my-custom-model
  family: custom
  name: My Custom Model
  service: openai

capabilities:
  context_window: 32000
  streaming: true

pricing:
  currency: USD
  input_per_1k_tokens: 0.001
  output_per_1k_tokens: 0.002
EOF

# Test custom model
cllient list custom
cllient ask my-custom-model "Hello custom model"
```

---

## Common Workflows

### Daily AI Assistant

```bash
#!/bin/bash
# ai-helper.sh

# Quick question
ask() {
    cllient ask gpt-4o-mini "$1"
}

# Code help
code() {
    cllient ask deepseek-coder "$1"
}

# Creative writing
write() {
    cllient stream claude-3-opus-20240229 "$1"
}

# Usage
ask "What's the weather API for getting current conditions?"
code "How do I handle JSON parsing errors in Rust?"
write "Write a product description for a smart coffee maker"
```

### Research Assistant

```bash
#!/bin/bash
# research.sh

TOPIC="$1"
if [ -z "$TOPIC" ]; then
    echo "Usage: $0 <research-topic>"
    exit 1
fi

echo "🔍 Researching: $TOPIC"
echo "===================="

echo -e "\n📊 Technical Analysis (DeepSeek):"
cllient ask deepseek-chat "Provide a technical analysis of: $TOPIC"

echo -e "\n🎯 Summary (Claude):"  
cllient ask claude-3-haiku-20240307 "Provide a concise summary of: $TOPIC"

echo -e "\n💡 Creative Perspective (GPT-4):"
cllient ask gpt-4o-mini "Provide a creative perspective on: $TOPIC"
```

### Code Review Helper

```bash
#!/bin/bash
# code-review.sh

FILE="$1"
if [ ! -f "$FILE" ]; then
    echo "Usage: $0 <code-file>"
    exit 1
fi

echo "🔍 Reviewing: $FILE"
echo "=================="

CONTENT=$(cat "$FILE")

echo -e "\n🐛 Bug Analysis:"
cllient ask deepseek-coder "Review this code for bugs and issues: $CONTENT"

echo -e "\n⚡ Performance Review:"
cllient ask gpt-4o-mini "Review this code for performance improvements: $CONTENT"

echo -e "\n📚 Documentation:"
cllient ask claude-3-haiku-20240307 "Suggest documentation improvements for: $CONTENT"
```

---

**Next**: [2. Advanced Scripting](2_advanced-scripting.md) | [2. CLI Usage Guide](../2_cli-usage.md)