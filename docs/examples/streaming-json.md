# Streaming JSON Output Examples

This guide demonstrates the streaming JSON output feature in cllient, which outputs JSON structures incrementally as they're being built.

## Overview

The streaming JSON feature provides two output modes:

1. **JSON Mode** (default) - Streams valid JSON structure as it's built
2. **Pretty Mode** (`--pretty` flag) - Human-readable output with emojis

## CLI Examples

### Basic Streaming JSON

The default behavior outputs JSON that streams in real-time:

```bash
cllient stream deepseek-chat "Count to 5"
```

Output (streams incrementally):
```json
{
  "model": "deepseek-chat",
  "prompt": "Count to 5",
  "response": "1, 2, 3, 4, 5",
  "streamed": true,
  "success": true
}
```

The `response` field populates character-by-character as tokens arrive from the LLM!

### Pretty Mode for Terminal Use

For interactive use in terminals:

```bash
cllient --pretty stream deepseek-chat "Write a haiku"
```

Output:
```
🤖 Model: deepseek-chat
💭 Prompt: Write a haiku
📡 Streaming response:

Silent code flows deep
Logic weaves through midnight streams
Bugs await the dawn
```

### Piping and Processing

JSON mode is perfect for automation:

```bash
# Extract just the response field
cllient stream gpt-4o-mini "Say hello" | jq -r '.response'

# Save full JSON response
cllient stream claude-3-haiku "Explain AI" > response.json

# Chain multiple operations
cllient stream deepseek-chat "Random number" | \
  jq -r '.response' | \
  xargs -I {} echo "The LLM said: {}"
```

### Debugging with Verbose Mode

See what's happening under the hood:

```bash
cllient --verbose stream deepseek-chat "Test"
```

Debug output shows:
```
DEBUG cllient::streaming_json: Initializing streaming JSON object
DEBUG field_string{key="model"}: Writing string field key=model value_len=13
DEBUG field_streaming_string{key="response"}: Starting streaming string field
```

## Rust API Examples

### Basic Usage

```rust
use cllient::streaming_json::StreamingJsonObject;

fn main() -> std::io::Result<()> {
    let mut json = StreamingJsonObject::new()?;

    json.field_string("status", "success")?;
    json.field_bool("ready", true)?;

    json.close()?;

    // Output:
    // {
    //   "status": "success",
    //   "ready": true
    // }

    Ok(())
}
```

### Streaming LLM Responses

```rust
use cllient::{ModelRegistry, streaming_json::StreamingJsonObject};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    let mut json = StreamingJsonObject::new()?;
    json.field_string("model", "gpt-4o-mini")?;
    json.field_string("prompt", "Tell me a joke")?;

    // Get streaming response
    let mut stream = registry
        .from_id("gpt-4o-mini")?
        .prompt("Tell me a joke")
        .stream()
        .await?;

    // Start streaming response field
    let mut response = json.field_streaming_string("response")?;

    // Stream each chunk
    while let Some(chunk) = stream.next().await {
        match chunk? {
            StreamEvent::Content(text) => {
                response.write_chunk(&text)?;
            },
            StreamEvent::Done => break,
            _ => {}
        }
    }

    response.close()?;
    json.field_bool("success", true)?;
    json.close()?;

    Ok(())
}
```

### Simulating Slow Streaming

See the incremental output clearly:

```rust
use cllient::streaming_json::StreamingJsonObject;
use std::thread;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let mut json = StreamingJsonObject::new()?;
    json.field_string("demo", "slow-stream")?;

    let mut response = json.field_streaming_string("message")?;

    let words = vec!["Hello", " ", "world", "!", " ", "Streaming", " ", "JSON", "!"];
    for word in words {
        response.write_chunk(word)?;
        thread::sleep(Duration::from_millis(200));
    }

    response.close()?;
    json.field_bool("complete", true)?;
    json.close()?;

    Ok(())
}
```

## Advanced Examples

### Error Handling

```rust
let mut json = StreamingJsonObject::new()?;
json.field_string("model", "gpt-4o-mini")?;

match stream_response().await {
    Ok(content) => {
        let mut response = json.field_streaming_string("response")?;
        response.write_chunk(&content)?;
        response.close()?;
        json.field_bool("success", true)?;
    },
    Err(e) => {
        json.field_bool("success", false)?;
        json.field_string("error", &e.to_string())?;
    }
}

json.close()?;
```

### Custom JSON Structures

```rust
let mut json = StreamingJsonObject::new()?;

json.field_string("version", "1.0")?;
json.field_bool("experimental", true)?;

let mut data = json.field_streaming_string("data")?;
data.write_chunk("First chunk")?;
data.write_chunk(" - Second chunk")?;
data.close()?;

json.field_string("timestamp", &current_timestamp())?;
json.close()?;
```

## Example Files

The repository includes complete working examples:

- **[`examples/test_streaming_json.rs`](../../examples/test_streaming_json.rs)** - Basic streaming JSON tests
- **[`examples/test_streaming_timing.rs`](../../examples/test_streaming_timing.rs)** - Timing demonstrations

Run them with:

```bash
cargo run --example test_streaming_json
cargo run --example test_streaming_timing
```

## Implementation Details

### How It Works

The streaming JSON writer is a **state machine** that:

1. **Initializes**: Outputs `{` immediately
2. **Writes Fields**: Outputs each field as added
3. **Streams Content**: For streaming fields, outputs character-by-character
4. **Closes**: Outputs `}` when done

All output is **flushed immediately** to stdout, ensuring real-time visibility.

### JSON Escaping

Special characters are automatically escaped:

| Input | Output |
|-------|--------|
| `"Hello"` | `\"Hello\"` |
| `Line 1\nLine 2` | `Line 1\\nLine 2` |
| `C:\path` | `C:\\path` |
| `Tab\there` | `Tab\\there` |

### Tracing

Enable debug logging to see internal operations:

```bash
# Full debug output
RUST_LOG=debug cllient --verbose stream model "prompt"

# Just streaming JSON module
RUST_LOG=cllient::streaming_json=debug cllient stream model "prompt"

# Trace level (shows every chunk)
RUST_LOG=cllient::streaming_json=trace cllient stream model "prompt"
```

## Best Practices

1. **Use JSON mode for automation** - Easy to parse with `jq` or programming languages
2. **Use pretty mode for terminals** - Better user experience with emojis
3. **Enable verbose for debugging** - See exactly what's happening
4. **Flush frequently** - Already handled automatically by the API
5. **Handle errors gracefully** - Always close objects even on errors

## Related Documentation

- **[CLI Usage Guide](../2_cli-usage.md)** - Complete CLI reference
- **[API Reference](../3_api-reference.md#streaming-json-output-api)** - Full API docs
- **[Architecture Guide](../5_architecture.md#6-streaming-json-output-system)** - System design
- **[Source Code](../../src/streaming_json.rs)** - Implementation

---

**Next**: [Back to Examples Index](../README.md)
