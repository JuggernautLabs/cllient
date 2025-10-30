# MessageList - Managing Conversations

`MessageList` is a container structure for managing messages with modification capabilities. It's perfect for building conversations that need to be modified, reordered, or managed dynamically.

## Table of Contents

1. [Basic Usage](#basic-usage)
2. [Modifying Messages](#modifying-messages)
3. [Clearing and Resetting](#clearing-and-resetting)
4. [Advanced Operations](#advanced-operations)
5. [Integration with RequestBuilder](#integration-with-requestbuilder)

---

## Basic Usage

### Creating a MessageList

```rust
use cllient::{MessageList, Message};

// Empty list
let mut msgs = MessageList::new();

// With pre-allocated capacity
let mut msgs = MessageList::with_capacity(10);

// From existing Vec<Message>
let msgs = MessageList::from_vec(vec![
    Message::user("Hello"),
    Message::assistant("Hi there!"),
]);

// Or use the From trait
let msgs: MessageList = vec![
    Message::user("Hello"),
    Message::assistant("Hi there!"),
].into();
```

### Adding Messages

```rust
let mut msgs = MessageList::new();

// Push to end
msgs.push(Message::system("You are helpful"));
msgs.push(Message::user("What is Rust?"));
msgs.push(Message::assistant("Rust is a systems programming language..."));

// Insert at specific position
msgs.insert(1, Message::system("Be concise"));

println!("Total messages: {}", msgs.len());
```

### Accessing Messages

```rust
let msgs = MessageList::from_vec(vec![
    Message::user("Hello"),
    Message::assistant("Hi!"),
]);

// Get by index
if let Some(msg) = msgs.get(0) {
    println!("Role: {}", msg.role());
}

// Index directly (panics if out of bounds)
let first = &msgs[0];

// Iterate
for msg in msgs.iter() {
    println!("{}: content", msg.role());
}

// Check if empty
if !msgs.is_empty() {
    println!("We have {} messages", msgs.len());
}
```

---

## Modifying Messages

### Replacing Messages by Index

```rust
let mut msgs = MessageList::new();
msgs.push(Message::user("What is 2+2?"));
msgs.push(Message::assistant("5")); // Oops, wrong answer!

// Replace the wrong answer
let old_msg = msgs.set(1, Message::assistant("4"));
assert!(old_msg.is_some());
```

### Modifying In-Place

```rust
let mut msgs = MessageList::from_vec(vec![
    Message::user("Hello"),
    Message::custom("bot").add_text("Initial response"),
]);

// Get mutable reference
if let Some(msg) = msgs.get_mut(1) {
    // Message is mutable here, but Message itself doesn't have
    // direct mutation methods - you'd replace it instead
}

// Or use indexing
msgs[0] = Message::user("Hi there!"); // Replace entire message
```

### Removing Messages

```rust
let mut msgs = MessageList::from_vec(vec![
    Message::system("You are helpful"),
    Message::user("Bad question"),
    Message::user("Good question"),
]);

// Remove the bad question
let removed = msgs.remove(1);
assert!(removed.is_some());
assert_eq!(msgs.len(), 2);
```

### Filtering Messages

```rust
let mut msgs = MessageList::from_vec(vec![
    Message::system("System prompt"),
    Message::user("User question 1"),
    Message::assistant("Answer 1"),
    Message::user("User question 2"),
]);

// Keep only user messages
msgs.retain(|msg| msg.role() == "user");

assert_eq!(msgs.len(), 2);
```

---

## Clearing and Resetting

### Clear and Retrieve All Messages

```rust
let mut msgs = MessageList::from_vec(vec![
    Message::user("Message 1"),
    Message::user("Message 2"),
    Message::user("Message 3"),
]);

// Clear returns all messages as Vec
let all_messages = msgs.clear();

assert_eq!(all_messages.len(), 3);
assert_eq!(msgs.len(), 0);
assert!(msgs.is_empty());

// Now you can do something with the cleared messages
for msg in all_messages {
    println!("Saved: {}", msg.role());
}
```

### Save and Restore Conversations

```rust
use cllient::{MessageList, Message};

struct ConversationManager {
    active: MessageList,
    saved: Vec<Vec<Message>>,
}

impl ConversationManager {
    fn new() -> Self {
        Self {
            active: MessageList::new(),
            saved: Vec::new(),
        }
    }

    fn save_and_clear(&mut self) {
        let messages = self.active.clear();
        self.saved.push(messages);
    }

    fn restore_last(&mut self) -> bool {
        if let Some(messages) = self.saved.pop() {
            self.active = messages.into();
            true
        } else {
            false
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = ConversationManager::new();

    // Build conversation
    manager.active.push(Message::user("Hello"));
    manager.active.push(Message::assistant("Hi!"));

    // Save it
    manager.save_and_clear();

    // Start new conversation
    manager.active.push(Message::user("New topic"));

    // Restore previous
    manager.restore_last();

    Ok(())
}
```

---

## Advanced Operations

### Converting Between Types

```rust
let mut msgs = MessageList::new();
msgs.push(Message::user("Hello"));

// Clone to Vec
let vec_clone: Vec<Message> = msgs.to_vec();

// Convert to Vec (consuming)
let vec_owned: Vec<Message> = msgs.into_vec();

// Can't use msgs anymore after into_vec()
```

### Extending from Iterators

```rust
let mut msgs = MessageList::new();
msgs.push(Message::system("You are helpful"));

// Extend from Vec
let more_messages = vec![
    Message::user("Question 1"),
    Message::assistant("Answer 1"),
];
msgs.extend(more_messages);

// Extend from iterator
msgs.extend(
    vec![("user", "Q2"), ("assistant", "A2")]
        .into_iter()
        .map(|(role, text)| {
            if role == "user" {
                Message::user(text)
            } else {
                Message::assistant(text)
            }
        })
);
```

### Iterating and Transforming

```rust
let msgs = MessageList::from_vec(vec![
    Message::user("Hello"),
    Message::assistant("Hi!"),
    Message::user("How are you?"),
]);

// Immutable iteration
for msg in &msgs {
    println!("Role: {}", msg.role());
}

// Mutable iteration
let mut msgs = msgs; // Make it mutable
for msg in msgs.iter_mut() {
    // Can modify messages in place
}

// Consuming iteration
for msg in msgs {
    // msgs is consumed here
    println!("Owned message: {}", msg.role());
}
```

### Working with Slices

```rust
let msgs = MessageList::from_vec(vec![
    Message::system("System"),
    Message::user("User"),
    Message::assistant("Assistant"),
]);

// Get as slice
let slice: &[Message] = msgs.as_slice();

// Use slice methods
let first_two = &slice[0..2];
```

---

## Integration with RequestBuilder

### Using MessageList with Requests

```rust
use cllient::{ModelRegistry, MessageList, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    let mut msgs = MessageList::new();
    msgs.push(Message::system("You are a helpful assistant"));
    msgs.push(Message::user("What is 2+2?"));

    // MessageList works directly with .messages()
    let response = registry
        .from_id("gpt-4o-mini")?
        .messages(msgs) // Accepts MessageList!
        .send()
        .await?;

    println!("{}", response.text());
    Ok(())
}
```

### Building Conversations Dynamically

```rust
use cllient::{ModelRegistry, MessageList, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;
    let mut conversation = MessageList::new();

    // Start with system prompt
    conversation.push(Message::system("You are a math tutor"));

    // Simulate multi-turn conversation
    let qa_pairs = vec![
        ("What is 2+2?", "4"),
        ("What is 4+4?", "8"),
        ("What is 8+8?", "16"),
    ];

    for (question, answer) in qa_pairs {
        conversation.push(Message::user(question));
        conversation.push(Message::assistant(answer));
    }

    // Add final question
    conversation.push(Message::user("What is 16+16?"));

    let response = registry
        .from_id("deepseek-chat")?
        .messages(conversation)
        .send()
        .await?;

    println!("Answer: {}", response.text());
    Ok(())
}
```

### Editing Conversation History

```rust
use cllient::{ModelRegistry, MessageList, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ModelRegistry::new()?;

    let mut msgs = MessageList::from_vec(vec![
        Message::system("You are helpful"),
        Message::user("What is the capital of France?"),
        Message::assistant("London"), // Wrong!
        Message::user("Are you sure?"),
    ]);

    // Fix the wrong answer before sending
    msgs.set(2, Message::assistant("Paris"));

    let response = registry
        .from_id("claude-3-haiku-20240307")?
        .messages(msgs)
        .send()
        .await?;

    println!("{}", response.text());
    Ok(())
}
```

### Managing Context Window

```rust
use cllient::{MessageList, Message};

fn trim_to_context_window(msgs: &mut MessageList, max_messages: usize) {
    if msgs.len() <= max_messages {
        return;
    }

    // Keep system message if it exists
    let has_system = msgs.get(0).map(|m| m.role() == "system").unwrap_or(false);

    let keep_count = if has_system { max_messages } else { max_messages };
    let remove_count = msgs.len() - keep_count;

    // Remove oldest messages (but keep system message)
    let start_index = if has_system { 1 } else { 0 };
    for _ in 0..remove_count {
        msgs.remove(start_index);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut conversation = MessageList::new();

    conversation.push(Message::system("You are helpful"));

    // Add many messages
    for i in 0..100 {
        conversation.push(Message::user(&format!("Question {}", i)));
        conversation.push(Message::assistant(&format!("Answer {}", i)));
    }

    println!("Before trim: {} messages", conversation.len());

    // Keep only last 10 messages (plus system)
    trim_to_context_window(&mut conversation, 11);

    println!("After trim: {} messages", conversation.len());

    Ok(())
}
```

### Session Management

```rust
use cllient::{MessageList, Message, ModelRegistry};
use std::collections::HashMap;

struct SessionManager {
    sessions: HashMap<String, MessageList>,
}

impl SessionManager {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    fn get_or_create(&mut self, session_id: &str) -> &mut MessageList {
        self.sessions
            .entry(session_id.to_string())
            .or_insert_with(MessageList::new)
    }

    fn clear_session(&mut self, session_id: &str) -> Option<Vec<Message>> {
        self.sessions
            .get_mut(session_id)
            .map(|msgs| msgs.clear())
    }

    fn delete_session(&mut self, session_id: &str) -> Option<MessageList> {
        self.sessions.remove(session_id)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = SessionManager::new();
    let registry = ModelRegistry::new()?;

    // User session 1
    let session1 = manager.get_or_create("user_123");
    session1.push(Message::user("Hello"));

    let response = registry
        .from_id("gpt-4o-mini")?
        .messages(session1.to_vec()) // Clone for request
        .send()
        .await?;

    // Add response to session
    session1.push(Message::assistant(response.text()));

    // User session 2
    let session2 = manager.get_or_create("user_456");
    session2.push(Message::user("Different conversation"));

    // Clear session 1
    let archived = manager.clear_session("user_123");
    println!("Archived {} messages", archived.unwrap().len());

    Ok(())
}
```

---

## API Summary

### Creation

| Method | Description |
|--------|-------------|
| `MessageList::new()` | Create empty list |
| `MessageList::with_capacity(n)` | Create with capacity |
| `MessageList::from_vec(vec)` | Create from Vec<Message> |

### Adding/Removing

| Method | Description |
|--------|-------------|
| `.push(msg)` | Add to end |
| `.insert(idx, msg)` | Insert at index |
| `.remove(idx)` | Remove and return |
| `.clear()` | Remove all and return them |
| `.extend(iter)` | Add from iterator |

### Accessing

| Method | Description |
|--------|-------------|
| `.get(idx)` | Get reference by index |
| `.get_mut(idx)` | Get mutable reference |
| `.set(idx, msg)` | Replace and return old |
| `[idx]` | Index access |
| `.as_slice()` | Get as slice |

### Iteration

| Method | Description |
|--------|-------------|
| `.iter()` | Immutable iterator |
| `.iter_mut()` | Mutable iterator |
| `for msg in &list` | Iterate references |
| `for msg in list` | Consume and iterate |

### Queries

| Method | Description |
|--------|-------------|
| `.len()` | Get count |
| `.is_empty()` | Check if empty |

### Conversion

| Method | Description |
|--------|-------------|
| `.to_vec()` | Clone to Vec |
| `.into_vec()` | Convert to Vec (consuming) |
| `Vec::from(list)` | Convert to Vec |
| `MessageList::from(vec)` | Convert from Vec |

---

**Next**: See [Message Builder](message-builder.md) for creating individual messages
