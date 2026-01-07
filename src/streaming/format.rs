//! Stream format configuration system
//!
//! This module provides a flexible, config-driven approach to parsing different
//! streaming response formats. It supports:
//!
//! - Standard Server-Sent Events (text/event-stream)
//! - Newline-delimited JSON (application/x-ndjson)
//! - JSON Lines (application/jsonl)
//! - Custom formats
//!
//! ## Architecture
//!
//! The system uses a builder pattern for constructing line parsers with
//! provider-specific configurations. This allows the streaming module to
//! parse responses from any provider by configuring the appropriate format
//! and line parsing rules.
//!
//! ## Usage
//!
//! ```rust
//! use cllient::streaming::format::{StreamFormat, LineParserConfig, LineParserBuilder};
//!
//! // Create an SSE line parser (OpenAI-style)
//! let parser = LineParserBuilder::new()
//!     .line_prefix("data: ")
//!     .comment_prefix(": ")
//!     .empty_line_behavior(EmptyLineBehavior::EventDelimiter)
//!     .build();
//!
//! // Parse a line
//! let result = parser.parse_line("data: {\"content\": \"hello\"}");
//! ```

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

// =============================================================================
// Stream Format Types
// =============================================================================

/// Stream format types representing different streaming response content types.
///
/// Different AI providers use various streaming formats:
/// - OpenAI, Anthropic: Server-Sent Events (text/event-stream)
/// - Some providers: Newline-delimited JSON
/// - Custom formats for specialized use cases
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StreamFormat {
    /// Standard Server-Sent Events (text/event-stream)
    ///
    /// SSE format with `data:` prefixed lines, optional `event:` type lines,
    /// and double-newline event delimiters.
    TextEventStream,

    /// Newline-delimited JSON (application/x-ndjson)
    ///
    /// Each line is a complete JSON object. No prefix stripping required.
    NdJson,

    /// JSON Lines (application/jsonl)
    ///
    /// Similar to NDJSON, each line is a JSON object. The format is
    /// semantically equivalent to NDJSON but uses a different MIME type.
    JsonLines,

    /// Custom format with content type string
    ///
    /// For providers with non-standard streaming formats. The string
    /// should be the MIME type (e.g., "text/plain").
    Custom(String),
}

impl StreamFormat {
    /// Get the MIME type string for this format
    pub fn mime_type(&self) -> &str {
        match self {
            StreamFormat::TextEventStream => "text/event-stream",
            StreamFormat::NdJson => "application/x-ndjson",
            StreamFormat::JsonLines => "application/jsonl",
            StreamFormat::Custom(mime) => mime,
        }
    }

    /// Check if this format uses SSE-style parsing (with data: prefix)
    pub fn is_sse(&self) -> bool {
        matches!(self, StreamFormat::TextEventStream)
    }

    /// Check if this format is JSON-lines based (no prefix)
    pub fn is_json_lines(&self) -> bool {
        matches!(self, StreamFormat::NdJson | StreamFormat::JsonLines)
    }

    /// Get the default line parser config for this format
    pub fn default_parser_config(&self) -> LineParserConfig {
        match self {
            StreamFormat::TextEventStream => LineParserConfig {
                line_prefix: Some("data: ".to_string()),
                event_prefix: Some("event: ".to_string()),
                empty_line_behavior: EmptyLineBehavior::EventDelimiter,
                comment_prefix: Some(": ".to_string()),
            },
            StreamFormat::NdJson | StreamFormat::JsonLines => LineParserConfig {
                line_prefix: None,
                event_prefix: None,
                empty_line_behavior: EmptyLineBehavior::Ignore,
                comment_prefix: None,
            },
            StreamFormat::Custom(_) => LineParserConfig::default(),
        }
    }
}

impl Default for StreamFormat {
    fn default() -> Self {
        StreamFormat::TextEventStream
    }
}

impl std::fmt::Display for StreamFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mime_type())
    }
}

impl Serialize for StreamFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.mime_type())
    }
}

impl<'de> Deserialize<'de> for StreamFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "text/event-stream" => StreamFormat::TextEventStream,
            "application/x-ndjson" => StreamFormat::NdJson,
            "application/jsonl" => StreamFormat::JsonLines,
            other => StreamFormat::Custom(other.to_string()),
        })
    }
}

// =============================================================================
// Empty Line Behavior
// =============================================================================

