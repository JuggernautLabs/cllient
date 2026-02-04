//! Plugin integration for Plexus RPC
//!
//! This module provides `CllientActivation`, a wrapper around `ModelRegistry`
//! that implements the Plexus RPC `Activation` trait using the `hub-macro`.
//!
//! # Feature Flags
//!
//! - `plugin` - Enables `CllientActivation` for use with external Plexus instances
//! - `hub` - Adds `serve()` and `into_plexus()` for standalone hub operation
//!
//! # Example
//!
//! ```rust,ignore
//! use cllient::{ModelRegistry, CllientActivation};
//!
//! let registry = ModelRegistry::new()?;
//! let plugin = CllientActivation::new(registry);
//!
//! // Register with a Plexus
//! let plexus = Plexus::new().register(plugin);
//! ```

#![cfg(feature = "plugin")]

use crate::events::{
    CompletionEvent, ModelEvent, ModelInfo, QueryEvent, ServiceEvent, ServiceInfo, VerifyEvent,
};
use crate::ModelRegistry;
use async_stream::stream;
use futures::Stream;
use hub_macro::hub_methods;
use std::sync::Arc;

/// Activation wrapper for ModelRegistry
///
/// Exposes ModelRegistry functionality as a Plexus RPC activation.
#[derive(Clone)]
pub struct CllientActivation {
    registry: Arc<ModelRegistry>,
}

impl CllientActivation {
    /// Create a new CllientActivation from a ModelRegistry
    pub fn new(registry: ModelRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }

    /// Create from a shared ModelRegistry reference
    pub fn from_shared(registry: Arc<ModelRegistry>) -> Self {
        Self { registry }
    }

    /// Get a reference to the underlying registry
    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }
}

