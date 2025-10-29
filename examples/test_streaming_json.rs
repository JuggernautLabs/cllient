//! Test the streaming JSON logger
//!
//! Run with: cargo run --example test_streaming_json

use cllient::streaming_json::StreamingJsonObject;
use std::thread;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    println!("=== Test 1: Basic streaming JSON object ===\n");

    let mut obj = StreamingJsonObject::new()?;
    obj.field_string("model", "test-model")?;
    obj.field_string("prompt", "hello world")?;
    obj.field_bool("success", true)?;
    obj.close()?;

    println!("\n");
    thread::sleep(Duration::from_secs(1));

    println!("=== Test 2: Streaming string field ===\n");

    let mut obj = StreamingJsonObject::new()?;
    obj.field_string("model", "gpt-4")?;
    obj.field_string("prompt", "tell me a story")?;

    // Start streaming response field
    let mut response = obj.field_streaming_string("response")?;

    // Simulate streaming tokens
    let words = vec!["Once", " upon", " a", " time", ",", " there", " was", " a", " robot", "."];
    for word in words {
        response.write_chunk(word)?;
        thread::sleep(Duration::from_millis(100)); // Simulate network delay
    }

    response.close()?;

    obj.field_bool("streamed", true)?;
    obj.field_bool("success", true)?;
    obj.close()?;

    println!("\n");
    thread::sleep(Duration::from_secs(1));

    println!("=== Test 3: JSON escaping ===\n");

    let mut obj = StreamingJsonObject::new()?;
    obj.field_string("test", "value with \"quotes\" and \n newlines")?;

    let mut response = obj.field_streaming_string("response")?;
    response.write_chunk("Line 1\n")?;
    response.write_chunk("Line 2 with \"quotes\"\n")?;
    response.write_chunk("Line 3 with \\ backslash")?;
    response.close()?;

    obj.close()?;

    println!("\n\n=== All tests complete! ===");

    Ok(())
}
