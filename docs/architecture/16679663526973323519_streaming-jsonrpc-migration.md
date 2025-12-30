# Streaming-First Architecture for Hub Integration

## Overview

This document outlines the migration of cllient to a fully streaming architecture suitable for integration with the substrate/hub system. The goal is to expose cllient as an Activation plugin that can be mounted into a Plexus hub, with all operations returning streaming responses.

## Target Integration: Substrate Hub

The substrate project provides:
- **Plexus**: Central routing hub for Activation plugins
- **Activation trait**: Plugin interface with streaming methods
- **PlexusStream**: Unified streaming type (`Pin<Box<dyn Stream<Item = PlexusStreamItem>>>`)
- **hub-macro**: Derive macro for generating Activation implementations
- **PlexusStreamItem**: Typed events (Data, Error, Progress, Done)

## Two Integration Approaches

### Option A: Hub-Derive Macro (Simpler)

Use `#[derive(HubStruct)]` or custom derive to generate plugin from registry export:

```rust
// In cllient
#[derive(Clone, Serialize, Deserialize, JsonSchema, HubStruct)]
#[hub_struct(namespace = "cllient.registry")]
pub struct RegistryExport {
    pub models: Vec<ModelInfo>,
    pub services: Vec<ServiceInfo>,
    pub stats: IndexStats,
}

// Generates:
// - RegistryExportPlugin
// - RegistryExportEvent { Data { value }, Error { message } }
// - impl with .get() method returning Stream<RegistryExportEvent>
```

**Pros**: Minimal code, automatic schema generation
**Cons**: Limited to simple get() semantics, can't do streaming completions

### Option B: Manual Activation Implementation (Full Control)

Implement `Activation` trait directly with custom methods:

```rust
pub struct CllientActivation {
    registry: Arc<ModelRegistry>,
}

impl Activation for CllientActivation {
    type Methods = CllientMethods;

    fn namespace(&self) -> &str { "cllient" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
    fn description(&self) -> &str { "Multi-provider LLM client" }
    fn methods(&self) -> Vec<&str> {
        vec!["complete", "models", "services", "verify", "query"]
    }

    async fn call(&self, method: &str, params: Value) -> Result<PlexusStream, PlexusError> {
        match method {
            "complete" => self.complete(params).await,
            "models" => self.list_models(params).await,
            "services" => self.list_services(params).await,
            "verify" => self.verify(params).await,
            "query" => self.query_models(params).await,
            _ => Err(PlexusError::MethodNotFound { ... })
        }
    }
}
```

**Pros**: Full streaming support, custom method signatures
**Cons**: More boilerplate, manual schema maintenance

### Option C: Hybrid with Custom Derive (Recommended)

Create a `#[derive(CllientPlugin)]` macro that understands our domain:

```rust
#[derive(CllientPlugin)]
#[cllient_plugin(namespace = "cllient")]
impl ModelRegistry {
    /// List all available models
    #[cllient_method]
    pub fn models(&self) -> impl Stream<Item = ModelInfo> { ... }

    /// Execute a completion request
    #[cllient_method(streaming)]
    pub fn complete(&self, request: CompletionRequest) -> impl Stream<Item = CompletionEvent> { ... }

    /// Query models with filters
    #[cllient_method]
    pub fn query(&self, filter: QueryFilter) -> impl Stream<Item = ModelInfo> { ... }

    /// Verify model connectivity
    #[cllient_method(streaming)]
    pub fn verify(&self, targets: VerifyTargets) -> impl Stream<Item = VerifyEvent> { ... }
}

// Macro generates:
// - CllientMethods enum (for schema)
// - impl Activation for ModelRegistry
// - PlexusStream wrapping for each method
```

---

## Per-Method Event Types

Each method returns its own strongly-typed event enum. This provides:
- Type safety at compile time
- Clear documentation of what each method can emit
- No runtime matching on irrelevant variants

### Completion Events

