# LLM Client System Makefile
# Provides convenient abstractions for managing models, providers, and pricing data

# Load environment variables from .env file if it exists
-include .env
export

# Colors for output
BOLD := \033[1m
RED := \033[31m
GREEN := \033[32m
YELLOW := \033[33m
BLUE := \033[34m
RESET := \033[0m

# Default target
.PHONY: help
help: ## Show this help message
	@echo "$(BOLD)LLM Client System$(RESET)"
	@echo "Runtime-configurable LLM client with unassociated truth architecture"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "$(BLUE)%-20s$(RESET) %s\n", $$1, $$2}'

# =============================================================================
# Build & Development
# =============================================================================

.PHONY: build test clean
build: ## Build the Rust project
	@echo "$(YELLOW)Building cllient...$(RESET)"
	cargo build

test: ## Run all tests
	@echo "$(YELLOW)Running tests...$(RESET)"
	cargo test

release: ## Build optimized release version
	@echo "$(YELLOW)Building release version...$(RESET)"
	cargo build --release

clean: ## Clean build artifacts
	@echo "$(YELLOW)Cleaning build artifacts...$(RESET)"
	cargo clean

# =============================================================================
# Model & Provider Information APIs
# =============================================================================

.PHONY: models-list models-openai models-anthropic models-deepseek models-per-service models-openrouter models-openrouter-all
models-list: ## List all configured models
	@echo "$(BOLD)Available Models:$(RESET)"
	@cargo run --bin cllient -- list

models-openai: ## Fetch latest OpenAI models from API
	@echo "$(BOLD)OpenAI API Models:$(RESET)"
	@if [ -z "$$OPENAI_API_KEY" ]; then \
		echo "$(RED)Error: OPENAI_API_KEY environment variable not set$(RESET)"; \
		echo "Set it with: export OPENAI_API_KEY=your_key_here"; \
		exit 1; \
	fi
	@curl -s https://api.openai.com/v1/models \
		-H "Authorization: Bearer $$OPENAI_API_KEY" \
		| jq -r '.data[] | select(.id | test("gpt|o1|chatgpt")) | .id' \
		| sort

models-anthropic: ## Fetch latest Anthropic models from API
	@echo "$(BOLD)Anthropic API Models:$(RESET)"
	@if [ -z "$$ANTHROPIC_API_KEY" ]; then \
		echo "$(RED)Error: ANTHROPIC_API_KEY environment variable not set$(RESET)"; \
		echo "Set it with: export ANTHROPIC_API_KEY=your_key_here"; \
		exit 1; \
	fi
	@curl -s https://api.anthropic.com/v1/models \
		-H "x-api-key: $$ANTHROPIC_API_KEY" \
		-H "anthropic-version: 2023-06-01" \
		| jq -r '.data[] | .id' \
		| sort

models-deepseek: ## Fetch latest DeepSeek models from API
	@echo "$(BOLD)DeepSeek API Models:$(RESET)"
	@if [ -z "$$DEEPSEEK_API_KEY" ]; then \
		echo "$(RED)Error: DEEPSEEK_API_KEY environment variable not set$(RESET)"; \
		echo "Set it with: export DEEPSEEK_API_KEY=your_key_here"; \
		exit 1; \
	fi
	@curl -s https://api.deepseek.com/models \
		-H "Authorization: Bearer $$DEEPSEEK_API_KEY" \
		| jq -r '.data[] | .id' \
		| sort

models-per-service: ## List models grouped by service
	@echo "$(BOLD)Models by Service:$(RESET)"
	@echo "$(BLUE)OpenAI Models:$(RESET)"
	@cargo run --bin cllient -- list | grep -E "gpt|o[0-9]|chatgpt" | sort | sed 's/^/  /'
	@echo ""
	@echo "$(BLUE)Anthropic Models:$(RESET)"
	@cargo run --bin cllient -- list | grep claude | sort | sed 's/^/  /'
	@echo ""
	@echo "$(BLUE)DeepSeek Models:$(RESET)"
	@cargo run --bin cllient -- list | grep deepseek | sort | sed 's/^/  /'

models-openrouter: ## Fetch latest models from OpenRouter API
	@echo "$(BOLD)OpenRouter API Models:$(RESET)"
	@if [ -z "$$OPEN_ROUTER_API_KEY" ]; then \
		echo "$(RED)Error: OPEN_ROUTER_API_KEY environment variable not set$(RESET)"; \
		echo "Set it with: export OPEN_ROUTER_API_KEY=your_key_here"; \
		exit 1; \
	fi
	@curl -s -H "Authorization: Bearer $$OPEN_ROUTER_API_KEY" \
		https://openrouter.ai/api/v1/models \
		| jq -r '.data[] | select(.id | startswith("openai/") or startswith("anthropic/")) | .id' \
		| sort

models-openrouter-all: ## Fetch all models from OpenRouter API with details (JSON format)
	@if [ -z "$$OPEN_ROUTER_API_KEY" ]; then \
		echo "$(RED)Error: OPEN_ROUTER_API_KEY environment variable not set$(RESET)" >&2; \
		exit 1; \
	fi
	@curl -s -H "Authorization: Bearer $$OPEN_ROUTER_API_KEY" \
		https://openrouter.ai/api/v1/models \
		| jq '.data[] | select(.id | startswith("openai/") or startswith("anthropic/")) | {id: .id, name: .name, canonical_slug: (.id | gsub("/"; "-")), context_length: .context_length, input_cost: (.pricing.prompt | tonumber * 1000), output_cost: (.pricing.completion | tonumber * 1000)}' \
		| jq -s 'sort_by(.input_cost)'

models-openrouter-complete: ## Fetch ALL models from OpenRouter API with complete details (JSON format)
	@if [ -z "$$OPEN_ROUTER_API_KEY" ]; then \
		echo "$(RED)Error: OPEN_ROUTER_API_KEY environment variable not set$(RESET)" >&2; \
		exit 1; \
	fi
	@curl -s -H "Authorization: Bearer $$OPEN_ROUTER_API_KEY" \
		https://openrouter.ai/api/v1/models \
		| jq '.data[] | {id: .id, name: .name, canonical_slug: (.id | gsub("/"; "-")), description: .description, context_length: .context_length, architecture: .architecture, pricing: {prompt: (.pricing.prompt | tonumber), completion: (.pricing.completion | tonumber), image: (.pricing.image // null), request: (.pricing.request // null)}, top_provider: .top_provider, per_request_limits: .per_request_limits}' \
		| jq -s 'sort_by(.id)'

# =============================================================================
# Pricing Information APIs
# =============================================================================

.PHONY: pricing-all pricing-cheapest pricing-provider pricing-update helicone-openai helicone-anthropic helicone-search helicone-compare helicone-csv
pricing-all: ## Get pricing for all models (via Helicone API)
	@echo "$(BOLD)All Model Pricing (per 1M tokens):$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs" \
		| jq -r '.data[] | select(.show_in_playground == true) | "\(.provider) \(.model): $\(.input_cost_per_1m)/$\(.output_cost_per_1m)"' \
		| sort

pricing-cheapest: ## Show the 10 cheapest models by input cost
	@echo "$(BOLD)10 Cheapest Models (by input cost per 1M tokens):$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs" \
		| jq -r '.data[] | select(.show_in_playground == true and .input_cost_per_1m != null) | "\(.input_cost_per_1m) \(.provider) \(.model)"' \
		| sort -n \
		| head -10 \
		| awk '{printf "$$%.2f %s %s\n", $$1, $$2, $$3}'

pricing-provider: ## Get pricing for specific provider (usage: make pricing-provider PROVIDER=openai)
	@if [ -z "$(PROVIDER)" ]; then \
		echo "$(RED)Error: PROVIDER not specified$(RESET)"; \
		echo "Usage: make pricing-provider PROVIDER=openai"; \
		exit 1; \
	fi
	@echo "$(BOLD)Pricing for $(PROVIDER):$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs?provider=$(shell echo $(PROVIDER) | tr '[:lower:]' '[:upper:]')" \
		| jq -r '.data[] | "\(.model): $\(.input_cost_per_1m)/$\(.output_cost_per_1m) per 1M tokens"' \
		| sort

pricing-update: ## Update pricing information in config files
	@echo "$(YELLOW)Updating pricing information in model configs...$(RESET)"
	@echo "$(RED)TODO: Implement automatic config update from Helicone API$(RESET)"

# =============================================================================
# Helicone API Helpers
# =============================================================================

helicone-openai: ## Get all OpenAI models from Helicone with pricing (JSON format)
	@curl -s "https://www.helicone.ai/api/llm-costs?provider=OPENAI" \
		| jq '.data[] | select(.show_in_playground == true) | {model: .model, input_cost_per_1m: .input_cost_per_1m, output_cost_per_1m: .output_cost_per_1m}' \
		| jq -s 'sort_by(.input_cost_per_1m)'

helicone-anthropic: ## Get all Anthropic models from Helicone with pricing
	@echo "$(BOLD)Anthropic Models from Helicone:$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs?provider=ANTHROPIC" \
		| jq -r '.data[] | select(.show_in_playground == true) | "\(.model): $\(.input_cost_per_1m)/$\(.output_cost_per_1m) per 1M tokens"' \
		| sort

helicone-search: ## Search Helicone for models by name (usage: make helicone-search MODEL=gpt-5)
	@if [ -z "$(MODEL)" ]; then \
		echo "$(RED)Error: MODEL not specified$(RESET)"; \
		echo "Usage: make helicone-search MODEL=gpt-5"; \
		exit 1; \
	fi
	@echo "$(BOLD)Helicone Models matching '$(MODEL)':$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs?model=$(MODEL)" \
		| jq -r '.data[] | select(.show_in_playground == true) | "\(.provider) \(.model): $\(.input_cost_per_1m)/$\(.output_cost_per_1m) per 1M tokens"' \
		| sort

helicone-compare: ## Compare costs between providers for similar models
	@echo "$(BOLD)Cost Comparison Across Providers:$(RESET)"
	@echo "$(BLUE)GPT-4 Class Models:$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs" \
		| jq -r '.data[] | select(.model | test("gpt-4|claude-3|claude-4")) | select(.show_in_playground == true) | "\(.provider) \(.model): $\(.input_cost_per_1m)/$\(.output_cost_per_1m)"' \
		| sort -t '$' -k2 -n \
		| head -10
	@echo ""
	@echo "$(BLUE)Mini/Small Models:$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs" \
		| jq -r '.data[] | select(.model | test("mini|haiku|nano")) | select(.show_in_playground == true) | "\(.provider) \(.model): $\(.input_cost_per_1m)/$\(.output_cost_per_1m)"' \
		| sort -t '$' -k2 -n \
		| head -10

helicone-csv: ## Export all pricing data to CSV (usage: make helicone-csv > pricing.csv)
	@curl -s "https://www.helicone.ai/api/llm-costs?format=csv"

# =============================================================================
# Testing & Streaming
# =============================================================================

.PHONY: test-deepseek test-openai test-anthropic test-streaming
test-deepseek: ## Test DeepSeek streaming (requires DEEPSEEK_API_KEY)
	@echo "$(BOLD)Testing DeepSeek streaming:$(RESET)"
	@if [ -z "$$DEEPSEEK_API_KEY" ]; then \
		echo "$(RED)Error: DEEPSEEK_API_KEY environment variable not set$(RESET)"; \
		exit 1; \
	fi
	@cargo run --bin cllient -- stream deepseek-chat "Hello, test from Makefile"

test-openai: ## Test OpenAI streaming (requires OPENAI_API_KEY)
	@echo "$(BOLD)Testing OpenAI streaming:$(RESET)"
	@if [ -z "$$OPENAI_API_KEY" ]; then \
		echo "$(RED)Error: OPENAI_API_KEY environment variable not set$(RESET)"; \
		exit 1; \
	fi
	@cargo run --bin cllient -- stream gpt-4o-mini "Hello, test from Makefile"

test-anthropic: ## Test Anthropic streaming (requires ANTHROPIC_API_KEY)
	@echo "$(BOLD)Testing Anthropic streaming:$(RESET)"
	@if [ -z "$$ANTHROPIC_API_KEY" ]; then \
		echo "$(RED)Error: ANTHROPIC_API_KEY environment variable not set$(RESET)"; \
		exit 1; \
	fi
	@cargo run --bin cllient -- stream claude-3-haiku-20240307 "Hello, test from Makefile"

test-streaming: ## Test all providers with streaming
	@echo "$(BOLD)Testing all providers:$(RESET)"
	@make test-deepseek || echo "$(RED)DeepSeek test failed$(RESET)"
	@make test-openai || echo "$(RED)OpenAI test failed$(RESET)"
	@make test-anthropic || echo "$(RED)Anthropic test failed$(RESET)"

# =============================================================================
# Configuration Management
# =============================================================================

.PHONY: config-check config-validate env-setup
config-check: ## Check configuration files for syntax errors
	@echo "$(YELLOW)Checking YAML configuration files...$(RESET)"
	@for file in config/service/*.yaml config/family/*/*.yaml; do \
		if [ -f "$$file" ]; then \
			echo "Checking $$file..."; \
			python3 -c "import yaml; yaml.safe_load(open('$$file'))" || echo "$(RED)Error in $$file$(RESET)"; \
		fi; \
	done
	@echo "$(GREEN)Configuration check complete$(RESET)"

config-validate: ## Validate that all models have corresponding service configs
	@echo "$(YELLOW)Validating model-service associations...$(RESET)"
	@cargo run --bin cllient -- list > /dev/null && echo "$(GREEN)All configurations valid$(RESET)" || echo "$(RED)Configuration validation failed$(RESET)"

env-setup: ## Set up environment from .env.example
	@if [ ! -f .env ]; then \
		echo "$(YELLOW)Creating .env from .env.example...$(RESET)"; \
		cp .env.example .env; \
		echo "$(GREEN)Created .env file. Please edit it with your API keys.$(RESET)"; \
	else \
		echo "$(GREEN).env file already exists$(RESET)"; \
	fi

# =============================================================================
# Development Utilities
# =============================================================================

.PHONY: format lint fix watch commit
format: ## Format Rust code
	@echo "$(YELLOW)Formatting Rust code...$(RESET)"
	cargo fmt

lint: ## Run linting checks
	@echo "$(YELLOW)Running clippy lints...$(RESET)"
	cargo clippy -- -D warnings

fix: ## Auto-fix linting issues
	@echo "$(YELLOW)Auto-fixing linting issues...$(RESET)"
	cargo clippy --fix --allow-dirty --allow-staged

watch: ## Watch for changes and rebuild
	@echo "$(YELLOW)Watching for changes...$(RESET)"
	cargo watch -x "build"

commit: ## Build, test, and commit changes
	@echo "$(YELLOW)Building and testing before commit...$(RESET)"
	@make build
	@make test
	@make lint
	@echo "$(GREEN)Ready to commit!$(RESET)"

# =============================================================================
# Provider-Specific Utilities
# =============================================================================

.PHONY: provider-costs provider-models provider-test
provider-costs: ## Get detailed cost breakdown for a provider (usage: make provider-costs PROVIDER=openai)
	@if [ -z "$(PROVIDER)" ]; then \
		echo "$(RED)Error: PROVIDER not specified$(RESET)"; \
		echo "Usage: make provider-costs PROVIDER=openai"; \
		exit 1; \
	fi
	@echo "$(BOLD)Detailed costs for $(PROVIDER):$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs?provider=$(shell echo $(PROVIDER) | tr '[:lower:]' '[:upper:]')" \
		| jq -r '.data[] | select(.show_in_playground == true) | "$(BLUE)\(.model)$(RESET): Input: $$\(.input_cost_per_1m | if . == null then "N/A" else . end), Output: $$\(.output_cost_per_1m | if . == null then "N/A" else . end)"'

provider-models: ## List models for a specific provider (usage: make provider-models PROVIDER=openai)
	@if [ -z "$(PROVIDER)" ]; then \
		echo "$(RED)Error: PROVIDER not specified$(RESET)"; \
		echo "Usage: make provider-models PROVIDER=openai"; \
		exit 1; \
	fi
	@case "$(PROVIDER)" in \
		openai) make models-openai ;; \
		anthropic) make models-anthropic ;; \
		deepseek) make models-deepseek ;; \
		*) echo "$(RED)Unknown provider: $(PROVIDER)$(RESET)"; echo "Supported: openai, anthropic, deepseek" ;; \
	esac

provider-test: ## Test a specific provider (usage: make provider-test PROVIDER=deepseek)
	@if [ -z "$(PROVIDER)" ]; then \
		echo "$(RED)Error: PROVIDER not specified$(RESET)"; \
		echo "Usage: make provider-test PROVIDER=deepseek"; \
		exit 1; \
	fi
	@case "$(PROVIDER)" in \
		openai) make test-openai ;; \
		anthropic) make test-anthropic ;; \
		deepseek) make test-deepseek ;; \
		*) echo "$(RED)Unknown provider: $(PROVIDER)$(RESET)"; echo "Supported: openai, anthropic, deepseek" ;; \
	esac

