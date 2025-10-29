---
description: Debug and fix a broken YAML template configuration
---

I need you to help debug a broken YAML template configuration.

**Ask me:**
1. Which service/model config is broken?
2. What error message am I seeing?
3. What command triggered the error?

**Then investigate:**

## 1. Read the Config Files

- Read the service config: `config/service/<service>.yaml`
- Read the model config: `config/family/<family>/<model>.yaml`
- Check for syntax errors, missing fields, or malformed templates

## 2. Common Template Issues

**Environment variables** - Use `${UPPERCASE}` syntax:
```yaml
Authorization: Bearer ${OPENAI_API_KEY}  # ✅ Correct
Authorization: Bearer {{api_key}}        # ❌ Wrong - this is for runtime vars
```

**Template variables** - Use `{{lowercase}}` for Handlebars:
```yaml
"model": "{{model_id}}"          # ✅ Correct
"model": "${model_id}"           # ❌ Wrong - this is for env vars
```

**JSON objects** - Use the `{{json var}}` helper:
```yaml
"messages": {{json messages}}    # ✅ Correct
"messages": {{messages}}         # ❌ Wrong - outputs [Object object]
```

**HTTP request format**:
```yaml
http:
  request: |
    POST /v1/endpoint HTTP/1.1   # ✅ Method + path + version
    Host: api.example.com         # ✅ Headers
    Content-Type: application/json

    {                             # ✅ Blank line before body
      "key": "value"
    }
```

## 3. Check Response Extraction

**JSONPath expressions** must match actual API response:
```yaml
response:
  extract:
    content: choices[0].message.content  # Check API docs for actual path!
    usage:
      input: usage.input_tokens          # Anthropic format
      # OR
      input: usage.prompt_tokens         # OpenAI format
```

## 4. Validate SSE Streaming

If streaming is broken, check:
```yaml
streaming:
  parser: openai_sse              # or claude_sse, custom parser
  line_prefix: "data: "           # SSE format prefix
  done_marker: "[DONE]"           # End of stream marker
  events:
    - type: data
      extract: choices[0].delta.content
      filter: choices[0].delta.content  # Only emit if this field exists
```

## 5. Test the Fix

After fixing, test with:
```bash
# Non-streaming
cargo run --bin cllient -- ask <model> "test prompt"

# Streaming
cargo run --bin cllient -- stream <model> "test prompt"

# Debug mode
cargo run --bin cllient -- --verbose ask <model> "test"
```

## 6. Document the Fix

- Note what was broken and how you fixed it
- Update any related documentation
- Consider if other configs have the same issue