```rust
/// Events from completion requests
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompletionEvent {
    /// Token chunk received (streaming)
    Delta { text: String },

    /// Full response complete
    Complete {
        content: String,
        model: String,
        usage: Option<Usage>,
        finish_reason: Option<String>,
    },

    /// Error during completion
    Error(CompletionError),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompletionError {
    /// Human-readable error message
    pub message: String,
    /// Error category for programmatic handling
    pub kind: CompletionErrorKind,
    /// Original provider error code (if any)
    pub provider_code: Option<String>,
    /// HTTP status code (if applicable)
    pub status_code: Option<u16>,
    /// Whether the request can be retried
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompletionErrorKind {
    /// Invalid API key or authentication
    Authentication,
    /// Rate limit exceeded
    RateLimit,
    /// Model not found or not accessible
    ModelNotFound,
    /// Invalid request parameters
    InvalidRequest,
    /// Provider server error
    ProviderError,
    /// Network/connection error
    NetworkError,
    /// Request timeout
    Timeout,
    /// Unknown/other error
    Unknown,
}
```

### Model Query Events

```rust
/// Events from model listing/querying
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelEvent {
    /// A model in the result set
    Model(ModelInfo),

    /// Query complete
    Done { count: usize },

    /// Error during query
    Error(QueryError),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct QueryError {
    pub message: String,
    pub kind: QueryErrorKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryErrorKind {
    InvalidFilter,
    ConfigError,
    Unknown,
}
```

### Service Events

```rust
/// Events from service listing
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServiceEvent {
    /// A service in the result set
    Service(ServiceInfo),

    /// Query complete
    Done { count: usize },

    /// Error
    Error { message: String },
}
```

### Verification Events

```rust
/// Events from verification tests
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerifyEvent {
    /// Starting verification of a target
    Start {
        target: String,
        target_type: VerifyTargetType,
    },

    /// Verification passed
    Pass {
        target: String,
        target_type: VerifyTargetType,
        response: String,  // The poem
        latency_ms: u64,
    },

    /// Verification failed
    Fail {
        target: String,
        target_type: VerifyTargetType,
        error: VerifyError,
        latency_ms: Option<u64>,
    },

    /// Config updated after successful verification
    ConfigUpdated {
        target: String,
        path: String,
    },

    /// All verifications complete
    Done {
        passed: usize,
        failed: usize,
        configs_updated: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerifyTargetType {
    Model,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerifyError {
    pub message: String,
    pub kind: VerifyErrorKind,
    /// Original error from provider (formatted)
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerifyErrorKind {
    /// API key missing or invalid
    Authentication,
    /// Model doesn't exist at provider
    ModelNotFound,
    /// Service config error (template, message builder)
    ConfigError,
    /// Network/timeout error
    NetworkError,
    /// Response parsing failed
    ParseError,
    /// Unknown error
    Unknown,
}
```

---

## Error Formatting

All errors are formatted consistently with structured information:

