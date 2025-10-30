# Message Builder API Examples

The Message Builder API allows you to construct complex multi-turn conversations with custom roles and multimodal content.

## Table of Contents

1. [Basic Message Creation](#basic-message-creation)
2. [Multi-Turn Conversations](#multi-turn-conversations)
3. [Setting Messages from Arrays](#setting-messages-from-arrays)
4. [Custom Roles](#custom-roles)
5. [Multimodal Messages](#multimodal-messages)
6. [Complete Examples](#complete-examples)

---

## Basic Message Creation

### Simple User Message

```rust
use cllient::{ModelRegistry, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    let response = registry
        .from_id("gpt-4o-mini")?
        .append_message(Message::user("What is Rust?"))
        .send()
        .await?;

    println!("{}", response.text());
    Ok(())
}
```

### System + User Messages

```rust
let response = registry
    .from_id("claude-3-haiku-20240307")?
    .append_message(Message::system("You are a helpful coding assistant."))
    .append_message(Message::user("Explain async/await in Rust"))
    .send()
    .await?;
```

---

## Multi-Turn Conversations

### Building Context with Previous Messages

```rust
let response = registry
    .from_id("gpt-4o-mini")?
    .append_message(Message::user("What is 2+2?"))
    .append_message(Message::assistant("2+2 equals 4."))
    .append_message(Message::user("And what's 4+4?"))
    .send()
    .await?;

println!("{}", response.text());
// Expected: "4+4 equals 8."
```

### Multi-Turn with System Prompt

```rust
let response = registry
    .from_id("deepseek-chat")?
    .append_message(Message::system("You are a math tutor. Be concise."))
    .append_message(Message::user("What's a prime number?"))
    .append_message(Message::assistant("A prime number is only divisible by 1 and itself."))
    .append_message(Message::user("Is 17 prime?"))
    .send()
    .await?;
```

---

## Setting Messages from Arrays

Instead of chaining multiple `.append_message()` calls, you can set all messages at once from a `Vec<Message>`.

### Basic Array Usage

```rust
use cllient::{ModelRegistry, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    let conversation = vec![
        Message::system("You are a helpful assistant"),
        Message::user("What is 2+2?"),
        Message::assistant("4"),
        Message::user("And 4+4?"),
    ];

    let response = registry
        .from_id("gpt-4o-mini")?
        .messages(conversation)
        .send()
        .await?;

    println!("{}", response.text());
    Ok(())
}
```

### Building Conversations Dynamically

```rust
let mut msgs = vec![Message::system("You are a coding assistant")];

// Add conversation history from database or state
let history = vec![
    ("user", "Explain Rust"),
    ("assistant", "Rust is a systems programming language..."),
    ("user", "What about ownership?"),
    ("assistant", "Ownership is Rust's key feature..."),
];

for (role, text) in history {
    msgs.push(match role {
        "user" => Message::user(text),
        "assistant" => Message::assistant(text),
        _ => Message::custom(role).add_text(text),
    });
}

// Add current user question
msgs.push(Message::user("Tell me more about borrowing"));

let response = registry
    .from_id("deepseek-chat")?
    .messages(msgs)
    .send()
    .await?;
```

### Loading from Chat History

```rust
use cllient::{Message, ModelRegistry};

struct ChatMessage {
    role: String,
    content: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simulate loading from database
    let chat_history = vec![
        ChatMessage { role: "system".into(), content: "You are helpful".into() },
        ChatMessage { role: "user".into(), content: "Hi there".into() },
        ChatMessage { role: "assistant".into(), content: "Hello!".into() },
    ];

    // Convert to Message array
    let messages: Vec<Message> = chat_history
        .into_iter()
        .map(|msg| match msg.role.as_str() {
            "user" => Message::user(&msg.content),
            "assistant" => Message::assistant(&msg.content),
            "system" => Message::system(&msg.content),
            role => Message::custom(role).add_text(&msg.content),
        })
        .collect();

    let registry = ModelRegistry::new()?;
    let response = registry
        .from_id("gpt-4o-mini")?
        .messages(messages)
        .send()
        .await?;

    Ok(())
}
```

### Combining with Other Parameters

```rust
let conversation = vec![
    Message::system("Be concise"),
    Message::user("Explain async in Rust"),
];

let response = registry
    .from_id("claude-3-haiku-20240307")?
    .messages(conversation)
    .temperature(0.7)
    .max_tokens(500)
    .send()
    .await?;
```

---

## Custom Roles

### Role-Playing Scenarios

```rust
let response = registry
    .from_id("claude-3-opus-20240229")?
    .append_message(Message::system("You are facilitating a conversation between characters."))
    .append_message(Message::custom("narrator").add_text("Once upon a time in a mystical forest..."))
    .append_message(Message::custom("elmo").add_text("Elmo loves exploring!"))
    .append_message(Message::custom("wizard").add_text("Young one, beware of the dragon!"))
    .append_message(Message::user("Continue the story"))
    .send()
    .await?;
```

### Interview Simulation

```rust
let response = registry
    .from_id("gpt-4o")?
    .append_message(Message::custom("interviewer").add_text("Tell me about your experience with Rust."))
    .append_message(Message::custom("candidate").add_text("I've been using Rust for 3 years..."))
    .append_message(Message::custom("interviewer").add_text("What's your favorite feature?"))
    .append_message(Message::user("Generate the candidate's response"))
    .send()
    .await?;
```

---

## Multimodal Messages

### Image Analysis

```rust
use cllient::{Message, ContentBlock, ImageFormat};

let image_data = std::fs::read("photo.jpg")?;

let response = registry
    .from_id("gpt-4o")?
    .append_message(
        Message::user("What's in this image?")
            .add_image(image_data, ImageFormat::Jpeg)
    )
    .send()
    .await?;
```

### Multiple Images

```rust
let photo1 = std::fs::read("before.jpg")?;
let photo2 = std::fs::read("after.jpg")?;

let response = registry
    .from_id("gpt-4o")?
    .append_message(
        Message::user("Compare these two images")
            .add_image(photo1, ImageFormat::Jpeg)
            .add_image(photo2, ImageFormat::Jpeg)
    )
    .send()
    .await?;
```

### Image URL Reference

```rust
let response = registry
    .from_id("claude-3-opus-20240229")?
    .append_message(
        Message::user("Describe this image")
            .add_image_url("https://example.com/photo.jpg")
    )
    .send()
    .await?;
```

### Audio Transcription

```rust
use cllient::AudioFormat;

let audio_data = std::fs::read("recording.wav")?;

let response = registry
    .from_id("gpt-4o")?
    .append_message(
        Message::user("Transcribe this audio")
            .add_audio(audio_data, AudioFormat::Wav, Some("recording.wav".to_string()))
    )
    .send()
    .await?;
```

### Document Analysis

```rust
use cllient::DocumentFormat;

let pdf_data = std::fs::read("contract.pdf")?;

let response = registry
    .from_id("claude-3-opus-20240229")?
    .append_message(
        Message::user("Summarize this document")
            .add_document(pdf_data, DocumentFormat::Pdf, Some("contract.pdf".to_string()))
    )
    .send()
    .await?;
```

### Mixed Multimodal Content

```rust
let image = std::fs::read("diagram.png")?;
let pdf = std::fs::read("report.pdf")?;

let response = registry
    .from_id("gpt-4o")?
    .append_message(
        Message::user_multimodal(vec![
            ContentBlock::text("Analyze this technical diagram and compare it to the report"),
            ContentBlock::image(image, ImageFormat::Png),
            ContentBlock::document(pdf, DocumentFormat::Pdf, Some("report.pdf".to_string())),
        ])
    )
    .send()
    .await?;
```

---

## Complete Examples

### Chat Bot with Context

```rust
use cllient::{ModelRegistry, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;
    let mut builder = registry.from_id("gpt-4o-mini")?;

    // Set system prompt
    builder = builder.append_message(
        Message::system("You are a helpful AI assistant. Be concise.")
    );

    // Simulate conversation history
    let conversation = vec![
        ("user", "Hi, I'm learning Rust"),
        ("assistant", "Great! Rust is a powerful systems programming language."),
        ("user", "What's the hardest part?"),
        ("assistant", "Many find the borrow checker challenging at first."),
        ("user", "Can you give me tips?"),
    ];

    for (role, text) in conversation {
        builder = builder.append_message(
            if role == "user" {
                Message::user(text)
            } else {
                Message::assistant(text)
            }
        );
    }

    let response = builder.send().await?;
    println!("Assistant: {}", response.text());

    Ok(())
}
```

### Multi-Modal Product Review

```rust
use cllient::{ModelRegistry, Message, ImageFormat};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    // Load product images
    let product_front = std::fs::read("product_front.jpg")?;
    let product_back = std::fs::read("product_back.jpg")?;

    let response = registry
        .from_id("gpt-4o")?
        .append_message(Message::system("You are a product reviewer. Be detailed and objective."))
        .append_message(
            Message::user("Review this product based on the images")
                .add_image(product_front, ImageFormat::Jpeg)
                .add_image(product_back, ImageFormat::Jpeg)
        )
        .send()
        .await?;

    println!("Review:\n{}", response.text());
    Ok(())
}
```

### Streaming with Custom Roles

```rust
use cllient::{ModelRegistry, Message};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    let mut stream = registry
        .from_id("deepseek-chat")?
        .append_message(Message::custom("narrator").add_text("The dragon awakened..."))
        .append_message(Message::custom("hero").add_text("I must be brave!"))
        .append_message(Message::user("Continue the epic battle scene"))
        .stream()
        .await?;

    while let Some(event) = stream.next().await {
        match event? {
            cllient::streaming::StreamEvent::Content(text) => {
                print!("{}", text);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
            cllient::streaming::StreamEvent::Finish(_) => {
                println!("\n[Stream complete]");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
```

### Custom Message Builder Helper

```rust
use cllient::Message;

// Create a helper for your specific use case
fn create_code_review_message(code: &str, language: &str) -> Message {
    Message::user_multimodal(vec![
        cllient::ContentBlock::text(&format!("Review this {} code:", language)),
        cllient::ContentBlock::text(code),
    ])
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = cllient::ModelRegistry::new()?;

    let rust_code = r#"
    fn factorial(n: u64) -> u64 {
        if n == 0 { 1 } else { n * factorial(n - 1) }
    }
    "#;

    let response = registry
        .from_id("claude-3-opus-20240229")?
        .append_message(Message::system("You are a code reviewer. Focus on correctness and best practices."))
        .append_message(create_code_review_message(rust_code, "Rust"))
        .send()
        .await?;

    println!("{}", response.text());
    Ok(())
}
```

---

## API Summary

### Message Creation

| Method | Description | Example |
|--------|-------------|---------|
| `Message::user(text)` | User message with text | `Message::user("Hello")` |
| `Message::assistant(text)` | Assistant response | `Message::assistant("Hi there!")` |
| `Message::system(text)` | System prompt | `Message::system("You are helpful")` |
| `Message::custom(role)` | Custom role | `Message::custom("narrator")` |
| `Message::user_multimodal(blocks)` | User with multiple content types | See multimodal examples |

### Content Building

| Method | Description |
|--------|-------------|
| `.add_text(text)` | Append text content |
| `.add_image(data, format)` | Append image binary |
| `.add_image_url(url)` | Append image URL |
| `.add_audio(data, format, filename)` | Append audio |
| `.add_document(data, format, filename)` | Append document |
| `.add_content(block)` | Append custom ContentBlock |
| `.content(blocks)` | Replace all content |

### RequestBuilder Methods

| Method | Description |
|--------|-------------|
| `.append_message(msg)` | Add message to conversation |
| `.messages(vec)` | Set all messages from Vec<Message> |
| `.prompt(text)` | Quick single user message |
| `.system(text)` | Set system prompt |
| `.send()` | Send non-streaming request |
| `.send_text(text)` | Quick send with text |
| `.stream()` | Start streaming request |
| `.stream_text(text)` | Quick stream with text |

---

**Next**: Explore [Chat API](../3_api-reference.md#chat-api) for persistent conversations