/// How to handle empty lines in the stream
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyLineBehavior {
    /// Ignore empty lines (skip them)
    #[default]
    Ignore,

    /// Treat empty lines as event delimiters (SSE style)
    ///
    /// In SSE, a blank line indicates the end of an event. Multiple
    /// data lines before a blank line are concatenated.
    EventDelimiter,

    /// Include empty lines in output (preserve them)
    Include,
}

impl EmptyLineBehavior {
    /// Check if empty lines should be treated as event delimiters
    pub fn is_event_delimiter(&self) -> bool {
        matches!(self, EmptyLineBehavior::EventDelimiter)
    }

    /// Check if empty lines should be ignored
    pub fn is_ignored(&self) -> bool {
        matches!(self, EmptyLineBehavior::Ignore)
    }
}

// =============================================================================
// Line Parser Configuration
// =============================================================================

/// Configuration for parsing individual lines from a stream.
///
/// This configuration is typically derived from the stream format but can
/// be customized for providers with non-standard implementations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineParserConfig {
    /// Prefix to strip from data lines (e.g., "data: " for SSE)
    ///
    /// Lines that don't match this prefix may be ignored or treated
    /// as event type lines depending on other configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_prefix: Option<String>,

    /// Event type prefix (e.g., "event: " for SSE)
    ///
    /// Lines with this prefix specify the event type for the following
    /// data. Used in SSE to distinguish message types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_prefix: Option<String>,

    /// How to handle empty lines
    #[serde(default)]
    pub empty_line_behavior: EmptyLineBehavior,

    /// Comment prefix to ignore (e.g., ": " for SSE keepalive)
    ///
    /// Lines starting with this prefix are comments and should be ignored.
    /// In SSE, lines starting with ":" are comments (often used for keepalive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_prefix: Option<String>,
}

impl Default for LineParserConfig {
    fn default() -> Self {
        Self {
            line_prefix: None,
            event_prefix: None,
            empty_line_behavior: EmptyLineBehavior::default(),
            comment_prefix: None,
        }
    }
}

impl LineParserConfig {
    /// Create a new empty configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create SSE-style configuration (OpenAI, Anthropic)
    pub fn sse() -> Self {
        Self {
            line_prefix: Some("data: ".to_string()),
            event_prefix: Some("event: ".to_string()),
            empty_line_behavior: EmptyLineBehavior::EventDelimiter,
            comment_prefix: Some(": ".to_string()),
        }
    }

    /// Create NDJSON-style configuration
    pub fn ndjson() -> Self {
        Self {
            line_prefix: None,
            event_prefix: None,
            empty_line_behavior: EmptyLineBehavior::Ignore,
            comment_prefix: None,
        }
    }

    /// Check if a line is a comment
    pub fn is_comment(&self, line: &str) -> bool {
        self.comment_prefix
            .as_ref()
            .is_some_and(|prefix| line.starts_with(prefix))
    }

    /// Check if a line is an event type line
    pub fn is_event_line(&self, line: &str) -> bool {
        self.event_prefix
            .as_ref()
            .is_some_and(|prefix| line.starts_with(prefix))
    }

    /// Check if a line is a data line
    pub fn is_data_line(&self, line: &str) -> bool {
        match &self.line_prefix {
            Some(prefix) => line.starts_with(prefix),
            None => !line.is_empty() && !self.is_comment(line) && !self.is_event_line(line),
        }
    }

    /// Extract event type from an event line
    pub fn extract_event_type<'a>(&self, line: &'a str) -> Option<&'a str> {
        self.event_prefix
            .as_ref()
            .and_then(|prefix| line.strip_prefix(prefix))
    }

    /// Strip prefix from a data line, returning the data content
    pub fn strip_prefix<'a>(&self, line: &'a str) -> Cow<'a, str> {
        match &self.line_prefix {
            Some(prefix) => {
                if let Some(stripped) = line.strip_prefix(prefix) {
                    Cow::Borrowed(stripped)
                } else {
                    Cow::Borrowed(line)
                }
            }
            None => Cow::Borrowed(line),
        }
    }
}

// =============================================================================
// Parsed Line Types
// =============================================================================

