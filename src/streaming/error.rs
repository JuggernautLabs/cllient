//! Error types for streaming module compatibility

use thiserror::Error;

/// AI-specific errors (compatibility layer)
#[derive(Error, Debug)]
pub enum AIError {
    #[error("Mock error: {0}")]
    Mock(String),
    
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl From<crate::error::ClientError> for AIError {
    fn from(err: crate::error::ClientError) -> Self {
        match err {
            crate::error::ClientError::Http(e) => AIError::Http(e),
            other => AIError::Other(other.to_string()),
        }
    }
}

/// Query resolver errors (compatibility layer)
#[derive(Error, Debug)]
pub enum QueryResolverError {
    #[error("AI error: {0}")]
    Ai(AIError),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl From<crate::error::ClientError> for QueryResolverError {
    fn from(err: crate::error::ClientError) -> Self {
        QueryResolverError::Ai(err.into())
    }
}