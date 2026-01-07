use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ClientError>;
pub type Error = ClientError;

// ============================================================================
// Normalized API Error
// ============================================================================

/// Normalized API error extracted from provider-specific error responses.
///
/// Different providers return errors in different formats:
/// - OpenAI: `{"error": {"message": "...", "type": "...", "code": "..."}}`
/// - Anthropic: `{"type": "error", "error": {"type": "...", "message": "..."}}`
/// - Google: `{"error": {"message": "...", "code": 400, "status": "INVALID_ARGUMENT"}}`
///
/// This struct provides a unified representation that can be extracted from any provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiError {
    /// The human-readable error message
    pub message: String,
    /// The type/category of error (e.g., "invalid_request_error", "authentication_error")
    pub error_type: Option<String>,
    /// Provider-specific error code (e.g., "context_length_exceeded", "INVALID_ARGUMENT")
    pub code: Option<String>,
    /// The parameter that caused the error, if applicable
    pub param: Option<String>,
    /// HTTP-like status code, if provided in the response body
    pub status: Option<u16>,
}

impl ApiError {
    /// Create a new ApiError with just a message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: None,
            code: None,
            param: None,
            status: None,
        }
    }

    /// Builder method to set error type
    pub fn with_type(mut self, error_type: impl Into<String>) -> Self {
        self.error_type = Some(error_type.into());
        self
    }

    /// Builder method to set error code
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Builder method to set the parameter that caused the error
    pub fn with_param(mut self, param: impl Into<String>) -> Self {
        self.param = Some(param.into());
        self
    }

    /// Builder method to set HTTP status
    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(ref error_type) = self.error_type {
            write!(f, " (type: {})", error_type)?;
        }
        if let Some(ref code) = self.code {
            write!(f, " [code: {}]", code)?;
        }
        if let Some(ref param) = self.param {
            write!(f, " (param: {})", param)?;
        }
        if let Some(status) = self.status {
            write!(f, " [status: {}]", status)?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

// ============================================================================
// Error Extractor Configuration
// ============================================================================

/// Configuration for extracting errors from API responses.
///
/// Defines JSONPath-like paths to extract error information from
/// provider-specific error response formats.
///
/// # Example YAML Configuration
///
/// ```yaml
/// error:
///   message: error.message
///   type: error.type
///   code: error.code
///   param: error.param
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ErrorExtractorConfig {
    /// Path to error message (required for extraction to succeed)
    #[serde(rename = "message")]
    pub message_path: String,
    /// Path to error type/kind
    #[serde(rename = "type")]
    pub type_path: Option<String>,
    /// Path to error code
    #[serde(rename = "code")]
    pub code_path: Option<String>,
    /// Path to parameter that caused error
    #[serde(rename = "param")]
    pub param_path: Option<String>,
    /// Path to HTTP-like status in response body
    #[serde(rename = "status")]
    pub status_path: Option<String>,
}

impl ErrorExtractorConfig {
    /// Create a new ErrorExtractorConfig with the given message path
    pub fn new(message_path: impl Into<String>) -> Self {
        Self {
            message_path: message_path.into(),
            type_path: None,
            code_path: None,
            param_path: None,
            status_path: None,
        }
    }

    /// Create a builder for constructing ErrorExtractorConfig
    pub fn builder() -> ErrorExtractorBuilder {
        ErrorExtractorBuilder::new()
    }

    /// Extract an ApiError from a JSON response using this configuration.
    ///
    /// Returns `None` if the message cannot be extracted (message is required).
    pub fn extract(&self, json: &Value) -> Option<ApiError> {
        // Message is required
        let message = extract_string_at_path(&self.message_path, json)?;

        let error_type = self
            .type_path
            .as_ref()
            .and_then(|path| extract_string_at_path(path, json));

        let code = self
            .code_path
            .as_ref()
            .and_then(|path| extract_string_at_path(path, json));

        let param = self
            .param_path
            .as_ref()
            .and_then(|path| extract_string_at_path(path, json));

        let status = self.status_path.as_ref().and_then(|path| {
            let value = extract_value_at_path(path, json)?;
            // Status can be a number or a string representation of a number
            value
                .as_u64()
                .map(|n| n as u16)
                .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        });

        Some(ApiError {
            message,
            error_type,
            code,
            param,
            status,
        })
    }

    /// Predefined configuration for OpenAI error format
    ///
    /// OpenAI errors: `{"error": {"message": "...", "type": "...", "code": "...", "param": "..."}}`
    pub fn openai() -> Self {
        Self::builder()
            .message("error.message")
            .error_type("error.type")
            .code("error.code")
            .param("error.param")
            .build()
    }

    /// Predefined configuration for Anthropic error format
    ///
    /// Anthropic errors: `{"type": "error", "error": {"type": "...", "message": "..."}}`
    pub fn anthropic() -> Self {
        Self::builder()
            .message("error.message")
            .error_type("error.type")
            .code("error.code")
            .build()
    }

    /// Predefined configuration for Google error format
    ///
    /// Google errors: `{"error": {"message": "...", "code": 400, "status": "INVALID_ARGUMENT"}}`
    pub fn google() -> Self {
        Self::builder()
            .message("error.message")
            .code("error.status")
            .status("error.code")
            .build()
    }
}

// ============================================================================
// Error Extractor Builder
// ============================================================================

/// Builder for constructing ErrorExtractorConfig with a fluent API.
///
/// # Example
///
/// ```
/// use cllient::error::ErrorExtractorBuilder;
///
/// let config = ErrorExtractorBuilder::new()
///     .message("error.message")
///     .error_type("error.type")
///     .code("error.code")
///     .param("error.param")
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct ErrorExtractorBuilder {
    message_path: Option<String>,
    type_path: Option<String>,
    code_path: Option<String>,
    param_path: Option<String>,
    status_path: Option<String>,
}

impl ErrorExtractorBuilder {
    /// Create a new ErrorExtractorBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the path to extract the error message (required)
    pub fn message(mut self, path: impl Into<String>) -> Self {
        self.message_path = Some(path.into());
        self
    }

    /// Set the path to extract the error type
    pub fn error_type(mut self, path: impl Into<String>) -> Self {
        self.type_path = Some(path.into());
        self
    }

    /// Set the path to extract the error code
    pub fn code(mut self, path: impl Into<String>) -> Self {
        self.code_path = Some(path.into());
        self
    }

    /// Set the path to extract the parameter that caused the error
    pub fn param(mut self, path: impl Into<String>) -> Self {
        self.param_path = Some(path.into());
        self
    }

    /// Set the path to extract the HTTP status code from the response body
    pub fn status(mut self, path: impl Into<String>) -> Self {
        self.status_path = Some(path.into());
        self
    }

    /// Build the ErrorExtractorConfig
    ///
    /// # Panics
    ///
    /// Panics if `message` path was not set.
    pub fn build(self) -> ErrorExtractorConfig {
        ErrorExtractorConfig {
            message_path: self
                .message_path
                .expect("ErrorExtractorConfig requires a message path"),
            type_path: self.type_path,
            code_path: self.code_path,
            param_path: self.param_path,
            status_path: self.status_path,
        }
    }

    /// Build the ErrorExtractorConfig, returning None if message path is not set
    pub fn try_build(self) -> Option<ErrorExtractorConfig> {
        Some(ErrorExtractorConfig {
            message_path: self.message_path?,
            type_path: self.type_path,
            code_path: self.code_path,
            param_path: self.param_path,
            status_path: self.status_path,
        })
    }
}

// ============================================================================
// Path Extraction Utilities
// ============================================================================

/// Extract a value at a JSONPath-like path from a JSON value.
///
/// Supports paths like:
/// - `error.message` - nested object access
/// - `choices[0].message.content` - array index access
/// - `error.details[0].field` - mixed access
fn extract_value_at_path(path: &str, json: &Value) -> Option<Value> {
    let mut current = json;
    let mut i = 0;
    let chars: Vec<char> = path.chars().collect();

    while i < chars.len() {
        // Find the next part (either before '.' or before '[')
        let mut part_end = i;
        while part_end < chars.len() && chars[part_end] != '.' && chars[part_end] != '[' {
            part_end += 1;
        }

        if part_end > i {
            // Extract object key
            let key: String = chars[i..part_end].iter().collect();
            current = current.get(&key)?;
            i = part_end;
        }

        // Handle array access
        if i < chars.len() && chars[i] == '[' {
            i += 1; // Skip '['
            let mut index_end = i;
            while index_end < chars.len() && chars[index_end] != ']' {
                index_end += 1;
            }

            if index_end >= chars.len() {
                return None; // Unclosed bracket
            }

            let index_str: String = chars[i..index_end].iter().collect();
            let index: usize = index_str.parse().ok()?;
            current = current.get(index)?;
            i = index_end + 1; // Skip ']'
        }

        // Skip '.'
        if i < chars.len() && chars[i] == '.' {
            i += 1;
        }
    }

    Some(current.clone())
}

/// Extract a string value at a JSONPath-like path.
///
/// If the value at the path is not a string, attempts to convert it:
/// - Numbers are converted to their string representation
/// - Other types return None
fn extract_string_at_path(path: &str, json: &Value) -> Option<String> {
    let value = extract_value_at_path(path, json)?;
    match value {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("YAML serialization error: {0}")]
    YamlSerialization(#[from] serde_yaml::Error),
    
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),
    
    #[error("Template error: {0}")]
    Template(#[from] Box<handlebars::TemplateError>),
    
    #[error("Render error: {0}")]
    Render(#[from] handlebars::RenderError),
    
    #[error("JSON path error: {0}")]
    JsonPath(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Stream error: {0}")]
    Stream(String),
    
    #[error("Rate limit exceeded")]
    RateLimit,
    
    #[error("Authentication failed")]
    Auth,
    
    #[error("Invalid model: {0}")]
    InvalidModel(String),

    #[error("Invalid service: {0}")]
    InvalidService(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Invalid configuration path: {0}")]
    InvalidPath(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid YAML: {0}")]
    InvalidYaml(String),

    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),

    #[error("Broken references in config: {}", .0.join("; "))]
    BrokenReferences(Vec<String>),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ------------------------------------------------------------------------
    // ApiError Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_api_error_new() {
        let error = ApiError::new("Something went wrong");
        assert_eq!(error.message, "Something went wrong");
        assert_eq!(error.error_type, None);
        assert_eq!(error.code, None);
        assert_eq!(error.param, None);
        assert_eq!(error.status, None);
    }

    #[test]
    fn test_api_error_builder_methods() {
        let error = ApiError::new("Invalid request")
            .with_type("invalid_request_error")
            .with_code("context_length_exceeded")
            .with_param("messages")
            .with_status(400);

        assert_eq!(error.message, "Invalid request");
        assert_eq!(error.error_type, Some("invalid_request_error".to_string()));
        assert_eq!(error.code, Some("context_length_exceeded".to_string()));
        assert_eq!(error.param, Some("messages".to_string()));
        assert_eq!(error.status, Some(400));
    }

    #[test]
    fn test_api_error_display() {
        let error = ApiError::new("Rate limit exceeded")
            .with_type("rate_limit_error")
            .with_code("rate_limit")
            .with_status(429);

        let display = format!("{}", error);
        assert!(display.contains("Rate limit exceeded"));
        assert!(display.contains("type: rate_limit_error"));
        assert!(display.contains("code: rate_limit"));
        assert!(display.contains("status: 429"));
    }

    #[test]
    fn test_api_error_display_minimal() {
        let error = ApiError::new("Simple error");
        assert_eq!(format!("{}", error), "Simple error");
    }

    #[test]
    fn test_api_error_serialization() {
        let error = ApiError::new("Test error")
            .with_type("test_type")
            .with_code("test_code");

        let json = serde_json::to_string(&error).unwrap();
        let deserialized: ApiError = serde_json::from_str(&json).unwrap();

        assert_eq!(error, deserialized);
    }

    // ------------------------------------------------------------------------
    // ErrorExtractorBuilder Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_error_extractor_builder_basic() {
        let config = ErrorExtractorBuilder::new()
            .message("error.message")
            .build();

        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, None);
        assert_eq!(config.code_path, None);
        assert_eq!(config.param_path, None);
        assert_eq!(config.status_path, None);
    }

    #[test]
    fn test_error_extractor_builder_full() {
        let config = ErrorExtractorBuilder::new()
            .message("error.message")
            .error_type("error.type")
            .code("error.code")
            .param("error.param")
            .status("error.status")
            .build();

        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, Some("error.type".to_string()));
        assert_eq!(config.code_path, Some("error.code".to_string()));
        assert_eq!(config.param_path, Some("error.param".to_string()));
        assert_eq!(config.status_path, Some("error.status".to_string()));
    }

    #[test]
    #[should_panic(expected = "ErrorExtractorConfig requires a message path")]
    fn test_error_extractor_builder_missing_message() {
        ErrorExtractorBuilder::new()
            .error_type("error.type")
            .build();
    }

    #[test]
    fn test_error_extractor_builder_try_build() {
        let config = ErrorExtractorBuilder::new()
            .message("error.message")
            .try_build();

        assert!(config.is_some());

        let config = ErrorExtractorBuilder::new()
            .error_type("error.type")
            .try_build();

        assert!(config.is_none());
    }

    // ------------------------------------------------------------------------
    // ErrorExtractorConfig Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_error_extractor_config_new() {
        let config = ErrorExtractorConfig::new("error.message");
        assert_eq!(config.message_path, "error.message");
    }

    #[test]
    fn test_error_extractor_config_builder() {
        let config = ErrorExtractorConfig::builder()
            .message("error.message")
            .error_type("error.type")
            .build();

        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, Some("error.type".to_string()));
    }

    // ------------------------------------------------------------------------
    // OpenAI Error Extraction Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_openai_error() {
        let config = ErrorExtractorConfig::openai();
        let response = json!({
            "error": {
                "message": "This model's maximum context length is 8192 tokens.",
                "type": "invalid_request_error",
                "code": "context_length_exceeded",
                "param": "messages"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "This model's maximum context length is 8192 tokens.");
        assert_eq!(error.error_type, Some("invalid_request_error".to_string()));
        assert_eq!(error.code, Some("context_length_exceeded".to_string()));
        assert_eq!(error.param, Some("messages".to_string()));
        assert_eq!(error.status, None);
    }

    #[test]
    fn test_extract_openai_auth_error() {
        let config = ErrorExtractorConfig::openai();
        let response = json!({
            "error": {
                "message": "Incorrect API key provided",
                "type": "invalid_request_error",
                "code": "invalid_api_key"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "Incorrect API key provided");
        assert_eq!(error.error_type, Some("invalid_request_error".to_string()));
        assert_eq!(error.code, Some("invalid_api_key".to_string()));
        assert_eq!(error.param, None);
    }

    // ------------------------------------------------------------------------
    // Anthropic Error Extraction Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_anthropic_error() {
        let config = ErrorExtractorConfig::anthropic();
        let response = json!({
            "type": "error",
            "error": {
                "type": "invalid_request_error",
                "message": "max_tokens: must be less than 4096"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "max_tokens: must be less than 4096");
        assert_eq!(error.error_type, Some("invalid_request_error".to_string()));
    }

    #[test]
    fn test_extract_anthropic_overloaded_error() {
        let config = ErrorExtractorConfig::anthropic();
        let response = json!({
            "type": "error",
            "error": {
                "type": "overloaded_error",
                "message": "Overloaded"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "Overloaded");
        assert_eq!(error.error_type, Some("overloaded_error".to_string()));
    }

    // ------------------------------------------------------------------------
    // Google Error Extraction Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_google_error() {
        let config = ErrorExtractorConfig::google();
        let response = json!({
            "error": {
                "message": "Invalid argument: messages",
                "code": 400,
                "status": "INVALID_ARGUMENT"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "Invalid argument: messages");
        assert_eq!(error.code, Some("INVALID_ARGUMENT".to_string()));
        assert_eq!(error.status, Some(400));
    }

    #[test]
    fn test_extract_google_quota_error() {
        let config = ErrorExtractorConfig::google();
        let response = json!({
            "error": {
                "message": "Quota exceeded for aiplatform.googleapis.com",
                "code": 429,
                "status": "RESOURCE_EXHAUSTED"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "Quota exceeded for aiplatform.googleapis.com");
        assert_eq!(error.code, Some("RESOURCE_EXHAUSTED".to_string()));
        assert_eq!(error.status, Some(429));
    }

    // ------------------------------------------------------------------------
    // Path Extraction Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_simple_path() {
        let json = json!({"message": "hello"});
        let result = extract_string_at_path("message", &json);
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn test_extract_nested_path() {
        let json = json!({"error": {"message": "nested error"}});
        let result = extract_string_at_path("error.message", &json);
        assert_eq!(result, Some("nested error".to_string()));
    }

    #[test]
    fn test_extract_deeply_nested_path() {
        let json = json!({
            "response": {
                "error": {
                    "details": {
                        "message": "very nested"
                    }
                }
            }
        });
        let result = extract_string_at_path("response.error.details.message", &json);
        assert_eq!(result, Some("very nested".to_string()));
    }

    #[test]
    fn test_extract_array_path() {
        let json = json!({"errors": [{"message": "first"}, {"message": "second"}]});
        let result = extract_string_at_path("errors[0].message", &json);
        assert_eq!(result, Some("first".to_string()));

        let result = extract_string_at_path("errors[1].message", &json);
        assert_eq!(result, Some("second".to_string()));
    }

    #[test]
    fn test_extract_mixed_path() {
        let json = json!({
            "data": {
                "items": [
                    {"name": "item1"},
                    {"name": "item2", "details": {"code": "ABC"}}
                ]
            }
        });
        let result = extract_string_at_path("data.items[1].details.code", &json);
        assert_eq!(result, Some("ABC".to_string()));
    }

    #[test]
    fn test_extract_number_as_string() {
        let json = json!({"error": {"code": 400}});
        let result = extract_string_at_path("error.code", &json);
        assert_eq!(result, Some("400".to_string()));
    }

    #[test]
    fn test_extract_missing_path() {
        let json = json!({"error": {"message": "test"}});
        let result = extract_string_at_path("error.code", &json);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_missing_nested_path() {
        let json = json!({"data": {}});
        let result = extract_string_at_path("data.error.message", &json);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_array_out_of_bounds() {
        let json = json!({"items": [{"id": 1}]});
        let result = extract_string_at_path("items[5].id", &json);
        assert_eq!(result, None);
    }

    // ------------------------------------------------------------------------
    // Error Extraction Edge Cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_returns_none_when_message_missing() {
        let config = ErrorExtractorConfig::openai();
        let response = json!({
            "error": {
                "type": "invalid_request_error",
                "code": "some_code"
            }
        });

        let error = config.extract(&response);
        assert!(error.is_none());
    }

    #[test]
    fn test_extract_with_partial_fields() {
        let config = ErrorExtractorConfig::openai();
        let response = json!({
            "error": {
                "message": "Partial error"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "Partial error");
        assert_eq!(error.error_type, None);
        assert_eq!(error.code, None);
        assert_eq!(error.param, None);
    }

    #[test]
    fn test_extract_empty_response() {
        let config = ErrorExtractorConfig::openai();
        let response = json!({});

        let error = config.extract(&response);
        assert!(error.is_none());
    }

    #[test]
    fn test_extract_null_values() {
        let config = ErrorExtractorConfig::openai();
        let response = json!({
            "error": {
                "message": "Error with nulls",
                "type": null,
                "code": null
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.message, "Error with nulls");
        // Null values should result in None
        assert_eq!(error.error_type, None);
        assert_eq!(error.code, None);
    }

    // ------------------------------------------------------------------------
    // YAML Serialization Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_error_extractor_config_yaml_serialization() {
        let config = ErrorExtractorConfig::openai();
        let yaml = serde_yaml::to_string(&config).unwrap();

        // The YAML should use the renamed field names
        assert!(yaml.contains("message:"));
        assert!(yaml.contains("type:"));
        assert!(yaml.contains("code:"));
        assert!(yaml.contains("param:"));

        let deserialized: ErrorExtractorConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_error_extractor_config_from_yaml() {
        let yaml = r#"
message: error.message
type: error.type
code: error.code
param: error.param
status: error.status
"#;

        let config: ErrorExtractorConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, Some("error.type".to_string()));
        assert_eq!(config.code_path, Some("error.code".to_string()));
        assert_eq!(config.param_path, Some("error.param".to_string()));
        assert_eq!(config.status_path, Some("error.status".to_string()));
    }

    #[test]
    fn test_error_extractor_config_from_yaml_minimal() {
        let yaml = r#"message: error.message"#;

        let config: ErrorExtractorConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, None);
        assert_eq!(config.code_path, None);
        assert_eq!(config.param_path, None);
        assert_eq!(config.status_path, None);
    }

    // ------------------------------------------------------------------------
    // Predefined Config Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_openai_predefined_config() {
        let config = ErrorExtractorConfig::openai();
        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, Some("error.type".to_string()));
        assert_eq!(config.code_path, Some("error.code".to_string()));
        assert_eq!(config.param_path, Some("error.param".to_string()));
        assert_eq!(config.status_path, None);
    }

    #[test]
    fn test_anthropic_predefined_config() {
        let config = ErrorExtractorConfig::anthropic();
        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, Some("error.type".to_string()));
        assert_eq!(config.code_path, Some("error.code".to_string()));
        assert_eq!(config.param_path, None);
        assert_eq!(config.status_path, None);
    }

    #[test]
    fn test_google_predefined_config() {
        let config = ErrorExtractorConfig::google();
        assert_eq!(config.message_path, "error.message");
        assert_eq!(config.type_path, None);
        assert_eq!(config.code_path, Some("error.status".to_string()));
        assert_eq!(config.param_path, None);
        assert_eq!(config.status_path, Some("error.code".to_string()));
    }

    // ------------------------------------------------------------------------
    // Status Extraction Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_extract_status_as_number() {
        let config = ErrorExtractorBuilder::new()
            .message("error.message")
            .status("error.code")
            .build();

        let response = json!({
            "error": {
                "message": "Bad request",
                "code": 400
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.status, Some(400));
    }

    #[test]
    fn test_extract_status_as_string() {
        let config = ErrorExtractorBuilder::new()
            .message("error.message")
            .status("error.code")
            .build();

        let response = json!({
            "error": {
                "message": "Bad request",
                "code": "500"
            }
        });

        let error = config.extract(&response).unwrap();
        assert_eq!(error.status, Some(500));
    }

    #[test]
    fn test_extract_status_invalid_string() {
        let config = ErrorExtractorBuilder::new()
            .message("error.message")
            .status("error.code")
            .build();

        let response = json!({
            "error": {
                "message": "Bad request",
                "code": "not_a_number"
            }
        });

        let error = config.extract(&response).unwrap();
        // Invalid string should result in None for status
        assert_eq!(error.status, None);
    }
}