/// Result of parsing a single line from the stream
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedLine<'a> {
    /// A data line with content (prefix stripped)
    Data(Cow<'a, str>),

    /// An event type specifier (for SSE)
    EventType(&'a str),

    /// A comment line (ignored)
    Comment,

    /// An empty line
    Empty,

    /// Line that doesn't match any known pattern
    Unknown(&'a str),
}

impl<'a> ParsedLine<'a> {
    /// Get the data content if this is a data line
    pub fn as_data(&self) -> Option<&str> {
        match self {
            ParsedLine::Data(data) => Some(data),
            _ => None,
        }
    }

    /// Get the event type if this is an event type line
    pub fn as_event_type(&self) -> Option<&str> {
        match self {
            ParsedLine::EventType(t) => Some(t),
            _ => None,
        }
    }

    /// Check if this is an empty line
    pub fn is_empty(&self) -> bool {
        matches!(self, ParsedLine::Empty)
    }

    /// Check if this line should be skipped (comment or ignored empty)
    pub fn should_skip(&self, config: &LineParserConfig) -> bool {
        match self {
            ParsedLine::Comment => true,
            ParsedLine::Empty => config.empty_line_behavior.is_ignored(),
            _ => false,
        }
    }
}

// =============================================================================
// Line Parser
// =============================================================================

/// Parser for individual lines from a streaming response.
///
/// The parser applies the configured rules to classify and extract data
/// from each line of the stream.
#[derive(Debug, Clone)]
pub struct LineParser {
    config: LineParserConfig,
}

impl LineParser {
    /// Create a new line parser with the given configuration
    pub fn new(config: LineParserConfig) -> Self {
        Self { config }
    }

    /// Create a line parser for SSE format
    pub fn sse() -> Self {
        Self::new(LineParserConfig::sse())
    }

    /// Create a line parser for NDJSON format
    pub fn ndjson() -> Self {
        Self::new(LineParserConfig::ndjson())
    }

    /// Get the parser configuration
    pub fn config(&self) -> &LineParserConfig {
        &self.config
    }

    /// Parse a single line from the stream
    pub fn parse_line<'a>(&self, line: &'a str) -> ParsedLine<'a> {
        let trimmed = line.trim_end_matches(['\r', '\n']);

        // Check for empty line first
        if trimmed.is_empty() {
            return ParsedLine::Empty;
        }

        // Check for comment
        if self.config.is_comment(trimmed) {
            return ParsedLine::Comment;
        }

        // Check for event type line
        if let Some(event_type) = self.config.extract_event_type(trimmed) {
            return ParsedLine::EventType(event_type);
        }

        // Check for data line
        if self.config.is_data_line(trimmed) {
            return ParsedLine::Data(self.config.strip_prefix(trimmed));
        }

        // Unknown line type
        ParsedLine::Unknown(trimmed)
    }

    /// Parse and extract data from a line, returning None for non-data lines
    pub fn extract_data<'a>(&self, line: &'a str) -> Option<Cow<'a, str>> {
        match self.parse_line(line) {
            ParsedLine::Data(data) => Some(data),
            _ => None,
        }
    }

    /// Check if a line indicates end of stream
    ///
    /// This checks for common end markers like "[DONE]" in SSE streams.
    pub fn is_end_marker(&self, line: &str, end_markers: &[&str]) -> bool {
        if let ParsedLine::Data(data) = self.parse_line(line) {
            end_markers.iter().any(|marker| data.trim() == *marker)
        } else {
            false
        }
    }
}

impl Default for LineParser {
    fn default() -> Self {
        Self::new(LineParserConfig::default())
    }
}

impl From<LineParserConfig> for LineParser {
    fn from(config: LineParserConfig) -> Self {
        Self::new(config)
    }
}

impl From<StreamFormat> for LineParser {
    fn from(format: StreamFormat) -> Self {
        Self::new(format.default_parser_config())
    }
}

// =============================================================================
// Line Parser Builder
// =============================================================================

/// Builder for constructing LineParser instances with custom configuration.
///
/// Provides a fluent API for configuring line parsing behavior.
#[derive(Debug, Clone, Default)]
pub struct LineParserBuilder {
    line_prefix: Option<String>,
    event_prefix: Option<String>,
    empty_line_behavior: EmptyLineBehavior,
    comment_prefix: Option<String>,
}