```rust
impl From<crate::error::ClientError> for CompletionError {
    fn from(err: crate::error::ClientError) -> Self {
        match err {
            ClientError::Http(http_err) => {
                // Parse provider-specific error responses
                let (kind, provider_code, retryable) = parse_http_error(&http_err);
                CompletionError {
                    message: format_error_message(&http_err),
                    kind,
                    provider_code,
                    status_code: http_err.status().map(|s| s.as_u16()),
                    retryable,
                }
            }
            ClientError::Config(cfg_err) => CompletionError {
                message: cfg_err.to_string(),
                kind: CompletionErrorKind::InvalidRequest,
                provider_code: None,
                status_code: None,
                retryable: false,
            },
            ClientError::Render(render_err) => {
                // Extract nested error info
                let (kind, details) = parse_render_error(&render_err);
                CompletionError {
                    message: format!("Request error: {}", render_err),
                    kind,
                    provider_code: extract_provider_code(&render_err),
                    status_code: extract_status_code(&render_err),
                    retryable: kind == CompletionErrorKind::RateLimit,
                }
            }
            _ => CompletionError {
                message: err.to_string(),
                kind: CompletionErrorKind::Unknown,
                provider_code: None,
                status_code: None,
                retryable: false,
            },
        }
    }
}

/// Format provider error responses nicely
fn format_error_message(err: &HttpError) -> String {
    // Try to extract message from JSON error body
    if let Some(body) = &err.body {
        if let Ok(json) = serde_json::from_str::<Value>(body) {
            // OpenAI format: {"error": {"message": "..."}}
            if let Some(msg) = json.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
                return msg.to_string();
            }
            // Anthropic format: {"error": {"message": "..."}}
            if let Some(msg) = json.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
                return msg.to_string();
            }
            // Generic: {"message": "..."}
            if let Some(msg) = json.get("message").and_then(|m| m.as_str()) {
                return msg.to_string();
            }
        }
    }

    // Fallback to status-based message
    match err.status {
        Some(401) => "Authentication failed - check your API key".to_string(),
        Some(403) => "Access denied - you may not have access to this model".to_string(),
        Some(404) => "Model or endpoint not found".to_string(),
        Some(429) => "Rate limit exceeded - please retry later".to_string(),
        Some(500..=599) => "Provider server error - please retry later".to_string(),
        _ => err.to_string(),
    }
}

/// Determine error kind from HTTP response
fn parse_http_error(err: &HttpError) -> (CompletionErrorKind, Option<String>, bool) {
    let status = err.status.unwrap_or(0);

    let kind = match status {
        401 => CompletionErrorKind::Authentication,
        429 => CompletionErrorKind::RateLimit,
        404 => CompletionErrorKind::ModelNotFound,
        400 => CompletionErrorKind::InvalidRequest,
        500..=599 => CompletionErrorKind::ProviderError,
        0 => CompletionErrorKind::NetworkError,
        _ => CompletionErrorKind::Unknown,
    };

    let provider_code = extract_error_code_from_body(&err.body);
    let retryable = matches!(kind, CompletionErrorKind::RateLimit | CompletionErrorKind::ProviderError);

    (kind, provider_code, retryable)
}
```

### Wrapping for PlexusStream

Each method wraps its typed events for the hub:

```rust
impl CllientActivation {
    async fn complete(&self, params: Value) -> Result<PlexusStream, PlexusError> {
        let request: CompletionRequest = serde_json::from_value(params)
            .map_err(|e| PlexusError::InvalidParams(e.to_string()))?;

        let registry = self.registry.clone();
        let model_id = request.model.clone();

        let stream = async_stream::stream! {
            // Validate model exists
            let builder = match registry.from_id(&model_id) {
                Ok(b) => b,
                Err(e) => {
                    yield CompletionEvent::Error(CompletionError::from(e));
                    return;
                }
            };

            // Execute with streaming
            let mut response_stream = builder
                .prompt(&request.prompt)
                .max_tokens(request.max_tokens.unwrap_or(1024))
                .execute();

            while let Some(event) = response_stream.next().await {
                yield event;
            }
        };

        Ok(wrap_stream(stream, "cllient.completion", vec!["cllient".into()]))
    }

    async fn list_models(&self, params: Value) -> Result<PlexusStream, PlexusError> {
        let filter: Option<QueryFilter> = serde_json::from_value(params).ok();
        let registry = self.registry.clone();

        let stream = async_stream::stream! {
            let models = match filter {
                Some(f) => registry.query().apply_filter(f).list(),
                None => registry.list_models().iter().map(|s| s.to_string()).collect(),
            };

            let mut count = 0;
            for model_id in &models {
                match registry.get_model_info(model_id) {
                    Ok(info) => {
                        count += 1;
                        yield ModelEvent::Model(info.into());
                    }
                    Err(e) => {
                        yield ModelEvent::Error(QueryError {
                            message: format!("Failed to load {}: {}", model_id, e),
                            kind: QueryErrorKind::ConfigError,
                        });
                    }
                }
            }

            yield ModelEvent::Done { count };
        };

        Ok(wrap_stream(stream, "cllient.model", vec!["cllient".into()]))
    }

    async fn verify(&self, params: Value) -> Result<PlexusStream, PlexusError> {
        let request: VerifyRequest = serde_json::from_value(params)
            .map_err(|e| PlexusError::InvalidParams(e.to_string()))?;

        let registry = self.registry.clone();

        let stream = async_stream::stream! {
            let targets = collect_verify_targets(&registry, &request);
            let mut passed = 0;
            let mut failed = 0;
            let mut configs_updated = 0;

            for target in targets {
                yield VerifyEvent::Start {
                    target: target.clone(),
                    target_type: VerifyTargetType::Model,
                };

                let start = std::time::Instant::now();

                match run_single_verify(&registry, &target).await {
                    Ok(response) => {
                        let latency = start.elapsed().as_millis() as u64;
                        passed += 1;

                        yield VerifyEvent::Pass {
                            target: target.clone(),
                            target_type: VerifyTargetType::Model,
                            response: response.clone(),
                            latency_ms: latency,
                        };

                        // Update config
                        if let Ok(path) = update_model_config(&target, &response) {
                            configs_updated += 1;
                            yield VerifyEvent::ConfigUpdated {
                                target: target.clone(),
                                path: path.display().to_string(),
                            };
                        }
                    }
                    Err(e) => {
                        let latency = start.elapsed().as_millis() as u64;
                        failed += 1;

                        yield VerifyEvent::Fail {
                            target: target.clone(),
                            target_type: VerifyTargetType::Model,
                            error: VerifyError::from(e),
                            latency_ms: Some(latency),
                        };
                    }
                }
            }

            yield VerifyEvent::Done { passed, failed, configs_updated };
        };

        Ok(wrap_stream(stream, "cllient.verify", vec!["cllient".into()]))
    }
}
```

