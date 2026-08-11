//! Error type for Ferrum mods.

use std::fmt;

/// Standard error type for Ferrum mod operations.
///
/// Returned from lifecycle hooks (`on_init`, `on_tick`, etc.)
/// to signal failure to the runtime.
#[derive(Debug)]
pub struct FerrumError {
    message: String,
}

impl FerrumError {
    /// Create a new error with a message.
    pub fn new(msg: impl Into<String>) -> Self {
        FerrumError {
            message: msg.into(),
        }
    }
}

impl fmt::Display for FerrumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FerrumError: {}", self.message)
    }
}

impl std::error::Error for FerrumError {}

// Convenience conversions
impl From<&str> for FerrumError {
    fn from(s: &str) -> Self {
        FerrumError::new(s)
    }
}

impl From<String> for FerrumError {
    fn from(s: String) -> Self {
        FerrumError::new(s)
    }
}