impl LineParserBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder starting from SSE defaults
    pub fn from_sse() -> Self {
        Self {
            line_prefix: Some("data: ".to_string()),
            event_prefix: Some("event: ".to_string()),
            empty_line_behavior: EmptyLineBehavior::EventDelimiter,
            comment_prefix: Some(": ".to_string()),
        }
    }

    /// Create a builder starting from NDJSON defaults
    pub fn from_ndjson() -> Self {
        Self {
            line_prefix: None,
            event_prefix: None,
            empty_line_behavior: EmptyLineBehavior::Ignore,
            comment_prefix: None,
        }
    }

    /// Set the line prefix to strip from data lines
    pub fn line_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.line_prefix = Some(prefix.into());
        self
    }

    /// Clear the line prefix (no prefix stripping)
    pub fn no_line_prefix(mut self) -> Self {
        self.line_prefix = None;
        self
    }

    /// Set the event type prefix
    pub fn event_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.event_prefix = Some(prefix.into());
        self
    }

    /// Clear the event prefix
    pub fn no_event_prefix(mut self) -> Self {
        self.event_prefix = None;
        self
    }

    /// Set the empty line behavior
    pub fn empty_line_behavior(mut self, behavior: EmptyLineBehavior) -> Self {
        self.empty_line_behavior = behavior;
        self
    }

    /// Set empty lines to be ignored
    pub fn ignore_empty_lines(mut self) -> Self {
        self.empty_line_behavior = EmptyLineBehavior::Ignore;
        self
    }

    /// Set empty lines to be event delimiters
    pub fn empty_lines_delimit_events(mut self) -> Self {
        self.empty_line_behavior = EmptyLineBehavior::EventDelimiter;
        self
    }

    /// Set empty lines to be included in output
    pub fn include_empty_lines(mut self) -> Self {
        self.empty_line_behavior = EmptyLineBehavior::Include;
        self
    }

    /// Set the comment prefix
    pub fn comment_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.comment_prefix = Some(prefix.into());
        self
    }

    /// Clear the comment prefix (no comment handling)
    pub fn no_comment_prefix(mut self) -> Self {
        self.comment_prefix = None;
        self
    }

    /// Build the LineParserConfig
    pub fn build_config(self) -> LineParserConfig {
        LineParserConfig {
            line_prefix: self.line_prefix,
            event_prefix: self.event_prefix,
            empty_line_behavior: self.empty_line_behavior,
            comment_prefix: self.comment_prefix,
        }
    }

    /// Build the LineParser
    pub fn build(self) -> LineParser {
        LineParser::new(self.build_config())
    }
}

// =============================================================================
// Stream Format Configuration (Combined)
// =============================================================================

/// Complete stream format configuration combining format type with parsing rules.
///
/// This is the top-level configuration used by the streaming system to
/// determine how to parse incoming stream data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamFormatConfig {
    /// The stream format type
    pub format: StreamFormat,

    /// Custom line parser configuration (overrides format defaults if provided)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser: Option<LineParserConfig>,

    /// End-of-stream markers (e.g., ["[DONE]"] for OpenAI)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub end_markers: Vec<String>,
}

impl StreamFormatConfig {
    /// Create a new stream format configuration
    pub fn new(format: StreamFormat) -> Self {
        Self {
            format,
            parser: None,
            end_markers: Vec::new(),
        }
    }

    /// Create SSE format configuration (default for most providers)
    pub fn sse() -> Self {
        Self {
            format: StreamFormat::TextEventStream,
            parser: None,
            end_markers: vec!["[DONE]".to_string()],
        }
    }

    /// Create NDJSON format configuration
    pub fn ndjson() -> Self {
        Self {
            format: StreamFormat::NdJson,
            parser: None,
            end_markers: Vec::new(),
        }
    }

    /// Set custom parser configuration
    pub fn with_parser(mut self, config: LineParserConfig) -> Self {
        self.parser = Some(config);
        self
    }

    /// Add an end-of-stream marker
    pub fn with_end_marker(mut self, marker: impl Into<String>) -> Self {
        self.end_markers.push(marker.into());
        self
    }

    /// Get the effective line parser configuration
    pub fn parser_config(&self) -> LineParserConfig {
        self.parser
            .clone()
            .unwrap_or_else(|| self.format.default_parser_config())
    }

    /// Create a line parser from this configuration
    pub fn create_parser(&self) -> LineParser {
        LineParser::new(self.parser_config())
    }

    /// Check if a line is an end marker
    pub fn is_end_marker(&self, data: &str) -> bool {
        let trimmed = data.trim();
        self.end_markers.iter().any(|m| m == trimmed)
    }
}

impl Default for StreamFormatConfig {
    fn default() -> Self {
        Self::sse()
    }
}

