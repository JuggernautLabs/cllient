# cllient Plugin & Hub Implementation Plan

## Executive Summary

Transform cllient so that:
1. `ModelRegistry` can morph into a substrate plugin (Activation)
2. Any plugin can generate a hub (standalone server)
3. cllient ships a hub binary (`cllient-hub`)

**Key insight**: `hub-core` already exists with all needed types. No new crate extraction needed.

---

## Current State

### Repository Structure

| Location | Purpose | Git Status |
|----------|---------|------------|
| `/cllient` | LLM client library | trunk branch |
| `/hypermemetic/hub-core` | Core plexus types | hub-core-lib branch |
| `/hypermemetic/hub-macro` | Activation derive macro | main branch |
| `/hypermemetic/substrate` | Hub runtime + activations | main branch |

### Worktree Configuration

```
substrate   → 7c6105c [main]
hub-core    → 8a2f252 [hub-core-lib]
```

### Duplication Problem

**~2,700 lines** of code duplicated between substrate and hub-core:

| Module | Status | Lines |
|--------|--------|-------|
| context.rs | 100% identical | ~38 |
| errors.rs | 100% identical | ~244 |
| guidance.rs | 100% identical | ~250 |
| method_enum.rs | 100% identical | ~21 |
| middleware.rs | 100% identical | ~42 |
| path.rs | 100% identical | ~48 |
| schema.rs | 100% identical | ~520 |
| streaming.rs | 100% identical | ~239 |
| types.rs | 100% identical | ~192 |
| plexus.rs | ~95% identical | ~900 |

---

## Implementation Phases

### Phase 1: Substrate Migration (hypermemetic)

**Goal**: Make substrate consume hub-core instead of duplicating types.

#### 1.1 Add hub-core dependency

```toml
# hypermemetic/substrate/Cargo.toml
[dependencies]
hub-core = { path = "../hub-core" }
```

#### 1.2 Replace plexus module with re-exports

```rust
// substrate/src/plexus/mod.rs
pub use hub_core::plexus::*;
```

#### 1.3 Delete duplicated files

- `src/plexus/context.rs`
- `src/plexus/errors.rs`
- `src/plexus/guidance.rs`
- `src/plexus/method_enum.rs`
- `src/plexus/middleware.rs`
- `src/plexus/path.rs`
- `src/plexus/schema.rs`
- `src/plexus/streaming.rs`
- `src/plexus/types.rs`
- `src/plexus/plexus.rs`

#### 1.4 Keep substrate-specific code

- Activations (bash, cone, solar, claudecode)
- builder.rs
- mcp_bridge.rs
- bin/ directory

**Estimated effort**: ~1 hour
**Can parallelize with**: Nothing (blocks Phase 2)

---

### Phase 2: CllientActivation Implementation (cllient)

**Goal**: Create plugin wrapper using hub-macro.

#### 2.1 Add dependencies

```toml
# cllient/Cargo.toml
[dependencies]
hub-core = { path = "../hypermemetic/hub-core" }
hub-macro = { path = "../hypermemetic/hub-macro" }

[features]
default = []
hub = ["substrate"]

[dependencies.substrate]
path = "../hypermemetic/substrate"
optional = true
```

#### 2.2 Create event types module

**File**: `src/events.rs`

```rust
//! Per-method event types for streaming responses

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

// --- Completion Events ---
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum CompletionEvent {
    /// Content chunk received
    Content { text: String },
    /// Stream started
    Start,
    /// Token usage info
    Usage {
        input_tokens: u32,
        output_tokens: u32,
        total_tokens: Option<u32>,
    },
    /// Stream completed
    Done { finish_reason: Option<String> },
    /// Error occurred
    Error { message: String, code: Option<String> },
}

// --- Model Listing Events ---
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ModelEvent {
    /// Single model info
    Model(ModelInfo),
    /// Listing complete
    Done { count: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub family: String,
    pub service: String,
    pub verified: bool,
    pub capabilities: CapabilitySummary,
}

// --- Service Listing Events ---
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ServiceEvent {
    /// Single service info
    Service(ServiceInfo),
    /// Listing complete
    Done { count: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceInfo {
    pub name: String,
    pub base_url: String,
    pub model_count: usize,
}

// --- Verification Events ---
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum VerifyEvent {
    /// Starting verification for a model
    Starting { model_id: String },
    /// Model verified successfully
    Success { model_id: String, latency_ms: u64 },
    /// Model verification failed
    Failed { model_id: String, error: String },
    /// All verifications complete
    Done { total: usize, passed: usize, failed: usize },
}

// --- Query Events ---
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum QueryEvent {
    /// Single matching model
    Match(ModelInfo),
    /// Query complete
    Done { count: usize },
}
```