// The hub_methods macro generates:
// - Activation trait implementation
// - RPC method dispatch
// - JSON schema for all methods
// - PLUGIN_ID constant (deterministic UUID v5)
#[hub_methods(
    namespace = "cllient",
    version = "1.0.0",
    description = "Multi-provider LLM client with streaming completions",
    crate_path = "hub_core"
)]
impl CllientActivation {
    /// Execute an LLM completion request with streaming response
    #[hub_macro::hub_method]
    pub async fn complete(
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
                Ok(response_stream) => {
                    use futures::StreamExt;
                    let mut stream = response_stream;

                    while let Some(event_result) = stream.next().await {
                        match event_result {
                            Ok(stream_event) => {
                                use crate::streaming::StreamEvent;
                                match stream_event {
                                    StreamEvent::Content(text) => {
                                        yield CompletionEvent::Content { text };
                                    }
                                    StreamEvent::Usage { input_tokens, output_tokens, total_tokens } => {
                                        yield CompletionEvent::Usage {
                                            input_tokens: input_tokens.unwrap_or(0),
                                            output_tokens: output_tokens.unwrap_or(0),
                                            total_tokens,
                                        };
                                    }
                                    StreamEvent::Finish(reason) => {
                                        yield CompletionEvent::Done { finish_reason: reason };
                                    }
                                    StreamEvent::Error(msg) => {
                                        yield CompletionEvent::Error { message: msg, code: None };
                                    }
                                    _ => {}
                                }
                            }
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

    /// List all available models in the registry
    #[hub_macro::hub_method]
    pub async fn models(&self) -> impl Stream<Item = ModelEvent> + Send + 'static {
        let registry = self.registry.clone();

        stream! {
            let model_ids = registry.list_models();
            let mut count = 0;

            for model_id in model_ids {
                if let Ok(config) = registry.get_model_info(model_id) {
                    yield ModelEvent::Model(ModelInfo::from(config));
                    count += 1;
                }
            }

            yield ModelEvent::Done { count };
        }
    }

    /// List all configured services
    #[hub_macro::hub_method]
    pub async fn services(&self) -> impl Stream<Item = ServiceEvent> + Send + 'static {
        let registry = self.registry.clone();

        stream! {
            let service_names = registry.list_services();
            let mut count = 0;

            for name in service_names {
                if let Ok(config) = registry.get_service(name) {
                    let model_count = registry.models_for_service(name).len();
                    let mut info = ServiceInfo::from(config);
                    info.model_count = model_count;
                    yield ServiceEvent::Service(info);
                    count += 1;
                }
            }

            yield ServiceEvent::Done { count };
        }
    }

    /// Verify model connectivity by sending test requests
    #[hub_macro::hub_method]
    pub async fn verify(
        &self,
        targets: Option<Vec<String>>,
    ) -> impl Stream<Item = VerifyEvent> + Send + 'static {
        let registry = self.registry.clone();

        stream! {
            let model_ids: Vec<String> = targets.unwrap_or_else(|| {
                registry.list_models().iter().map(|s| s.to_string()).collect()
            });

            let mut passed = 0;
            let mut failed = 0;
            let mut skipped = 0;
            let total = model_ids.len();

            for model_id in &model_ids {
                yield VerifyEvent::Starting { model_id: model_id.clone() };

                let start = std::time::Instant::now();

                match registry.from_id(model_id) {
                    Ok(builder) => {
                        // Use a minimal test prompt
                        match builder.send_text("respond with exactly: ok").await {
                            Ok(_) => {
                                passed += 1;
                                yield VerifyEvent::Success {
                                    model_id: model_id.clone(),
                                    latency_ms: start.elapsed().as_millis() as u64,
                                };
                            }
                            Err(e) => {
                                let error_str = e.to_string();
                                // Check if it's an auth/config issue vs actual failure
                                if error_str.contains("API key") || error_str.contains("authentication") {
                                    skipped += 1;
                                    yield VerifyEvent::Skipped {
                                        model_id: model_id.clone(),
                                        reason: "Missing or invalid API key".to_string(),
                                    };
                                } else {
                                    failed += 1;
                                    yield VerifyEvent::Failed {
                                        model_id: model_id.clone(),
                                        error: error_str,
                                    };
                                }
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
                total,
                passed,
                failed,
                skipped,
            };
        }
    }

    /// Query models with filters
    #[hub_macro::hub_method]
    pub async fn query(
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

            let configs = query.configs();
            let count = configs.len();

            for config in configs {
                yield QueryEvent::Match(ModelInfo::from(config));
            }

            yield QueryEvent::Done { count };
        }
    }
}

// ============================================================================
// Hub Feature - Standalone serving
// ============================================================================

impl CllientActivation {
    /// Create a Plexus containing this plugin
    ///
    /// Useful when you want to add other plugins before serving.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let plexus = CllientActivation::new(registry)
    ///     .into_plexus()
    ///     .register(other_plugin);
    /// ```
    pub fn into_plexus(self) -> hub_core::Plexus {
        hub_core::Plexus::new().register(self)
    }

    /// Mount this plugin into an existing Plexus
    pub fn mount_into(self, plexus: hub_core::Plexus) -> hub_core::Plexus {
        plexus.register(self)
    }
}

#[cfg(feature = "hub")]
impl CllientActivation {
    /// Serve as a standalone hub server
    ///
    /// This creates a Plexus with just this activation and serves it over JSON-RPC.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let registry = ModelRegistry::new()?;
    /// CllientActivation::new(registry)
    ///     .serve("127.0.0.1:8080")
    ///     .await?;
    /// ```
    pub async fn serve(self, addr: &str) -> anyhow::Result<()> {
        use hub_core::Plexus;
        use jsonrpsee::server::Server;
        use std::net::SocketAddr;

        let plexus = Plexus::new().register(self);
        let module = plexus.into_rpc_module()?;

        let socket_addr: SocketAddr = addr.parse()?;
        let server = Server::builder().build(socket_addr).await?;
        let handle = server.start(module);

        tracing::info!("cllient-hub listening on {}", addr);
        handle.stopped().await;
        Ok(())
    }
}

// ============================================================================
// ModelRegistry convenience methods
// ============================================================================

impl crate::ModelRegistry {
    /// Convert this registry into a plugin activation
    #[cfg(feature = "plugin")]
    pub fn into_plugin(self) -> CllientActivation {
        CllientActivation::new(self)
    }

    /// Create a plugin from a shared registry reference
    #[cfg(feature = "plugin")]
    pub fn as_plugin(self: &Arc<Self>) -> CllientActivation {
        CllientActivation::from_shared(self.clone())
    }

    /// Serve this registry as a standalone hub
    #[cfg(feature = "hub")]
    pub async fn serve(self, addr: &str) -> anyhow::Result<()> {
        self.into_plugin().serve(addr).await
    }

    /// Convert this registry into a Plexus for composition with other plugins
    #[cfg(feature = "plugin")]
    pub fn into_plexus(self) -> hub_core::Plexus {
        self.into_plugin().into_plexus()
    }
}
