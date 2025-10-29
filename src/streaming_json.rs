//! Streaming JSON writer - outputs JSON structure incrementally to stdout
//!
//! This is a specialized logger that prints JSON as it's being constructed,
//! perfect for showing live streaming responses in valid JSON format.

use std::io::{self, Write};
use tracing::{debug, trace, instrument};

/// A streaming JSON object writer that outputs to stdout in real-time
pub struct StreamingJsonObject {
    first_field: bool,
    closed: bool,
}

impl StreamingJsonObject {
    /// Initialize and output the opening brace
    #[instrument]
    pub fn new() -> io::Result<Self> {
        debug!("Initializing streaming JSON object");
        print!("{{\n");
        io::stdout().flush()?;
        trace!("Wrote opening brace");
        Ok(Self {
            first_field: true,
            closed: false,
        })
    }

    /// Write a string field
    #[instrument(skip(self))]
    pub fn field_string(&mut self, key: &str, value: &str) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::Other, "Object already closed"));
        }

        debug!(key = %key, value_len = value.len(), "Writing string field");

        if !self.first_field {
            print!(",\n");
        }
        self.first_field = false;

        // Escape the value for JSON
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");

        print!("  \"{}\": \"{}\"", key, escaped);
        io::stdout().flush()?;
        trace!(key = %key, "String field written");
        Ok(())
    }

    /// Write a boolean field
    #[instrument(skip(self))]
    pub fn field_bool(&mut self, key: &str, value: bool) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::Other, "Object already closed"));
        }

        debug!(key = %key, value = %value, "Writing boolean field");

        if !self.first_field {
            print!(",\n");
        }
        self.first_field = false;

        print!("  \"{}\": {}", key, value);
        io::stdout().flush()?;
        trace!(key = %key, "Boolean field written");
        Ok(())
    }

    /// Start a streaming string field - returns a writer for the value
    #[instrument(skip(self))]
    pub fn field_streaming_string(&mut self, key: &str) -> io::Result<StreamingJsonString> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::Other, "Object already closed"));
        }

        debug!(key = %key, "Starting streaming string field");

        if !self.first_field {
            print!(",\n");
        }
        self.first_field = false;

        print!("  \"{}\": \"", key);
        io::stdout().flush()?;

        trace!(key = %key, "Streaming field started");

        Ok(StreamingJsonString {
            closed: false,
        })
    }

    /// Close the object and output the closing brace
    #[instrument(skip(self))]
    pub fn close(mut self) -> io::Result<()> {
        if !self.closed {
            debug!("Closing JSON object");
            print!("\n}}\n");
            io::stdout().flush()?;
            self.closed = true;
            trace!("JSON object closed");
        }
        Ok(())
    }
}

/// A streaming JSON string value writer
pub struct StreamingJsonString {
    closed: bool,
}

impl StreamingJsonString {
    /// Append a chunk to the streaming string value
    #[instrument(skip(self))]
    pub fn write_chunk(&mut self, chunk: &str) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::Other, "String already closed"));
        }

        trace!(chunk_len = chunk.len(), chunk_preview = %chunk.chars().take(20).collect::<String>(), "Writing chunk");

        // Escape special characters for JSON
        let escaped = chunk
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");

        // Use locked stdout for unbuffered writes
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(escaped.as_bytes())?;
        handle.flush()?;

        trace!(chunk_len = chunk.len(), "Chunk written and flushed");
        Ok(())
    }

    /// Close the string value (output closing quote)
    #[instrument(skip(self))]
    pub fn close(mut self) -> io::Result<()> {
        if !self.closed {
            debug!("Closing streaming string");
            print!("\"");
            io::stdout().flush()?;
            self.closed = true;
            trace!("Streaming string closed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_json_basic() -> io::Result<()> {
        let mut obj = StreamingJsonObject::new()?;
        obj.field_string("model", "gpt-4")?;
        obj.field_string("prompt", "hello")?;
        obj.field_bool("success", true)?;
        obj.close()?;
        Ok(())
    }

    #[test]
    fn test_streaming_json_with_streaming_field() -> io::Result<()> {
        let mut obj = StreamingJsonObject::new()?;
        obj.field_string("model", "gpt-4")?;

        let mut response = obj.field_streaming_string("response")?;
        response.write_chunk("Hello")?;
        response.write_chunk(" ")?;
        response.write_chunk("world")?;
        response.close()?;

        obj.field_bool("success", true)?;
        obj.close()?;
        Ok(())
    }
}