---

## Streaming RequestBuilder Migration

### Current API

```rust
// Non-streaming
let response = registry.from_id("model")?.prompt("hello").send().await?;

// Streaming (separate method)
let stream = registry.from_id("model")?.prompt("hello").stream().await?;
```

### New API (Stream-First)

```rust
impl RequestBuilder {
    /// Primary API - always returns stream of CompletionEvent
    pub fn execute(self) -> impl Stream<Item = CompletionEvent> {
        // Implementation handles both streaming and non-streaming providers
    }

    /// Convenience - collect stream into response
    pub async fn collect(self) -> Result<CompletionResponse> {
        let mut events = self.execute();
        let mut content = String::new();
        let mut usage = None;
        let mut finish_reason = None;

        while let Some(event) = events.next().await {
            match event {
                CompletionEvent::Delta { text } => content.push_str(&text),
                CompletionEvent::Complete { usage: u, finish_reason: fr, .. } => {
                    usage = u;
                    finish_reason = fr;
                }
                CompletionEvent::Error(e) => return Err(e.into()),
            }
        }

        Ok(CompletionResponse { content, usage, finish_reason, .. })
    }

    /// Legacy (deprecated)
    #[deprecated(note = "Use .execute() for streaming or .collect() for one-shot")]
    pub async fn send(self) -> Result<CompletionResponse> {
        self.collect().await
    }
}
```

### Provider Adaptation

```rust
/// Adapter for non-streaming providers
impl RequestBuilder {
    fn execute(self) -> impl Stream<Item = CompletionEvent> {
        let model_id = self.model_id.clone();

        async_stream::stream! {
            match self.send_internal().await {
                Ok(response) => {
                    // Emit content as single delta for non-streaming
                    yield CompletionEvent::Delta {
                        text: response.content.clone()
                    };

                    yield CompletionEvent::Complete {
                        content: response.content,
                        model: model_id,
                        usage: response.usage,
                        finish_reason: response.finish_reason,
                    };
                }
                Err(e) => {
                    yield CompletionEvent::Error(CompletionError::from(e));
                }
            }
        }
    }
}

/// For streaming providers
impl RequestBuilder {
    fn execute_streaming(self) -> impl Stream<Item = CompletionEvent> {
        let model_id = self.model_id.clone();

        async_stream::stream! {
            let stream_result = self.stream_internal().await;

            match stream_result {
                Ok(mut stream) => {
                    let mut full_content = String::new();
                    let mut last_usage = None;

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(StreamEvent::Text(text)) => {
                                full_content.push_str(&text);
                                yield CompletionEvent::Delta { text };
                            }
                            Ok(StreamEvent::Usage(usage)) => {
                                last_usage = Some(usage);
                            }
                            Ok(StreamEvent::Done) => {
                                yield CompletionEvent::Complete {
                                    content: full_content.clone(),
                                    model: model_id.clone(),
                                    usage: last_usage.clone(),
                                    finish_reason: Some("stop".into()),
                                };
                            }
                            Err(e) => {
                                yield CompletionEvent::Error(CompletionError {
                                    message: e.to_string(),
                                    kind: CompletionErrorKind::ProviderError,
                                    provider_code: None,
                                    status_code: None,
                                    retryable: false,
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    yield CompletionEvent::Error(CompletionError::from(e));
                }
            }
        }
    }
}
```

