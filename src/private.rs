use std::fmt;
use serde::{Deserialize, Serialize};

/// A wrapper type for sensitive data that redacts the value when displayed
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Private<T> {
    value: T,
}

impl<T> Private<T> {
    /// Create a new Private wrapper around a value
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// Get the inner value (use carefully - only when actually needed)
    pub fn expose(&self) -> &T {
        &self.value
    }

    /// Consume the Private wrapper and return the inner value
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> fmt::Display for Private<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "*****")
    }
}

impl<T> fmt::Debug for Private<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Private(*****)")
    }
}

impl<T> From<T> for Private<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> AsRef<T> for Private<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_display() {
        let secret = Private::new("my-secret-key");
        assert_eq!(format!("{}", secret), "*****");
    }

    #[test]
    fn test_private_debug() {
        let secret = Private::new("my-secret-key");
        assert_eq!(format!("{:?}", secret), "Private(*****)");
    }

    #[test]
    fn test_private_expose() {
        let secret = Private::new("my-secret-key");
        assert_eq!(secret.expose(), &"my-secret-key");
    }

    #[test]
    fn test_private_into_inner() {
        let secret = Private::new("my-secret-key");
        assert_eq!(secret.into_inner(), "my-secret-key");
    }

    #[test]
    fn test_private_from() {
        let secret: Private<String> = "my-secret-key".to_string().into();
        assert_eq!(secret.expose(), "my-secret-key");
    }
}