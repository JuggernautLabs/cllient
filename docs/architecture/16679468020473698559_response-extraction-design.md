# Config-Driven Response Extraction System Design

**Date**: 2026-01-01
**Status**: Analysis Complete
**Component**: `src/client.rs` - Response Extraction (lines 153-377)

---

## 1. Current State Analysis

### 1.1 What Exists Today

The response extraction system uses a config-driven approach where each service defines JSONPath-like extraction rules in YAML:

```yaml
# Example from anthropic.yaml
response:
  extract:
    content: content[0].text
    role: role
    finish_reason: stop_reason
    usage:
      input: usage.input_tokens
      output: usage.output_tokens
```

**Implementation Location**: `src/client.rs` (lines 153-377)

Key methods:
- `extract_response_data()` - Orchestrates extraction using config paths
- `extract_json_path()` - Simple dot-notation path extraction
- `extract_complex_json_path()` - Handles array indexing like `choices[0].message.content`

### 1.2 Config-Driven vs Hardcoded Assessment

| Aspect | Config-Driven | Hardcoded | Notes |
|--------|---------------|-----------|-------|
| Field paths | Yes | - | All paths come from YAML config |
| Field names | Yes | - | `content`, `role`, `finish_reason`, `usage` are configurable |
| Type coercion | - | **Yes** | `.as_str()`, `.as_u64()` are hardcoded |
| Output structure | - | **Yes** | `CompletionResponse` struct is fixed |
| Error extraction | Partial | - | Paths configurable but not used |
| Optional handling | - | **Yes** | Which fields are optional is hardcoded |

**Verdict**: The system is ~70% config-driven. Paths are fully configurable but type handling, output structure, and optional field logic are hardcoded.

### 1.3 Provider Variations Discovered

Analyzing the 9 service configs reveals three response structure families:

#### OpenAI Family (6 services)
```
content: choices[0].message.content
role: choices[0].message.role
finish_reason: choices[0].finish_reason
usage.input: usage.prompt_tokens
usage.output: usage.completion_tokens
```
Used by: openai.yaml, openrouter.yaml, azure.yaml, deepseek.yaml, avian.yaml

#### Anthropic Family (1 service)
```
content: content[0].text
role: role
finish_reason: stop_reason
usage.input: usage.input_tokens
usage.output: usage.output_tokens
```
Used by: anthropic.yaml

#### Google Family (1 service)
```
content: candidates[0].content.parts[0].text
role: candidates[0].content.role
finish_reason: candidates[0].finishReason
usage.input: usageMetadata.promptTokenCount
usage.output: usageMetadata.candidatesTokenCount
```
Used by: google.yaml

#### Legacy Completions (1 service)
```
content: choices[0].text    # No .message wrapper
role: (not present)
finish_reason: choices[0].finish_reason
```
Used by: openai-completions.yaml

---

## 2. Bottlenecks and Limitations

### 2.1 No Transformation Support

**Problem**: Paths can only extract values, not transform them.

Real-world needs that cannot be expressed:
```yaml
# Cannot do: concatenate all choices
content: choices[*].message.content | join

# Cannot do: normalize finish reasons
finish_reason: stop_reason | lowercase | map("stop" => "complete", "length" => "max_tokens")

# Cannot do: calculate derived values
usage.total: usage.input_tokens + usage.output_tokens

# Cannot do: conditional extraction
content: content[0].text ?? choices[0].message.content
```

### 2.2 Hardcoded Type Coercion

**Problem**: Type conversion is embedded in Rust code.

```rust
// client.rs:170-171 - Always expects string content
value.as_str().unwrap_or("").to_string()

// client.rs:191-197 - Always expects u64 for usage
.as_u64().unwrap_or(0) as u32
```

This breaks for providers that return:
- Content as array of strings (needs joining)
- Token counts as strings (needs parsing)
- Nested usage objects (needs flattening)

### 2.3 Fixed Output Schema

**Problem**: `CompletionResponse` struct is rigid.

```rust
pub struct CompletionResponse {
    content: String,           // Always single string
    role: Option<String>,
    finish_reason: Option<String>,
    usage: Option<Usage>,
    raw_response: Value,       // Only escape hatch
}
```

