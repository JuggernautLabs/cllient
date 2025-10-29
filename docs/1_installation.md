# Installation & Setup

Complete guide for installing and configuring cllient.

## Prerequisites

### System Requirements

- **Rust** 1.70.0 or later
- **Git** for cloning the repository
- **jq** for JSON processing in Makefile utilities (optional)

### Operating Systems

- ✅ **Linux** (Ubuntu 20.04+, RHEL 8+, Arch Linux)
- ✅ **macOS** (10.15+, Apple Silicon and Intel)
- ✅ **Windows** (Windows 10+, WSL2 recommended)

## Installation Methods

### Option 1: Install from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/cllient.git
cd cllient

# Install with Cargo
cargo install --path .

# Verify installation
cllient --help
```

### Option 2: Development Installation

```bash
# Clone and build for development
git clone https://github.com/yourusername/cllient.git
cd cllient

# Build in development mode
cargo build

# Run directly (without installing)
cargo run --bin cllient -- list
```

### Option 3: Pre-built Binaries (Future)

```bash
# Download from GitHub releases (when available)
curl -L https://github.com/yourusername/cllient/releases/latest/download/cllient-linux.tar.gz | tar xz
sudo mv cllient /usr/local/bin/
```

## Environment Setup

### 1. Copy Environment Template

```bash
# Copy the example environment file
cp .env.example .env
```

### 2. Add API Keys

Edit `.env` with your API keys:

```bash
# Core providers (required for basic functionality)
OPENAI_API_KEY=sk-proj-your-openai-key-here
ANTHROPIC_API_KEY=sk-ant-api03-your-anthropic-key-here
DEEPSEEK_API_KEY=sk-your-deepseek-key-here

# Additional providers (optional)
GOOGLE_AI_API_KEY=your-google-ai-key-here
COHERE_API_KEY=your-cohere-key-here
AZURE_OPENAI_API_KEY=your-azure-key-here
AZURE_OPENAI_ENDPOINT=https://your-resource.openai.azure.com
```

### 3. Get API Keys

#### OpenAI
1. Visit [OpenAI Platform](https://platform.openai.com/api-keys)
2. Sign in or create account
3. Click "Create new secret key"
4. Copy the key to your `.env` file

#### Anthropic
1. Visit [Anthropic Console](https://console.anthropic.com/)
2. Sign in or create account
3. Navigate to "API Keys"
4. Create a new key
5. Copy the key to your `.env` file

#### DeepSeek
1. Visit [DeepSeek Platform](https://platform.deepseek.com/api_keys)
2. Sign in or create account
3. Generate API key
4. Copy the key to your `.env` file

#### Google AI
1. Visit [Google AI Studio](https://makersuite.google.com/app/apikey)
2. Sign in with Google account
3. Create API key
4. Copy the key to your `.env` file

### 4. Test Configuration

```bash
# Test API connectivity
cllient ask gpt-4o-mini "Hello, world!" && echo "✅ OpenAI working"
cllient ask claude-3-haiku-20240307 "Hello!" && echo "✅ Anthropic working"
cllient ask deepseek-chat "Hello!" && echo "✅ DeepSeek working"

# List available models
cllient list | head -10
```

## Configuration Options

### Custom Configuration Directory

```bash
# Create custom config directory
mkdir -p ~/.config/cllient/{service,family}

# Set environment variable
export CLLIENT_CONFIG_DIR="$HOME/.config/cllient"

# Or add to shell profile
echo 'export CLLIENT_CONFIG_DIR="$HOME/.config/cllient"' >> ~/.bashrc
```

### Shell Integration

Add convenient aliases to your shell profile:

```bash
# Add to ~/.bashrc, ~/.zshrc, or ~/.profile

# Quick AI assistant
alias ai='cllient ask gpt-4o-mini'
alias ask='cllient ask'
alias stream='cllient stream'
alias chat='cllient chat'

# Model shortcuts
alias gpt='cllient ask gpt-4o-mini'
alias claude='cllient ask claude-3-haiku-20240307'
alias deepseek='cllient ask deepseek-chat'

# Development helpers
alias ai-code='cllient ask deepseek-coder'
alias ai-creative='cllient ask claude-3-opus-20240229'
```

### Logging Configuration

```bash
# Enable debug logging
export RUST_LOG=debug

# Component-specific logging
export RUST_LOG=cllient::client=debug,cllient::streaming=trace

# Log to file
export RUST_LOG=info
cllient ask gpt-4o-mini "test" 2>&1 | tee cllient.log
```

## Verification

### Basic Functionality Test

```bash
# Test installation
cllient --version

# Test configuration
cllient list | head -5

# Test basic completion
cllient ask deepseek-chat "What is 2+2?"

# Test streaming
cllient stream gpt-4o-mini "Count to 5"

