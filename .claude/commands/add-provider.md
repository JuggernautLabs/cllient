---
description: Add a new LLM provider to the project with proper testing
---

I need you to add a new LLM provider integration to this project.

**Before starting, ask me:**
1. Which provider to add (e.g., "Mistral", "Cohere", "Together AI")
2. Do I have API access to test it?
3. Should this be a direct integration or via OpenRouter?

**Then follow these steps:**

## 1. Research the Provider API

- Find their official API documentation
- Identify the endpoint URL and authentication method
- Check what format they use (OpenAI-compatible? Custom?)
- Note any special headers or parameters required

## 2. Create Service Config

Create `config/service/<provider>.yaml`:

```yaml
service:
  name: ProviderName
  base_url: https://api.provider.com

http:
  request: |
    POST /v1/chat/completions HTTP/1.1
    Host: api.provider.com
    Content-Type: application/json
    Authorization: Bearer ${PROVIDER_API_KEY}

    {
      "model": "{{model_id}}",
      "messages": {{json messages}},
      "temperature": {{temperature}},
      "max_tokens": {{max_tokens}},
      "stream": {{stream}}
    }

streaming:
  format: text/event-stream
  parser: openai_sse  # or custom if needed
  # ... SSE parsing rules

response:
  success_codes: [200]
  extract:
    content: choices[0].message.content
    # ... response field extraction
```

## 3. Create Model Config(s)

For each model, create `config/family/<family>/<model-id>.yaml`:

```yaml
model:
  id: provider-model-name
  family: provider
  service: provider  # References service config
  status: untested   # Mark as untested initially!

capabilities:
  context_window: 8192
  max_output_tokens: 4096
  streaming: true
  # ... other capabilities

pricing:
  currency: USD
  input_per_1k_tokens: 0.001
  output_per_1k_tokens: 0.002
```

## 4. Update .env.example

Add the new API key variable:
```bash
# Provider Name
PROVIDER_API_KEY=your_key_here
```

## 5. Test Thoroughly

Use the `/test-provider` command to validate:
- Basic completion works
- Streaming works
- Error handling is reasonable

## 6. Document Honestly

Update these files:
- `README.md` - Add to appropriate section (tested vs. untested)
- `.claude/project.md` - Note what was tested
- Create a test report file if needed

## 7. Mark Status

In the model config, update:
```yaml
status: verified  # Only if you actually tested it!
```

**Remember:** Don't claim something works without testing. It's better to mark it `status: untested` than to lie about validation.