---

## Plugin Schema Generation

For hub discovery, we need to generate schema:

```rust
impl CllientActivation {
    fn plugin_schema(&self) -> PluginSchema {
        PluginSchema::leaf(
            "cllient",
            env!("CARGO_PKG_VERSION"),
            "Multi-provider LLM client with unified streaming API",
            vec![
                MethodSchema::new(
                    "complete",
                    "Execute a completion request, streaming tokens as they arrive",
                    schema_hash::<CompletionRequest>(),
                ).with_input::<CompletionRequest>()
                 .with_output::<CllientEvent>(),

                MethodSchema::new(
                    "models",
                    "List all available models",
                    schema_hash::<()>(),
                ).with_output::<CllientEvent>(),

                MethodSchema::new(
                    "services",
                    "List all configured services",
                    schema_hash::<()>(),
                ).with_output::<CllientEvent>(),

                MethodSchema::new(
                    "verify",
                    "Verify model/service connectivity with test requests",
                    schema_hash::<VerifyRequest>(),
                ).with_input::<VerifyRequest>()
                 .with_output::<CllientEvent>(),

                MethodSchema::new(
                    "query",
                    "Query models with filters (service, family, capabilities)",
                    schema_hash::<QueryFilter>(),
                ).with_input::<QueryFilter>()
                 .with_output::<CllientEvent>(),
            ],
        )
    }
}
```

---

## Implementation Plan

### Parallelization Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              PHASE 1                                     │
│                         (All Independent)                                │
├──────────────────┬──────────────────┬──────────────────┬────────────────┤
│  Event Types     │  Error Types     │  Tests           │  Docs          │
│  (Worker A)      │  (Worker B)      │  (Worker C)      │  (Worker D)    │
├──────────────────┴──────────────────┴──────────────────┴────────────────┤
│                              PHASE 2                                     │
│                    (Depends on Phase 1 types)                            │
├──────────────────────────────────────┬──────────────────────────────────┤
│  RequestBuilder.execute()            │  Error formatting impl           │
│  (Worker A)                          │  (Worker B)                      │
├──────────────────────────────────────┴──────────────────────────────────┤
│                              PHASE 3                                     │
│              (Depends on Phase 2, all methods parallel)                  │
├────────────────┬────────────────┬────────────────┬──────────────────────┤
│  complete()    │  models()      │  verify()      │  services()          │
│  (Worker A)    │  (Worker B)    │  (Worker C)    │  (Worker D)          │
├────────────────┴────────────────┴────────────────┴──────────────────────┤
│                              PHASE 4                                     │
│                         (Hub Integration)                                │
├─────────────────────────────────────────────────────────────────────────┤
│  Activation impl + feature flag + tests                                  │
└─────────────────────────────────────────────────────────────────────────┘
```

### Phase 1: Foundation Types (Parallel - 4 Workers)

All tasks in Phase 1 are independent and can run simultaneously:

| Task | Worker | File | Dependencies |
|------|--------|------|--------------|
| **1A: Completion events** | A | `src/events/completion.rs` | None |
| **1B: Model/Service events** | B | `src/events/model.rs`, `src/events/service.rs` | None |
| **1C: Verify events** | C | `src/events/verify.rs` | None |
| **1D: Error types** | D | `src/events/errors.rs` | None |

```rust
// 1A: src/events/completion.rs
pub enum CompletionEvent { Delta, Complete, Error(CompletionError) }
pub struct CompletionError { message, kind, provider_code, status_code, retryable }
pub enum CompletionErrorKind { Authentication, RateLimit, ModelNotFound, ... }

// 1B: src/events/model.rs
pub enum ModelEvent { Model(ModelInfo), Done { count }, Error(QueryError) }