#### 2.3 Create plugin module

**File**: `src/plugin.rs`

```rust
//! Plugin integration for substrate hub

use crate::ModelRegistry;
use crate::events::{CompletionEvent, ModelEvent, ServiceEvent, VerifyEvent, QueryEvent};
use async_stream::stream;
use futures::Stream;
use hub_macro::hub_methods;
use std::sync::Arc;

/// Activation wrapper for ModelRegistry
#[derive(Clone)]
pub struct CllientActivation {
    registry: Arc<ModelRegistry>,
}

impl CllientActivation {
    pub fn new(registry: ModelRegistry) -> Self {
        Self { registry: Arc::new(registry) }
    }

    pub fn from_shared(registry: Arc<ModelRegistry>) -> Self {
        Self { registry }
    }
}

#[hub_methods(
    namespace = "cllient",
    version = "1.0.0",
    description = "Multi-provider LLM client with streaming",
    crate_path = "hub_core"
)]
impl CllientActivation {
    /// Execute an LLM completion request
    #[hub_macro::hub_method]
    async fn complete(
        &self,
        model: String,
        prompt: String,
        system: Option<String>,
        max_tokens: Option<u32>,
        temperature: Option<f64>,
    ) -> impl Stream<Item = CompletionEvent> + Send + 'static {
        let registry = self.registry.clone();
        stream! {
            yield CompletionEvent::Start;

            let result = async {
                let mut builder = registry.from_id(&model)?;
                if let Some(sys) = system {
                    builder = builder.system(&sys);
                }
                if let Some(tokens) = max_tokens {
                    builder = builder.max_tokens(tokens);
                }
                if let Some(temp) = temperature {
                    builder = builder.temperature(temp);
                }
                builder.stream_text(&prompt).await
            }.await;

            match result {
                Ok(mut stream) => {
                    use futures::StreamExt;
                    while let Some(event) = stream.next().await {
                        match event {
                            Ok(e) => yield e.into(),
                            Err(e) => {
                                yield CompletionEvent::Error {
                                    message: e.to_string(),
                                    code: None,
                                };
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    yield CompletionEvent::Error {
                        message: e.to_string(),
                        code: None,
                    };
                }
            }
        }
    }

    /// List all available models
    #[hub_macro::hub_method]
    async fn models(&self) -> impl Stream<Item = ModelEvent> + Send + 'static {
        let registry = self.registry.clone();
        stream! {
            let models = registry.list_models();
            for model_id in &models {
                if let Ok(config) = registry.get_model_info(model_id) {
                    yield ModelEvent::Model(config.into());
                }
            }
            yield ModelEvent::Done { count: models.len() };
        }
    }

    /// List all configured services
    #[hub_macro::hub_method]
    async fn services(&self) -> impl Stream<Item = ServiceEvent> + Send + 'static {
        let registry = self.registry.clone();
        stream! {
            let services = registry.list_services();
            for name in &services {
                if let Ok(config) = registry.get_service(name) {
                    let model_count = registry.models_for_service(name).len();
                    yield ServiceEvent::Service(ServiceInfo {
                        name: name.to_string(),
                        base_url: config.base_url.clone(),
                        model_count,
                    });
                }
            }
            yield ServiceEvent::Done { count: services.len() };
        }
    }

    /// Verify model connectivity
    #[hub_macro::hub_method]
    async fn verify(
        &self,
        targets: Option<Vec<String>>,
    ) -> impl Stream<Item = VerifyEvent> + Send + 'static {
        let registry = self.registry.clone();
        stream! {
            let models: Vec<String> = targets.unwrap_or_else(|| {
                registry.list_models().iter().map(|s| s.to_string()).collect()
            });

            let mut passed = 0;
            let mut failed = 0;

            for model_id in &models {
                yield VerifyEvent::Starting { model_id: model_id.clone() };

                let start = std::time::Instant::now();
                match registry.from_id(model_id) {
                    Ok(builder) => {
                        match builder.send_text("respond with exactly: ok").await {
                            Ok(_) => {
                                passed += 1;
                                yield VerifyEvent::Success {
                                    model_id: model_id.clone(),
                                    latency_ms: start.elapsed().as_millis() as u64,
                                };
                            }
                            Err(e) => {
                                failed += 1;
                                yield VerifyEvent::Failed {
                                    model_id: model_id.clone(),
                                    error: e.to_string(),
                                };
                            }
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        yield VerifyEvent::Failed {
                            model_id: model_id.clone(),
                            error: e.to_string(),
                        };
                    }
                }
            }

            yield VerifyEvent::Done {
                total: models.len(),
                passed,
                failed,
            };
        }
    }

    /// Query models with filters
    #[hub_macro::hub_method]
    async fn query(
        &self,
        service: Option<String>,
        family: Option<String>,
        verified: Option<bool>,
    ) -> impl Stream<Item = QueryEvent> + Send + 'static {
        let registry = self.registry.clone();
        stream! {
            let mut query = registry.query();

            if let Some(s) = service {
                query = query.service(&s);
            }
            if let Some(f) = family {
                query = query.family(&f);
            }
            if verified == Some(true) {
                query = query.verified();
            }

            let models = query.configs();
            let count = models.len();

            for config in models {
                yield QueryEvent::Match(config.into());
            }
            yield QueryEvent::Done { count };
        }
    }
}
```

