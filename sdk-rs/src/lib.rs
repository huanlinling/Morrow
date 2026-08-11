//! # Morrow SDK
//!
//! Write Minecraft mods in Rust. Morrow compiles your code to a native
//! library, loaded by the Fabric host adapter via Panama FFI.
//!
//! ## Quick Start
//!
//! ```ignore
//! use morrow::prelude::*;
//!
//! #[morrow::mod_main]
//! fn init(ctx: &mut Context, api: *const RuntimeApi) -> Result<(), MorrowError> {
//!     morrow::info!("Hello from Rust!");
//!     Ok(())
//! }
//! ```
//!
//! ## RuntimeApi
//!
//! The [`RuntimeApi`] vtable provides access to game state:
//!
//! | Function | Returns | Description |
//! |----------|---------|-------------|
//! | `get_player_count(handle)` | i32 | Online player count |
//! | `get_player_list(handle, buf, cap)` | u32 | Comma-separated player names |
//! | `send_message(handle, ptr, len)` | — | Broadcast to chat |
//! | `execute_command(handle, ptr, len)` | — | Run server command |
//! | `get_world_time(handle)` | i64 | World time in ticks |
//! | `register_command(handle, name, len, cb)` | — | Register `/` command |
//! | `get_config(handle, name, len, buf, cap)` | u32 | Read config.toml |
//! | `request_capability(handle, cap, len)` | u32 | Check feature availability |
//!
//! ## Optional Exports
//!
//! Export any of these functions to receive events:
//!
//! | Export | Signature | Called when |
//! |--------|-----------|-------------|
//! | `morrow_mod_tick` | `fn(u64)` | Every game tick (20 TPS) |
//! | `morrow_mod_server_start` | `fn()` | Server finished starting |
//! | `morrow_mod_server_stop` | `fn()` | Server begins stopping |
//! | `morrow_mod_player_join` | `fn(*const u8, u32)` | Player joins |
//! | `morrow_mod_player_leave` | `fn(*const u8, u32)` | Player leaves |
//! | `morrow_mod_chat_message` | `fn(*const u8, u32, *const u8, u32)` | Chat message sent |
//! | `morrow_mod_block_break` | `fn(*const u8, u32, *const u8, u32)` | Block broken |
//! | `morrow_mod_block_place` | `fn(*const u8, u32, *const u8, u32)` | Block placed |
//! | `morrow_mod_player_death` | `fn(*const u8, u32, *const u8, u32)` | Player dies |

pub mod context;
pub mod error;
pub mod runtime_api;

// ─── Zero-copy helpers ─────────────────────────

/// Read a `&str` from FFI pointer + length. Zero-copy — borrows the
/// original buffer. Falls back to `"<invalid>"` on bad UTF-8.
///
/// Use this in event callbacks instead of `String::from_utf8_lossy`
/// to avoid allocation.
#[inline]
pub fn read_str<'a>(ptr: *const u8, len: u32) -> &'a str {
    unsafe {
        let bytes = std::slice::from_raw_parts(ptr, len as usize);
        std::str::from_utf8(bytes).unwrap_or("<invalid utf-8>")
    }
}

// Re-export the proc macro
pub use morrow_macros::mod_main;

// Re-export commonly used types
pub use context::Context;
pub use error::MorrowError;
pub use runtime_api::RuntimeApi;

/// Prelude: everything most mods need.
pub mod prelude {
    pub use crate::context::Context;
    pub use crate::error::MorrowError;
    pub use crate::runtime_api::RuntimeApi;
    pub use morrow_macros::mod_main;
    pub use crate::{info, warn, error};
}

// ─── Logging macros ────────────────────────────

/// Log an info-level message to the Minecraft server log.
/// Format: `[mod-name] msg`
#[macro_export]
macro_rules! info {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        eprintln!(concat!("[", env!("CARGO_PKG_NAME"), "] ", $fmt), $($arg),*)
    };
}

/// Log a warning.
#[macro_export]
macro_rules! warn {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        eprintln!(concat!("[", env!("CARGO_PKG_NAME"), "] WARN: ", $fmt), $($arg),*)
    };
}

/// Log an error.
#[macro_export]
macro_rules! error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        eprintln!(concat!("[", env!("CARGO_PKG_NAME"), "] ERROR: ", $fmt), $($arg),*)
    };
}
