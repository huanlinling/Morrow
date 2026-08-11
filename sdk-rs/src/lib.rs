//! Ferrum SDK — Write Minecraft mods in Rust.
//!
//! # Quick Start
//!
//! ```ignore
//! use ferrum::prelude::*;
//!
//! #[ferrum::mod_main]
//! fn init(ctx: &mut Context) -> Result<(), FerrumError> {
//!     ferrum::info!("Hello from my mod!");
//!     Ok(())
//! }
//! ```
//!
//! The `#[ferrum::mod_main]` macro generates the `extern "C"` entry points
//! the Ferrum runtime expects. During `ferrum_mod_init`, the runtime calls
//! your init function with a [`Context`] for capability access.

pub mod context;
pub mod error;

// Re-export the proc macro
pub use ferrum_macros::mod_main;

// Re-export commonly used types
pub use context::Context;
pub use error::FerrumError;

/// Prelude: everything most mods need.
pub mod prelude {
    pub use crate::context::Context;
    pub use crate::error::FerrumError;
    pub use ferrum_macros::mod_main;
    pub use crate::{info, warn, error};
}

// ---------------------------------------------------------------------------
// Logging macros — output to stderr, captured by Minecraft log
// ---------------------------------------------------------------------------

/// Log an info-level message to the Minecraft server log.
#[macro_export]
macro_rules! info {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        eprintln!(concat!("[", env!("CARGO_PKG_NAME"), "] ", $fmt), $($arg),*)
    };
}

/// Log a warning to the Minecraft server log.
#[macro_export]
macro_rules! warn {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        eprintln!(concat!("[", env!("CARGO_PKG_NAME"), "] WARN: ", $fmt), $($arg),*)
    };
}

/// Log an error to the Minecraft server log.
#[macro_export]
macro_rules! error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        eprintln!(concat!("[", env!("CARGO_PKG_NAME"), "] ERROR: ", $fmt), $($arg),*)
    };
}
