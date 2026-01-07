pub mod config;
pub mod embedded_config;
pub mod client;
pub mod streaming;
pub mod streaming_json;
pub mod template;
pub mod error;
pub mod types;
pub mod private;
pub mod runtime;
pub mod chat;
pub mod export;
pub mod registry_index;
pub mod validation;
pub mod query;
pub mod message_format;
pub mod transform;
pub mod jsonpath;

#[cfg(feature = "plugin")]
pub mod events;
#[cfg(feature = "plugin")]
pub mod plugin;

pub use config::{
    ServiceConfig, ModelConfig, ConfigLoader, VerificationStatus,
    MessageFormat, SseParser, StreamingFormat, Currency,
};
pub use export::{RegistryExport, ServiceExport, ModelExport, RegistryStats};
pub use embedded_config::EmbeddedConfigLoader;
pub use registry_index::{RegistryIndex, IndexStats, BrokenReference};
pub use validation::{ValidationLevel, ValidationReport, ValidationIssue, Severity, ConfigValidator};
pub use query::{ModelQuery, Filter, CapabilityFilter, OrderBy};
pub use message_format::{
    MessageFormatConfig, MessageFormatter, MessageFormatBuilder, MessageFormatRegistry,
    ContentBlockConfig, ContentBlockType, MessageTemplate,
    anthropic_format, openai_format,
};
pub use client::{
    LowLevelClient, HttpClient, ClientFactory, ConfigProvider,
    FileBasedClientFactory, EmbeddedClientFactory
};
pub use error::{ClientError, Result, ApiError, ErrorExtractorConfig, ErrorExtractorBuilder};
pub use types::{
    CompletionRequest, CompletionResponse, MessageContent, ContentBlock,
    ImageFormat, AudioFormat, DocumentFormat, Usage, FromFile, RequestBuilder, Message, MessageList
};
pub use private::Private;
pub use runtime::ModelRegistry;
pub use chat::{ChatterId, ChatBuilder};
pub use transform::{
    ValueTransform, TransformType, TransformBuilder, TransformEngine,
    TransformConfig, FieldTransformConfig, get_value,
};
pub use jsonpath::{
    JsonPath, JsonPathBuilder, JsonPathError, JsonPathSet, Segment as JsonPathSegment,
};

// Plugin exports (feature-gated)
#[cfg(feature = "plugin")]
pub use events::{
    CompletionEvent, ModelEvent, ModelInfo, ServiceEvent, ServiceInfo,
    VerifyEvent, QueryEvent, CapabilitySummary, PricingSummary,
};
#[cfg(feature = "plugin")]
pub use plugin::CllientActivation;

// Re-export common types
pub use reqwest::Response;
pub use tokio_stream::Stream;