//! # Morrow SDK
//!
//! Write Minecraft mods in Rust. Morrow compiles your code to a native
//! library, loaded by the host runtime via Panama FFI.
//!
//! ## Quick Start
//!
//! ```ignore
//! use morrow::prelude::*;
//!
//! #[morrow::mod_main]
//! fn init(ctx: &mut Context) -> Result<(), MorrowError> {
//!     ctx.register_command("ping", ping)?;
//!     Ok(())
//! }
//!
//! fn ping(_args: &str) {
//!     morrow::send_message("Pong!");
//! }
//!
//! #[morrow::event(player_join)]
//! fn on_join(player: &str) {
//!     morrow::send_message(&format!("Welcome, {player}!"));
//! }
//! ```
//!
//! ## Events
//!
//! | Attribute | Called when | Handler signature |
//! |-----------|-------------|-------------------|
//! | `#[morrow::event(tick)]` | Every game tick (20 TPS) | `fn(u64)` |
//! | `#[morrow::event(server_start)]` | Server finished starting | `fn()` |
//! | `#[morrow::event(server_stop)]` | Server begins stopping | `fn()` |
//! | `#[morrow::event(player_join)]` | Player joins | `fn(&str)` |
//! | `#[morrow::event(player_leave)]` | Player leaves | `fn(&str)` |
//! | `#[morrow::event(chat_message)]` | Chat message sent | `fn(&str, &str)` |
//! | `#[morrow::event(block_break)]` | Block broken | `fn(&str, &str)` |
//! | `#[morrow::event(block_place)]` | Block placed | `fn(&str, &str)` |
//! | `#[morrow::event(player_death)]` | Player dies | `fn(&str, &str)` |
//!
//! ## Global API (usable inside event handlers)
//!
//! [`send_message`], [`execute_command`], [`player_count`], [`player_list`],
//! [`world_time`], [`config`], [`log`] — thin wrappers over the runtime API,
//! reading the current runtime vtable from a global static set during init.
//!
//! Thread safety: writes ([`send_message`], [`execute_command`]) are safe
//! from any thread — off the game main thread the runtime queues them and
//! delivers on the main thread at the next tick (≤ 50 ms). Reads
//! ([`player_count`], [`player_list`], [`world_time`]) are snapshot-backed
//! (a per-tick world cache, opened by the first query, ≤ 1 tick stale,
//! empty until the first refresh) and are safe from any thread.

pub mod __internal;
pub mod context;
pub mod error;
pub mod runtime_api;

// ─── Zero-copy helpers ─────────────────────────

/// Read a `&str` from FFI pointer + length. Zero-copy — borrows the
/// original buffer. Falls back to `"<invalid>"` on bad UTF-8 and to `""`
/// on null pointers (e.g. `player_death`'s cause).
///
/// Use this in event callbacks instead of `String::from_utf8_lossy`
/// to avoid allocation.
#[inline]
pub fn read_str<'a>(ptr: *const u8, len: u32) -> &'a str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let bytes = std::slice::from_raw_parts(ptr, len as usize);
        std::str::from_utf8(bytes).unwrap_or("<invalid utf-8>")
    }
}

/// Parse a comma-separated player list (host `get_player_list` format)
/// into names, skipping empties.
pub(crate) fn parse_player_list(s: &str) -> Vec<String> {
    s.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

// ─── Log levels ─────────────────────────────────

/// Log levels understood by the runtime (`1`=info, `2`=warn, `3`=error).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info = 1,
    Warn = 2,
    Error = 3,
}

// ─── Global API (event-side access) ─────────────
//
// The runtime vtable is stored in a global `static` (per-library, set by
// the generated `morrow_mod_init`). Writes (`send_message`,
// `execute_command`) work from any thread — the runtime marshals
// off-main-thread calls onto the game main thread. Reads (`player_count`,
// `player_list`, `world_time`) are snapshot-backed: they serve a per-tick
// world cache (opened automatically by the first query, ≤ 1 tick stale,
// empty until then) and are safe from ANY thread. If the runtime was
// never initialized (mod code running outside init/handlers, or the
// library not loaded by the host), these panic with a descriptive
// message rather than silently no-oping; [`log`] is the exception and
// always works.

/// Broadcast a message to all players' chat. Panics if runtime not set.
pub fn send_message(msg: &str) {
    let api = __internal::api();
    unsafe { (api.send_message)(0, msg.as_ptr(), msg.len() as u32) }
}

/// Run a server command. Panics if runtime not set.
pub fn execute_command(cmd: &str) {
    let api = __internal::api();
    unsafe { (api.execute_command)(0, cmd.as_ptr(), cmd.len() as u32) }
}

