# Development Guide

Complete guide for developing, testing, and contributing to cllient.

> **Source**: [`Makefile`](../Makefile) | **Workflow Tools**: [`workflow/`](../workflow/) | **Tests**: [`tests/`](../tests/)

## Quick Start

```bash
# Clone and setup
git clone <repository>
cd cllient

# Setup environment
cp .env.example .env
# Edit .env with your API keys

# Build and test
make build
make test

# See all available commands
make help
```

---

## Build & Development

> **Build Configuration**: [`Cargo.toml`](../Cargo.toml)

### Feature Flags

The library uses Cargo features to enable optional functionality:

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `default` | No optional features enabled | - |
| `plugin` | Enable plugin system for substrate integration | `hub-core`, `hub-macro`, `uuid`, `jsonrpsee` |
| `hub` | Full hub server support (includes `plugin`) | Same as `plugin` |

**Building with features:**

```bash
# Default build (no optional features)
cargo build

# Build with plugin support
cargo build --features plugin

# Build with full hub support
cargo build --features hub

# Build the hub binary (requires hub feature)
cargo build --bin cllient-hub --features hub
```

### Basic Commands

```bash
# Build the project
make build

# Build optimized release version
make release

# Run tests
make test

# Clean build artifacts
make clean

# Format code
make format

# Run linter
make lint

# Watch for changes and rebuild
make watch
```

### Development Workflow

```bash
# Start development session
make dev-setup      # Install dependencies, setup hooks

# During development
make watch          # Auto-rebuild on file changes
make test-quick     # Run fast tests only
make lint-fix       # Auto-fix linting issues

# Before committing
make pre-commit     # Run all checks (format, lint, test)
make commit         # Interactive commit with validation
```

---

## Testing

> **Test Suite**: [`tests/`](../tests/) | **Integration Tests**: [`tests/integration/`](../tests/integration/)

### Test Categories

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --tests

# Specific test files
cargo test --test cli_api_tests
cargo test --test runtime_api_tests
cargo test --test low_level_client_tests

# All tests
make test

# Tests with feature flags
cargo test --features plugin
cargo test --features hub
cargo test --all-features
```

### Provider Testing

```bash
# Test specific providers
make test-openai
make test-anthropic  
make test-deepseek
make test-google

# Test all providers
make test-streaming

# Test cheapest models from each provider
make test-cheapest
```

### Test Configuration

```bash
# Run tests with specific config
CLLIENT_CONFIG_DIR=./test-configs cargo test

# Test with different log levels
RUST_LOG=debug cargo test --test streaming_test

# Test specific model
MODEL=gpt-4o-mini cargo test test_model_completion
```

---

## Model Discovery & Management

> **Workflow Scripts**: [`workflow/`](../workflow/)

### Fetch Latest Models

```bash
# Fetch models from provider APIs
make models-openai       # Get latest OpenAI models
make models-anthropic    # Get latest Anthropic models
make models-deepseek     # Get latest DeepSeek models
make models-google       # Get latest Google models
make models-cohere       # Get latest Cohere models

# Fetch all provider models
make models-all
```

### Model Information

```bash
# List all configured models
make models-list

# Show model details
make models-info

# Export model data
make models-export       # Export to JSON
make models-csv         # Export to CSV
```

### Pricing Analysis

```bash
# Get pricing information
make pricing-all         # All model pricing
make pricing-cheapest    # 10 cheapest models
make pricing-expensive   # 10 most expensive models

# Provider-specific pricing
make pricing-provider PROVIDER=openai
make pricing-provider PROVIDER=anthropic

# Cost analysis
make cost-analysis       # Detailed cost breakdown
make cost-comparison     # Compare providers
```

---

## Configuration Management

> **Config System**: [`src/config.rs`](../src/config.rs) | **Embedded Configs**: [`config/`](../config/)

### Configuration Commands

```bash
# Validate all configurations
make config-validate

# Update existing configurations
make config-update

# Generate new configurations
make config-generate

# Check for missing configurations
make config-check
```

### Adding New Providers

```bash
# Generate service template
make new-service PROVIDER=newprovider

# Generate model configs from API
make discover-models PROVIDER=newprovider

# Validate new configurations
make validate-provider PROVIDER=newprovider
```

### Configuration Workflow

```bash
# 1. Create service configuration
cat > config/service/newprovider.yaml << EOF
service:
  name: NewProvider
  base_url: https://api.newprovider.com
# ... rest of config
EOF

# 2. Generate models
make discover-models PROVIDER=newprovider

# 3. Validate
make validate-provider PROVIDER=newprovider

# 4. Test
make test-provider PROVIDER=newprovider
```

---

## Code Quality

> **Linting**: [`src/`](../src/) | **Formatting**: [rustfmt](https://github.com/rust-lang/rustfmt)

### Recent Improvements

The codebase has undergone significant refactoring to improve quality and performance:

**Eliminated Redundancy:**
- Replaced duplicate trait implementations with declarative macros
- Consolidated duplicate SSE extractor/provider files into generic config-driven implementations
- Removed approximately 230 net lines while adding functionality

**Performance Improvements:**
- Static regex compilation using `OnceLock` instead of repeated `Regex::new()` calls
- Index-based lookups instead of O(n) iterations for model/family queries
- Pre-allocated HashMap capacity and `Entry` API usage in registry indexing
- Consuming `into_*` methods to avoid unnecessary cloning

### Code Formatting

```bash
# Format all code
make format

# Check formatting without changes
make format-check

# Format specific files
cargo fmt --package cllient
```

### Linting

```bash
# Run all lints
make lint

# Run clippy with pedantic lints
make clippy

# Fix auto-fixable lint issues
make lint-fix

