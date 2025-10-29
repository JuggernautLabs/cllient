//! Test streaming JSON with timing info
//!
//! Run with: cargo run --example test_streaming_timing

use cllient::streaming_json::StreamingJsonObject;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> std::io::Result<()> {
    let start = Instant::now();

    eprintln!("[{:6.2}s] Starting streaming JSON object", start.elapsed().as_secs_f64());

    let mut obj = StreamingJsonObject::new()?;
    eprintln!("[{:6.2}s] Wrote opening brace", start.elapsed().as_secs_f64());

    obj.field_string("model", "test-model")?;
    eprintln!("[{:6.2}s] Wrote model field", start.elapsed().as_secs_f64());

    obj.field_string("prompt", "simulate streaming")?;
    eprintln!("[{:6.2}s] Wrote prompt field", start.elapsed().as_secs_f64());

    let mut response = obj.field_streaming_string("response")?;
    eprintln!("[{:6.2}s] Started response field", start.elapsed().as_secs_f64());

    // Simulate slow streaming
    let words = vec!["The", " sky", " is", " blue", " because", " of", " scattering", "."];
    for (i, word) in words.iter().enumerate() {
        eprintln!("[{:6.2}s] Writing chunk {}: {:?}", start.elapsed().as_secs_f64(), i + 1, word);
        response.write_chunk(word)?;
        thread::sleep(Duration::from_millis(200));
    }

    response.close()?;
    eprintln!("[{:6.2}s] Closed response field", start.elapsed().as_secs_f64());

    obj.field_bool("streamed", true)?;
    eprintln!("[{:6.2}s] Wrote streamed field", start.elapsed().as_secs_f64());

    obj.field_bool("success", true)?;
    eprintln!("[{:6.2}s] Wrote success field", start.elapsed().as_secs_f64());

    obj.close()?;
    eprintln!("[{:6.2}s] Closed JSON object", start.elapsed().as_secs_f64());

    eprintln!("\n[{:6.2}s] DONE! Total time: {:.2}s",
             start.elapsed().as_secs_f64(),
             start.elapsed().as_secs_f64());

    Ok(())
}
