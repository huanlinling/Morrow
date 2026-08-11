//! Error type for Morrow mods.

use std::fmt;

/// Standard error type for Morrow mod operations.
///
/// Returned from lifecycle hooks (`on_init`, `on_tick`, etc.)
/// to signal failure to the runtime.
#[derive(Debug)]
pub struct MorrowError {
    message: String,
}

impl MorrowError {
    /// Create a new error with a message.
    pub fn new(msg: impl Into<String>) -> Self {
        MorrowError {
            message: msg.into(),
        }
    }
}

impl fmt::Display for MorrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MorrowError: {}", self.message)
    }
}

impl std::error::Error for MorrowError {}

// Convenience conversions
impl From<&str> for MorrowError {
    fn from(s: &str) -> Self {
        MorrowError::new(s)
    }
}

impl From<String> for MorrowError {
    fn from(s: String) -> Self {
        MorrowError::new(s)
    }
}
