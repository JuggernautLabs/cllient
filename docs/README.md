# cllient Documentation

Welcome to the comprehensive documentation for **cllient** - a runtime-configurable LLM client system written in Rust.

## 📚 Documentation Structure

### Getting Started
- **[1. Installation & Setup](1_installation.md)** - Prerequisites, installation, and environment configuration
- **[2. CLI Usage Guide](2_cli-usage.md)** - Complete command-line interface reference
- **[Examples: Basic Usage](examples/1_basic-usage.md)** - Get started with common use cases

### Core Documentation
- **[3. API Reference](3_api-reference.md)** - Runtime API, CLI API, and Low-level Client API
- **[4. Configuration](4_configuration.md)** - Service and model configuration system
- **[5. Architecture](5_architecture.md)** - Flexible design and core components

### Advanced Usage
- **[Examples](examples/)** - Real-world usage patterns and integration examples
- **[6. Development](6_development.md)** - Contributing, testing, and workflow automation

## 🔗 Source Code References

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
- **[HTTP Client](../src/client.rs)** - Low-level HTTP communication
- **[Template Engine](../src/template.rs)** - Handlebars request templating

## 🚀 Quick Navigation

### I want to...
- **Use cllient from command line** → [2. CLI Usage Guide](2_cli-usage.md)
- **Integrate cllient into my Rust app** → [3. API Reference](3_api-reference.md#runtime-api)
- **Add a new LLM provider** → [4. Configuration](4_configuration.md#adding-providers)
- **Understand the architecture** → [5. Architecture](5_architecture.md)
- **See usage examples** → [Examples](examples/)
- **Contribute to the project** → [6. Development](6_development.md)

### I need help with...
- **Installation issues** → [1. Installation & Setup](1_installation.md#troubleshooting)
- **API authentication** → [4. Configuration](4_configuration.md#api-keys)
- **Command syntax** → [2. CLI Usage Guide](2_cli-usage.md)
- **Performance optimization** → [Examples](examples/2_advanced-scripting.md)

## 📋 Project Overview

**cllient** implements a flexible architecture where:
- **Services** (HTTP providers) exist independently of models
- **Models** can be associated with any compatible service at runtime  
- **Configuration** drives behavior without code changes
- **HTTP transparency** allows seeing exactly what requests are sent

This design enables maximum flexibility for LLM integration while maintaining type safety through Rust.

---

*For the main project overview, see the [main README](../README.md)*