# Test JSON output
cllient --json list claude | head -1
```

### Performance Test

```bash
# Test response times
time cllient ask gpt-4o-mini "Quick test"
time cllient ask claude-3-haiku-20240307 "Quick test"
time cllient ask deepseek-chat "Quick test"

# Test streaming performance
time cllient stream deepseek-chat "Count from 1 to 10"
```

## Troubleshooting

### Common Issues

#### "cllient: command not found"

**Solution**: Ensure Cargo's bin directory is in your PATH:

```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"

# Reload shell
source ~/.bashrc
```

#### "API key not found" errors

**Solution**: Check environment configuration:

```bash
# Verify .env file exists
ls -la .env

# Check environment variables
echo $OPENAI_API_KEY
echo $ANTHROPIC_API_KEY

# Test API key format
if [[ $OPENAI_API_KEY == sk-* ]]; then echo "✅ OpenAI key format OK"; else echo "❌ Invalid OpenAI key format"; fi
```

#### "Model not found" errors

**Solution**: List available models and check spelling:

```bash
# List all models
cllient list

# Search for specific model
cllient list | grep -i "gpt-4o"
cllient list | grep -i "claude"

# Use exact model IDs
cllient ask gpt-4o-mini "test"  # ✅ Correct
cllient ask gpt4o-mini "test"   # ❌ Incorrect
```

#### Network/SSL issues

**Solution**: Check network connectivity and SSL:

```bash
# Test network connectivity
curl -I https://api.openai.com
curl -I https://api.anthropic.com

# Update CA certificates (Linux)
sudo apt-get update && sudo apt-get install ca-certificates

# Update CA certificates (macOS)
brew install ca-certificates
```

#### Permission errors

**Solution**: Fix file permissions:

```bash
# Fix .env permissions
chmod 600 .env

# Fix configuration directory permissions
chmod -R 755 ~/.config/cllient
```

### Advanced Debugging

#### Enable verbose logging

```bash
# Maximum verbosity
RUST_LOG=trace cllient --verbose ask gpt-4o-mini "debug test"

# Component-specific debugging
RUST_LOG=cllient::config=debug cllient list
RUST_LOG=cllient::client=debug cllient ask gpt-4o-mini "test"
RUST_LOG=cllient::streaming=trace cllient stream deepseek-chat "test"
```

#### Validate configuration

```bash
# Check embedded configurations
cllient list-services

# Validate specific model
cllient --json list | jq '.[] | select(.id == "gpt-4o-mini")'

# Test configuration loading
CLLIENT_CONFIG_DIR=/nonexistent cllient list  # Should still work with embedded configs
```

#### Network diagnostics

```bash
# Test API endpoints
curl -H "Authorization: Bearer $OPENAI_API_KEY" https://api.openai.com/v1/models
curl -H "x-api-key: $ANTHROPIC_API_KEY" https://api.anthropic.com/v1/messages

# Check proxy settings
echo $http_proxy
echo $https_proxy

# Bypass proxy temporarily
unset http_proxy https_proxy
cllient ask gpt-4o-mini "proxy test"
```

## System Integration

### systemd Service (Linux)

```bash
# Create service file
sudo tee /etc/systemd/system/cllient-server.service << EOF
[Unit]
Description=cllient LLM Client Server
After=network.target

[Service]
Type=simple
User=cllient
WorkingDirectory=/opt/cllient
Environment=RUST_LOG=info
EnvironmentFile=/opt/cllient/.env
ExecStart=/usr/local/bin/cllient server
Restart=always

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl enable cllient-server
sudo systemctl start cllient-server
```

### Docker Setup

```dockerfile
# Dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/cllient /usr/local/bin/
COPY .env.example /.env

ENTRYPOINT ["cllient"]
CMD ["--help"]
```

```bash
# Build and run
docker build -t cllient .
docker run -it --env-file .env cllient list
```

### GitHub Codespaces

```json
// .devcontainer/devcontainer.json
{
  "name": "cllient development",
  "image": "mcr.microsoft.com/devcontainers/rust:1.70",
  "features": {
    "ghcr.io/devcontainers/features/github-cli:1": {}
  },
  "postCreateCommand": "cargo build",
  "customizations": {
    "vscode": {
      "extensions": [
        "rust-lang.rust-analyzer"
      ]
    }
  }
}
```

## Performance Optimization

### Configuration Tuning

```bash
# Use faster models for development
export AI_MODEL=deepseek-chat  # Fastest, cheapest

# Enable connection reuse
export RUST_LOG=reqwest::connect=debug

# Optimize for batch processing
export CLLIENT_BATCH_SIZE=10
```

### Shell Completion

```bash
# Generate completions (future feature)
cllient completions bash > ~/.bash_completion.d/cllient
cllient completions zsh > ~/.oh-my-zsh/completions/_cllient
cllient completions fish > ~/.config/fish/completions/cllient.fish
```

---

**Next**: [2. CLI Usage Guide](2_cli-usage.md) | [4. Configuration](4_configuration.md)