# =============================================================================
# Documentation & Analysis
# =============================================================================

.PHONY: docs costs-report models-report
docs: ## Generate documentation
	@echo "$(YELLOW)Generating Rust documentation...$(RESET)"
	cargo doc --open

costs-report: ## Generate comprehensive cost analysis report
	@echo "$(BOLD)LLM Cost Analysis Report$(RESET)" > costs-report.md
	@echo "Generated on: $$(date)" >> costs-report.md
	@echo "" >> costs-report.md
	@echo "## Cheapest Models" >> costs-report.md
	@curl -s "https://www.helicone.ai/api/llm-costs" \
		| jq -r '.data[] | select(.show_in_playground == true and .input_cost_per_1m != null) | "- \(.provider) \(.model): $$\(.input_cost_per_1m)/$$\(.output_cost_per_1m) per 1M tokens"' \
		| sort -t '$$' -k2 -n \
		| head -20 >> costs-report.md
	@echo "$(GREEN)Cost report generated: costs-report.md$(RESET)"

models-report: ## Generate comprehensive models report
	@echo "$(BOLD)Available Models Report$(RESET)" > models-report.md
	@echo "Generated on: $$(date)" >> models-report.md
	@echo "" >> models-report.md
	@echo "## Configured Models" >> models-report.md
	@cargo run --bin cllient -- list >> models-report.md 2>/dev/null || true
	@echo "$(GREEN)Models report generated: models-report.md$(RESET)"

