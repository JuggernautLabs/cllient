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

pub use config::{ServiceConfig, ModelConfig, ConfigLoader};
pub use embedded_config::EmbeddedConfigLoader;
pub use client::{
    LowLevelClient, HttpClient, ClientFactory, ConfigProvider,
    FileBasedClientFactory, EmbeddedClientFactory
};
pub use error::{ClientError, Result};
pub use types::{
    CompletionRequest, CompletionResponse, MessageContent, ContentBlock,
    ImageFormat, AudioFormat, DocumentFormat, Usage, FromFile, RequestBuilder, Message, MessageList
};
pub use private::Private;
pub use runtime::ModelRegistry;
pub use chat::{ChatterId, ChatBuilder};

// Re-export common types
pub use reqwest::Response;
pub use tokio_stream::Stream;