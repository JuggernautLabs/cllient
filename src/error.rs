use thiserror::Error;

pub type Result<T> = std::result::Result<T, ClientError>;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("YAML serialization error: {0}")]
    YamlSerialization(#[from] serde_yaml::Error),
    
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),
    
    #[error("Template error: {0}")]
    Template(#[from] Box<handlebars::TemplateError>),
    
    #[error("Render error: {0}")]
    Render(#[from] handlebars::RenderError),
    
    #[error("JSON path error: {0}")]
    JsonPath(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Stream error: {0}")]
    Stream(String),
    
    #[error("Rate limit exceeded")]
    RateLimit,
    
    #[error("Authentication failed")]
    Auth,
    
    #[error("Invalid model: {0}")]
    InvalidModel(String),
    
    #[error("Invalid service: {0}")]
    InvalidService(String),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    
    #[error("Invalid configuration path: {0}")]
    InvalidPath(String),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Invalid YAML: {0}")]
    InvalidYaml(String),
    
    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),
}