# =============================================================================
# Environment & Dependencies
# =============================================================================

.PHONY: deps-check deps-install env-check
deps-check: ## Check for required dependencies
	@echo "$(YELLOW)Checking dependencies...$(RESET)"
	@command -v cargo >/dev/null 2>&1 || { echo "$(RED)Rust/Cargo not found$(RESET)"; exit 1; }
	@command -v curl >/dev/null 2>&1 || { echo "$(RED)curl not found$(RESET)"; exit 1; }
	@command -v jq >/dev/null 2>&1 || { echo "$(RED)jq not found. Install with: brew install jq$(RESET)"; exit 1; }
	@echo "$(GREEN)All dependencies found$(RESET)"

deps-install: ## Install missing dependencies (macOS)
	@echo "$(YELLOW)Installing dependencies...$(RESET)"
	@if ! command -v jq >/dev/null 2>&1; then \
		echo "Installing jq..."; \
		brew install jq; \
	fi
	@if ! command -v cargo >/dev/null 2>&1; then \
		echo "Installing Rust..."; \
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh; \
	fi

env-check: ## Check environment variables
	@echo "$(BOLD)Environment Check:$(RESET)"
	@if [ -n "$$OPENAI_API_KEY" ]; then echo "$(GREEN)✓ OPENAI_API_KEY set$(RESET)"; else echo "$(RED)✗ OPENAI_API_KEY not set$(RESET)"; fi
	@if [ -n "$$ANTHROPIC_API_KEY" ]; then echo "$(GREEN)✓ ANTHROPIC_API_KEY set$(RESET)"; else echo "$(RED)✗ ANTHROPIC_API_KEY not set$(RESET)"; fi
	@if [ -n "$$DEEPSEEK_API_KEY" ]; then echo "$(GREEN)✓ DEEPSEEK_API_KEY set$(RESET)"; else echo "$(RED)✗ DEEPSEEK_API_KEY not set$(RESET)"; fi