Cannot represent:
- Multiple content blocks (Anthropic tool use responses)
- Multiple choices (OpenAI n>1)
- Provider-specific metadata (OpenAI `logprobs`, Google `safety_ratings`)

### 2.4 Error Extraction Not Used

**Problem**: Error paths are configured but never consumed.

```yaml
error:
  message: error.message
  type: error.type
  code: error.code
```

Current code only checks success codes and returns raw error text (client.rs:146-147).

### 2.5 Streaming vs Non-Streaming Divergence

**Problem**: Two separate extraction systems exist.

| Location | System | Approach |
|----------|--------|----------|
| `src/client.rs` | Non-streaming | Uses `ResponseExtract` from config |
| `src/streaming/sse/extractors/mod.rs` | Streaming | Uses hardcoded `ExtractorConfig` |

The streaming system has its own `JsonPath` type and `ExtractorConfig` that duplicates concepts:

```rust
// streaming/sse/extractors/mod.rs - Hardcoded paths
pub fn openai_config() -> ExtractorConfig {
    ExtractorConfig {
        token_path: JsonPath(vec![Key("choices"), Index(0), Key("delta"), Key("content")]),
        // ...
    }
}
```

These should share the same config-driven approach.

---

## 3. Edge Cases Requiring Provider-Specific Code

### 3.1 Content Block Arrays

Anthropic can return multiple content blocks:
```json
{
  "content": [
    {"type": "text", "text": "Here's the result:"},
    {"type": "tool_use", "id": "toolu_01...", "name": "calculator", "input": {...}}
  ]
}
```

Current path `content[0].text` only gets first block.

### 3.2 Choices with n>1

OpenAI with `n: 2` returns multiple choices:
```json
{
  "choices": [
    {"message": {"content": "Answer A"}},
    {"message": {"content": "Answer B"}}
  ]
}
```

Current path `choices[0].message.content` ignores alternatives.

### 3.3 Refusal Responses

OpenAI can return a refusal instead of content:
```json
{
  "choices": [{
    "message": {
      "content": null,
      "refusal": "I cannot help with that request."
    }
  }]
}
```

No way to express fallback: `content ?? refusal`

### 3.4 Nested Usage Structures

Some providers nest usage differently:
```json
// Anthropic detailed usage
{
  "usage": {
    "input_tokens": 100,
    "cache_creation_input_tokens": 50,
    "cache_read_input_tokens": 25,
    "output_tokens": 200
  }
}
```

Current config cannot express: `input = input_tokens + cache_creation_input_tokens + cache_read_input_tokens`

---

## 4. Recommendations

### 4.1 Short-term: Add Transform Expressions

Extend extraction paths with simple transforms:

```yaml
response:
  extract:
    content:
      path: content[0].text
      type: string
      fallback: ""

    finish_reason:
      path: stop_reason
      type: string
      transform: lowercase
      map:
        end_turn: stop
        max_tokens: length

    usage:
      input:
        path: usage.input_tokens
        type: integer
        default: 0
      output:
        path: usage.output_tokens
        type: integer
        default: 0
```

### 4.2 Short-term: Unify Streaming Extraction

Refactor streaming to use the same `ResponseExtract` config:

```yaml
streaming:
  events:
    - type: content_block_delta
      extract:
        content: delta.text          # Use same path syntax
        index: index
      action: content
```

Delete the hardcoded `ExtractorConfig` in favor of config-driven paths.

### 4.3 Medium-term: Expression Language

Add a lightweight expression language for complex extractions:

```yaml
response:
  extract:
    content:
      expr: |
        if content then
          content.filter(c => c.type == "text").map(c => c.text).join("\n")
        else
          choices[0].message.content
        end
      type: string

    all_content_blocks:
      expr: content
      type: array

    usage:
      input:
        expr: usage.input_tokens + (usage.cache_creation_input_tokens ?? 0)
        type: integer
```

Consider using existing expression libraries:
- `jmespath` - AWS query language, good for paths
- `cel` - Common Expression Language, good for logic
- Custom mini-language - simpler but maintenance burden

### 4.4 Medium-term: Flexible Output Schema

Allow providers to define additional extracted fields:

```yaml
response:
  extract:
    # Standard fields (used by CompletionResponse)
    content: choices[0].message.content
    role: choices[0].message.role

    # Extended fields (stored in metadata map)
    extended:
      model_id: model
      system_fingerprint: system_fingerprint
      logprobs: choices[0].logprobs
      safety_ratings: candidates[0].safetyRatings
```