#### 2.4 Hub convenience methods (feature-gated)

```rust
// src/plugin.rs (continued)

#[cfg(feature = "hub")]
impl CllientActivation {
    /// Serve as standalone hub
    pub async fn serve(self, addr: &str) -> anyhow::Result<()> {
        use hub_core::Plexus;
        let plexus = Plexus::new().register(self);
        plexus.serve(addr).await
    }

    /// Create Plexus containing this plugin
    pub fn into_plexus(self) -> hub_core::Plexus {
        hub_core::Plexus::new().register(self)
    }

    /// Mount into existing Plexus
    pub fn mount_into(self, plexus: hub_core::Plexus) -> hub_core::Plexus {
        plexus.register(self)
    }
}
```

**Estimated effort**: ~2 hours
**Can parallelize with**: Phase 3 (after 2.1-2.2 complete)

---

### Phase 3: ModelRegistry Integration (cllient)

**Goal**: Add plugin conversion methods to ModelRegistry.

#### 3.1 Add plugin conversion

```rust
// src/runtime.rs (add to ModelRegistry impl)

impl ModelRegistry {
    /// Convert to activation plugin
    pub fn into_plugin(self) -> crate::plugin::CllientActivation {
        crate::plugin::CllientActivation::new(self)
    }

    /// Convert to plugin (shared registry)
    pub fn as_plugin(self: &Arc<Self>) -> crate::plugin::CllientActivation {
        crate::plugin::CllientActivation::from_shared(self.clone())
    }
}
```

#### 3.2 Hub methods (feature-gated)

```rust
// src/runtime.rs (continued)

#[cfg(feature = "hub")]
impl ModelRegistry {
    /// Serve registry as standalone hub
    pub async fn serve(self, addr: &str) -> anyhow::Result<()> {
        self.into_plugin().serve(addr).await
    }

    /// Create Plexus from registry
    pub fn into_plexus(self) -> hub_core::Plexus {
        self.into_plugin().into_plexus()
    }
}
```

**Estimated effort**: ~30 minutes
**Can parallelize with**: Phase 2.3-2.4

---

### Phase 4: Hub Binary (cllient)

**Goal**: Ship standalone `cllient-hub` binary.

#### 4.1 Create binary

**File**: `src/bin/cllient-hub.rs`

```rust
//! Standalone hub server for cllient

use clap::Parser;
use cllient::ModelRegistry;

#[derive(Parser)]
#[command(name = "cllient-hub")]
#[command(about = "Serve cllient as a substrate hub")]
struct Args {
    /// Address to bind to
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    bind: String,

    /// Config directory (optional)
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let registry = match args.config {
        Some(path) => ModelRegistry::from_directory(&path)?,
        None => ModelRegistry::new()?,
    };

    println!("Starting cllient hub on {}", args.bind);
    println!("Models: {}", registry.list_models().len());
    println!("Services: {}", registry.list_services().len());

    registry.serve(&args.bind).await
}
```

#### 4.2 Update Cargo.toml

```toml
[[bin]]
name = "cllient-hub"
path = "src/bin/cllient-hub.rs"
required-features = ["hub"]
```

**Estimated effort**: ~30 minutes
**Can parallelize with**: Phase 2, Phase 3