# =============================================================================
# Advanced Features
# =============================================================================

.PHONY: benchmark compare-costs model-search
benchmark: ## Run performance benchmarks
	@echo "$(YELLOW)Running benchmarks...$(RESET)"
	@echo "$(RED)TODO: Implement performance benchmarks$(RESET)"

compare-costs: ## Compare costs across providers for similar models
	@echo "$(BOLD)Cost Comparison for Similar Models:$(RESET)"
	@echo "$(BLUE)GPT-4 Class Models:$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs" \
		| jq -r '.data[] | select(.model | test("gpt-4|claude-3|deepseek")) | "\(.provider) \(.model): $$\(.input_cost_per_1m)/$$\(.output_cost_per_1m)"' \
		| sort

model-search: ## Search for models by name (usage: make model-search QUERY=gpt)
	@if [ -z "$(QUERY)" ]; then \
		echo "$(RED)Error: QUERY not specified$(RESET)"; \
		echo "Usage: make model-search QUERY=gpt"; \
		exit 1; \
	fi
	@echo "$(BOLD)Models matching '$(QUERY)':$(RESET)"
	@curl -s "https://www.helicone.ai/api/llm-costs?model=$(QUERY)" \
		| jq -r '.data[] | "\(.provider) \(.model): $$\(.input_cost_per_1m)/$$\(.output_cost_per_1m)"' \
		| sort