use handlebars::{Handlebars, Helper, Context, RenderContext, Output, HelperResult};
use serde_json::Value;
use std::collections::HashMap;
use regex::Regex;
use base64::Engine;
use crate::error::{ClientError, Result};
use tracing::{debug, trace};

pub struct TemplateProcessor {
    handlebars: Handlebars<'static>,
}

impl TemplateProcessor {
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();
        
        // Register custom helpers
        handlebars.register_helper("json", Box::new(json_helper));
        handlebars.register_helper("env", Box::new(env_helper));
        
        // Disable HTML escaping since we're dealing with JSON/HTTP
        handlebars.register_escape_fn(handlebars::no_escape);
        
        Self { handlebars }
    }

    pub fn process_http_template(
        &self,
        template: &str,
        variables: &HashMap<String, Value>,
    ) -> Result<HttpRequest> {
        // First, substitute environment variables
        let template_with_env = self.substitute_env_vars(template)?;
        
        // Log template with env vars at trace level
        trace!("Template with environment variables substituted:\n{}", template_with_env);
        
        // Then process with handlebars
        let rendered = self.handlebars.render_template(&template_with_env, variables)?;
        
        // Log rendered template at debug level with sensitive data hidden
        debug!("Rendered HTTP template:\n{}", self.sanitize_for_logging(&rendered));
        
        // Clean up empty JSON fields
        let cleaned_rendered = self.clean_empty_json_fields(&rendered)?;
        
        // Log cleaned template at debug level with sensitive data hidden
        debug!("Cleaned HTTP template:\n{}", self.sanitize_for_logging(&cleaned_rendered));
        
        // Parse the HTTP request
        let request = self.parse_http_request(&cleaned_rendered)?;
        
        // Log parsed request body at debug level with sensitive data hidden
        debug!("Final request body: {}", self.sanitize_for_logging(&request.body));
        
        Ok(request)
    }

    fn substitute_env_vars(&self, template: &str) -> Result<String> {
        let env_var_regex = Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").unwrap();
        
        let result = env_var_regex.replace_all(template, |caps: &regex::Captures| {
            let var_name = &caps[1];
            std::env::var(var_name).unwrap_or_else(|_| {
                format!("${{{}}}", var_name) // Keep original if not found
            })
        });
        
        Ok(result.to_string())
    }

    /// Sanitize sensitive information for logging by replacing API keys and auth tokens with asterisks
    fn sanitize_for_logging(&self, content: &str) -> String {
        let mut sanitized = content.to_string();
        
        // List of common API key patterns to redact
        let sensitive_patterns = vec![
            // Authorization headers (Bearer token)
            (Regex::new(r"(?i)(authorization:\s*bearer\s+)([^\s\n]+)").unwrap(), "${1}*****"),
            
            // Common API key headers
            (Regex::new(r"(?i)(x-api-key:\s*)([^\s\n]+)").unwrap(), "${1}*****"),
            (Regex::new(r"(?i)(api-key:\s*)([^\s\n]+)").unwrap(), "${1}*****"),
            (Regex::new(r"(?i)(anthropic-version:\s*[^\n]*\n\s*x-api-key:\s*)([^\s\n]+)").unwrap(), "${1}*****"),
            
            // OpenAI specific headers  
            (Regex::new(r"(?i)(openai-api-key:\s*)([^\s\n]+)").unwrap(), "${1}*****"),
            
            // API keys in JSON bodies
            (Regex::new(r#"(?i)("api_key":\s*")([^"]+)""#).unwrap(), r#"${1}*****""#),
            (Regex::new(r#"(?i)("key":\s*")([^"]+)""#).unwrap(), r#"${1}*****""#),
            
            // JWT-like tokens (long base64-ish strings) - should go after Bearer pattern
            (Regex::new(r"(sk-[a-zA-Z0-9]{20,})").unwrap(), "*****"),
        ];
        
        for (pattern, replacement) in sensitive_patterns {
            sanitized = pattern.replace_all(&sanitized, replacement).to_string();
        }
        
        sanitized
    }

    fn clean_empty_json_fields(&self, content: &str) -> Result<String> {
        // Find JSON content in the HTTP body
        let lines: Vec<&str> = content.lines().collect();
        let mut result_lines = Vec::new();
        let mut in_json = false;
        let mut json_content = Vec::new();
        
        for line in lines {
            if line.trim().starts_with('{') {
                in_json = true;
                json_content.clear();
                json_content.push(line);
            } else if in_json && line.trim().ends_with('}') {
                json_content.push(line);
                
                // Process the JSON block
                let json_block = json_content.join("\n");
                let cleaned_json = self.clean_json_block(&json_block)?;
                result_lines.extend(cleaned_json.lines().map(|s| s.to_string()));
                
                in_json = false;
            } else if in_json {
                json_content.push(line);
            } else {
                result_lines.push(line.to_string());
            }
        }
        
        Ok(result_lines.join("\n"))
    }

    fn clean_json_block(&self, json_str: &str) -> Result<String> {
        let lines: Vec<&str> = json_str.lines().collect();
        let mut cleaned_lines = Vec::new();
        
        for line in lines {
            let trimmed = line.trim();
            // Skip lines that are just field name followed by empty value and comma
            // e.g. "temperature": ,
            // But be careful not to skip valid null values: "temperature": null,
            if trimmed.contains(": ,") && !trimmed.contains(": null,") {
                continue;
            }
            cleaned_lines.push(line);
        }
        
        let mut result = cleaned_lines.join("\n");
        
        // Remove trailing commas before closing braces
        let trailing_comma_regex = Regex::new(r#",(\s*[}\]])"#).unwrap();
        result = trailing_comma_regex.replace_all(&result, "$1").to_string();
        
        Ok(result)
    }

    fn parse_http_request(&self, http_text: &str) -> Result<HttpRequest> {
        let lines: Vec<&str> = http_text.lines().collect();
        
        if lines.is_empty() {
            return Err(ClientError::Render(handlebars::RenderError::new(
                "Empty HTTP template"
            )));
        }

        // Skip empty lines at the beginning
        let mut start_idx = 0;
        while start_idx < lines.len() && lines[start_idx].trim().is_empty() {
            start_idx += 1;
        }
        
        if start_idx >= lines.len() {
            return Err(ClientError::Render(handlebars::RenderError::new(
                "Empty HTTP template"
            )));
        }

        // Parse request line (e.g., "POST /v1/messages HTTP/1.1")
        let request_line = lines[start_idx].trim();
        let request_parts: Vec<&str> = request_line.split_whitespace().collect();
        
        if request_parts.len() < 2 {
            return Err(ClientError::Render(handlebars::RenderError::new(
                "Invalid HTTP request line"
            )));
        }

        let method = request_parts[0].to_string();
        let mut url = request_parts[1].to_string();
        
        // If URL doesn't start with http, it's a path - we'll need base_url later
        let is_full_url = url.starts_with("http://") || url.starts_with("https://");
        
        // Parse headers
        let mut headers = HashMap::new();
        let mut header_end = start_idx + 1;
        
        for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
            if line.trim().is_empty() {
                header_end = i;
                break;
            }
            
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }
        
        // Parse body (everything after the empty line)
        let body = if header_end + 1 < lines.len() {
            lines[header_end + 1..].join("\n").trim().to_string()
        } else {
            String::new()
        };

        // Extract host from headers if URL is not full
        if !is_full_url {
            if let Some(host) = headers.get("Host").or_else(|| headers.get("host")) {
                url = format!("https://{}{}", host, url);
            }
        }

        Ok(HttpRequest {
            method,
            url,
            headers,
            body,
        })
    }
}

impl Default for TemplateProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

// Helper function for JSON serialization
fn json_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h.param(0).unwrap().value();
    let json_str = serde_json::to_string(value)?;
    out.write(&json_str)?;
    Ok(())
}

// Helper function for environment variables
fn env_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let var_name = h.param(0).unwrap().value().as_str().unwrap_or("");
    let env_value = std::env::var(var_name).unwrap_or_default();
    out.write(&env_value)?;
    Ok(())
}

pub struct MessageBuilder;

impl MessageBuilder {
    pub fn build_messages(
        builder_type: &str,
        messages: &[crate::types::MessageContent],
    ) -> Result<Value> {
        match builder_type {
            "anthropic" => Self::build_anthropic_messages(messages),
            "openai" => Self::build_openai_messages(messages),
            _ => Err(ClientError::Render(handlebars::RenderError::new(
                format!("Unknown message builder: {}", builder_type)
            ))),
        }
    }

    fn build_anthropic_messages(messages: &[crate::types::MessageContent]) -> Result<Value> {
        use crate::types::{MessageContent, ContentBlock};
        
        let mut result = Vec::new();
        
        for message in messages {
            match message {
                MessageContent::Text { role, content } => {
                    result.push(serde_json::json!({
                        "role": role,
                        "content": content
                    }));
                }
                MessageContent::Multimodal { role, content } => {
                    let mut content_array = Vec::new();
                    
                    for block in content {
                        match block {
                            ContentBlock::Text(text) => {
                                content_array.push(serde_json::json!({
                                    "type": "text",
                                    "text": text
                                }));
                            }
                            ContentBlock::Binary { data, mime_type, .. } => {
                                let base64_data = base64::prelude::BASE64_STANDARD.encode(data);
                                content_array.push(serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": mime_type,
                                        "data": base64_data
                                    }
                                }));
                            }
                            ContentBlock::Url { url, mime_type } => {
                                if mime_type.as_ref().is_some_and(|mt| mt.starts_with("image/")) {
                                    content_array.push(serde_json::json!({
                                        "type": "image",
                                        "source": {
                                            "type": "url",
                                            "url": url
                                        }
                                    }));
                                }
                            }
                        }
                    }
                    
                    result.push(serde_json::json!({
                        "role": role,
                        "content": content_array
                    }));
                }
            }
        }
        
        Ok(Value::Array(result))
    }

    fn build_openai_messages(messages: &[crate::types::MessageContent]) -> Result<Value> {
        use crate::types::{MessageContent, ContentBlock};
        
        let mut result = Vec::new();
        
        for message in messages {
            match message {
                MessageContent::Text { role, content } => {
                    result.push(serde_json::json!({
                        "role": role,
                        "content": content
                    }));
                }
                MessageContent::Multimodal { role, content } => {
                    let mut content_array = Vec::new();
                    
                    for block in content {
                        match block {
                            ContentBlock::Text(text) => {
                                content_array.push(serde_json::json!({
                                    "type": "text",
                                    "text": text
                                }));
                            }
                            ContentBlock::Binary { data, mime_type, .. } => {
                                let base64_data = base64::prelude::BASE64_STANDARD.encode(data);
                                
                                if mime_type.starts_with("image/") {
                                    content_array.push(serde_json::json!({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": format!("data:{};base64,{}", mime_type, base64_data)
                                        }
                                    }));
                                } else if mime_type.starts_with("audio/") {
                                    content_array.push(serde_json::json!({
                                        "type": "audio",
                                        "source_type": "base64",
                                        "mime_type": mime_type,
                                        "data": base64_data
                                    }));
                                }
                            }
                            ContentBlock::Url { url, .. } => {
                                content_array.push(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {
                                        "url": url
                                    }
                                }));
                            }
                        }
                    }
                    
                    result.push(serde_json::json!({
                        "role": role,
                        "content": content_array
                    }));
                }
            }
        }
        
        Ok(Value::Array(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_template_processing() {
        let processor = TemplateProcessor::new();
        
        let template = r#"
POST /v1/messages HTTP/1.1
Host: api.anthropic.com
Content-Type: application/json
x-api-key: ${ANTHROPIC_API_KEY}

{
  "model": "{{model_id}}",
  "messages": {{json messages}},
  "max_tokens": {{max_tokens}}
}
"#;

        let mut variables = HashMap::new();
        variables.insert("model_id".to_string(), json!("claude-3-opus"));
        variables.insert("messages".to_string(), json!([{"role": "user", "content": "hello"}]));
        variables.insert("max_tokens".to_string(), json!(100));

        std::env::set_var("ANTHROPIC_API_KEY", "test-key");
        
        let result = processor.process_http_template(template, &variables).unwrap();
        
        assert_eq!(result.method, "POST");
        assert!(result.url.contains("api.anthropic.com"));
        assert_eq!(result.headers.get("Content-Type").unwrap(), "application/json");
        assert!(result.body.contains("claude-3-opus"));
    }

    #[test]
    fn test_message_builders() {
        use crate::types::MessageContent;
        
        let messages = vec![
            MessageContent::Text {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }
        ];
        
        let anthropic_messages = MessageBuilder::build_anthropic_messages(&messages).unwrap();
        let openai_messages = MessageBuilder::build_openai_messages(&messages).unwrap();
        
        // Both formats should have 1 message (user message)
        assert_eq!(anthropic_messages.as_array().unwrap().len(), 1);
        assert_eq!(openai_messages.as_array().unwrap().len(), 1);
        
        // Check message structure
        assert_eq!(
            anthropic_messages.as_array().unwrap()[0]["role"],
            "user"
        );
        assert_eq!(
            anthropic_messages.as_array().unwrap()[0]["content"],
            "Hello"
        );
    }

    #[test]
    fn test_sanitize_for_logging() {
        let processor = TemplateProcessor::new();
        
        // Test API key hiding in headers
        let content_with_api_key = r#"
POST /v1/messages HTTP/1.1
Host: api.anthropic.com
x-api-key: sk-ant-api03-abcdef1234567890
Authorization: Bearer sk-proj-abcdef1234567890

{
  "model": "claude-3-opus"
}
"#;
        
        let sanitized = processor.sanitize_for_logging(content_with_api_key);
        
        // API keys should be replaced with asterisks
        assert!(sanitized.contains("x-api-key: *****"));
        assert!(sanitized.contains("Authorization: Bearer *****"));
        assert!(!sanitized.contains("sk-ant-api03-abcdef1234567890"));
        assert!(!sanitized.contains("sk-proj-abcdef1234567890"));
        
        // Non-sensitive content should remain unchanged
        assert!(sanitized.contains("POST /v1/messages HTTP/1.1"));
        assert!(sanitized.contains("Host: api.anthropic.com"));
        assert!(sanitized.contains(r#""model": "claude-3-opus""#));
    }

    #[test]
    fn test_template_with_null_values() {
        let processor = TemplateProcessor::new();
        
        let template = r#"
POST /v1/messages HTTP/1.1
Host: api.anthropic.com
Content-Type: application/json

{
  "model": "{{model_id}}",
  "system": {{json system}},
  "max_tokens": {{max_tokens}},
  "temperature": {{temperature}}
}
"#;

        let mut variables = HashMap::new();
        variables.insert("model_id".to_string(), json!("claude-3-opus"));
        variables.insert("system".to_string(), json!(null));
        variables.insert("max_tokens".to_string(), json!(4096));
        variables.insert("temperature".to_string(), json!(1.0));

        let result = processor.process_http_template(template, &variables).unwrap();
        
        // Verify that null values are preserved
        assert!(result.body.contains(r#""system": null"#));
        assert!(result.body.contains(r#""max_tokens": 4096"#));
        assert!(result.body.contains(r#""temperature": 1"#));
    }
    
    #[test]
    fn test_template_cleaning_edge_cases() {
        let processor = TemplateProcessor::new();
        
        // Test cleaning of various empty value patterns
        let json_with_empties = r#"{
  "field1": "value1",
  "field2": ,
  "field3": null,
  "field4": ,
  "field5": "value5"
}"#;
        
        let cleaned = processor.clean_json_block(json_with_empties).unwrap();
        
        // Empty fields should be removed
        assert!(!cleaned.contains(r#""field2": ,"#));
        assert!(!cleaned.contains(r#""field4": ,"#));
        
        // Null fields should be preserved
        assert!(cleaned.contains(r#""field3": null"#));
        
        // Valid fields should remain
        assert!(cleaned.contains(r#""field1": "value1""#));
        assert!(cleaned.contains(r#""field5": "value5""#));
    }
}