/// Online player count. Panics if runtime not set.
pub fn player_count() -> i32 {
    let api = __internal::api();
    unsafe { (api.get_player_count)(0) }
}

/// Online player names. Panics if runtime not set.
pub fn player_list() -> Vec<String> {
    let api = __internal::api();
    // Matches the runtime's 64 KiB snapshot buffer ceiling.
    let mut buf = vec![0u8; 65536];
    let n = unsafe { (api.get_player_list)(0, buf.as_mut_ptr(), buf.len() as u32) };
    parse_player_list(crate::read_str(buf.as_ptr(), n))
}

/// World time in ticks. Panics if runtime not set.
pub fn world_time() -> i64 {
    let api = __internal::api();
    unsafe { (api.get_world_time)(0) }
}

/// Read this mod's config.toml as raw TOML text. None when not set/packaged.
pub fn config_raw() -> Option<String> {
    let name = __internal::current_mod_name();
    let api = __internal::api();
    let mut buf = [0u8; 4096];
    let n = unsafe {
        (api.get_config)(0, name.as_ptr(), name.len() as u32, buf.as_mut_ptr(), buf.len() as u32)
    };
    (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
}

/// Read and parse this mod's config.toml into a typed struct
/// (`Ok(None)` when the package has no config.toml). See
/// [`Context::config`] for an example.
pub fn config<T: serde::de::DeserializeOwned>() -> Result<Option<T>, String> {
    config_raw()
        .map(|raw| toml::from_str(&raw).map_err(|e| format!("config.toml: {e}")))
        .transpose()
}

/// Log through the host; falls back to stderr when runtime not set.
pub fn log(level: LogLevel, msg: &str) {
    __internal::log(level as u32, msg)
}

// Re-export the proc macros
pub use morrow_macros::{event, mod_main};

// Re-export commonly used types
pub use context::Context;
pub use error::MorrowError;
pub use runtime_api::RuntimeApi;

/// Prelude: everything most mods need.
pub mod prelude {
    pub use crate::context::Context;
    pub use crate::error::MorrowError;
    pub use crate::runtime_api::RuntimeApi;
    pub use crate::LogLevel;
    pub use crate::{
        config, config_raw, error, execute_command, info, log, player_count, player_list,
        send_message, warn, world_time,
    };
    pub use morrow_macros::{event, mod_main};
}

// ─── Logging macros ────────────────────────────
//
// Routed through the host log (level 1/2/3) once the runtime is set;
// before that they fall back to stderr. `[mod-name]` prefix is applied
// at the call site via `CARGO_PKG_NAME`.

/// Log an info-level message to the Minecraft server log.
///
/// Note: explicit format arguments are required — implicit capture
/// (`info!("{player}")`) does not work inside a macro.
#[macro_export]
macro_rules! info {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::__internal::log(
            1,
            &format!(concat!("[", env!("CARGO_PKG_NAME"), "] ", $fmt), $($arg),*),
        )
    };
}

/// Log a warning.
#[macro_export]
macro_rules! warn {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::__internal::log(
            2,
            &format!(concat!("[", env!("CARGO_PKG_NAME"), "] WARN: ", $fmt), $($arg),*),
        )
    };
}

/// Log an error.
#[macro_export]
macro_rules! error {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::__internal::log(
            3,
            &format!(concat!("[", env!("CARGO_PKG_NAME"), "] ERROR: ", $fmt), $($arg),*),
        )
    };
}

// ─── Tests (pure — no FFI) ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_str_null_is_empty() {
        assert_eq!(read_str(std::ptr::null(), 0), "");
    }

    #[test]
    fn read_str_ok() {
        let bytes = b"hello";
        assert_eq!(read_str(bytes.as_ptr(), 5), "hello");
    }

    #[test]
    fn read_str_bad_utf8() {
        assert_eq!(read_str(b"\xff\xfe".as_ptr(), 2), "<invalid utf-8>");
    }

    #[test]
    fn parse_player_list_basic() {
        assert_eq!(
            parse_player_list("alice,bob,carol"),
            vec!["alice".to_string(), "bob".to_string(), "carol".to_string()]
        );
    }

    #[test]
    fn parse_player_list_empty() {
        assert!(parse_player_list("").is_empty());
        assert_eq!(parse_player_list(",,,"), Vec::<String>::new());
    }

    #[test]
    fn parse_player_list_single() {
        assert_eq!(parse_player_list("solo"), vec!["solo".to_string()]);
    }

    #[test]
    fn log_level_encoding() {
        assert_eq!(LogLevel::Info as u32, 1);
        assert_eq!(LogLevel::Warn as u32, 2);
        assert_eq!(LogLevel::Error as u32, 3);
    }
}
