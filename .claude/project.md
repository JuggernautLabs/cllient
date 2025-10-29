# Project: cllient - Experimental Config-Driven LLM Client

## What This Project Actually Is

An **experimental** Rust LLM client that uses YAML configuration files to define HTTP request templates for different LLM providers. The goal is to separate "services" (API endpoints) from "models" (what you call) so new providers can be added via config files instead of code changes.

## Current State (Be Honest!)

**What's Actually Working:**
- ✅ 3 tested providers: OpenAI, Anthropic, DeepSeek
- ✅ SSE streaming works reliably
- ✅ CLI tool with basic commands (ask, stream, chat, compare)
- ✅ Config-driven architecture (YAML templates with Handlebars)
- ✅ 339 model configs total

**What's Not Really Tested:**
- ⚠️ 242 models just proxy through OpenRouter (auto-generated, untested)
- ⚠️ Google integration exists but hasn't been validated
- ⚠️ Azure integration exists but untested
- ⚠️ Error handling is minimal and breaks ungracefully
- ⚠️ No production hardening whatsoever

**This is AI-generated code with minimal human oversight!**

## Architecture: "Unassociated Truth"

The interesting idea here is separation of concerns:

1. **Service configs** (`config/service/*.yaml`) - HTTP request templates
   - Use Handlebars for templating: `{{model_id}}`, `{{json messages}}`
   - Environment variable substitution: `${OPENAI_API_KEY}`
   - Define SSE parsing rules and response extraction paths

2. **Model configs** (`config/family/*/*.yaml`) - Metadata about models
   - Reference a service config
   - Define capabilities (context window, vision, etc.)
   - Include pricing data (scraped from OpenRouter)

3. **Runtime** - Loads configs, renders templates, makes HTTP calls
   - Can use embedded configs (compiled into binary) or external files
   - `ModelRegistry` provides high-level API
   - `LowLevelClient` handles HTTP + streaming

## Key Files

- `src/runtime.rs` - High-level `ModelRegistry` API
- `src/client.rs` - HTTP client with streaming support
- `src/config.rs` - YAML config loading and validation
- `src/template.rs` - Handlebars template rendering
- `src/streaming/` - SSE parsing for different providers
- `config/service/` - Service configurations (9 files)
- `config/family/` - Model configs (339 files across 57 families)
- `Makefile` - 40+ automation commands

## Common Tasks

```bash
# Build and run
make build
cargo run --bin cllient -- list

# Test with real providers
cargo run --bin cllient -- ask gpt-4o-mini "test"
cargo run --bin cllient -- stream deepseek-chat "test"
cargo run --bin cllient -- chat claude-3-haiku-20240307

# Development
make test
make lint
make check

# Model management
make models-openai    # Fetch latest OpenAI models
make pricing-cheapest # Find cheapest models
```

## When Working on This Project

**DO:**
- Be honest about what's tested vs. untested
- Test any new provider integrations thoroughly
- Add error handling when touching existing code
- Document what you actually verified works
- Keep the realistic tone in documentation

**DON'T:**
- Claim things work without testing them
- Auto-generate configs for providers without validation
- Make production promises (this is research code!)
- Assume the AI-generated code is correct
- Skip testing because "it should work"

## Known Issues

1. **Template fragility**: YAML HTTP templates break easily
2. **Poor error messages**: Fails with cryptic errors
3. **No retry logic**: Single request failures = hard fail
4. **OpenRouter dependency**: 71% of configs rely on one service
5. **Minimal tests**: Coverage is very low
6. **AI-generated**: Most code written with minimal human review

## Good First Tasks for Contributors

- Test one of the 242 OpenRouter models individually
- Add proper error handling to a specific module
- Write integration tests for existing providers
- Validate the Google/Azure configs actually work
- Improve template error messages
- Add retry logic with backoff