// 1C: src/events/verify.rs
pub enum VerifyEvent { Start, Pass, Fail, ConfigUpdated, Done }
pub struct VerifyError { message, kind, details }

// 1D: src/events/errors.rs
// Common error formatting functions, From impls
```

**Sync point**: All workers merge into `src/events/mod.rs` with re-exports.

### Phase 2: Streaming Infrastructure (Parallel - 2 Workers)

Depends on Phase 1 types being complete:

| Task | Worker | File | Dependencies |
|------|--------|------|--------------|
| **2A: RequestBuilder.execute()** | A | `src/types.rs` | Phase 1 complete |
| **2B: Error formatting** | B | `src/error_format.rs` | Phase 1D complete |

```rust
// 2A: Add to RequestBuilder
pub fn execute(self) -> impl Stream<Item = CompletionEvent>
pub async fn collect(self) -> Result<CompletionResponse>

// 2B: Error formatting utilities
fn format_error_message(err: &HttpError) -> String
fn parse_http_error(err: &HttpError) -> (CompletionErrorKind, Option<String>, bool)
impl From<ClientError> for CompletionError
impl From<ClientError> for VerifyError
```

### Phase 3: Activation Methods (Parallel - 4 Workers)

Each activation method can be implemented independently:

| Task | Worker | File | Dependencies |
|------|--------|------|--------------|
| **3A: complete()** | A | `src/activation/complete.rs` | 2A |
| **3B: models() + query()** | B | `src/activation/models.rs` | 1B |
| **3C: verify()** | C | `src/activation/verify.rs` | 1C, 2B |
| **3D: services()** | D | `src/activation/services.rs` | 1B |

All methods have the same signature:
```rust
async fn method(&self, params: Value) -> Result<PlexusStream, PlexusError>
```

**Sync point**: All workers merge into `src/activation/mod.rs` with the `call()` dispatcher.

### Phase 4: Hub Integration (Sequential)

Must wait for Phase 3 to complete:

| Task | Order | File | Dependencies |
|------|-------|------|--------------|
| **4A: Activation trait impl** | 1 | `src/activation/mod.rs` | Phase 3 complete |
| **4B: Feature flag setup** | 2 | `Cargo.toml`, `src/lib.rs` | 4A |
| **4C: Integration tests** | 3 | `tests/hub_integration.rs` | 4B |

---

## Work Breakdown by File

### Can Be Done In Parallel (No Dependencies)

| File | Purpose | Estimated Size |
|------|---------|----------------|
| `src/events/completion.rs` | CompletionEvent, CompletionError | ~80 lines |
| `src/events/model.rs` | ModelEvent, QueryError | ~40 lines |
| `src/events/service.rs` | ServiceEvent | ~30 lines |
| `src/events/verify.rs` | VerifyEvent, VerifyError | ~60 lines |
| `src/events/errors.rs` | Error formatting utilities | ~100 lines |

### Sequential (Has Dependencies)

| File | Purpose | Depends On |
|------|---------|------------|
| `src/events/mod.rs` | Re-exports | All event files |
| `src/types.rs` (modify) | Add .execute() | events/completion.rs |
| `src/error_format.rs` | From impls | events/errors.rs |
| `src/activation/complete.rs` | complete() method | types.rs changes |
| `src/activation/models.rs` | models(), query() methods | events/model.rs |
| `src/activation/verify.rs` | verify() method | events/verify.rs, error_format.rs |
| `src/activation/services.rs` | services() method | events/service.rs |
| `src/activation/mod.rs` | Activation impl | All activation/*.rs |

---

## Recommended Work Assignment

### For 2 Workers

```
Worker A: Events + RequestBuilder
  Phase 1: completion.rs, model.rs, service.rs, mod.rs
  Phase 2: types.rs (execute/collect)
  Phase 3: complete.rs, models.rs

Worker B: Errors + Activation
  Phase 1: verify.rs, errors.rs
  Phase 2: error_format.rs
  Phase 3: verify.rs, services.rs
  Phase 4: activation/mod.rs, integration
```

### For 4 Workers

```
Worker A: Completion flow
  Phase 1: events/completion.rs
  Phase 2: types.rs (execute/collect)
  Phase 3: activation/complete.rs

