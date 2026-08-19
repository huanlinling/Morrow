//! Per-tick callback type.
//!
//! Mods can optionally export `morrow_mod_tick(tick: u64)` to receive
//! tick events. The runtime discovers this symbol during mod loading
//! and stores it in the dispatch tables
//! (`host_api::DispatchTables::tick`).

/// A tick callback: `fn(tick_number: u64)`.
pub type TickCallback = unsafe extern "C" fn(u64);