---

### Phase 5: Testing & Examples (cllient)

#### 5.1 Integration test

**File**: `tests/hub_integration.rs`

```rust
#[cfg(feature = "hub")]
mod hub_tests {
    use cllient::ModelRegistry;

    #[tokio::test]
    async fn test_registry_to_plugin() {
        let registry = ModelRegistry::new().unwrap();
        let plugin = registry.into_plugin();
        assert_eq!(plugin.namespace(), "cllient");
    }

    #[tokio::test]
    async fn test_plugin_schema() {
        let registry = ModelRegistry::new().unwrap();
        let plugin = registry.into_plugin();
        let schema = plugin.plugin_schema();
        assert_eq!(schema.namespace, "cllient");
        assert!(schema.methods.len() >= 5);
    }
}
```

#### 5.2 Example

**File**: `examples/hub_standalone.rs`

```rust
//! Example: Run cllient as standalone hub

use cllient::ModelRegistry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let registry = ModelRegistry::new()?;
    println!("Starting hub with {} models", registry.list_models().len());
    registry.serve("127.0.0.1:8080").await
}
```

**Estimated effort**: ~30 minutes
**Can parallelize with**: Everything after Phase 2

---

## Parallelization Diagram

```
Phase 1 (substrate migration)
    │
    ├───────────────────────────────────────┐
    ▼                                       │
Phase 2.1-2.2 (deps + events)               │
    │                                       │
    ├──────────┬──────────┬────────────────┤
    ▼          ▼          ▼                 │
Phase 2.3   Phase 3    Phase 4.1-4.2       │
(plugin)    (runtime)  (binary)            │
    │          │          │                 │
    └──────────┴──────────┴─────────────────┤
                                            ▼
                                    Phase 5 (tests)
```

**For 2 workers:**
- Worker A: Phase 1 → Phase 2.1-2.3
- Worker B: Phase 3 → Phase 4 → Phase 5

**For 3 workers:**
- Worker A: Phase 1
- Worker B: Phase 2 (all)
- Worker C: Phase 3 → Phase 4 → Phase 5 (waits for 2.1-2.2)

---

## File Summary

### New Files

| File | Location | Purpose |
|------|----------|---------|
| `src/events.rs` | cllient | Per-method event types |
| `src/plugin.rs` | cllient | CllientActivation |
| `src/bin/cllient-hub.rs` | cllient | Hub binary |
| `tests/hub_integration.rs` | cllient | Integration tests |
| `examples/hub_standalone.rs` | cllient | Usage example |

### Modified Files

| File | Location | Changes |
|------|----------|---------|
| `Cargo.toml` | cllient | hub-core dep, hub feature |
| `src/lib.rs` | cllient | Export events, plugin modules |
| `src/runtime.rs` | cllient | into_plugin(), serve() |
| `src/plexus/mod.rs` | substrate | Re-export from hub-core |
| `Cargo.toml` | substrate | hub-core dependency |

### Deleted Files (substrate)

- `src/plexus/context.rs`
- `src/plexus/errors.rs`
- `src/plexus/guidance.rs`
- `src/plexus/method_enum.rs`
- `src/plexus/middleware.rs`
- `src/plexus/path.rs`
- `src/plexus/schema.rs`
- `src/plexus/streaming.rs`
- `src/plexus/types.rs`
- `src/plexus/plexus.rs`

---

## Definition of Done

1. [ ] **Substrate uses hub-core**: `cargo build` in substrate passes
2. [ ] **cllient compiles**: `cargo build` in cllient passes
3. [ ] **cllient with hub**: `cargo build --features hub` passes
4. [ ] **Hub binary works**: `cargo run --bin cllient-hub --features hub`
5. [ ] **Tests pass**: `cargo test --features hub`
6. [ ] **One-liner serve**:
   ```rust
   ModelRegistry::new()?.serve("0.0.0.0:8080").await?;
   ```
7. [ ] **Composition works**:
   ```rust
   let plugin = registry.into_plugin();
   plexus.register(plugin);
   ```

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking substrate activations | Test each activation after migration |
| hub-macro crate_path issues | Verify Echo activation pattern first |
| Feature flag complexity | Keep hub feature minimal |
| Performance regression | Benchmark registry methods |

---

## Next Steps

1. **Start Phase 1**: Migrate substrate to hub-core
2. **Verify**: All substrate activations still compile
3. **Continue**: Phase 2-5 in parallel where possible