Update `CompletionResponse`:
```rust
pub struct CompletionResponse {
    pub content: String,
    pub role: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
    pub raw_response: Value,
    pub metadata: HashMap<String, Value>,  // New: extended fields
}
```

### 4.5 Long-term: Schema-Driven Extraction

Full schema definition for any response structure:

```yaml
response:
  schema:
    type: object
    properties:
      content:
        type: string
        extract: choices[0].message.content
      choices:
        type: array
        extract: choices
        items:
          type: object
          properties:
            content:
              extract: message.content
            finish_reason:
              extract: finish_reason
```

This would generate response types at compile time or runtime.

---

## 5. Enhanced Config Schema

### 5.1 Proposed `ResponseExtract` V2

```yaml
# Current (v1)
response:
  extract:
    content: choices[0].message.content
    role: choices[0].message.role

# Proposed (v2) - backwards compatible
response:
  version: 2
  extract:
    content:
      path: choices[0].message.content
      fallback:
        - choices[0].text              # Try legacy format
        - candidates[0].content.parts[0].text  # Try Google format
      type: string
      required: true

    role:
      path: choices[0].message.role
      type: string
      default: assistant

    finish_reason:
      path: choices[0].finish_reason
      type: string
      transform:
        type: map
        values:
          stop: completed
          length: max_tokens
          content_filter: filtered
          tool_calls: tool_use
          end_turn: completed        # Anthropic
          STOP: completed            # Google
          MAX_TOKENS: max_tokens     # Google

    usage:
      input:
        path: usage.prompt_tokens
        fallback: usage.input_tokens
        type: integer
        default: 0
      output:
        path: usage.completion_tokens
        fallback: usage.output_tokens
        type: integer
        default: 0
      total:
        path: usage.total_tokens
        fallback:
          expr: usage.input + usage.output
        type: integer
        optional: true

    # New: provider-specific fields
    extended:
      model:
        path: model
        type: string
      logprobs:
        path: choices[0].logprobs
        type: object
        optional: true
      tool_calls:
        path: choices[0].message.tool_calls
        type: array
        optional: true
```

### 5.2 Rust Type Changes

```rust
// New extraction config types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ExtractPath {
    Simple(String),
    Complex(ExtractConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractConfig {
    pub path: String,
    #[serde(default)]
    pub fallback: Vec<String>,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default = "default_string_type")]
    pub r#type: ExtractType,
    #[serde(default)]
    pub transform: Option<Transform>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtractType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Transform {
    Lowercase,
    Uppercase,
    Map { values: HashMap<String, String> },
    Join { separator: String },
    First,
    Last,
}
```

---

## 6. Migration Path

### Phase 1: Backwards-Compatible Extraction (2 weeks)
1. Add `version` field to `ResponseExtract` (default: 1)
2. Implement `ExtractPath` enum that accepts both string and object
3. Add type coercion based on `type` field
4. Add `default` value support

### Phase 2: Transforms and Fallbacks (2 weeks)
1. Implement fallback path chain
2. Add basic transforms (lowercase, map, join)
3. Migrate existing configs to use normalized finish_reason values

### Phase 3: Streaming Unification (2 weeks)
1. Refactor `streaming/sse/extractors` to use config paths
2. Remove hardcoded `openai_config()` and `claude_config()`
3. Add streaming-specific config for incremental extraction

### Phase 4: Extended Fields (1 week)
1. Add `metadata` field to `CompletionResponse`
2. Implement `extended` extraction config
3. Document provider-specific fields

---

## 7. Summary

The current response extraction system is partially config-driven with significant room for improvement:

| Area | Current State | Recommendation |
|------|--------------|----------------|
| Path extraction | Config-driven | Keep, enhance with fallbacks |
| Type coercion | Hardcoded | Add `type` field to config |
| Transforms | Not supported | Add transform expressions |
| Output schema | Fixed struct | Add metadata map for extensions |
| Streaming | Separate system | Unify with non-streaming config |
| Error handling | Config unused | Implement error extraction |

The proposed v2 extraction schema maintains backwards compatibility while enabling the flexibility needed for diverse LLM provider responses.