impl From<StreamFormat> for StreamFormatConfig {
    fn from(format: StreamFormat) -> Self {
        Self::new(format)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // StreamFormat Tests
    // -------------------------------------------------------------------------

    mod stream_format {
        use super::*;

        #[test]
        fn test_mime_types() {
            assert_eq!(StreamFormat::TextEventStream.mime_type(), "text/event-stream");
            assert_eq!(StreamFormat::NdJson.mime_type(), "application/x-ndjson");
            assert_eq!(StreamFormat::JsonLines.mime_type(), "application/jsonl");
            assert_eq!(
                StreamFormat::Custom("text/plain".to_string()).mime_type(),
                "text/plain"
            );
        }

        #[test]
        fn test_is_sse() {
            assert!(StreamFormat::TextEventStream.is_sse());
            assert!(!StreamFormat::NdJson.is_sse());
            assert!(!StreamFormat::JsonLines.is_sse());
            assert!(!StreamFormat::Custom("text/plain".to_string()).is_sse());
        }

        #[test]
        fn test_is_json_lines() {
            assert!(!StreamFormat::TextEventStream.is_json_lines());
            assert!(StreamFormat::NdJson.is_json_lines());
            assert!(StreamFormat::JsonLines.is_json_lines());
            assert!(!StreamFormat::Custom("text/plain".to_string()).is_json_lines());
        }

        #[test]
        fn test_display() {
            assert_eq!(format!("{}", StreamFormat::TextEventStream), "text/event-stream");
            assert_eq!(format!("{}", StreamFormat::NdJson), "application/x-ndjson");
        }

        #[test]
        fn test_serialize_deserialize() {
            let format = StreamFormat::TextEventStream;
            let json = serde_json::to_string(&format).unwrap();
            assert_eq!(json, "\"text/event-stream\"");

            let parsed: StreamFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, StreamFormat::TextEventStream);
        }

        #[test]
        fn test_deserialize_custom() {
            let json = "\"text/plain\"";
            let parsed: StreamFormat = serde_json::from_str(json).unwrap();
            assert_eq!(parsed, StreamFormat::Custom("text/plain".to_string()));
        }

        #[test]
        fn test_default_parser_config_sse() {
            let config = StreamFormat::TextEventStream.default_parser_config();
            assert_eq!(config.line_prefix, Some("data: ".to_string()));
            assert_eq!(config.event_prefix, Some("event: ".to_string()));
            assert_eq!(config.empty_line_behavior, EmptyLineBehavior::EventDelimiter);
            assert_eq!(config.comment_prefix, Some(": ".to_string()));
        }

        #[test]
        fn test_default_parser_config_ndjson() {
            let config = StreamFormat::NdJson.default_parser_config();
            assert_eq!(config.line_prefix, None);
            assert_eq!(config.event_prefix, None);
            assert_eq!(config.empty_line_behavior, EmptyLineBehavior::Ignore);
            assert_eq!(config.comment_prefix, None);
        }
    }

    // -------------------------------------------------------------------------
    // EmptyLineBehavior Tests
    // -------------------------------------------------------------------------

    mod empty_line_behavior {
        use super::*;

        #[test]
        fn test_default() {
            assert_eq!(EmptyLineBehavior::default(), EmptyLineBehavior::Ignore);
        }

        #[test]
        fn test_is_event_delimiter() {
            assert!(!EmptyLineBehavior::Ignore.is_event_delimiter());
            assert!(EmptyLineBehavior::EventDelimiter.is_event_delimiter());
            assert!(!EmptyLineBehavior::Include.is_event_delimiter());
        }

        #[test]
        fn test_is_ignored() {
            assert!(EmptyLineBehavior::Ignore.is_ignored());
            assert!(!EmptyLineBehavior::EventDelimiter.is_ignored());
            assert!(!EmptyLineBehavior::Include.is_ignored());
        }