# Check for common issues
make audit           # Security audit
make check-deps     # Dependency check
```

### Code Analysis

```bash
# Generate documentation
make docs

# Check documentation coverage
make docs-check

# Run benchmarks
make bench

# Profile performance
make profile
```

---

## Release Management

### Version Management

```bash
# Bump version
make version-patch      # 1.0.0 -> 1.0.1
make version-minor      # 1.0.0 -> 1.1.0  
make version-major      # 1.0.0 -> 2.0.0

# Prepare release
make release-prep       # Update changelog, validate
make release-build      # Build release artifacts
make release-test       # Test release build
```

### Publishing

```bash
# Dry run
make publish-dry-run

# Publish to crates.io
make publish

# Create GitHub release
make github-release

# Complete release workflow
make release           # Full release process
```

---

## Workflow Automation

> **GitHub Actions**: [`.github/workflows/`](../.github/workflows/) | **Scripts**: [`workflow/`](../workflow/)

### Automated Workflows

**CI/CD Pipeline**:
- Build and test on multiple platforms
- Run security audits
- Check code formatting and linting
- Test with multiple Rust versions

**Model Updates**:
- Scheduled model discovery from provider APIs
- Automatic pricing updates
- Configuration validation
- Pull request creation for updates

### Local Automation

```bash
# Setup git hooks
make hooks-install

# Generate all configurations
make generate-all

# Update all data
make update-all

# Full project health check
make health-check
```

---

## Contributing

### Getting Started

```bash
# Fork and clone
git clone https://github.com/yourusername/cllient.git
cd cllient

# Setup development environment
make dev-setup

# Create feature branch
git checkout -b feature/new-feature

# Make changes and test
make test
make lint
make format

# Commit changes
make commit
```

### Code Standards

**Rust Guidelines**:
- Follow official Rust API guidelines
- Use `rustfmt` for formatting (enforced in CI)
- Address all `clippy` warnings
- Maintain test coverage > 80%

**Documentation**:
- Document all public APIs
- Include examples in documentation
- Update relevant guides in `docs/`
- Add code attribution links

**Testing**:
- Unit tests for all new functionality
- Integration tests for API changes
- Test with multiple providers
- Include error case testing

### Pull Request Process

```bash
# Before submitting PR
make pre-commit         # Run all checks
make test-all          # Comprehensive testing
make docs-check        # Validate documentation

# Create PR with:
# - Clear description of changes
# - Link to related issues
# - Test results summary
# - Breaking change notes (if any)
```

---

## Debugging & Troubleshooting

### Debug Builds

```bash
# Build with debug info
cargo build --features debug

# Run with detailed logging
RUST_LOG=debug cargo run --bin cllient -- ask gpt-4o-mini "test"

# Debug specific modules
RUST_LOG=cllient::streaming=debug,cllient::client=trace cargo run
```

### Common Issues

**API Authentication**:
```bash
# Test API keys
make test-auth

# Validate environment
make check-env

# Debug API calls
RUST_LOG=cllient::client=debug cllient ask gpt-4o-mini "test"
```

**Configuration Issues**:
```bash
# Validate configs
make config-validate

# Debug config loading
RUST_LOG=cllient::config=debug cllient list

# Check specific model
cllient --verbose ask model-name "test"
```

**Performance Issues**:
```bash
# Profile performance
make profile

# Benchmark specific operations
make bench-streaming
make bench-config-loading

# Memory analysis
valgrind --tool=massif target/release/cllient list
```

---

## Advanced Development

### Custom Features

**Adding New Content Types**:
```rust
// In src/types.rs
pub enum ContentBlock {
    Text { text: String },
    Image { url: String, detail: Option<String> },
    Audio { data: Vec<u8>, format: AudioFormat },  // New
}
```

**Custom SSE Parsers**:
```rust
// In src/streaming/sse/providers/
impl SseParser for CustomParser {
    fn parse_chunk(&self, data: &str) -> Result<StreamChunk> {
        // Custom parsing logic
    }
}
```

**Custom Message Builders**:
```rust
// In src/streaming/
impl MessageBuilder for CustomBuilder {
    fn build_messages(&self, messages: &[Message]) -> serde_json::Value {
        // Custom message formatting
    }
}
```

### Performance Optimization

**HTTP Client Tuning**:
```rust
// In src/client.rs
let client = ClientBuilder::new()
    .pool_max_idle_per_host(10)
    .pool_idle_timeout(Duration::from_secs(30))
    .timeout(Duration::from_secs(120))
    .build()?;
```

**Configuration Caching**:
```rust
// In src/config.rs  
#[derive(Clone)]
pub struct CachedConfigProvider {
    cache: Arc<RwLock<HashMap<String, ConfigSet>>>,
}
```

---

## Resources

### Documentation

- **[Architecture Guide](architecture.md)** - System design and components
- **[API Reference](api-reference.md)** - Complete API documentation
- **[Configuration Guide](configuration.md)** - Config system details
- **[CLI Usage](cli-usage.md)** - Command-line interface

### External Resources

- **[Rust Book](https://doc.rust-lang.org/book/)** - Learn Rust programming
- **[Tokio Tutorial](https://tokio.rs/tokio/tutorial)** - Async Rust
- **[Serde Guide](https://serde.rs/)** - Serialization library
- **[Clap Book](https://docs.rs/clap/latest/clap/)** - CLI argument parsing

### Community

- **Issues**: Report bugs and request features
- **Discussions**: Ask questions and share ideas  
- **Contributing**: See [CONTRIBUTING.md](../CONTRIBUTING.md)
- **License**: See [LICENSE](../LICENSE)

---

**Next**: [3. API Reference](3_api-reference.md) | [Examples](examples/)