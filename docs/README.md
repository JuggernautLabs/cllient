# cllient Documentation

Welcome to the comprehensive documentation for **cllient** - a runtime-configurable LLM client system written in Rust.

## Recent Updates

> **Commit c0ef5a1**: Refactoring to eliminate redundancy and improve code quality across the codebase.

---

## Quick Links by Use Case

### I want to...

| Goal | Resource |
|------|----------|
| :rocket: Get started quickly | [1. Installation & Setup](1_installation.md) |
| :computer: Use from command line | [2. CLI Usage Guide](2_cli-usage.md) |
| :gear: Integrate into my Rust app | [3. API Reference](3_api-reference.md#runtime-api) |
| :zap: Use streaming JSON output | [Streaming JSON Examples](examples/streaming-json.md) |
| :heavy_plus_sign: Add a new LLM provider | [4. Configuration](4_configuration.md#adding-providers) |
| :building_construction: Understand the architecture | [5. Architecture](5_architecture.md) |
| :package: Build multi-turn conversations | [Message Builder](examples/message-builder.md) |
| :arrows_counterclockwise: Manage conversation history | [MessageList Guide](examples/message-list.md) |
| :hammer_and_wrench: Contribute to the project | [6. Development](6_development.md) |

### I need help with...

| Issue | Resource |
|-------|----------|
| :warning: Installation issues | [Troubleshooting](1_installation.md#troubleshooting) |
| :key: API authentication | [API Keys](4_configuration.md#api-keys) |
| :question: Command syntax | [CLI Usage Guide](2_cli-usage.md) |
| :chart_with_upwards_trend: Performance optimization | [Advanced Scripting](examples/2_advanced-scripting.md) |

---

## Core Documentation

The numbered documentation provides a structured learning path:

| # | Document | Description |
|---|----------|-------------|
| 1 | [Installation & Setup](1_installation.md) | Prerequisites, installation, and environment configuration |
| 2 | [CLI Usage Guide](2_cli-usage.md) | Complete command-line interface reference |
| 3 | [API Reference](3_api-reference.md) | Runtime API, CLI API, and Low-level Client API |
| 4 | [Configuration](4_configuration.md) | Service and model configuration system |
| 5 | [Architecture](5_architecture.md) | Flexible design and core components |
| 6 | [Development](6_development.md) | Contributing, testing, and workflow automation |

---

## Architecture Documents

Deep-dive technical documents about system design and integration plans (newest first):

| Document | Description |
|----------|-------------|
| [Plugin & Hub Implementation](architecture/16679607251007423487_cllient-plugin-hub-implementation.md) | Plan to transform ModelRegistry into substrate plugin with hub generation |
| [Action Plan: Plugin & Hub](architecture/16679661413902674687_action-plan-plugin-hub.md) | Step-by-step integration plan using hub-macro |
| [Hub Architecture Gaps](architecture/16679661485544781567_hub-architecture-gaps.md) | Analysis of Registry -> Plugin -> Hub transformations |
| [Streaming JSON-RPC Migration](architecture/16679663526973323519_streaming-jsonrpc-migration.md) | Streaming-first architecture for hub integration |
| [Registry Export API](architecture/16680999631625794559_registry-export.md) | Serializable registry view for RPC boundaries |

---

## Examples

Practical usage patterns and integration examples:

| Example | Description |
|---------|-------------|
| [Basic Usage](examples/1_basic-usage.md) | Get started with common use cases |
| [Advanced Scripting](examples/2_advanced-scripting.md) | Production-ready scripts and automation patterns |
| [Streaming JSON](examples/streaming-json.md) | Incremental JSON output for real-time processing |
| [Message Builder](examples/message-builder.md) | Construct complex multi-turn conversations |
| [MessageList](examples/message-list.md) | Manage conversations with modification capabilities |

---

## Source Code References

### Core Components
- **[Main Library](../src/lib.rs)** - Public API exports and re-exports
- **[ModelRegistry](../src/runtime.rs#L12)** - Main runtime object for model selection
- **[CLI Binary](../src/bin/cllient.rs)** - Command-line interface implementation
- **[Configuration System](../src/config.rs)** - Config loading and validation
- **[Request Types](../src/types.rs)** - Core data structures and types

### Configuration
- **[Service Configs](../config/service/)** - HTTP templates for LLM providers
- **[Model Configs](../config/family/)** - 330+ pre-configured models
- **[Embedded Config Loader](../src/embedded_config.rs)** - Built-in configuration provider

### Streaming & Communication
- **[Streaming Infrastructure](../src/streaming/)** - Real-time response processing
- **[Streaming JSON Output](../src/streaming_json.rs)** - Incremental JSON state machine
- **[HTTP Client](../src/client.rs)** - Low-level HTTP communication
- **[Template Engine](../src/template.rs)** - Handlebars request templating

---

## Project Overview

**cllient** implements a flexible architecture where:
- **Services** (HTTP providers) exist independently of models
- **Models** can be associated with any compatible service at runtime
- **Configuration** drives behavior without code changes
- **HTTP transparency** allows seeing exactly what requests are sent

This design enables maximum flexibility for LLM integration while maintaining type safety through Rust.

---

*For the main project overview, see the [main README](../README.md)*