        #[test]
        fn test_serialize_deserialize() {
            let behavior = EmptyLineBehavior::EventDelimiter;
            let json = serde_json::to_string(&behavior).unwrap();
            assert_eq!(json, "\"event_delimiter\"");

            let parsed: EmptyLineBehavior = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, EmptyLineBehavior::EventDelimiter);
        }
    }

    // -------------------------------------------------------------------------
    // LineParserConfig Tests
    // -------------------------------------------------------------------------

    mod line_parser_config {
        use super::*;

        #[test]
        fn test_sse_config() {
            let config = LineParserConfig::sse();
            assert_eq!(config.line_prefix, Some("data: ".to_string()));
            assert_eq!(config.event_prefix, Some("event: ".to_string()));
            assert_eq!(config.empty_line_behavior, EmptyLineBehavior::EventDelimiter);
            assert_eq!(config.comment_prefix, Some(": ".to_string()));
        }

        #[test]
        fn test_ndjson_config() {
            let config = LineParserConfig::ndjson();
            assert_eq!(config.line_prefix, None);
            assert_eq!(config.event_prefix, None);
            assert_eq!(config.empty_line_behavior, EmptyLineBehavior::Ignore);
            assert_eq!(config.comment_prefix, None);
        }

        #[test]
        fn test_is_comment() {
            let config = LineParserConfig::sse();
            assert!(config.is_comment(": this is a comment"));
            assert!(config.is_comment(": keepalive"));
            assert!(!config.is_comment("data: some data"));
            assert!(!config.is_comment("event: message"));
        }

        #[test]
        fn test_is_event_line() {
            let config = LineParserConfig::sse();
            assert!(config.is_event_line("event: message"));
            assert!(config.is_event_line("event: content_block_delta"));
            assert!(!config.is_event_line("data: some data"));
            assert!(!config.is_event_line(": comment"));
        }

        #[test]
        fn test_is_data_line() {
            let config = LineParserConfig::sse();
            assert!(config.is_data_line("data: {\"content\": \"hello\"}"));
            assert!(config.is_data_line("data: [DONE]"));
            assert!(!config.is_data_line("event: message"));
            assert!(!config.is_data_line(": comment"));
            assert!(!config.is_data_line(""));
        }

        #[test]
        fn test_extract_event_type() {
            let config = LineParserConfig::sse();
            assert_eq!(config.extract_event_type("event: message"), Some("message"));
            assert_eq!(
                config.extract_event_type("event: content_block_delta"),
                Some("content_block_delta")
            );
            assert_eq!(config.extract_event_type("data: something"), None);
        }

        #[test]
        fn test_strip_prefix() {
            let config = LineParserConfig::sse();
            assert_eq!(
                config.strip_prefix("data: {\"content\": \"hello\"}"),
                Cow::Borrowed("{\"content\": \"hello\"}")
            );
            assert_eq!(
                config.strip_prefix("no prefix here"),
                Cow::Borrowed("no prefix here")
            );
        }

        #[test]
        fn test_ndjson_is_data_line() {
            let config = LineParserConfig::ndjson();
            assert!(config.is_data_line("{\"content\": \"hello\"}"));
            assert!(!config.is_data_line(""));
        }
    }

    // -------------------------------------------------------------------------
    // ParsedLine Tests
    // -------------------------------------------------------------------------

    mod parsed_line {
        use super::*;

        #[test]
        fn test_as_data() {
            let line = ParsedLine::Data(Cow::Borrowed("test data"));
            assert_eq!(line.as_data(), Some("test data"));

            let line = ParsedLine::EventType("message");
            assert_eq!(line.as_data(), None);
        }

        #[test]
        fn test_as_event_type() {
            let line = ParsedLine::EventType("message");
            assert_eq!(line.as_event_type(), Some("message"));

            let line = ParsedLine::Data(Cow::Borrowed("data"));
            assert_eq!(line.as_event_type(), None);
        }

        #[test]
        fn test_is_empty() {
            assert!(ParsedLine::<'_>::Empty.is_empty());
            assert!(!ParsedLine::Data(Cow::Borrowed("")).is_empty());
            assert!(!ParsedLine::Comment.is_empty());
        }

        #[test]
        fn test_should_skip() {
            let config = LineParserConfig::sse();

            assert!(ParsedLine::<'_>::Comment.should_skip(&config));
            assert!(!ParsedLine::<'_>::Empty.should_skip(&config)); // EventDelimiter, not Ignore
            assert!(!ParsedLine::Data(Cow::Borrowed("data")).should_skip(&config));

            let ndjson_config = LineParserConfig::ndjson();
            assert!(ParsedLine::<'_>::Empty.should_skip(&ndjson_config)); // Ignore
        }
    }

    // -------------------------------------------------------------------------
    // LineParser Tests
    // -------------------------------------------------------------------------

    mod line_parser {
        use super::*;

        #[test]
        fn test_parse_sse_data_line() {
            let parser = LineParser::sse();
            match parser.parse_line("data: {\"content\": \"hello\"}") {
                ParsedLine::Data(data) => assert_eq!(data, "{\"content\": \"hello\"}"),
                other => panic!("Expected Data, got {:?}", other),
            }
        }

        #[test]
        fn test_parse_sse_event_line() {
            let parser = LineParser::sse();
            match parser.parse_line("event: message") {
                ParsedLine::EventType(t) => assert_eq!(t, "message"),
                other => panic!("Expected EventType, got {:?}", other),
            }
        }

        #[test]
        fn test_parse_sse_comment() {
            let parser = LineParser::sse();
            match parser.parse_line(": keepalive") {
                ParsedLine::Comment => {}
                other => panic!("Expected Comment, got {:?}", other),
            }
        }

        #[test]
        fn test_parse_sse_empty() {
            let parser = LineParser::sse();
            match parser.parse_line("") {
                ParsedLine::Empty => {}
                other => panic!("Expected Empty, got {:?}", other),
            }
            match parser.parse_line("\n") {
                ParsedLine::Empty => {}
                other => panic!("Expected Empty, got {:?}", other),
            }
        }

        #[test]
        fn test_parse_ndjson_line() {
            let parser = LineParser::ndjson();
            match parser.parse_line("{\"content\": \"hello\"}") {
                ParsedLine::Data(data) => assert_eq!(data, "{\"content\": \"hello\"}"),
                other => panic!("Expected Data, got {:?}", other),
            }
        }

        #[test]
        fn test_extract_data() {
            let parser = LineParser::sse();
            assert_eq!(
                parser.extract_data("data: hello"),
                Some(Cow::Borrowed("hello"))
            );
            assert_eq!(parser.extract_data("event: message"), None);
            assert_eq!(parser.extract_data(": comment"), None);
            assert_eq!(parser.extract_data(""), None);
        }

        #[test]
        fn test_is_end_marker() {
            let parser = LineParser::sse();
            assert!(parser.is_end_marker("data: [DONE]", &["[DONE]"]));
            assert!(parser.is_end_marker("data:  [DONE] ", &["[DONE]"])); // with whitespace
            assert!(!parser.is_end_marker("data: {\"content\": \"hello\"}", &["[DONE]"]));
            assert!(!parser.is_end_marker("event: message", &["[DONE]"]));
        }

        #[test]
        fn test_from_stream_format() {
            let parser: LineParser = StreamFormat::TextEventStream.into();
            assert_eq!(parser.config().line_prefix, Some("data: ".to_string()));

            let parser: LineParser = StreamFormat::NdJson.into();
            assert_eq!(parser.config().line_prefix, None);
        }
    }

    // -------------------------------------------------------------------------
    // LineParserBuilder Tests
    // -------------------------------------------------------------------------

    mod line_parser_builder {
        use super::*;

        #[test]
        fn test_build_empty() {
            let parser = LineParserBuilder::new().build();
            assert_eq!(parser.config().line_prefix, None);
            assert_eq!(parser.config().event_prefix, None);
            assert_eq!(parser.config().empty_line_behavior, EmptyLineBehavior::Ignore);
            assert_eq!(parser.config().comment_prefix, None);
        }

        #[test]
        fn test_build_custom() {
            let parser = LineParserBuilder::new()
                .line_prefix("msg: ")
                .event_prefix("type: ")
                .comment_prefix("# ")
                .empty_lines_delimit_events()
                .build();

            assert_eq!(parser.config().line_prefix, Some("msg: ".to_string()));
            assert_eq!(parser.config().event_prefix, Some("type: ".to_string()));
            assert_eq!(parser.config().comment_prefix, Some("# ".to_string()));
            assert_eq!(
                parser.config().empty_line_behavior,
                EmptyLineBehavior::EventDelimiter
            );
        }

        #[test]
        fn test_from_sse() {
            let parser = LineParserBuilder::from_sse().build();
            assert_eq!(parser.config().line_prefix, Some("data: ".to_string()));
            assert_eq!(parser.config().event_prefix, Some("event: ".to_string()));
        }

        #[test]
        fn test_from_ndjson() {
            let parser = LineParserBuilder::from_ndjson().build();
            assert_eq!(parser.config().line_prefix, None);
            assert_eq!(parser.config().event_prefix, None);
        }

        #[test]
        fn test_chain_methods() {
            let parser = LineParserBuilder::from_sse()
                .no_event_prefix() // Override SSE default
                .ignore_empty_lines() // Override SSE default
                .build();

            assert_eq!(parser.config().line_prefix, Some("data: ".to_string())); // Kept
            assert_eq!(parser.config().event_prefix, None); // Removed
            assert_eq!(parser.config().empty_line_behavior, EmptyLineBehavior::Ignore);
        }
    }

    // -------------------------------------------------------------------------
    // StreamFormatConfig Tests
    // -------------------------------------------------------------------------

    mod stream_format_config {
        use super::*;

        #[test]
        fn test_sse_config() {
            let config = StreamFormatConfig::sse();
            assert_eq!(config.format, StreamFormat::TextEventStream);
            assert_eq!(config.end_markers, vec!["[DONE]".to_string()]);
        }

        #[test]
        fn test_ndjson_config() {
            let config = StreamFormatConfig::ndjson();
            assert_eq!(config.format, StreamFormat::NdJson);
            assert!(config.end_markers.is_empty());
        }

        #[test]
        fn test_with_parser() {
            let custom_parser = LineParserConfig {
                line_prefix: Some("custom: ".to_string()),
                ..Default::default()
            };

            let config = StreamFormatConfig::sse().with_parser(custom_parser);
            assert_eq!(
                config.parser_config().line_prefix,
                Some("custom: ".to_string())
            );
        }

        #[test]
        fn test_with_end_marker() {
            let config = StreamFormatConfig::ndjson()
                .with_end_marker("[END]")
                .with_end_marker("[STOP]");

            assert_eq!(config.end_markers, vec!["[END]", "[STOP]"]);
        }

        #[test]
        fn test_is_end_marker() {
            let config = StreamFormatConfig::sse();
            assert!(config.is_end_marker("[DONE]"));
            assert!(config.is_end_marker("  [DONE]  ")); // With whitespace
            assert!(!config.is_end_marker("{\"content\": \"hello\"}"));
        }

        #[test]
        fn test_create_parser() {
            let config = StreamFormatConfig::sse();
            let parser = config.create_parser();
            assert_eq!(parser.config().line_prefix, Some("data: ".to_string()));
        }

        #[test]
        fn test_serialize_deserialize() {
            let config = StreamFormatConfig::sse();
            let json = serde_json::to_string(&config).unwrap();

            let parsed: StreamFormatConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.format, StreamFormat::TextEventStream);
            assert_eq!(parsed.end_markers, vec!["[DONE]".to_string()]);
        }
    }

    // -------------------------------------------------------------------------
    // Integration Tests
    // -------------------------------------------------------------------------

    mod integration {
        use super::*;

        #[test]
        fn test_parse_openai_sse_stream() {
            let parser = LineParser::sse();

            // Typical OpenAI SSE stream
            let lines = vec![
                "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}",
                "",
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}",
                "",
                "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}",
                "",
                "data: [DONE]",
            ];

            let mut data_lines = Vec::new();
            let mut empty_count = 0;

            for line in lines {
                match parser.parse_line(line) {
                    ParsedLine::Data(data) => data_lines.push(data.to_string()),
                    ParsedLine::Empty => empty_count += 1,
                    _ => {}
                }
            }

            assert_eq!(data_lines.len(), 4);
            assert_eq!(empty_count, 3);
            assert!(data_lines[0].contains("role"));
            assert!(data_lines[1].contains("Hello"));
            assert!(data_lines[2].contains("world"));
            assert_eq!(data_lines[3], "[DONE]");
        }

        #[test]
        fn test_parse_anthropic_sse_stream() {
            let parser = LineParser::sse();

            // Typical Anthropic SSE stream
            let lines = vec![
                "event: message_start",
                "data: {\"type\":\"message_start\",\"message\":{}}",
                "",
                "event: content_block_delta",
                "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hello\"}}",
                "",
                "event: message_stop",
                "data: {\"type\":\"message_stop\"}",
            ];

            let mut events = Vec::new();
            let mut data_lines = Vec::new();

            for line in lines {
                match parser.parse_line(line) {
                    ParsedLine::EventType(t) => events.push(t.to_string()),
                    ParsedLine::Data(d) => data_lines.push(d.to_string()),
                    _ => {}
                }
            }

            assert_eq!(events, vec!["message_start", "content_block_delta", "message_stop"]);
            assert_eq!(data_lines.len(), 3);
        }

        #[test]
        fn test_parse_ndjson_stream() {
            let parser = LineParser::ndjson();

            let lines = vec![
                "{\"content\":\"Hello\"}",
                "{\"content\":\" world\"}",
                "{\"done\":true}",
            ];

            let data_lines: Vec<_> = lines
                .iter()
                .filter_map(|line| parser.extract_data(line))
                .map(|d| d.to_string())
                .collect();

            assert_eq!(data_lines.len(), 3);
            assert!(data_lines[0].contains("Hello"));
            assert!(data_lines[1].contains("world"));
            assert!(data_lines[2].contains("done"));
        }

        #[test]
        fn test_end_to_end_with_format_config() {
            let config = StreamFormatConfig::sse();
            let parser = config.create_parser();

            let line = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}";
            let data = parser.extract_data(line).unwrap();
            assert!(data.contains("Hello"));

            // Check end marker
            let end_line = "data: [DONE]";
            let end_data = parser.extract_data(end_line).unwrap();
            assert!(config.is_end_marker(&end_data));
        }
    }
}
