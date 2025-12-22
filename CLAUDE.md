# cllient

A config-driven LLM client library that provides a unified interface to multiple LLM providers (Anthropic, OpenAI, Google, DeepSeek, etc.).

## Architecture Documentation

Architecture documents live in `docs/architecture/` and use reverse-chronological naming to ensure newest documents appear first.

**Naming formula**: `(u64::MAX - nanotime)_title.md`

Where:
- `nanotime` = current Unix timestamp in nanoseconds
- Creates a descending numeric prefix (newer = smaller number = sorts first)

**To generate a filename**:
```python
import time
nanotime = int(time.time() * 1_000_000_000)
filename = (2**64 - 1) - nanotime
print(f'{filename}_your-title.md')
```

## Key Components

- `ModelRegistry` (`src/runtime.rs`) - Main entry point for model discovery and request building
- `ConfigProvider` trait (`src/client.rs`) - Abstraction for loading service/model configs
- `EmbeddedConfigLoader` (`src/embedded_config.rs`) - Embedded YAML configs compiled into binary
- Service configs in `config/service/*.yaml` - Define HTTP templates for each provider
- Model configs in `config/family/**/*.yaml` - Define model capabilities, pricing, constraints

## Common Tasks

```bash
# Run tests
cargo test

# Build
cargo build
```