Worker B: Model/Query flow
  Phase 1: events/model.rs, events/service.rs
  Phase 3: activation/models.rs, activation/services.rs

Worker C: Verification flow
  Phase 1: events/verify.rs
  Phase 3: activation/verify.rs

Worker D: Error handling + Integration
  Phase 1: events/errors.rs
  Phase 2: error_format.rs
  Phase 4: activation/mod.rs, feature flag, tests
```

---

## Critical Path

The minimum sequential chain (longest dependency path):

```
events/completion.rs → types.rs (execute) → activation/complete.rs → activation/mod.rs
        ↓
   ~80 lines         ~50 lines           ~60 lines              ~100 lines

Total: ~290 lines on critical path
```

Everything else can proceed in parallel with the critical path.

---

## File Changes Summary

| File | Action | Purpose | Parallel Group |
|------|--------|---------|----------------|
| `src/events/mod.rs` | Create | Module exports | - |
| `src/events/completion.rs` | Create | CompletionEvent | P1-A |
| `src/events/model.rs` | Create | ModelEvent | P1-B |
| `src/events/service.rs` | Create | ServiceEvent | P1-B |
| `src/events/verify.rs` | Create | VerifyEvent | P1-C |
| `src/events/errors.rs` | Create | Error utilities | P1-D |
| `src/error_format.rs` | Create | From impls | P2-B |
| `src/types.rs` | Modify | Add .execute() | P2-A |
| `src/activation/mod.rs` | Create | Activation impl | P4 |
| `src/activation/complete.rs` | Create | complete() | P3-A |
| `src/activation/models.rs` | Create | models(), query() | P3-B |
| `src/activation/verify.rs` | Create | verify() | P3-C |
| `src/activation/services.rs` | Create | services() | P3-D |
| `src/lib.rs` | Modify | Feature-gated export | P4 |
| `Cargo.toml` | Modify | Add hub feature | P4 |

### Cargo.toml Changes

```toml
[features]
default = []
hub = ["substrate-hub"]

[dependencies]
substrate-hub = { version = "0.2", optional = true }

# For streaming
async-stream = "0.3"
futures = "0.3"
```

---

## Usage Example

### As Standalone Library

```rust
use cllient::{ModelRegistry, CompletionEvent, CompletionError};

let registry = ModelRegistry::new()?;

// Stream-first API with typed events
let mut events = registry.from_id("gpt-4")?.prompt("hello").execute();
while let Some(event) = events.next().await {
    match event {
        CompletionEvent::Delta { text } => print!("{}", text),
        CompletionEvent::Complete { content, usage, .. } => {
            println!("\nDone!");
            if let Some(u) = usage {
                println!("Tokens: {} in, {} out", u.input_tokens, u.output_tokens);
            }
        }
        CompletionEvent::Error(err) => {
            eprintln!("Error: {}", err.message);
            if err.retryable {
                eprintln!("  (retryable)");
            }
            if let Some(code) = &err.provider_code {
                eprintln!("  Provider code: {}", code);
            }
        }
    }
}
```

### Mounted in Hub

```rust
use substrate_hub::{Plexus, PlexusBuilder};
use cllient::CllientActivation;

let cllient = CllientActivation::new(ModelRegistry::new()?);

let plexus = PlexusBuilder::new()
    .mount(cllient)
    .build();

// Now accessible via plexus.call("cllient.complete", params)
```

### Via Hub CLI

```bash
# Start hub with cllient mounted
hub serve --mount cllient

# Call from another process
echo '{"model":"gpt-4","prompt":"hello"}' | hub call cllient.complete
```

---

## Open Questions

1. **Macro vs Manual**: Start with manual Activation impl, add macro later?
2. **Feature gating**: Make hub integration optional via feature flag?
3. **Schema versioning**: How to handle schema changes across versions?
4. **Handle resolution**: Should cllient support hub Handle references?

---

## Success Metrics

- All completions stream by default
- Verification emits progress in real-time
- CllientActivation passes hub schema validation
- Backward compatible with existing .send() calls
- Optional hub integration doesn't bloat core library
